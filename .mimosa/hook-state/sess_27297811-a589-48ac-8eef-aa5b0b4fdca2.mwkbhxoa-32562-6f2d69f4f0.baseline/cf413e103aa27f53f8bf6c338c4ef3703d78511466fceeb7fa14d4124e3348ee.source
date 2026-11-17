#!/usr/bin/env bash
# 国产数据库备份/恢复（docs/xinchuang-adaptation-plan.md §4.7.1）：
#   kingbase  → sys_dump / sys_restore（金仓自带 PG 系工具）
#   opengauss → gs_dump  / gs_restore
#   dm        → dexp     / dimp（达梦导出导入）
#
# 语义与 script/backup/postgres-backup.sh 对齐：自定义/原生格式导出、
# SHA256 自校验、按 --keep 轮转、与对象存储清单构成同一恢复集。
# 本仓库无国产库客户端，脚本在装有厂商客户端的跳板机上运行；
# 每种数据库至少完成一次空环境恢复演练（§6.6.1）后方可交付。
#
# 用法：
#   bash script/backup/native-db-backup.sh --flavor kingbase --url 'sys_dump 连接串' \
#        [--dir 备份目录] [--keep 14]
#   bash script/backup/native-db-backup.sh --flavor dm --url 'SYSDBA/密码@host:5236' --schema RUSTSET ...
set -euo pipefail

FLAVOR=""
CONN=""
BACKUP_DIR="${BACKUP_DIR:-/var/backups/rustset}"
KEEP="${KEEP:-14}"
EXTRA=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --flavor) FLAVOR="$2"; shift 2 ;;
    --url) CONN="$2"; shift 2 ;;
    --dir) BACKUP_DIR="$2"; shift 2 ;;
    --keep) KEEP="$2"; shift 2 ;;
    --) shift; EXTRA=("$@"); break ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

case "$FLAVOR" in
  kingbase)
    DUMP_CMD=(sys_dump --format=custom)
    ;;
  opengauss)
    DUMP_CMD=(gs_dump --format=custom)
    ;;
  dm)
    # dexp 按用户/模式导出；SCHEMAS 通过 -- 附加参数传入（如 -- SCHEMAS=RUSTSET）。
    DUMP_CMD=(dexp)
    ;;
  "")
    echo "需要 --flavor <kingbase|opengauss|dm>" >&2; exit 2 ;;
  *)
    echo "未知方言：$FLAVOR（PG 用 script/backup/postgres-backup.sh）" >&2; exit 2 ;;
esac

[[ -n "$CONN" ]] || { echo "需要 --url <厂商工具连接串>" >&2; exit 1; }
command -v "${DUMP_CMD[0]}" >/dev/null \
  || { echo "需要厂商客户端 ${DUMP_CMD[0]}（金仓/开放 Gauss 在其 bin 目录，达梦在 dmdbms/bin）" >&2; exit 1; }

mkdir -p "$BACKUP_DIR"
STAMP="$(date +%Y%m%d-%H%M%S)"
DUMP="$BACKUP_DIR/rustset-$FLAVOR-$STAMP.dump"

echo "=== 备份开始 $(date -Is)（$FLAVOR）==="
"${DUMP_CMD[@]}" "$CONN" --file="$DUMP" "${EXTRA[@]}"
( cd "$BACKUP_DIR" && sha256sum "$(basename "$DUMP")" > "$(basename "$DUMP").sha256" )
( cd "$BACKUP_DIR" && sha256sum -c "$(basename "$DUMP").sha256" --quiet ) && echo "完整性自校验通过"
echo "备份完成：$DUMP ($(du -h "$DUMP" | cut -f1))"

mapfile -t OLD < <( ls -1 "$BACKUP_DIR"/rustset-"$FLAVOR"-*.dump 2>/dev/null | head -n "-$KEEP" || true )
for old in "${OLD[@]:-}"; do
  [[ -n "$old" ]] || continue
  rm -f "$old" "$old.sha256"
  echo "轮转移除：$(basename "$old")"
done

echo "=== 恢复提示 ==="
case "$FLAVOR" in
  kingbase) echo "sys_restore --dbname=<目标库> $DUMP" ;;
  opengauss) echo "gs_restore --dbname=<目标库> $DUMP" ;;
  dm) echo "dimp <连接串> FILE=$DUMP（建议先空库演练，§6.6.1）" ;;
esac
echo "同一恢复集：紧接着运行 rustfs-manifest.sh 记录对象清单（§4.7.1）"
