# 对外 MCP 服务

RustSet 网关内置 MCP（Model Context Protocol）服务端，把平台的资产、CMDB、
风险、扫描任务、资源工单与 AI 对话能力以标准 MCP 工具形式暴露给外部 AI
客户端（Claude Code、Cursor、Cline 等）。

## 端点与传输

- `POST /mcp`：Streamable HTTP 传输，单条 JSON-RPC 2.0，响应纯 JSON。
  notification（无 `id` 的消息，如 `notifications/initialized`）返回 `202`。
- `GET /mcp` / `DELETE /mcp`：`405`（无服务端推送流；工具均为请求-响应型）。
- 无状态实现：不分配 `Mcp-Session-Id`，客户端可任意重试。
- 协议版本：`2024-11-05` / `2025-03-26` / `2025-06-18`（initialize 时回显
  客户端携带的已知版本，否则回落 `2025-03-26`）。
- 开关：网关环境变量 `MCP_ENABLED`（默认 `1`；设为 `0` 完全不挂载路由）。

## 鉴权

所有 `/mcp` 请求都要求 API Key（含 `initialize`）：

```
Authorization: Bearer mcp_xxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxxx
```

也接受 `X-Api-Key: mcp_...` 请求头。

- 密钥格式：`mcp_` + 43 位 base62（≈256bit）；**库里只存 SHA-256 哈希**，
  创建响应中明文仅出现一次，丢失只能删除重建。
- 密钥绑定一个平台用户：权限校验（每个工具对应一个权限码）与租户数据
  隔离完全继承该用户，且以数据库中的最新角色/权限为准。
- 密钥可停用（`enabled=0`）、可设过期时间；停用/过期/绑定用户被停用均
  立即拒绝（401）。
- 请求经网关审计与限流中间件，与普通 API 一致。

## 密钥管理

- 界面：系统管理 → 基础设置 → **MCP 服务**（`/system/mcp-key`，菜单 +
  `system:mcp-key:*` 权限由迁移 0060 下发，超管默认可见）。
- 接口：`/system/mcp-key/page|get|create|update|delete`。
- 创建时选择绑定用户（必填）与可选过期时间；成功弹窗展示完整密钥并仅此一次。

## 工具清单（tools/list）

| 工具 | 参数 | 权限码 |
| --- | --- | --- |
| `asset_page` | pageNo, pageSize | infra:asset:query |
| `asset_get` | id | infra:asset:query |
| `network_policy_page` | pageNo, pageSize | infra:network-policy:query |
| `risk_page` | pageNo, pageSize | infra:risk:query |
| `scan_task_page` | pageNo, pageSize | infra:task:query |
| `scan_task_get` | id | infra:task:query |
| `ticket_page` | pageNo, pageSize | infra:resource-ticket:query |
| `ticket_get` | id | infra:resource-ticket:query |
| `cmdb_model_page` | keyword, pageNo, pageSize | cmdb:model:query |
| `cmdb_instance_search` | modelId, keyword, pageNo, pageSize | cmdb:instance:query |
| `cmdb_instance_get` | id | cmdb:instance:query |
| `ai_model_list` | type? | ai:model:query |
| `ai_chat_send` | content, conversationId?, useContext? | ai:chat:query |

- `ai_chat_send` 使用平台已配置的模型与知识库（非流式）；`conversationId`
  省略或为 0 时自动创建新对话。
- `ai_model_list` 绝不返回 api_key/url/config 等敏感字段。
- 业务错误（含权限拒绝）按 MCP 惯例以 `isError: true` 的工具结果返回；
  权限拒绝额外带 `forbidden: true` 标记。

## 客户端接入示例

Claude Code（Streamable HTTP + Bearer）：

```bash
claude mcp add --transport http rustset http://<网关地址>/mcp \
  --header "Authorization: Bearer mcp_你的密钥"
```

Cursor（`.cursor/mcp.json`）：

```json
{
  "mcpServers": {
    "rustset": {
      "url": "http://<网关地址>/mcp",
      "headers": { "Authorization": "Bearer mcp_你的密钥" }
    }
  }
}
```

curl 手工验证（会话三步）：

```bash
# 1. initialize（注意回显协议版本）
curl -s http://127.0.0.1:8080/mcp \
  -H 'Authorization: Bearer mcp_...' -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"curl","version":"0"}}}'

# 2. notification（202，无响应体）
curl -s -o /dev/null -w '%{http_code}\n' http://127.0.0.1:8080/mcp \
  -H 'Authorization: Bearer mcp_...' -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","method":"notifications/initialized"}'

# 3. 工具调用
curl -s http://127.0.0.1:8080/mcp \
  -H 'Authorization: Bearer mcp_...' -H 'Content-Type: application/json' \
  -d '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"cmdb_model_page","arguments":{"pageSize":5}}}'
```

## 反向代理说明

Nginx 按既有 `/api/` 规则反代即可（JSON 请求-响应，无流式特殊要求）；
若未来增加 SSE 形态的推送，需要 `proxy_buffering off`（与 AI 流式一致）。

## 实现说明

- 代码：`crates/modules/mcp-server`（JSON-RPC 层 + 工具注册表），
  `system-server` 的 `mcp.rs`（密钥鉴权）与 `management/mcp_key.rs`（CRUD）。
- 工具直接复用业务层查询函数（无内部 HTTP 转发）；表 `system_mcp_api_key`
  由迁移 `0060` 建立（kingbase/opengauss 同步拷贝，DM8 目录暂空遵循现状）。
- 批量 JSON-RPC 请求暂不支持（返回 `-32600`）。
