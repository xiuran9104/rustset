#!/usr/bin/env bash
# 本地开发网关一键重启：先清掉残留进程（包括外层包装被杀后留下的
# 孤儿网关），等端口释放，再启动新进程并等待健康检查。任何一个
# 环节失败都明确报错退出，不会"卡死"。
set -euo pipefail
cd "$(dirname "$0")/.."

export DATABASE_URL="${DATABASE_URL:-postgres://rustset:rustset@127.0.0.1:5432/rustset}"
export REDIS_URL="${REDIS_URL:-redis://127.0.0.1:6379}"
export JWT_SECRET="${JWT_SECRET:-local-development-jwt-secret-change-me-32bytes}"
export RUST_LOG="${RUST_LOG:-info}"

if [[ ! -x ./target/debug/rustset-gateway ]]; then
  echo "error: ./target/debug/rustset-gateway not built; run cargo build -p rustset-gateway" >&2
  exit 1
fi

# 1) 清理残留网关，等待 8080 释放（最多 10 秒）
pkill -f 'target/debug/rustset-gateway' 2>/dev/null || true
for _ in $(seq 1 20); do
  ss -ltn 2>/dev/null | grep -q ':8080 ' || break
  sleep 0.5
done
if ss -ltn 2>/dev/null | grep -q ':8080 '; then
  echo "error: port 8080 still occupied after killing leftovers" >&2
  ss -ltnp 2>/dev/null | grep ':8080 ' >&2 || true
  exit 1
fi

# 2) 启动并等待健康检查（最多 30 秒）
nohup ./target/debug/rustset-gateway >> /tmp/rustset-gateway.log 2>&1 &
gateway_pid=$!
for _ in $(seq 1 30); do
  if curl -fsS --max-time 2 http://127.0.0.1:8080/health >/dev/null 2>&1; then
    echo "gateway healthy (pid ${gateway_pid}, migrations applied on startup)"
    exit 0
  fi
  if ! kill -0 "$gateway_pid" 2>/dev/null; then
    echo "error: gateway exited during startup; last log lines:" >&2
    tail -20 /tmp/rustset-gateway.log >&2
    exit 1
  fi
  sleep 1
done
echo "error: gateway did not become healthy in 30s; last log lines:" >&2
tail -20 /tmp/rustset-gateway.log >&2
exit 1
