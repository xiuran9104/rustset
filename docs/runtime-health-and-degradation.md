# 运行时健康检查与降级

网关提供三个健康端点：

- `GET /health`：兼容原有部署探针的存活检查。
- `GET /health/live`：只判断进程能否处理 HTTP 请求，不访问外部依赖。
- `GET /health/ready`：检查 PostgreSQL，以及配置为必需的 Redis 能力。依赖不可用时返回 HTTP 503。

就绪响应同时包含 PostgreSQL 连接池大小和空闲连接数、扫描任务排队/重试/运行数量、最老任务等待时间，以及访问审计队列的排队、成功、丢弃和失败计数。Redis 未启用或作为可选依赖不可用时会显示降级状态，但不会阻止实例接收流量。

## Redis 故障策略

缓存和限流分别建立连接并独立配置：

| 环境变量 | 默认值 | 含义 |
| --- | --- | --- |
| `CACHE_REDIS_ENABLED` | 设置 `REDIS_URL` 时为 true | 启用认证用户缓存 |
| `CACHE_REDIS_REQUIRED` | false | 缓存连接失败时阻止网关启动并使就绪失败 |
| `RATE_LIMIT_REDIS_ENABLED` | 设置 `REDIS_URL` 时为 true | 启用 Redis 限流 |
| `RATE_LIMIT_REDIS_REQUIRED` | false | 限流连接失败时阻止网关启动并使就绪失败 |

运行期间 Redis 限流检查失败采用放行策略并记录告警；缓存读取失败回退数据库。生产环境需要强制限流时，应设置 `RATE_LIMIT_REDIS_REQUIRED=true` 并对就绪状态告警。

## 访问日志队列

普通 HTTP 访问日志属于可丢弃的运行日志，使用有界内存队列批量写入 PostgreSQL，不再为每个请求创建 Tokio 任务。健康检查不写访问日志。

| 环境变量 | 默认值 | 范围 |
| --- | ---: | ---: |
| `AUDIT_QUEUE_CAPACITY` | 4096 | 64–65536 |
| `AUDIT_BATCH_SIZE` | 100 | 1–1000 |
| `AUDIT_FLUSH_INTERVAL_MS` | 1000 | 50–60000 |

队列已满时请求本身不失败，日志记入 `dropped` 计数；数据库批量写入失败记入 `failed`。登录、令牌和业务操作审计继续走同步数据库路径，不进入这条可丢弃队列。
