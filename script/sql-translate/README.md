# sql-translate

SQL 方言翻译工具：将 `sql/postgresql/` 迁移文件翻译为金仓 / openGauss / 达梦方言。

## 用法（实现后）

```bash
# 翻译到金仓（大部分直接复制）
cargo run -p sql-translate -- --target kingbase

# 翻译到 openGauss（大部分直接复制）
cargo run -p sql-translate -- --target opengauss

# 翻译到达梦（需要规则映射）
cargo run -p sql-translate -- --target dm --review
```

## 翻译规则

详见 `docs/xinchuang-adaptation-plan.md` §4.2 和 `sql/dm/README.md`。

## 状态

- [ ] 金仓翻译器（预期大部分直接复制）
- [ ] openGauss 翻译器（预期大部分直接复制）
- [ ] 达梦翻译器（jsonb→TEXT, uuid→CHAR(36), ON CONFLICT→MERGE, interval→数值）
- [ ] 0001 流水线（PG 参考库导出 → 翻译 → 导入 → checksum 比对）
