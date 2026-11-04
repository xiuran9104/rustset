# 金仓 KingbaseES V8R6 迁移目录

## 生成方式
由 `script/sql-translate` 工具从 `sql/postgresql/` 翻译产出，不手工编写。

## 与 PostgreSQL 的关系
金仓 V8R6 以 PG 兼容模式运行，大部分 SQL 直接兼容：
- `jsonb`/`uuid` 类型：兼容（PG 内核）
- `ON CONFLICT`：兼容
- `ilike`：兼容
- `ANY(数组)`：兼容

`0001_initial.sql` 由"PG 参考库 → 金仓逻辑导出 → 比对"流水线生成。
`0002+` 如完全兼容则直接复制 `sql/postgresql/` 对应文件。

## 验收
`bash script/test-database-migrations.sh --db kingbase` 在金仓容器中
执行空库到最新 schema + 基线数据验收。
