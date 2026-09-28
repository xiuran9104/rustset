# RustSet

RustSet 是面向企业资产、CMDB、云资源和运维流程的一体化管理平台。后端采用 Rust、Axum 与 SQLx，管理端采用 Vue 3、Vben 和 Ant Design Vue；系统以租户和 RBAC 权限为边界，将资产台账、基础设施、云资源与物理资源台账、资源交付及 AI 能力集中到同一个平台。

## 核心能力

- **资产中心**：按 `docs/资产采集表.xlsx` 管理资产台账与网络策略，支持扫描任务、风险记录及资产与策略联查。
- **CMDB**：自定义配置项模型、属性和实例，维护实例关系。这里的“模型”是资产数据结构，与 AI 模型配置相互独立。
- **基础设施中心**：维护服务商、机房、区域和安全产品。
- **云管理中心**：维护云平台、云区域及云厂商接入凭据。
- **业务中心**：维护业务应用，以及按云资源（云服务器）和物理资源（物理服务器）分开的资源台账。
- **运维中心**：管理资源工单与自动审批规则，通过 OpenTofu 执行资源开通并回写 CMDB。
- **AI 大模型**：统一管理供应商、API Key 和各类型模型；对话、知识库、绘图、音乐、写作和运维 Agent 共用这一套模型配置。
- **系统与基础能力**：认证、用户、部门、角色、菜单、租户、租户网段、字典、通知、日志、文件和定时任务。

BPM 已从产品范围、菜单、前端代码和数据库活动数据中移除。租户网段在“系统管理 → 租户管理”中维护，不再提供独立的网段页面。

## 技术架构

```text
Vue 3 / Ant Design Vue
          |
          v
Rust Gateway (Axum)
  |-- system       认证、租户、RBAC 与后台管理
  |-- infra        资产、基础设施、云资源与运维工单
  |-- cmdb         动态模型、属性、实例与关系
  `-- ai           统一模型、对话、知识库与媒体生成
          |
          v
PostgreSQL / Redis / NATS / RustFS
```

数据库迁移由网关启动时的 SQLx Migrator 统一执行。`sql/bootstrap/current.sql` 仅供审查和比对，应用启动不依赖该快照。

## 环境要求

- Rust stable，支持 Rust 2024 edition
- PostgreSQL 18
- bun `1.4+`
- Docker 与 Docker Compose（推荐用于本地基础设施）
- OpenTofu `1.6+`（仅资源自动开通功能需要）

## 本地启动

启动 PostgreSQL、Redis、NATS 和 RustFS：

```bash
docker compose -f script/docker/docker-compose.yml up -d
```

RustFS 的 S3 API 地址为 `http://127.0.0.1:9000`，控制台地址为
`http://127.0.0.1:9001/rustfs/console/`。本地默认账号为 `rustset`，密码为
`rustset_password`；部署前通过 `RUSTFS_ACCESS_KEY` 和 `RUSTFS_SECRET_KEY`
覆盖默认值。网关启动时会确保 `rustset` bucket 存在；服务端上传、前端预签名直传和文件下载均通过 S3 API 读写 RustFS。已有本地上传文件需要另行迁移到 bucket。

旧版本地文件迁移并清理：`bash script/migrate-local-uploads-to-rustfs.sh`。脚本会先校验全部对象大小，再删除对应本地文件。单文件上传默认限制为 50 MiB，可通过 `INFRA_UPLOAD_MAX_BYTES` 调整。

启动网关：

```bash
export DATABASE_URL='postgres://rustset:rustset@127.0.0.1:5432/rustset'
export REDIS_URL='redis://127.0.0.1:6379'
export JWT_SECRET='replace-with-at-least-32-random-bytes'
cargo run -p rustset-gateway
```

启动管理端：

```bash
cd apps/web
bun install
bun run dev:antd
```

- 管理端：<http://127.0.0.1:5666>
- 健康检查：<http://127.0.0.1:8080/health>
- OpenAPI：<http://127.0.0.1:8080/openapi.json>
- 本地基线账号：`admin` / `admin123`

生产环境必须更换基线密码和 JWT 密钥。

## 验证

```bash
cargo test --workspace
bash script/test-database-migrations.sh
bash script/test-ai-e2e.sh
bun run --cwd apps/web check:type
bun run --cwd apps/web build:antd
```

## 当前边界

- 扫描任务目前提供轻量级端口扫描及任务记录，不等同于完整漏洞扫描平台。
- 云厂商连接、OpenTofu Provider 和外部 AI 模型需要使用实际凭据在目标环境验收。
- 未实现能力必须返回明确错误，不以空数据或固定成功结果代替真实实现。

## 文档

- [AI 开发与部署交接指南](AGENTS.md)
- [技术架构](docs/technical-solution.md)
- [配置与统一 AI 模型接入](docs/configuration.md)
- [启动、部署与运维](docs/deployment.md)
- [CMDB 与云平台路线图](docs/cmdb-roadmap.md)
- [功能范围与验收标准](docs/parity-roadmap.md)
- [前端开发说明](apps/web/README.md)
