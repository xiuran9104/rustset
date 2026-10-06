# 启动、部署与运维

AI 编码助手接手项目时应先阅读仓库根目录的 [AI 启动交接指南](../AGENTS.md)。该文件包含本地启动、新服务器启动、systemd、Nginx 和常见故障的可执行命令。

## 本地开发方案

1. 执行 `docker compose -f script/docker/docker-compose.yml up -d`。
2. 按 [配置文档](configuration.md) 导出数据库、JWT 和管理员环境变量。
3. 执行 `cargo run -p rustset-gateway`（首次启动自动初始化数据库并执行全部迁移）。
4. 在 `apps/web` 执行 `bun install && bun run dev:antd`。
5. 检查 `GET http://127.0.0.1:8080/health`，然后访问 `http://127.0.0.1:5666`。

数据库结构由网关的 SQLx Migrator 自动管理，启动时仅执行迁移，禁止同时把 SQL 文件挂载进 `/docker-entrypoint-initdb.d`。

## 后端生产构建

```bash
cargo build --release -p rustset-gateway
```

产物为 `target/release/rustset-gateway`。生产服务至少需要：

- `DATABASE_URL`
- 强随机 `JWT_SECRET`
- `GATEWAY_HOST` 和 `GATEWAY_PORT`
- 可选 `REDIS_URL`

推荐由 systemd、Docker、Kubernetes 或其他进程管理器注入环境变量并负责重启。多个网关实例可以共享 PostgreSQL 和 Redis；异步任务轮询使用持久化状态，重复轮询应由供应商任务查询接口保持幂等。

新服务器首次部署建议流程：

1. 安装 Rust stable、Docker Compose 和 bun `1.4+`（node 22/24 可选，供部分工具链使用）。
2. 克隆代码到固定目录，例如 `/opt/rustset`。
3. 启动基础设施：`docker compose -f script/docker/docker-compose.yml up -d`。使用托管 PostgreSQL/Redis 时，改为在环境变量中指向托管地址。
   Compose 同时启动 RustFS：S3 API 为 `9000`，控制台为 `9001/rustfs/console/`，宿主机端口仅绑定 `127.0.0.1`。网关通过 `RUSTFS_ENDPOINT`、`RUSTFS_ACCESS_KEY`、`RUSTFS_SECRET_KEY`、`RUSTFS_REGION` 和 `RUSTFS_BUCKET` 连接对象存储；单机本地默认 endpoint 是 `http://127.0.0.1:9000`。远端 endpoint 必须显式设置独立凭据。网关启动时会确保 bucket 存在，上传写入 RustFS，下载由网关代理读取。单文件默认上限为 50 MiB，可通过 `INFRA_UPLOAD_MAX_BYTES` 调整。
4. 创建 `/etc/rustset/gateway.env`，写入 `DATABASE_URL`、`REDIS_URL`、强随机 `JWT_SECRET`、`GATEWAY_HOST`、`GATEWAY_PORT`、`RUST_LOG` 和首次管理员变量。
5. 执行 `cargo build --release -p rustset-gateway`。
6. 手动加载环境变量运行一次 `target/release/rustset-gateway`，确认迁移成功和管理员可登录。
7. 管理员创建后，从环境文件移除 `BOOTSTRAP_ADMIN_PASSWORD`。
8. 使用 systemd、Docker 或 Kubernetes 托管网关进程。

旧版本保存在 `storage/uploads` 的文件需迁移到 RustFS 后再清理。维护窗口设置数据库和 RustFS 环境变量后运行 `bash script/migrate-local-uploads-to-rustfs.sh`；脚本确认对象大小和数据库记录一致后才删除本地文件。

systemd 示例：

```ini
[Unit]
Description=RustSet Gateway
After=network-online.target docker.service
Wants=network-online.target

[Service]
Type=simple
WorkingDirectory=/opt/rustset
EnvironmentFile=/etc/rustset/gateway.env
ExecStart=/opt/rustset/target/release/rustset-gateway
Restart=always
RestartSec=5

[Install]
WantedBy=multi-user.target
```

启用服务：

```bash
sudo systemctl daemon-reload
sudo systemctl enable --now rustset-gateway
sudo systemctl status rustset-gateway
curl -fsS http://127.0.0.1:8080/health
```

