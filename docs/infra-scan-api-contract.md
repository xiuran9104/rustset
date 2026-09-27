# 扫描与巡检接口契约

扫描任务和资产巡检的 HTTP 请求、响应结构定义在 `rustset-infra-api`。所有 JSON 字段使用 camelCase；数据库列名和内部任务载荷不属于公开接口。

## 稳定约定

- 任务 ID 为字符串，当前实现使用 UUID。
- 时间字段以 RFC 3339 字符串返回；尚未开始或结束的时间为 `null`。
- 布尔字段始终返回 JSON boolean，不再暴露数据库中的 `0`/`1`。
- 任务状态包括 `queued`、`running`、`retrying`、`completed`、`failed` 和 `cancelled`。
- 列表、详情和巡检接口都从认证用户获取租户，不接受请求方指定 `tenantId`。
- 动态巡检差异保留结构化数组，但差异项、风险引用和外层响应均有明确类型。

前端扫描 API 直接使用相同的 camelCase 字段，不再调用 `scan/compat.ts` 做递归字段转换。新增或修改字段时，应同时更新 `rustset-infra-api` 的契约测试和前端 TypeScript 接口，并确认 OpenAPI、`bun run check:type` 与生产构建通过。
