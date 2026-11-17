#!/usr/bin/env bash
set -euo pipefail

: "${DATABASE_URL:?DATABASE_URL is required}"
: "${RUSTFS_ENDPOINT:?RUSTFS_ENDPOINT is required}"
: "${RUSTFS_ACCESS_KEY:?RUSTFS_ACCESS_KEY is required}"
: "${RUSTFS_SECRET_KEY:?RUSTFS_SECRET_KEY is required}"

RUSTFS_BUCKET="${RUSTFS_BUCKET:-rustset}"
export AWS_ACCESS_KEY_ID="$RUSTFS_ACCESS_KEY"
export AWS_SECRET_ACCESS_KEY="$RUSTFS_SECRET_KEY"
export AWS_DEFAULT_REGION="${RUSTFS_REGION:-us-east-1}"
upload_root="$(realpath -m "${INFRA_UPLOAD_DIR:-storage/uploads}")"
manifest="$(mktemp)"
trap 'rm -f "$manifest"' EXIT

command -v psql >/dev/null || { echo "psql is required" >&2; exit 1; }
command -v aws >/dev/null || { echo "AWS CLI v2 is required" >&2; exit 1; }

psql "$DATABASE_URL" -v ON_ERROR_STOP=1 -At -F $'\t' \
  -c "SELECT id, path, size FROM public.infra_file WHERE deleted=0 AND path LIKE '/upload/%' ORDER BY id" \
  > "$manifest"

while IFS=$'\t' read -r file_id path expected_size; do
  [[ -n "$file_id" ]] || continue
  key="${path#/upload/}"
  if [[ "$key" == "$path" || -z "$key" || "$key" == /* ]]; then
    echo "Invalid upload path for file id $file_id: $path" >&2
    exit 1
  fi

  source_path="$(realpath -m "$upload_root/$key")"
  if [[ "$source_path" != "$upload_root/"* ]]; then
    echo "Upload path escapes INFRA_UPLOAD_DIR for file id $file_id" >&2
    exit 1
  fi

  if [[ -f "$source_path" ]]; then
    aws --endpoint-url "$RUSTFS_ENDPOINT" s3 cp --only-show-errors \
      "$source_path" "s3://$RUSTFS_BUCKET/$key"
  elif ! aws --endpoint-url "$RUSTFS_ENDPOINT" s3api head-object \
      --bucket "$RUSTFS_BUCKET" --key "$key" >/dev/null 2>&1; then
    echo "Neither local file nor RustFS object exists for file id $file_id: $key" >&2
    exit 1
  fi

  object_size="$(aws --endpoint-url "$RUSTFS_ENDPOINT" s3api head-object \
    --bucket "$RUSTFS_BUCKET" --key "$key" --query ContentLength --output text)"
  if [[ "$object_size" != "$expected_size" ]]; then
    echo "Size mismatch for file id $file_id: database=$expected_size RustFS=$object_size" >&2
    exit 1
  fi
done < "$manifest"

# Delete local copies only after every selected database row has a verified object.
while IFS=$'\t' read -r file_id path expected_size; do
  [[ -n "$file_id" ]] || continue
  key="${path#/upload/}"
  source_path="$(realpath -m "$upload_root/$key")"
  if [[ "$source_path" == "$upload_root/"* && -f "$source_path" ]]; then
    rm -- "$source_path"
  fi
done < "$manifest"

echo "Verified RustFS migration and removed local copies for active upload records."
