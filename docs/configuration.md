# 配置与模型接入

## 后端环境变量

| 变量                               | 必填 | 默认值        | 说明                                                   |
| ---------------------------------- | ---- | ------------- | ------------------------------------------------------ |
| `DATABASE_URL`                     | 是   | 无            | PostgreSQL 连接串                                      |
| `DATABASE_MIN_CONNECTIONS`         | 否   | `1`           | 最小连接数                                             |
| `DATABASE_MAX_CONNECTIONS`         | 否   | `20`          | 最大连接数                                             |
| `DATABASE_ACQUIRE_TIMEOUT_SECONDS` | 否   | `5`           | 获取连接超时                                           |
| `JWT_SECRET`                       | 是   | 无            | 至少 32 字节                                           |
| `JWT_ISSUER`                       | 否   | `rustset`     | Token 签发方                                           |
| `JWT_AUDIENCE`                     | 否   | `rustset-api` | Token 受众                                             |
| `JWT_ACCESS_TOKEN_TTL_SECONDS`     | 否   | `900`         | Access Token 有效期                                    |
| `GATEWAY_HOST`                     | 否   | `0.0.0.0`     | 网关监听地址                                           |
| `GATEWAY_PORT`                     | 否   | `8080`        | 网关端口                                               |
| `WEB_PERMISSIVE_CORS`              | 否   | `false`       | 开发调试跨域开关，设为 `true`/`1`/`yes` 时允许任意来源 |
| `REDIS_URL`                        | 否   | 无            | 不配置时缓存和限流自动关闭                             |
| `REDIS_KEY_PREFIX`                 | 否   | `rustset`     | Redis 键前缀                                           |
| `BOOTSTRAP_ADMIN_USERNAME`         | 否   | `admin`       | 初始管理员用户名                                       |
| `BOOTSTRAP_ADMIN_PASSWORD`         | 否   | 无            | 不配置时跳过管理员初始化                               |
| `RUST_LOG`                         | 否   | 框架默认      | tracing 日志过滤规则                                   |

## RustFS 对象存储

网关使用 AWS S3 SDK 访问 RustFS，Compose 环境下 S3 API 默认监听
`http://127.0.0.1:9000`，控制台为 `http://127.0.0.1:9001/rustfs/console/`。
网关支持以下变量：

| 变量 | 默认值 | 说明 |
| --- | --- | --- |
| `RUSTFS_ENDPOINT` | `http://127.0.0.1:9000` | S3 API 地址；网关与 RustFS 在同一 Compose 网络时可设为 `http://rustfs:9000` |
| `RUSTFS_ACCESS_KEY` | `rustset` | S3 access key |
| `RUSTFS_SECRET_KEY` | `rustset_password` | S3 secret key |
| `RUSTFS_REGION` | `us-east-1` | S3 签名区域 |
| `RUSTFS_BUCKET` | `rustset` | 文件 bucket；网关启动时自动检查并创建 |
| `INFRA_UPLOAD_MAX_BYTES` | `52428800` | 单文件大小上限（字节），服务端上传和预签名直传均校验 |

部署时设置独立凭据，例如：

```bash
export RUSTFS_ENDPOINT='http://127.0.0.1:9000'
export RUSTFS_ACCESS_KEY='rustset-local'
export RUSTFS_SECRET_KEY='replace-with-a-long-random-secret'
export RUSTFS_BUCKET='rustset'
docker compose -f script/docker/docker-compose.yml up -d
cargo run -p rustset-gateway
```

服务端上传将文件存入 RustFS，数据库保存文件元数据；下载由网关从 RustFS 读取后返回。前端直传模式会获取有效期 15 分钟的 S3 预签名 PUT URL，再将元数据登记到网关。启用 `VITE_UPLOAD_TYPE=client` 时，`RUSTFS_ENDPOINT` 必须能被浏览器访问；服务端上传模式只要求网关能访问该地址。生产环境应使用独立凭据并限制 API/控制台的网络访问。已有数据库中的本地磁盘文件不会自动搬迁到 RustFS，需要单独迁移旧文件及其对象键。

迁移本地旧文件可在维护窗口运行 `bash script/migrate-local-uploads-to-rustfs.sh`。脚本依赖 `psql` 和 AWS CLI v2；它会迁移所有有效 `/upload/` 文件记录，逐个比对对象大小，并且只在全部对象校验成功后删除对应本地副本。设置 `DATABASE_URL`、`RUSTFS_ENDPOINT`、`RUSTFS_ACCESS_KEY`、`RUSTFS_SECRET_KEY`，可选设置 `RUSTFS_BUCKET`、`RUSTFS_REGION` 和 `INFRA_UPLOAD_DIR`。

## 前端环境变量

开发配置位于 `apps/web/apps/web-antd/.env.development`：

- `VITE_PORT=5666`
- `VITE_BASE_URL=http://127.0.0.1:8080`
- `VITE_GLOB_API_URL=/api`
- `VITE_UPLOAD_TYPE=server`

生产环境应通过部署环境文件或运行时 `_app-config` 将 API 地址设置为网关地址。

## 统一 AI 模型管理

这里的模型配置专指 AI 供应商和推理模型，与 CMDB 中用于定义资产属性结构的“模型管理”相互独立。对话、知识库、Embedding、媒体生成和运维 Agent 全部复用本节所述配置，不再为业务功能单独保存 API Key 或模型端点。

AI 页面统一使用 `ai.model_configs`，业务表只保存模型 ID。模型记录包含：

- `platform`：供应商协议，如 `OpenAICompatible`、`OpenAI`、`Anthropic`、`Gemini`、`AzureOpenAI`、`Midjourney`、`Suno`。
- `type`：`chat`、`image`、`video`、`speech`、`transcription`、`music`、`embedding` 或 `rerank`。
- `model`、`url`、`apiKey`：模型标识、服务根地址和密钥。
- `config`：供应商差异化 JSON 配置。

OpenAI 兼容服务常用配置键：

| 配置键                  | 默认值                    | 用途                                        |
| ----------------------- | ------------------------- | ------------------------------------------- |
| `textPath`              | `/chat/completions`       | 聊天及流式聊天                              |
| `imageGeneratePath`     | `/images/generations`     | 图片生成                                    |
| `videoGeneratePath`     | `/videos/generations`     | 视频生成                                    |
| `speechPath`            | `/audio/speech`           | TTS                                         |
| `embeddingPath`         | `/embeddings`             | Embedding                                   |
| `musicGeneratePath`     | `/music/generations`      | 音乐提交                                    |
| `musicTaskPath`         | `/music/tasks/{taskId}`   | 音乐轮询                                    |
| `midjourneyImaginePath` | `/mj/submit/imagine`      | Midjourney Imagine                          |
| `midjourneyActionPath`  | `/mj/submit/action`       | Midjourney Action                           |
| `midjourneyTaskPath`    | `/mj/task/{taskId}/fetch` | Midjourney 轮询                             |
| `authHeader`            | `Authorization`           | 自定义鉴权头；Authorization 自动使用 Bearer |

图片、视频、配音和 Agent 功能必须选择类型匹配且启用的统一模型 ID，不支持旧 `vendor:model` 配置。

## 异步任务状态

AI 图片和音乐统一采用：

- `10`：处理中
- `20`：成功
- `30`：失败

服务启动后每 10 秒恢复并轮询数据库中的待处理任务；也可调用对应 `/poll` 接口触发立即同步。任务、错误、结果 URL、按钮和轮询次数均持久化，因此重启不会丢失任务。
