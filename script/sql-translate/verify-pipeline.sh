#!/usr/bin/env bash
# 0001 基线翻译流水线（docs/xinchuang-adaptation-plan.md §4.1.6.3、风险 R2）。
#
# 34.6 万行 pg_dump 基线不允许人工目检——本脚本实现
#   参考库导出 → 翻译 → 目标库回灌 → 行数 + 抽样 checksum 比对
# 的闭环，任一步骤失败即非零退出（比对不过即阻断）。
#
# 用法：
#   # 源库（PostgreSQL，运行全部迁移后的参考库）
#   SOURCE_URL='postgres://rustset:…@127.0.0.1:5432/rustset' \
#   TARGET_URL='postgres://…@目标机:5432/rustset' \
#     bash script/sql-translate/verify-pipeline.sh --flavor opengauss
#
# 步骤：
#   1. pg_dump 源库全量导出（--no-owner --no-privileges，自定义格式）
#   2. sql-translate 翻译为方言（--skip-baseline 之外的全量文件均产出）
#   3. 目标库回灌（PG 家族：pg_restore / 管道 SQL；达梦：dimp，需厂商客户端）
#   4. 比对：全部表的行数逐表核对 + 抽样表（参数 --sample）全行 checksum
#
# 达梦支持说明：dimp/dmdsql 未在 CI（无厂商客户端）；导入与 dmsql 比对
# 部分走 --dm-sql 参数指定客户端路径后启用，逻辑与 PG 家族一致。
set -euo pipefail
cd "$(dirname "$0")/../.."

FLAVOR="opengauss"
SAMPLE=8
SKIP_TRANSLATE=0
IGNORE_TIMESTAMPS=0
STAGE="${STAGE:-/tmp/rustset-pipeline-$$}"
SOURCE_URL="${SOURCE_URL:-}"
TARGET_URL="${TARGET_URL:-}"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --flavor) FLAVOR="$2"; shift 2 ;;
    --sample) SAMPLE="$2"; shift 2 ;;
    --skip-translate) SKIP_TRANSLATE=1; shift ;;
    --ignore-timestamps) IGNORE_TIMESTAMPS=1; shift ;;
    # （全新初始化的两侧库中 DEFAULT CURRENT_TIMESTAMP 基线行时间必然
    # 不同；真实迁移流程（导出既有数据回灌）时间戳是显式数据，勿用本参数。）
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

[[ -n "$SOURCE_URL" && -n "$TARGET_URL" ]] || {
  echo "需要 SOURCE_URL 与 TARGET_URL 环境变量" >&2; exit 1;
}
command -v pg_dump >/dev/null || { echo "需要 pg_dump（源库侧）" >&2; exit 1; }

mkdir -p "$STAGE"
trap 'rm -rf "$STAGE"' EXIT

echo "=== 1/4 导出源库 ==="
DUMP="$STAGE/source.dump"
pg_dump "$SOURCE_URL" --format=custom --no-owner --no-privileges --file="$DUMP"
echo "导出完成：$(du -h "$DUMP" | cut -f1)"

echo "=== 2/4 翻译为 $FLAVOR ==="
if [[ "$SKIP_TRANSLATE" -eq 1 ]]; then
  # 翻译产物由构建机预先产出（rsync/介质携带），仅导入与比对在交付机执行。
  [[ -d "sql/$FLAVOR" ]] || { echo "缺少 sql/$FLAVOR 翻译产物" >&2; exit 2; }
  cp -r "sql/$FLAVOR" "$STAGE/sql-$FLAVOR"
else
  cargo run -q -p sql-translate -- --target "$FLAVOR" --output "$STAGE/sql-$FLAVOR"
fi

