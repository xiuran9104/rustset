# openGauss 迁移目录

## 生成方式
由 `script/sql-translate --target opengauss` 从 `sql/postgresql/` 翻译产出，
一致性测试（`flavor_migration_directories_stay_in_sync_with_postgresql`）把关。

## 验证状态（2026-11-06 实测）
**已在 openGauss 7.0.0-RC2（容器）端到端验证**：
- 认证机制实测（修正）：镜像预置 `gaussdb` 远程用户且口令哈希为 **MD5**
  （= `GS_PASSWORD`）——libpq/sqlx 直连即可，无需 gs_guc 改参；初始超户
  `omm` 是 sha256 且仅限本地；`gsql` 客户端永远先协商 sha256，**不能**
  用它探测 md5 认证（会误报 Invalid username/password）
- 建库必须 `DBCOMPATIBILITY = 'PG'`（默认 A 模式下 `''` 即 NULL，破坏 NOT NULL 语义）
- 59 个迁移（含 34.6 万行 0001 基线）全部导入成功
- 迁移 + 合同测试套件在 og 上全绿（幂等重跑含在内）

## 翻译规则（全部由真机兼容矩阵探测驱动）
| PG 构造 | openGauss 处置 |
|---|---|
| `ON CONFLICT … DO UPDATE` | `ON DUPLICATE KEY UPDATE`（EXCLUDED 可用） |
| `ON CONFLICT DO NOTHING` | 首列自赋值 upsert（og 无 DO NOTHING） |
| `NULLS NOT DISTINCT` | 逐表达式 `COALESCE(col::text, 标记)` 唯一索引 |
| `ADD COLUMN IF NOT EXISTS` | 逐列 DO 块 + `duplicate_column` 异常（保持幂等） |
| 内联/ALTER 形式 identity | 显式 `CREATE SEQUENCE OWNED BY` + `SET DEFAULT nextval` |
| `to_regclass`/`to_regnamespace` | `pg_class`/`pg_namespace` EXISTS 查询 |
| `U&'\XXXX'` | `chr()` 连接 |
| `jsonb -> text键` | `jsonb_extract_path`（og 把 `->` 解析为 int 下标） |
| gin `jsonb_path_ops` | 剥离（og 默认 jsonb_ops） |
| 同列多子句 ALTER COLUMN | 拆分语句（og 误报 cannot alter type twice） |
| 未知 GUC SET | 剥离（`lock_timeout`/`row_security` 等 og 不识别） |

## 已登记平台差异（§1.4.6）
- og 无 `lock_timeout` GUC——锁等待超时不变量在 og 上依赖死锁检测与
  `statement_timeout`，合同测试该项跳过并留痕
- 唯一冲突错误响应不回传约束名字段——分类不变量保持，名字透传仅 PG 家族断言

## 恢复演练
`bash script/test-database-migrations.sh --db opengauss`（CI 矩阵门禁
`XINCHUANG_DB_MATRIX` 开启后自动执行）。
