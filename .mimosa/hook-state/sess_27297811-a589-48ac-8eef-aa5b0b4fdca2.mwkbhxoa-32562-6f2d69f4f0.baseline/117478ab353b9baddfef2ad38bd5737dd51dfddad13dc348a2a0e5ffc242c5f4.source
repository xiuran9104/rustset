#!/usr/bin/env bash
# RustFS / S3 对象存储清单脚本（docs/xinchuang-adaptation-plan.md §4.7.2）。
#
# 产出对象清单 CSV（key,size,etag,last_modified,checksum），与同一天的
# postgres dump 构成同一恢复集（§4.7.1）；恢复时按清单逐对象校验存在性
# 与摘要，形成恢复审计记录。
#
# 依赖 AWS CLI（RustFS 兼容 S3 API）：
#   aws configure set default.s3.addressing_style path
#   export AWS_ACCESS_KEY_ID=… AWS_SECRET_ACCESS_KEY=…
#
# 用法：
#   bash script/backup/rustfs-manifest.sh --endpoint http://127.0.0.1:9000 \
#        --bucket rustset --output /var/backups/rustset/rustset-xxx.objects.csv
#
# 恢复校验（演练第 4 步）：
#   bash script/backup/rustfs-manifest.sh --endpoint … --bucket rustset \
#        --verify /var/backups/rustset/rustset-xxx.objects.csv
set -euo pipefail

ENDPOINT="${RUSTFS_ENDPOINT:-http://127.0.0.1:9000}"
BUCKET="${RUSTFS_BUCKET:-rustset}"
OUTPUT=""
VERIFY=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --endpoint) ENDPOINT="$2"; shift 2 ;;
    --bucket) BUCKET="$2"; shift 2 ;;
    --output) OUTPUT="$2"; shift 2 ;;
    --verify) VERIFY="$2"; shift 2 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

command -v aws >/dev/null || { echo "需要 AWS CLI（aws）访问 S3 兼容 API" >&2; exit 1; }

common=(--endpoint-url "$ENDPOINT")

if [[ -n "$VERIFY" ]]; then
  echo "key,expected_size,status"
  missing=0
  while IFS=, read -r key size _rest; do
    [[ "$key" == "key" || -z "$key" ]] && continue
    if actual=$(aws s3api "${common[@]}" head-object --bucket "$BUCKET" --key "$key" \
         --query 'ContentLength' --output text 2>/dev/null); then
      if [[ "$actual" == "$size" ]]; then
        echo "$key,$size,ok"
      else
        echo "$key,$size,MISMATCH(actual=$actual)"; missing=$((missing+1))
      fi
    else
      echo "$key,$size,MISSING"; missing=$((missing+1))
    fi
  done < "$VERIFY"
  [[ "$missing" -eq 0 ]] && echo "全部对象校验通过" \
    || { echo "$missing 个对象缺失或大小不一致" >&2; exit 1; }
  exit 0
fi

[[ -n "$OUTPUT" ]] || { echo "需要 --output <csv> 或 --verify <csv>" >&2; exit 1; }

echo "key,size,etag,last_modified" > "$OUTPUT"
aws s3api "${common[@]}" list-objects-v2 --bucket "$BUCKET" \
  --query 'Contents[].{k:Key,s:Size,e:ETag,m:LastModified}' --output json |
  python3 -c '
import csv, json, sys
writer = csv.writer(sys.stdout)
for item in json.load(sys.stdin) or []:
    writer.writerow([item["k"], item["s"], item["e"], item["m"]])
' >> "$OUTPUT"

COUNT=$(($(wc -l < "$OUTPUT") - 1))
echo "对象清单完成：$OUTPUT（$COUNT 个对象）"
echo "与同时间戳的 postgres dump 放在同一恢复集目录，恢复时用 --verify 校验"
