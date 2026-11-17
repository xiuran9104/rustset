#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
web="$root/apps/web"
schema="$web/apps/web-antd/src/api/generated/infra.openapi.json"
types="$web/apps/web-antd/src/api/generated/infra.openapi.ts"

mkdir -p "$(dirname "$schema")"
cargo run --quiet -p rustset-infra-server --example export_openapi > "$schema"
cd "$web"
bunx openapi-typescript@7.13.0 "$schema" --output "$types"
