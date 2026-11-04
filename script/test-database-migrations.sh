#!/usr/bin/env bash
set -euo pipefail

# 信创适配：--db <flavor> 指定数据库方言（默认 postgres）
# 支持值：postgres（默认）| kingbase | opengauss | dm
# 国产数据库容器需要厂商许可镜像，当前仅 postgres 有 CI 容器。
DB_FLAVOR_ARG="postgres"
for arg in "$@"; do
  case "$prev_arg" in
    --db) DB_FLAVOR_ARG="$arg" ;;
  esac
  prev_arg="$arg"
done
export DB_FLAVOR="${DB_FLAVOR_ARG}"

# Supply an empty disposable database when Docker is unavailable. The script
# never drops or recreates a caller-provided database.
if [[ -n "${TEST_DATABASE_URL:-}" ]]; then
  cargo test -p rustset-framework-database --test migrations -- --ignored --nocapture
  exit 0
fi

container="rustset-migration-test"
port="${TEST_POSTGRES_PORT:-55432}"
cleanup() { docker rm -f "$container" >/dev/null 2>&1 || true; }
trap cleanup EXIT
cleanup
docker run -d --name "$container" \
  -e POSTGRES_USER=rustset_test \
  -e POSTGRES_PASSWORD=rustset_test \
  -e POSTGRES_DB=rustset_test \
  -p "$port:5432" postgres:18 >/dev/null
for _ in $(seq 1 30); do
  # The image starts a temporary Unix-socket server during initdb. Waiting on
  # TCP avoids racing that temporary process before the published port is ready.
  docker exec "$container" pg_isready -h 127.0.0.1 -p 5432 -U rustset_test -d rustset_test >/dev/null 2>&1 && break
  sleep 1
done
export TEST_DATABASE_URL="postgres://rustset_test:rustset_test@127.0.0.1:${port}/rustset_test"
cargo test -p rustset-framework-database --test migrations -- --ignored --nocapture
