#!/usr/bin/env bash
# PostgreSQL 备份脚本（docs/xinchuang-adaptation-plan.md §4.7）。
#
# 产出：<备份目录>/rustset-<时间戳>.dump（pg_dump 自定义格式）+ .sha256，
# 并按 --keep 轮转旧备份（只删除备份文件，不触碰数据库）。
#
# 恢复同一恢复集原则（§4.7.1）：数据库与对象存储（RustFS）必须取自同一
# 恢复点；先备份数据库，紧接着运行 rustfs-manifest.sh 记录当时的对象
# 清单，两个产物放同一目录。
#
# 用法：
#   DATABASE_URL=postgres://… bash script/backup/postgres-backup.sh [--dir 备份目录] [--keep 14]
set -euo pipefail

BACKUP_DIR="${BACKUP_DIR:-/var/backups/rustset}"
KEEP="${KEEP:-14}"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --dir) BACKUP_DIR="$2"; shift 2 ;;
    --keep) KEEP="$2"; shift 2 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

: "${DATABASE_URL:?DATABASE_URL is required}"
command -v pg_dump >/dev/null || { echo "需要 pg_dump（与服务器大版本一致）" >&2; exit 1; }

mkdir -p "$BACKUP_DIR"
STAMP="$(date +%Y%m%d-%H%M%S)"
DUMP="$BACKUP_DIR/rustset-$STAMP.dump"

echo "=== 备份开始 $(date -Is) ==="
# 自定义格式：支持并行恢复、选择性恢复与压缩；-C 便于空库一键还原。
pg_dump "$DATABASE_URL" --format=custom --file="$DUMP"
( cd "$BACKUP_DIR" && sha256sum "rustset-$STAMP.dump" > "rustset-$STAMP.dump.sha256" )
( cd "$BACKUP_DIR" && sha256sum -c "rustset-$STAMP.dump.sha256" --quiet ) \
  && echo "完整性自校验通过"

SIZE="$(du -h "$DUMP" | cut -f1)"
echo "备份完成：$DUMP ($SIZE)"

# 轮转：仅删除最旧的 dump 及其校验文件，保留 KEEP 份。
mapfile -t OLD < <( ls -1 "$BACKUP_DIR"/rustset-*.dump 2>/dev/null | head -n "-$KEEP" || true )
for old in "${OLD[@]:-}"; do
  [[ -n "$old" ]] || continue
  rm -f "$old" "$old.sha256"
  echo "轮转移除：$(basename "$old")"
done

echo "=== 下一步（§4.7.1 同一恢复集）==="
echo "立即执行：DATABASE_URL=$DATABASE_URL bash script/backup/rustfs-manifest.sh --output $BACKUP_DIR/rustset-$STAMP.objects.csv"
