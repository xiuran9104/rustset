# 金仓 KingbaseES V8R6 迁移目录

## 生成方式
`cargo run -p sql-translate -- --target kingbase --output sql/kingbase`，
一致性测试把关；`migrate_with_flavor(Kingbase)` 编译期内嵌本目录。

## 验证状态（2026-11-08 真机实测）
**已在 KingbaseES V008R006C005B0013（容器）端到端验证**：
- 59 个迁移（含 34.6 万行 0001 基线）全部执行成功，幂等重跑通过
- 数据库合同套件 7/7 全绿（事务原子性/FOR UPDATE/锁超时/错误分类/隔离级别）
- R2 数据比对：PG 参考库 → 金仓 全表行数 + 抽样 checksum 一致

## 关键事实（实测）
- `database_mode` 是 **initdb 级**参数：默认 oracle（`''` 即 NULL，击穿
  NOT NULL 语义），必须以 `-m pg` 初始化集群；容器入口脚本写死默认模式，
  需卷预初始化绕过（见 `script/test-database-migrations.sh` start_kingbase）
- libpq/sqlx 直连 `kingbase://` 线协议无障碍（社区镜像 system/123456）
- 许可限制 `max_connections=10`（社区镜像；企业许可另议）

## 翻译规则（唯一一条）
| PG 构造 | 金仓处置 |
|---|---|
| `NULLS NOT DISTINCT`（索引+约束） | 逐表达式 `COALESCE` 唯一索引（与 openGauss 同构） |

其余（`ON CONFLICT`/`DO NOTHING`/`FILTER`/jsonb/`to_regclass`/
`ADD COLUMN IF NOT EXISTS`/`FOR UPDATE SKIP LOCKED`）实测原生支持。

## 验收
`bash script/test-database-migrations.sh --db kingbase`（社区镜像全自动；
`KINGBASE_IMAGE` 可覆盖为企业许可镜像）。
