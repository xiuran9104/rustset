#!/bin/bash
cd "$(dirname "$0")"
echo "=== RustSet ==="
# Infrastructure (PostgreSQL/Redis/NATS/RustFS) should already be running:
#   docker compose -f script/docker/docker-compose.yml up -d
export DATABASE_URL="postgres://rustset:rustset@127.0.0.1:5432/rustset"
export REDIS_URL="redis://127.0.0.1:6379"
export JWT_SECRET="local-development-jwt-secret-change-me-32bytes"
export WEB_PERMISSIVE_CORS="true"
# Start backend
echo ">> 启动后端 :8080"
cargo run -p rustset-gateway &
BACKEND_PID=$!
sleep 2
# Start Vben frontend if requested
if [ "$1" = "-f" ] || [ "$1" = "--with-frontend" ]; then
    echo ">> 启动前端 :5666"
    (cd apps/web/apps/web-antd && bun run dev) &
    FRONTEND_PID=$!
    echo ""
    echo "  后端 API : http://localhost:8080"
    echo "  前端 UI  : http://localhost:5666"
    wait $BACKEND_PID $FRONTEND_PID
else
    echo "  后端 API : http://localhost:8080"
    wait $BACKEND_PID
fi