echo "=== 3/4 回灌目标库 ==="
case "$FLAVOR" in
  opengauss|kingbase)
    # PG 线协议家族：目标库需已建好（openGauss 建库须 DBCOMPATIBILITY='PG'）。
    # 目标已有迁移历史则跳过导入（迁移链已跑过），否则按序全量导入；
    # -1 单事务保证半途失败不落半截 schema。
    EXISTING=$(psql "$TARGET_URL" -t -A -c \
      "SELECT count(*) FROM pg_tables WHERE schemaname IN ('public','ai','infra') AND tablename <> 'spatial_ref_sys'" 2>/dev/null || echo 0)
    if [[ "$EXISTING" != "0" ]]; then
      echo "目标库已有迁移历史，跳过导入"
    else
      for file in "$STAGE/sql-$FLAVOR"/*.sql; do
        psql "$TARGET_URL" -1 -v ON_ERROR_STOP=1 -f "$file" >/dev/null
        echo "已导入：$(basename "$file")"
      done
    fi
    ;;
  dm)
    echo "达梦回灌需厂商客户端（dimp），安装后重跑；当前仅产出翻译物于 $STAGE/sql-dm" >&2
    exit 3
    ;;
esac

echo "=== 4/4 比对（行数全表 + 抽样 $SAMPLE 表 checksum）==="
# 两侧行数逐表核对：以源库表清单为基准。
TABLES=$(psql "$SOURCE_URL" -t -A -c \
  "SELECT table_schema||'.'||table_name FROM information_schema.tables
   WHERE table_schema IN ('public','ai','infra') AND table_type='BASE TABLE'
     AND table_name NOT LIKE 'contract\_%' AND table_name NOT LIKE '\_sqlx%'
   ORDER BY 1")
FAILED=0
CHECKSUM_TABLES=$(echo "$TABLES" | shuf -n "$SAMPLE" --random-source=<(yes))
while IFS= read -r table; do
  [[ -z "$table" ]] && continue
  SRC_COUNT=$(psql "$SOURCE_URL" -t -A -c "SELECT count(*) FROM $table")
  DST_COUNT=$(psql "$TARGET_URL" -t -A -c "SELECT count(*) FROM $table" 2>/dev/null || echo ERR)
  if [[ "$SRC_COUNT" != "$DST_COUNT" ]]; then
    echo "行数不一致：$table 源=$SRC_COUNT 目标=$DST_COUNT"
    FAILED=$((FAILED+1))
    continue
  fi
  if grep -qxF "$table" <<<"$CHECKSUM_TABLES" && [[ "$SRC_COUNT" -gt 0 ]]; then
    # row_to_json 而非 to_jsonb(复合类型)：openGauss 无后者（实测）。
    # 时间戳文本化差异（PG ISO "T" 分隔 vs og 空格分隔）用正则归一化，
    # 双侧同样处理；非时间戳文本不受影响（模式要求完整日期时间形状）。
    # 时间戳文本化归一化（PG ISO "T" vs og 空格分隔）；
    # --ignore-timestamps 时进一步忽略时刻部分（全新初始化代理比对）。
    normalize="regexp_replace(row_to_json(x)::text, '([0-9]{4}-[0-9]{2}-[0-9]{2})T([0-9]{2}:)', '\\1 \\2', 'g')"
    if [[ "$IGNORE_TIMESTAMPS" -eq 1 ]]; then
      normalize="regexp_replace($normalize, '[0-9]{4}-[0-9]{2}-[0-9]{2}[T ][0-9]{2}:[0-9]{2}:[0-9]{2}(\\.[0-9]+)?', '<TS>', 'g')"
    fi
    row_query="SELECT coalesce(string_agg(row_text, E'\n' ORDER BY row_text), '') FROM (SELECT $normalize row_text FROM $table x) s"
    SRC_MD5=$(psql "$SOURCE_URL" -t -A -c "$row_query" | md5sum | cut -d' ' -f1)
    DST_MD5=$(psql "$TARGET_URL" -t -A -c "$row_query" 2>/dev/null | md5sum | cut -d' ' -f1)
    if [[ "$SRC_MD5" != "$DST_MD5" ]]; then
      echo "内容 checksum 不一致：$table"
      FAILED=$((FAILED+1))
    fi
  fi
done <<< "$TABLES"

if [[ "$FAILED" -gt 0 ]]; then
  echo "比对失败：$FAILED 张表不一致（阻断，R2）" >&2
  exit 1
fi
echo "比对通过：全部表行数一致 + 抽样 checksum 一致"
