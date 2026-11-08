#!/usr/bin/env bash
set -euo pipefail

# 信创适配：--db <flavor> 启动对应的一次性容器并执行
# 迁移套件 + 数据库合同套件（§4.1.7/§6.1 第二层）。
#
# 支持值：
#   postgres（默认）postgres:18 公开镜像
#   opengauss        opengauss/opengauss 公开镜像（R3 认证处置固化：
#                    password_encryption_type=1 + 口令重哈希 + DBCOMPATIBILITY='PG' 建库）
#   kingbase / dm    商业许可镜像，由环境变量提供：
#                    KINGBASE_IMAGE / KINGBASE_TEST_URL（或 DM_IMAGE / DM_TEST_URL）
#                    镜像来自内网许可仓库；未提供时报错退出（不静默换库）。
#
# 已设置 TEST_DATABASE_URL 时直接对该库执行（脚本从不删除/重建调用方提供的库），
# DB_FLAVOR 同样从环境或 --db 读取。
DB_FLAVOR_ARG="postgres"
prev_arg=""
for arg in "$@"; do
  case "$prev_arg" in
    --db) DB_FLAVOR_ARG="$arg" ;;
  esac
  prev_arg="$arg"
done
export DB_FLAVOR="${DB_FLAVOR_ARG:-postgres}"

run_suites() {
  cargo test -p rustset-framework-database --test migrations -- --ignored --nocapture
  # 数据库合同测试（§4.1.7/§6.1 第二层）：事务原子性、FOR UPDATE 互斥、
  # 锁超时分类、唯一/外键冲突分类、隔离级别、行读取类型集。
  cargo test -p rustset-framework-database --test contract -- --ignored --nocapture
}

if [[ -n "${TEST_DATABASE_URL:-}" ]]; then
  run_suites
  exit 0
fi

container="rustset-migration-test"

cleanup() { docker rm -f "$container" >/dev/null 2>&1 || true; }
trap cleanup EXIT
cleanup

start_postgres() {
  local port="${TEST_POSTGRES_PORT:-55432}"
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
}

start_opengauss() {
  local port="${TEST_OPENGAUSS_PORT:-55433}"
  local password='Rustset@2026'
  docker run -d --name "$container"     -e GS_PASSWORD="$password"     -p "$port:5432" opengauss/opengauss:7.0.0-RC2.B015 >/dev/null
  # 初始化较慢（initdb + undo 元数据），等 gsql 本地可用。
  for _ in $(seq 1 60); do
    docker exec -u omm "$container" bash -lc 'gsql -d postgres -t -A -c "SELECT 1"' >/dev/null 2>&1 && break
    sleep 3
  done
  # 认证事实（实测修正）：镜像把初始超户 omm 以 sha256 哈希存储且仅限本地；
  # 另预置 gaussdb 远程用户，其口令哈希为 MD5（= GS_PASSWORD）——这正是
  # libpq/sqlx 可直连的原因，无需 gs_guc 改参。注意 gsql 客户端永远先协商
  # sha256，不能用它探测 md5 认证；TCP 就绪探测只看端口，认证由套件证明。
  # 业务库必须 PG 兼容模式（默认 A 模式下 '' 即 NULL，破坏 NOT NULL 语义）。
  local ok=0
  for _ in $(seq 1 5); do
    if docker exec -u omm "$container" bash -lc \
      "gsql -d postgres -c \"CREATE DATABASE rustset_test DBCOMPATIBILITY = 'PG'\"" >/dev/null 2>&1; then
      ok=1; break
    fi
    # 重试前容忍「已存在」（上次成功但输出丢失的场景）。
    docker exec -u omm "$container" bash -lc \
      "gsql -d postgres -t -A -c \"SELECT 1 FROM pg_database WHERE datname='rustset_test'\"" \
      | grep -q 1 && { ok=1; break; }
    sleep 3
  done
  [[ "$ok" == "1" ]] || { echo "openGauss 建库失败" >&2; exit 1; }
  for _ in $(seq 1 30); do
    (echo > /dev/tcp/127.0.0.1/"$port") >/dev/null 2>&1 && break
    sleep 2
  done
  export TEST_DATABASE_URL="opengauss://gaussdb:Rustset%402026@127.0.0.1:${port}/rustset_test"
}

start_licensed() {
  # 金仓 / 达梦：镜像与连接串来自环境（内网许可仓库）。
  local image_var="$1" url_var="$2" port_map="$3"
  local image="${!image_var:-}" url="${!url_var:-}"
  [[ -n "$image" && -n "$url" ]] || {
    echo "缺少 ${image_var} 与 ${url_var}（商业许可镜像在内网仓库；不提供则跳过该方言，不静默换库）" >&2
    exit 2
  }
  docker run -d --name "$container" -p "$port_map" "$image" >/dev/null
  # 就绪探测交给 TEST_DATABASE_URL 本身：套件连接失败会给出明确错误。
  sleep 30
  export TEST_DATABASE_URL="$url"
}

case "$DB_FLAVOR" in
  postgres) start_postgres ;;
  opengauss) start_opengauss ;;
  kingbase) start_licensed KINGBASE_IMAGE KINGBASE_TEST_URL "${TEST_KINGBASE_PORT:-55434}:54321" ;;
  dm) start_licensed DM_IMAGE DM_TEST_URL "${TEST_DM_PORT:-55236}:5236" ;;
  *) echo "未知方言：$DB_FLAVOR（postgres|kingbase|opengauss|dm）" >&2; exit 2 ;;
esac

run_suites
