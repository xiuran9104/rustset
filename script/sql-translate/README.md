# sql-translate

SQL 方言翻译工具：将 `sql/postgresql/` 迁移文件翻译为金仓 / openGauss / 达梦方言。

## 用法

```bash
# 达梦：规则翻译 + 评审报告，输出到暂存目录（默认建议）
cargo run -p sql-translate -- --target dm --output target/sql-translate/dm --skip-baseline

# 查看逐条评审项（文件 + 行号 + 类别）
cargo run -p sql-translate -- --target dm --output target/sql-translate/dm --report

# 金仓 / openGauss：PG 兼容模式，逐字复制
cargo run -p sql-translate -- --target kingbase --output target/sql-translate/kingbase

# CI 门禁：仍有评审项时以非零码失败（翻译链固化后接入）
cargo run -p sql-translate -- --target dm --output sql/dm --strict
```

参数：`--target <kingbase|opengauss|dm>`、`--output <dir>`（必填，防止误写源目录）、
`--source <dir>`（默认 `sql/postgresql`）、`--report`（逐条打印评审项）、
`--strict`（评审项即失败）、`--skip-baseline`（跳过 `0001_initial.sql`）。

## 翻译规则

详见 `docs/xinchuang-adaptation-plan.md` §4.1.2 与 `sql/dm/README.md`。

确定性改写（达梦）：`jsonb→TEXT`、`uuid→CHAR(36)`、`boolean→BIT`、`bytea→BLOB`、
`timestamptz→TIMESTAMP WITH TIME ZONE`、`nextval('x'::regclass)→x.NEXTVAL`、
`ILIKE→UPPER() LIKE UPPER()`、剥离 `USING btree`。

评审项（只标记不改写，人工确认后登记）：`ON CONFLICT`（达梦 MERGE 重写）、
`EXCLUDE` 约束、数组列/`'{}'` 字面量、`interval` 字面量、`FOR UPDATE` 事务流、
`IF NOT EXISTS`（达梦无此语法，剥离后需逐条确认幂等）、gin/部分索引降级、
PG 专有函数。

## 0001 基线流水线（未实现）

`0001_initial.sql`（34.6 万行 pg_dump）按方案 R2 走独立流水线：
PG 参考库全量导出 → 翻译 → 达梦导入 → **行数 + 抽样 checksum 自动比对**，
禁止人工目检；比对不过即阻断。`--skip-baseline` 之外的情形当前只支持
逐文件翻译入口。

## 状态

- [x] 规则引擎 + 评审报告（`src/rules.rs`，11 个单元测试）
- [x] CLI（`--target/--source/--output/--report/--strict/--skip-baseline`）
- [x] 金仓 / openGauss：PG 兼容模式逐字复制 + 评审扫描
- [x] 达梦：确定性规则已实现；57 个手写迁移（0002–0058）翻译产出 337 个评审项
- [ ] 达梦：`IF NOT EXISTS` 幂等化改写（需达梦异常块语法 + 真库验证）
- [ ] 达梦：`ON CONFLICT → MERGE INTO` 语句级重写
- [ ] 0001 流水线（PG 参考库导出 → 翻译 → 导入 → checksum 比对）
- [ ] 翻译产出入库 `sql/{kingbase,opengauss,dm}`（需厂商容器回归 + 人工评审后）

## 入库约定

达梦翻译产物默认写暂存目录（如 `target/sql-translate/dm`）；**不得**在
0001 流水线验证 + 人工评审完成前把生成物提交进 `sql/dm`——
`crates/framework/database/tests/migrations.rs` 的
`flavor_migration_directories_stay_in_sync_with_postgresql` 测试会在
`sql/<flavor>` 出现与 `sql/postgresql` 编号不一致的文件集时阻断构建。
