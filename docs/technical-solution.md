# RustSet 技术架构

## 产品目标

RustSet 将企业资产、CMDB、基础设施、云资源与物理资源台账、资源交付和 AI 辅助运维纳入统一租户与权限体系。系统采用前后端分离架构，网关负责组合模块、执行鉴权和暴露统一 API，业务规则保留在各 Rust 模块中。

## 总体架构

```text
browser
  |
  v
apps/web                         # Vue 3 / Vben / Ant Design Vue
  |
  v
services/gateway                # Axum HTTP 网关、OpenAPI、迁移启动
  |
  +-- crates/modules/system-*    # 认证、RBAC、租户和后台管理
  +-- crates/modules/infra-*     # 资产、基础设施、云与运维工单
  +-- crates/modules/cmdb-*      # 模型、属性、实例和关系
  `-- crates/modules/ai-*        # 统一 AI 模型与 AI 应用
  |
  +-- PostgreSQL                 # 业务数据和 SQLx 迁移
  +-- Redis                      # 缓存、会话辅助与限流
  +-- NATS                       # 消息基础设施
  `-- RustFS                      # 对象存储
```

## Rust 工作区边界

- `services/gateway`：应用入口、配置加载、数据库迁移、模块路由和横切中间件。
- `crates/framework/common`：统一配置、响应和应用状态。
- `crates/framework/database`：SQLx 连接池、迁移及数据库健康检查。
- `crates/framework/security`：JWT、当前用户、权限校验和密码兼容。
- `crates/framework/web`：HTTP 错误映射、请求追踪与 CORS。
- `crates/framework/redis`、`mq`、`tenant`、`telemetry`：缓存、消息、租户上下文和可观测性。
- `crates/modules/*-api`：跨模块 DTO 与公开契约。
- `crates/modules/*-server`：HTTP 接口、应用逻辑和持久化实现。

依赖方向保持为：

```text
gateway -> module-server -> module-api
module-server -> framework/*
framework/* 不依赖具体业务模块
```

## 领域划分

### System

负责登录、刷新令牌、用户、部门、岗位、角色、菜单、租户、租户网段、字典、通知、OAuth2、日志、文件及任务等后台能力。菜单由数据库动态返回，页面可见性与按钮权限均来自同一套 RBAC 数据。

### Infra 与资产运营

负责资产采集表台账、网络策略、扫描任务、风险、服务商、机房、云平台、云凭据、业务应用、云资源与物理资源台账（infra_cloud_resource / infra_physical_resource 分表存储，工单经 target_resource_type 鉴别引用）、资源工单和审批规则。OpenTofu 开通从云凭据读取 Provider 参数，执行结果写回工单与 CMDB。

资产核查（`/infra/inspection/**`）复用扫描任务表（`infra_task`，`task_kind='inspection'`）与风险表：对明确 IP 列表做 TCP 连通性探测，与资产台账登记、`infra_inspection_baseline` 已确认端口基线比对，仅对「台账未登记」与「超出基线的开放端口」两类差异生成 `infra_risk` 预警；连接超时视为不确定，不产生也不消除预警。预警在独立页面查看、确认基线与复核处置，基线确认须登记在案并填写依据。

### CMDB

CMDB 模型定义配置项的属性结构，实例使用 JSONB 保存动态字段，并支持实例关系。CMDB 的“模型”属于资产数据建模，不包含大模型供应商或 API Key。

### AI

`ai.model_configs` 是全平台唯一的 AI 模型配置源，保存供应商协议、模型类型、端点、API Key 和扩展参数。对话、知识库、Embedding、图片、音乐、写作和运维 Agent 均通过 `ai-server` 的 Provider 工厂加载该配置，业务模块不重复保存大模型连接信息。

## 权限与租户

后端从 JWT 构造当前用户上下文，根据 `system_user_role`、`system_role_menu` 和 `system_menu.permission` 计算权限。接口通过集中路由注册和 `module:resource:action` 权限码校验；未注册或无权限请求默认拒绝。

租户 ID 由认证上下文和受控访问租户共同决定。租户网段归属到具体租户，在租户管理页维护；资产、网段及业务数据的查询和写入必须遵守租户边界。

## 前端

正式前端位于 `apps/web`，主应用包为 `@vben/web-antd`。它使用后端动态菜单和按钮权限，统一请求格式为：

```json
{ "code": 0, "data": {}, "message": "ok" }
```

开发环境的 `/api` 代理到 `http://127.0.0.1:8080`。产品页面统一使用 RustSet 品牌；`@vben/*` 仅作为内部技术包名保留。

## 数据库与迁移

网关在监听端口前运行 `sql/postgresql` 下的 SQLx 迁移。迁移文件是数据库初始化和升级的唯一来源，必须幂等且不能修改已发布版本。`sql/bootstrap/current.sql` 是应用全部迁移后的审查快照，不参与启动。

数据库变更必须执行：

```bash
bash script/test-database-migrations.sh
```

并同步更新迁移数量断言和参考快照。

## 服务入口

- `GET /health`：健康检查
- `GET /openapi.json`：OpenAPI（由 aide 从 axum 路由与处理器类型推导生成，启动时组装；前端“基础功能 → API 接口”页面内嵌 Scalar 渲染，可直接调试）
- `/system/**`：认证和系统管理
- `/infra/**`：资产、基础设施、云与运维
- `/cmdb/**`：CMDB
- `/ai/**`：统一模型和 AI 应用

## 功能边界

- BPM 不属于 RustSet 当前产品范围。
- 扫描执行器目前是轻量能力，不能描述为完整漏洞扫描平台；资产核查是 TCP 差异比对（台账与端口基线），不是漏洞发现。
- 云 Provider 与外部 AI 服务需要在部署环境使用真实凭据验证。
- API 不得通过固定成功值或空数据伪装未实现能力。
