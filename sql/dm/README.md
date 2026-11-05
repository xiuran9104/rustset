# 达梦 DM8 迁移目录

## 生成方式
由 `script/sql-translate` 工具从 `sql/postgresql/` 翻译产出，需人工评审。

## 翻译规则（详见 docs/xinchuang-adaptation-plan.md §4.2）
| PG 构造 | 达梦 DM8 |
|---|---|
| `jsonb` | `TEXT`（应用侧 serde_json 编解码） |
| `uuid` | `CHAR(36)` |
| `ON CONFLICT` | `MERGE INTO` |
| `ilike` | `LIKE UPPER()`（源码已改为通用写法） |
| `ANY(数组)` | `IN (…)` 展开（源码已改为通用写法） |
| `interval` | 数值列 + chrono 计算 |
| `serial` | `IDENTITY(1,1)` |
| `boolean` | `BIT` |
| gin 索引 | 普通索引（功能降级登记） |
| EXCLUDE 约束 | 唯一索引或应用保证 |

## 验收
`0001_initial.sql`（34.6万行）走流水线：PG 参考库导出 → 翻译 → DM 导入 → 行数+checksum 比对。
流水线脚本已就绪（`script/sql-translate/verify-pipeline.sh`，在 openGauss 上
端到端实测通过）；达梦侧等 ODBC spike（社区镜像不可得，需厂商介质）。
