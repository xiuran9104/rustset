# CMDB 租户边界

迁移 `0026_cmdb_tenant_isolation.sql` 将实例、实例关系和作为开通来源的资源工单按租户隔离。模型和属性定义仍是共享目录，由既有模型/属性管理权限控制；这些权限允许修改共享结构，应只授予负责共享目录的管理员。

## 身份与访问规则

- `TenantContext` 只能从认证后的 `CurrentUser` 或已授权记录的持久化归属构造。请求体、查询参数、`tenant-id` / `visit-tenant-id` 请求头和 AI 工具参数不能选择实例租户。
- 认证层使用数据库中账户的最新归属；归属与权限缓存不一致时重新加载权限。超级管理员也必须有有效的租户上下文，不存在实例跨租户绕过。
- 实例列表、详情、更新、单条/批量删除、导入导出、模型列表实例数量和 AI 查询均过滤租户。访问其他租户或待确认历史实例的详情/更新/单删返回 404；批量删除只处理当前租户内的 ID，其他 ID 与不存在的 ID 一样忽略。
- 关系仅连接同租户实例，由复合外键兜底；绑定时锁住两个端点，防止与删除竞争。关系查询、解绑以及实例删除时的关系清理均使用相同租户。
- 唯一值按 `(tenant_id, model_id, value)` 登记，不同租户可复用相同值。`NULL` 归属的历史实例仍作为独立隔离范围参与唯一性检查。分配历史归属时也必须通过唯一约束。
- 新工单保存认证租户，工单 CRUD、审批、开通、交付和 AI 建单/查询使用该归属。自动开通携带提交时的租户，CMDB 回写同时校验工单持久化归属。已归属记录不可经 API 改租户，数据库触发器也禁止重分配。

## 历史数据待确认

迁移不会依据创建人、名称、工单号或最低租户 ID 猜测归属。既有实例、关系和工单的 `tenant_id` 保持 `NULL`，普通接口和 AI 查询不可见，新记录禁止空归属。没有提供自动认领 API。

管理员可在数据库中查看待确认清单（这些查询不对业务接口开放）：

```sql
SELECT id, model_id, creator, create_time
FROM cmdb_instance WHERE tenant_id IS NULL AND deleted = 0 ORDER BY id;

SELECT id, source_id, target_id
FROM cmdb_relation WHERE tenant_id IS NULL AND deleted = 0 ORDER BY id;

SELECT id, ecs_name, created_by, create_time
FROM infra_resource_ticket WHERE tenant_id IS NULL AND deleted = 0 ORDER BY id;
```

确认一批具体 ID 的归属后，在维护事务中先按 ID 顺序锁住相关模型，再锁实例并将这些指定实例的 `tenant_id` 从 `NULL` 更新为已核实的租户 ID。逐项检查影响行数。相关工单也必须逐项确认后分配，不能用执行开通操作的人员归属代替历史工单归属。最后仅为两端已归属同一租户、且关系归属已确认的指定关系设置租户。唯一冲突、外键冲突或影响行数不符合预期时回滚并核对，不删除冲突记录。无法确认的记录继续保持 `NULL`。

本次没有将扫描任务、资产台账、云凭据、云资源或物理资源改为租户所有；不能将 CMDB 的隔离保证扩展理解为整个 Infra 模块已经隔离。

## 验证

设置 `TEST_DATABASE_URL` 指向可创建测试 schema/数据库的临时 PostgreSQL 后：

```bash
cargo test -p rustset-cmdb-api -p rustset-cmdb-server -p rustset-infra-server \
  -p rustset-ai-server -p rustset-framework-tenant --lib -- --include-ignored
bash script/test-database-migrations.sh
cargo check -p rustset-gateway
```

测试覆盖跨租户 CRUD、导入导出、关系、模型数量、历史隔离、伪造租户参数、AI 权限与租户过滤、工单归属及并发回写。数据库迁移脚本验证完整 28 个迁移，也重复执行最新迁移验证幂等性。参考快照只用于审阅，不参与启动。