## 前端生产构建

```bash
bun install --frozen-lockfile --cwd apps/web
bun run --cwd apps/web build:antd
```

静态产物位于 `apps/web/apps/web-antd/dist`，可由 Nginx 或对象存储/CDN 托管。SPA 部署需要把未知前端路径回退至 `index.html`，并将 `/api/` 反向代理到 Rust 网关。

Nginx 核心配置示例：

```nginx
location / {
    try_files $uri $uri/ /index.html;
}

location /api/ {
    proxy_pass http://rustset-gateway:8080/;
    proxy_http_version 1.1;
    proxy_buffering off; # SSE 必须关闭代理缓冲
    proxy_read_timeout 600s;
    proxy_set_header Host $host;
    proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
}
```

## 数据库迁移

数据库结构由 Rust 网关启动时的 SQLx Migrator 统一管理，不依赖 Docker 卷挂载或外部 SQL 导入。

- 迁移文件位于 `sql/postgresql`，版本号必须是唯一整数前缀。
- 网关连接数据库后、监听端口前自动执行迁移。
- 空数据库首次启动时直接执行全部迁移。`0001_initial.sql` 提供当前完整 schema 和基准数据，不依赖 `sql/bootstrap/current.sql`。
- 每次修改 schema 或基准数据都必须新增更高版本的迁移，并更新空库迁移测试的版本数和数据断言。
- 全部迁移在干净参考库执行成功后，重新导出 `sql/bootstrap/current.sql` 用于人工比对；应用不得依赖该文件启动。
- 发布前运行 `bash script/test-database-migrations.sh`，验证空 PostgreSQL 18 可完成全部迁移。
- 已在生产执行的迁移不得修改；后续结构变化应新增更高版本迁移。
- 本次迁移历史已合并为新的 `0001`，保留旧 `_sqlx_migrations` 记录的数据库需要清空后重建。
- 正式升级前必须备份数据库，并先在备份副本验证升级。

## 首次管理员

仅首次部署设置：

```bash
export BOOTSTRAP_ADMIN_USERNAME=admin
export BOOTSTRAP_ADMIN_PASSWORD='替换为高强度密码'
```

管理员存在后可以移除 `BOOTSTRAP_ADMIN_PASSWORD`，网关会跳过初始化。不要把生产密码提交到仓库或镜像。

## 健康检查与日志

- 存活检查：`GET /health`
- 就绪检查：`GET /health/ready`，覆盖数据库、Redis 缓存/限流与 RustFS 对象存储（签名 HEAD，无副作用）；对象存储默认在 `RUST_ENV=production` 或显式设置 `RUSTFS_ENDPOINT` 时列为必需，可用 `READINESS_REQUIRE_OBJECT_STORAGE` 覆盖
- OpenAPI 文档：`GET /openapi.json`
- 请求自动生成或透传 `x-request-id`
- 使用 `RUST_LOG=info` 或模块级过滤规则控制 tracing 输出

## 测试和发布检查

```bash
cargo fmt --all -- --check
cargo test --workspace
bash script/test-database-migrations.sh
bash script/test-ai-e2e.sh
bun run --cwd apps/web check:type
bun run --cwd apps/web build:antd
```

`test-ai-e2e.sh` 会启动临时 PostgreSQL，并用本地模拟模型验证 JWT、普通聊天、SSE、消息落库及 Midjourney Imagine/Action 状态机，不调用外部付费模型。

## 常见问题

- 网关提示 `DATABASE_URL is required`：未配置数据库连接串。
- 网关提示 JWT 密钥过短：`JWT_SECRET` 必须至少 32 字节。
- 没有初始管理员：确认首次启动时设置了 `BOOTSTRAP_ADMIN_PASSWORD`。
- SSE 到前端后一次性出现：关闭 Nginx/Ingress 的响应缓冲并增加读取超时。
- AI 任务一直处理中：检查模型配置的任务查询路径、鉴权头和供应商任务 ID；可调用 `/poll` 接口立即同步。
- 前端请求 404：确认 RustSet 管理端 API 基址或 Nginx `/api/` 转发是否去掉了正确的前缀。
