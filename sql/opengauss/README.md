# openGauss 迁移目录

## 生成方式
由 `script/sql-translate` 工具从 `sql/postgresql/` 翻译产出。

## 与 PostgreSQL 的关系
openGauss 基于 PG 内核，SQL 兼容度高。注意事项：
- 认证：默认 sha256，sqlx 不支持；安装时须设 `password_encryption_type=1`（MD5）
- 其余差异对应用透明

## 验收
`bash script/test-database-migrations.sh --db opengauss`
