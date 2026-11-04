#!/usr/bin/env bash
# RustSet 一键部署脚本：构建 + 打包 + 生成部署配置
# 用法: bash script/deploy.sh [输出目录]
set -euo pipefail
cd "$(dirname "$0")/.."

OUTPUT_DIR="$(cd "$(dirname "$0")/.." && pwd)/${1:-dist-deploy}"

VERSION="$(date +%Y%m%d%H%M%S)"

echo "=== RustSet 部署打包 ==="
mkdir -p "${OUTPUT_DIR}"

# 1) 后端 Release 构建
echo "--- 构建后端 (release) ---"
export PATH="$HOME/.local/bin:$PATH"
mise exec -- cargo build --release -p rustset-gateway
cp target/release/rustset-gateway "${OUTPUT_DIR}/rustset-gateway"

# 2) 前端构建
echo "--- 构建前端 ---"
cd apps/web
mise exec -- bun install
mise exec -- bun run build:antd
cp -r apps/web-antd/dist "${OUTPUT_DIR}/frontend"
cd ../..

# 3) 生成 Nginx 配置
cat > "${OUTPUT_DIR}/nginx-rustset.conf" << 'NGINX'
server {
    listen 80;
    server_name _;

    # 前端静态资源
    root /opt/rustset/frontend;
    index index.html;

    # gzip
    gzip on;
    gzip_types text/plain text/css application/json application/javascript text/xml application/xml;
    gzip_min_length 1000;

    # 静态资源长缓存（带 hash 的文件名）
    location ~* \.(js|css|png|jpg|jpeg|gif|ico|svg|woff2|webp)$ {
        expires 30d;
        add_header Cache-Control "public, immutable";
    }

    # SPA 路由
    location / {
        try_files $uri $uri/ /index.html;
    }

    # API 反代
    location /api/ {
        proxy_pass http://127.0.0.1:8080/;
        proxy_http_version 1.1;
        proxy_buffering off;
        proxy_read_timeout 600s;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
    }
}
NGINX

# 4) 生成 systemd 服务文件
cat > "${OUTPUT_DIR}/rustset-gateway.service" << 'SYSTEMD'
[Unit]
Description=RustSet Gateway
After=network-online.target docker.service
Wants=network-online.target

[Service]
Type=simple
WorkingDirectory=/opt/rustset
EnvironmentFile=/etc/rustset/gateway.env
ExecStart=/opt/rustset/rustset-gateway
Restart=always
RestartSec=5
LimitNOFILE=65535

[Install]
WantedBy=multi-user.target
SYSTEMD

# 5) 生成环境变量模板
cat > "${OUTPUT_DIR}/gateway.env.example" << 'ENV'
DATABASE_URL=postgres://rustset:CHANGE_ME@127.0.0.1:5432/rustset
REDIS_URL=redis://127.0.0.1:6379
JWT_SECRET=CHANGE_ME_TO_A_STRONG_SECRET_AT_LEAST_32_BYTES_LONG
GATEWAY_HOST=0.0.0.0
GATEWAY_PORT=8080
RUST_LOG=info

# 对象存储（RustFS / MinIO）
RUSTFS_ENDPOINT=http://127.0.0.1:9000
RUSTFS_ACCESS_KEY=CHANGE_ME
RUSTFS_SECRET_KEY=CHANGE_ME
RUSTFS_BUCKET=rustset
RUSTFS_REGION=us-east-1

# 初始管理员（首次启动创建，之后删除密码行）
BOOTSTRAP_ADMIN_USERNAME=admin
BOOTSTRAP_ADMIN_PASSWORD=CHANGE_ME_IMMEDIATELY
ENV

# 6) 生成部署说明
cat > "${OUTPUT_DIR}/DEPLOY.md" << 'DEPLOY'
# RustSet 部署指南

## 前置条件
- Docker + Docker Compose（基础设施）
- Nginx（前端 + API 反代）
- systemd（后端服务管理）

## 步骤

1. **基础设施**：将 `script/docker/docker-compose.yml` 复制到目标机器并启动
   ```bash
   docker compose -f docker-compose.yml up -d
   ```

2. **后端**：
   ```bash
   sudo mkdir -p /opt/rustset /etc/rustset
   sudo cp rustset-gateway /opt/rustset/
   sudo cp gateway.env.example /etc/rustset/gateway.env
   # 编辑 gateway.env，修改所有 CHANGE_ME
   sudo cp rustset-gateway.service /etc/systemd/system/
   sudo systemctl daemon-reload
   sudo systemctl enable --now rustset-gateway
   ```

3. **前端**：
   ```bash
   sudo mkdir -p /opt/rustset/frontend
   sudo cp -r frontend/* /opt/rustset/frontend/
   ```

4. **Nginx**：
   ```bash
   sudo cp nginx-rustset.conf /etc/nginx/sites-available/rustset
   sudo ln -sf /etc/nginx/sites-available/rustset /etc/nginx/sites-enabled/
   sudo nginx -t && sudo systemctl reload nginx
   ```

5. **验证**：
   ```bash
   curl -fsS http://127.0.0.1:8080/health
   # 浏览器打开 http://<服务器IP>
   ```

## 首次登录后
- 修改 admin 密码
- 删除 gateway.env 中的 BOOTSTRAP_ADMIN_PASSWORD
- 配置自动化开关（运营流程 → 自动化配置）
DEPLOY

# 7) 打包迁移文件
cp -r sql "${OUTPUT_DIR}/sql"

echo ""
echo "=== 部署包已生成: ${OUTPUT_DIR}/ ==="
ls -la "${OUTPUT_DIR}/"
