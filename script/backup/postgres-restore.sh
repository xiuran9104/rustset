#!/usr/bin/env bash
# PostgreSQL 恢复脚本（docs/xinchuang-adaptation-plan.md §4.7.4 恢复演练）。
#
# 演练步骤（每次升级前至少完成一次空环境恢复演练，§6.6.1）：
#   1. 停止写入：systemctl stop rustset-gateway（或切换到维护页）
#   2. 准备空目标库（演练机或目标实例；不覆盖生产库）
#   3. 本脚本恢复 dump：pg_restore --dbname=… --clean --if-exists
#   4. 恢复对象存储：找到与 dump 同一恢复集的 objects.csv，
#      按 rustfs-manifest.sh 的说明校验对象存在与摘要
#   5. 启动服务（网关自动执行迁移链 reconciliation），抽查业务
#   6. 记录演练结论：恢复耗时、校验结果、差异清单
#
# 用法：
#   bash script/backup/postgres-restore.sh --dump rustset-xxx.dump --url postgres://… [--jobs 4]
set -euo pipefail

DUMP=""
TARGET_URL="${DATABASE_URL:-}"
JOBS=4

while [[ $# -gt 0 ]]; do
  case "$1" in
    --dump) DUMP="$2"; shift 2 ;;
    --url) TARGET_URL="$2"; shift 2 ;;
    --jobs) JOBS="$2"; shift 2 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

[[ -n "$DUMP" && -f "$DUMP" ]] || { echo "需要 --dump <文件>" >&2; exit 1; }
[[ -n "$TARGET_URL" ]] || { echo "需要 --url <目标库连接串>" >&2; exit 1; }
command -v pg_restore >/dev/null || { echo "需要 pg_restore" >&2; exit 1; }

# 完整性先行（§4.7.1：恢复损坏的备份不如不恢复）。
SHA_FILE="${DUMP}.sha256"
if [[ -f "$SHA_FILE" ]]; then
  ( cd "$(dirname "$DUMP")" && sha256sum -c "$(basename "$SHA_FILE")" --quiet ) \
    && echo "校验文件通过" || { echo "SHA256 校验失败，拒绝恢复" >&2; exit 1; }
else
  echo "警告：缺少 $SHA_FILE，无法验证备份完整性" >&2
fi

echo "目标库：${TARGET_URL%%\?*}"
read -r -p "确认恢复将覆盖目标库现有对象（--clean --if-exists）？输入 yes 继续：" CONFIRM
[[ "$CONFIRM" == "yes" ]] || { echo "已取消"; exit 1; }

echo "=== 恢复开始 $(date -Is) ==="
START=$SECONDS
pg_restore --dbname="$TARGET_URL" --clean --if-exists --jobs="$JOBS" --no-owner "$DUMP"
echo "=== 恢复完成，耗时 $((SECONDS - START))s ==="

echo "=== 抽查（§4.7.4 第 5 步前置）==="
TABLES=$(psql "$TARGET_URL" -t -A -c "SELECT count(*) FROM information_schema.tables WHERE table_schema='public'")
USERS=$(psql "$TARGET_URL" -t -A -c "SELECT count(*) FROM system_users" 2>/dev/null || echo "n/a")
echo "public 表数量：$TABLES；system_users 行数：$USERS"
echo "后续：启动网关执行迁移 reconciliation → 业务抽查 → 记录演练结论"
