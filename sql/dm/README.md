# 达梦 DM8 迁移目录

## 生成方式
`cargo run -p sql-translate -- --target dm --output sql/dm`，
一致性测试把关；`migrate_with_flavor(Dameng)` 当前显式拒绝（ODBC 适配器
未实装，M3）——本目录是翻译产物，等待 Rust 侧接入。

## 真机实测状态（2026-11-08，DM8 1-3-12 ENT，sizx/dm8 容器）
- disql 连通（CHARSET=1 UTF-8、LENGTH_IN_CHAR=1、CASE_SENSITIVE=0）
- 0002 全量 0 错（17 表落库）；其余 57 个文件错误已收敛到逐文件清单
  （每文件 1–18 条，集中在 plpgsql 触发器语法、VALUES CTE 复杂形态、
  序列默认值作用域），是后续规则迭代的直接输入
- 0001（34.6 万行 pg_dump）不能直接走 disql include（大文件下静默跳过），
  需按 §4.1.6.3 走拆分导入流水线

## 翻译规则（全部真机证据驱动，17 个单测）
| PG 构造 | 达梦处置 |
|---|---|
| `jsonb`/`uuid`/`boolean`/`bytea` | TEXT / CHAR(36) / BIT / BLOB（词边界安全） |
| `character varying(n)` | VARCHAR(n) |
| `timestamptz` | TIMESTAMP WITH TIME ZONE；`without time zone` 词组剥离 |
| `nextval('x'::regclass)` | `x.NEXTVAL`（含闭括号平衡） |
| `ILIKE` | `UPPER() LIKE UPPER()`（操作数边界提取） |
| `USING gin`/`USING btree` | 剥离（达梦默认索引类型） |
| 部分索引 WHERE | 剥离谓词**保留分号**（实测：吞分号导致语句粘连） |
| `IF NOT EXISTS` | 剥离 + 登记幂等风险（267 处待 DO 块化，同 og 方案） |
| `public.` 前缀 | 剥离（达梦保留字且无此 schema） |
| 保留字列名（`domain` 等实测命中项） | 双引号包裹 |
| `''::TYPE(n)` cast | 剥离（达梦无 `::` 语法） |
| 序列 `NO MINVALUE`/`NO MAXVALUE`/`AS type`/`CACHE n` | 剥离（-3602/-2223 实测拒绝） |
| `WITH x AS (VALUES …)` 行集 | `x AS (SELECT … FROM DUAL UNION ALL …)`（达梦无 VALUES 行集构造；含多字节字符安全配对扫描、SQL `''` 转义感知） |

## 已登记未解决（后续迭代输入）
- plpgsql 触发器/函数体（`$$` 块）需按达梦语法逐个改写（0026 等 14 处）
- `ADD COLUMN IF NOT EXISTS` 幂等化：按 openGauss 方案做 DO 块 + 异常捕获
- `ON CONFLICT` 1 处（0049）→ MERGE INTO
- disql 大文件 include 限制：0001 需语句级拆分（流水线脚本预留）

## 验收
达梦真机端到端回归需 Rust ODBC 适配器（M3）+ 全部规则收敛后执行。
