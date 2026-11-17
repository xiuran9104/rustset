# RustSet 信创适配方案

> 状态：待评审（评审通过后作为分阶段实施依据）
> 编制基线：2026-11，`main` 分支，迁移链至 `0058_firewall_credential.sql`

## 1. 目标与范围

### 1.1 适配矩阵

| 维度 | 目标 | 说明 |
| --- | --- | --- |
| CPU | 海光 7000 系列、兆芯 KH 系列（x86_64）；鲲鹏 920、飞腾 D2000/S2500（aarch64） | **不含龙芯 LoongArch64** |
| 操作系统 | 银河麒麟高级服务器版 V10 SP2/SP3（rpm 系）；统信 UOS 服务器版 V20 1060a/1070e（deb 系） | 以实际采购版本为准 |
| 数据库 | 达梦 DM8、人大金仓 KingbaseES V8R6、openGauss（集中式） | PostgreSQL 保留为开发基线 |
| 中间件 | Redis 或目录内替代（TongRDS）；Nginx 或目录内替代（Tengine/TongHttpServer）；对象存储 RustFS | 见 §6 |
| 浏览器 | 麒麟/UOS 自带浏览器及主流国产浏览器（Chromium/Firefox 内核） | 前端零改造，仅回归声明 |
| 密码合规 | 国密算法全面落地 + 商用密码应用安全性评估（密评，依据 GB/T 39786-2021） | 目标级别待决策（见 §10） |

### 1.2 不在范围内

- 龙芯 LoongArch64（rustls 的 aws-lc-rs/ring 在该架构编译支持差，如后续需要单独立项）。
- 数据库分布式部署形态（openGauss 仅支持集中式单机/主备）。
- 申威 SW64 等其他国产 CPU。

## 2. 现状盘点（代码事实）

以下数字来自当前 `main` 的实测，是工作量的直接依据：

| 项 | 现状 | 信创影响 |
| --- | --- | --- |
| 国密算法 | `crates/framework/gm`：SM3、SM4-CBC 落盘密封（`enc:sm4:v2:`）、PBKDF2-HMAC-SM3 口令、HMAC-SM3 令牌；`0018` 迁移已完成存量 re-seal | 密评的基础已具备，缺口在传输层与密钥管理（§7） |
| 数据库访问 | sqlx 仅开 `postgres` feature；**0 个编译期宏**，全部运行时查询（约 750 处、62 个文件）；`PgPool` 在业务代码直接出现 213 处；`framework/database` 直接 re-export `sqlx::{PgPool, Postgres, Transaction}` | 没有编译期宏是重大利好（不依赖活库连接编译）；但 `PgPool` 类型耦合必须先做门面重构 |
| 迁移链 | 58 个迁移共 34.9 万行，其中 `0001_initial.sql`（pg_dump 合并基线）独占约 34.6 万行；其余 57 个仅约 3,400 行 | `0001` 翻译必须走"参考库导出→翻译→回灌"流水线，不能手工 |
| 迁移中的 PG 方言 | `jsonb`×62、`ON CONFLICT`×1、`ilike`×6、`interval`×7、`ARRAY[`×6、`EXCLUDE` 约束×1、gin 索引×1、`FOR UPDATE`×6 | 总量小，可逐条规则化翻译 |
| 运行时 SQL 中的 PG 方言 | `ANY(`×78、`now()`×285、`ilike`×33、`::jsonb`×31、`::uuid`×24、`interval`×27、`FOR UPDATE`×21、`ON CONFLICT`×12、json 操作符×4、`to_char`/`EXTRACT`×5+5 | SQL 可移植化改造的主要对象（§4.3） |
| ID 生成 | `gen_random_uuid` 为 0，UUID 全部应用侧 `uuid::Uuid::new_v4()` 生成 | 达梦无 uuid 类型时翻译为 `CHAR(36)` 无额外成本 |
| 消息队列 | NATS 仅存在于 compose，**业务代码零引用**；`framework/mq` 只有 `DomainEvent` 空壳 | 从交付物中移除 NATS；后续 MQ 直接按目录选型 |
| 对象存储 | RustFS（国产开源，S3 协议） | 已合规，保留 |
| TLS | reqwest + rustls（aws-lc-rs）；x86_64 与 aarch64 均有官方支持 | 无龙芯则无需更换 TLS provider |
| 前端 | Vue 3 / Vben / Ant Design Vue，vite 产物纯静态，无插件依赖（无 ActiveX 等） | 架构通用，双架构一份产物 |
| 审计 | 网关已有 audit 中间件 | 等保/密评需补留存期核对（§7.4） |

## 3. 总体策略

四条并行主线，关键路径是数据库线：

```text
M0 决策与基线（锁版本、拿介质、密评预沟通）
 |
 +-- 数据库线：M1 门面重构+SQL可移植化+金仓 → M2 openGauss → M3 达梦   ← 关键路径
 +-- 构建交付线：M4 双架构编译、OS 镜像、离线包（与 M1 并行）
 +-- 密码合规线：M5 TLCP 网关、密钥管理、硬件密码模块（依赖 M0 决策）
 +-- M6 全矩阵联调 + 测评支撑（收口）
```

核心原则：

1. **先抽象，后适配**：任何数据库适配动手前，先完成 `PgPool` → 数据库门面的机械重构，把方言差异隔离在 `framework/database` 一处，业务模块 62 个文件只改一次。
2. **PG 家族与达梦分轨**：PG / 金仓 / openGauss 都讲 PG 线协议，继续走 sqlx；达梦无 PG 协议，走 ODBC 独立适配器，挂在同一门面下。
3. **SQL 尽量方言中立化**：能改成三库通用写法的（`ilike`、`::cast`、`ANY(数组)`），在源码里直接改掉，而不是维护多份查询；只有类型系统差异（jsonb/uuid）留给门面映射。
4. **迁移链四套目录、一份源头**：`sql/postgresql` 仍是唯一手写源头，金仓/openGauss/达梦目录由翻译工具产出 + 人工评审，测试脚本按方言验收。

## 4. 数据库适配设计（核心）

### 4.1 数据库门面（M1 前置重构）

在 `crates/framework/database` 内新增门面层：

- `DbPool`：连接池门面。PG 家族实现包装 `sqlx::PgPool`；达梦实现包装 ODBC 连接池（§4.6）。
- `DbRow`：行读取门面。统一 `get::<T>()` 覆盖现有用到的类型集（`i64`、`String`、`bool`、`f64`、`chrono::DateTime<Utc>`、`uuid::Uuid`、`serde_json::Value`）。达梦侧 ODBC 读出的字符串/字节在此层解码。
- `DbTransaction`：事务句柄，覆盖现有 `FOR UPDATE`（21 处）对应的事务流。
- `DatabaseFlavor` 枚举：`Postgres | Kingbase | OpenGauss | Dameng`，由 `DATABASE_URL` scheme 或 `DB_FLAVOR` 环境变量决定，供少数确实需要方言分支的 SQL 使用。
- 迁移执行器按 flavor 选择 `sql/{postgresql,kingbase,opengauss,dm}` 目录。

业务代码改动是机械的：213 处 `PgPool` 引用替换为 `DbPool`，查询字符串不变。**这是一次性成本，必须最先做，否则每个数据库适配都要重复侵入 62 个文件。**

### 4.2 方言差异与翻译规则

| PG 构造 | 金仓 V8R6 | openGauss | 达梦 DM8 | 处理策略 |
| --- | --- | --- | --- | --- |
| `jsonb` 列 | 兼容（PG 内核） | 兼容 | 无 → `TEXT`/`CLOB`，应用侧 serde_json 编解码 | 类型由门面映射；达梦翻译为 TEXT |
| `uuid` 列 | 兼容 | 兼容 | 无 → `CHAR(36)` | 翻译规则；ID 已应用侧生成 |
| `ON CONFLICT` | 支持 | 支持 | 无 → `MERGE INTO` | 迁移×1 + 运行时×12，达梦分支改写 |
| `ilike` | 支持 | 支持 | 无 → `LIKE UPPER()` | 源码直接改成 `LIKE UPPER()`（三库通用） |
| `ANY($1)` 数组绑定 | 支持 | 支持 | 无 | 源码改为 `IN (…)` 展开（门面提供 `expand_in()` 工具） |
| `::jsonb`/`::uuid` cast | 兼容 | 兼容 | 无 | 源码去掉 cast，绑定参数类型由门面处理 |
| `interval` | 兼容 | 兼容 | 支持但语义有差 | 运行时×27 处改用 chrono 计算 + 传数值；迁移×7 逐条翻译 |
| `now()` | 兼容 | 兼容 | 兼容 | 不动（285 处） |
| `EXCLUDE` 约束 | 兼容 | 兼容 | 无 | 唯一 1 处（0035 租户唯一）→ 唯一索引或应用保证 |
| gin 索引 | 兼容 | 兼容 | 无 | 唯一 1 处 → 普通索引，功能降级登记 |
| `to_char`/`EXTRACT` | 兼容 | 兼容 | 部分兼容 | 运行时 10 处改 chrono 格式化后传参 |
| serial/identity | 兼容 | 兼容 | `IDENTITY(1,1)` | 翻译规则 |

### 4.3 运行时 SQL 可移植化（M1 内容）

按 §4.2 把源码里的 PG 专有写法改成三库通用子集，预计触及 150–200 处查询点。完成后，运行时 SQL 仅剩三类合法方言分支：`ON CONFLICT`（达梦走 MERGE 分支）、jsonb 读写（门面处理）、极少数日期运算。

验收：`cargo test --workspace` 全绿 + 纯 PG 环境全功能回归不劣化。

### 4.4 金仓 KingbaseES（M1，最低成本）

- 实例以 **PG 兼容模式**建库（金仓支持 Oracle/PG/MySQL 多兼容模式，必须选 PG 模式）。
- sqlx PG 协议直连；重点回归 uuid/jsonb 的 OID 扩展类型编解码、`FOR UPDATE`、事务隔离行为。
- `sql/kingbase/`：`0001` 由"PG 参考库 → 金仓逻辑导出 → 比对"生成，`0002+` 直接复用 `sql/postgresql`（金仓兼容则符号链接或拷贝，由翻译工具判断）。

### 4.5 openGauss（M2）

- **认证是第一坑**：openGauss 默认 `password_encryption_type=2`（sha256），libpq 系客户端（含 sqlx）不支持。方案：建库时为应用账号设 `password_encryption_type=1`（MD5）或 `0`，安装文档写死该步骤。
- 其余与 PG 差异（线程池、`enable_opfusion` 等）对应用透明；全量回归 + 压测一轮。

### 4.6 达梦 DM8（M3，最高成本与风险）

达梦无 PG 线协议，官方无 Rust 驱动。路线：

1. **连接层**：`odbc-api` crate + 达梦随介质提供的 ODBC 驱动（`libdodbc.so`），实现 `DbPool`/`DbRow`/`DbTransaction`。备选：达梦 DPI（C 接口）自行 FFI，成本更高，仅在 ODBC 质量不达标时启用。
2. **类型映射**：`uuid→CHAR(36)`、`jsonb→TEXT`、`timestamptz→TIMESTAMP(6) WITH TIME ZONE`、`boolean→BIT`、`bigserial→IDENTITY`、`interval→数值列`。
3. **迁移链**：新增 `script/sql-translate`（Rust 小工具）：输入 `sql/postgresql/*.sql`，输出达梦方言（§4.2 规则 + `CREATE INDEX IF NOT EXISTS` 等 DDL 幂等改写）。`0001_initial.sql` 走流水线：PG 参考库全量导出 → 翻译 → 达梦导入 → **行数与抽样 checksum 比对**（34.6 万行不允许人工目检）。
4. **并发与驱动质量**：ODBC 连接池参数压测调优；`FOR UPDATE`、死锁、超时行为专项测试。
5. 交付物：DM ODBC 驱动 so 文件进入离线包清单，安装脚本配置 `odbcinst.ini`。

### 4.7 迁移与测试基础设施

- `sql/` 目录结构：`postgresql/`（手写源头）、`kingbase/`、`opengauss/`、`dm/`（生成 + 评审后入库）。
- `script/test-database-migrations.sh` 增加 `--db <flavor>`：起对应容器（金仓/openGauss/DM8 官方镜像）执行"空库到最新 schema + 基线数据"验收。
- `crates/framework/database/tests/migrations.rs` 的迁移计数断言按四套目录分别校验。
- AGENTS.md 的数据库工作流同步更新：**新迁移提交时必须同时产出并验证四个方言目录**。
- CI 矩阵：`{x86_64, aarch64} × {PG, 金仓, openGauss, DM8}` 的后端全量测试（镜像许可注意国产库容器在 CI 的授权条款）。

## 5. CPU/OS 适配与构建交付（M4，可与 M1 并行）

### 5.1 编译

- 双 target：`x86_64-unknown-linux-gnu`、`aarch64-unknown-linux-gnu`。
- glibc 基线：麒麟 V10 与 UOS V20 均为 glibc 2.28 级别 → 统一在 glibc ≤ 2.28 的构建容器（对标 manylinux_2_28）内编译，不在目标机编译。
- aws-lc-rs/ring、libsm、calamine、zip 等依赖在双架构均可用；aarch64 优先用原生构建机（QEMU 模拟编译 Rust 过慢），次选 `cross-rs` 交叉。
- 构建即验：CI 增加双架构 `cargo build --release` + `cargo test`。

### 5.2 前端

- 纯静态产物与 CPU 架构无关：**只在 x86_64 构建机出一份 `dist`**，双架构共用；不在国产环境装 bun。
- 国产浏览器（Chromium/Firefox 内核）天然兼容 Vue3/AntD；补一份《浏览器兼容声明》+ 常见国产浏览器截图回归（麒麟自带浏览器、UOS 浏览器、360/奇安信浏览器）。

### 5.3 交付形态（两种都要，信创现场以离线为主）

1. **离线 tarball（基线形态）**：`rustset-<版本>-<arch>.tar.gz`，含 gateway 二进制、web dist、DM ODBC so（按需）、`install.sh`（解压、写 systemd、载入 env）、SHA256 清单。
2. **容器形态**：gateway 与 RustFS 镜像用 buildx 出 `linux/amd64` + `linux/arm64` 双 manifest；compose 模板按目标库替换 postgres 服务为金仓/openGauss/DM8 对应部署；**NATS 从交付物移除**（代码零引用），Redis 按目录决策保留或换 TongRDS。
3. 离线可复现构建：`cargo vendor` + `.cargo/config.toml`（目前缺失，需补）、bun 锁定与 `node_modules` 缓存内网化、私有镜像仓库或 U 盘介质交付。
4. 可选：麒麟 rpm / UOS deb（fpm 打包），二期视客户要求。

### 5.4 OS 认证清单

每 OS × CPU 组合跑：systemd 托管启停、开机自启、`Restart=always`、日志（journald 中文）、时区/字符集、防火墙端口、升级安装（迁移链在旧库上滚动）。

## 6. 中间件目录化

| 现组件 | 信创定位 | 动作 |
| --- | --- | --- |
| PostgreSQL | 开发基线保留；生产按目标库替换 | 无代码动作，部署文档分库 |
| Redis | 开源可用；目录强制时换 TongRDS（协议兼容） | 代码零改动（`REDIS_URL` 换址）；限流/缓存功能回归即可 |
| NATS | 未使用 | 从 compose/交付物移除；`framework/mq` 后续若实装事件总线，直接选 RocketMQ 5（或目录强制的 TongLINK/Q），预留 publisher/subscriber trait |
| Nginx | 反代 | 换 **Tengine**（与 §7 TLCP 合并实施）或 TongHttpServer；现有 location 规则平移 |
| RustFS | 国产开源 | 保留；确认其镜像/二进制双架构可用性，不可用则源码自构建入离线包 |

## 7. 国密与密评（M5–M6）

依据：GB/T 39786-2021《信息安全技术 信息系统密码应用基本要求》（通常伴随等保三级同步做）。

### 7.1 已具备

- 摘要/完整性：SM3、HMAC-SM3（含会话令牌签名）。
- 存储保密：SM4-CBC 落盘密封（`enc:sm4:v2:`），`0018` 已完成存量 XOR→SM4 re-seal。
- 口令：PBKDF2-HMAC-SM3（16384 轮）。
- 身份：SM3 派生 per-user `identity_uuid`。

### 7.2 缺口与整改

| # | 缺口 | 整改 | 备注 |
| --- | --- | --- | --- |
| 1 | 传输层无国密 TLS（TLCP，GB/T 38656-2020） | 入口网关用 **Tengine + 铜锁（TongsSSL）** 终结国密双证书（SM2 签名证书 + SM2 加密证书），后端维持现状 | 应用代码不动；浏览器需国密支持的场景由国密安全浏览器/客户端网关承接 |
| 2 | 纯软件密码模块无商用密码产品认证证书 | 决策点：接 SDF 密码卡/加密机（GM/T 0018 接口），或采购有认证的软件密码模块 | `gm` crate 抽象 `Sm4Provider`/`RandomProvider` trait，预留硬件实现位；纯软件作为非密评场景默认 |
| 3 | 密钥管理薄弱：SM4 密钥 = SM3(环境变量密钥) 派生，无独立保管与轮换 | 第一步：独立主密钥文件（0600，启动加载，与 JWT_SECRET 分离）；第二步：re-seal 升版机制支持密钥轮换（沿用 `legacy_v1` → `sm4:v2` 的既有先例，扩展 `v3`） | 密钥生成、分发、归档、销毁流程写入管理制度（密评文档部分） |
| 4 | SM2 未使用 | TLS 双证书必用（#1）；JWT 是否升级 SM2 签名视测评口径——HMAC-SM3 已满足完整性要求，建议保留并备注说明 | libsm 0.6.1 自带 sm2 模块，成本低，可选项 |
| 5 | 随机数来自 OS（getrandom） | 一般可接受；若测评要求 GM/T 0005 检测，通过 SDF 硬件取随机 | 随 #2 决策联动 |
| 6 | 审计留存期未核对（等保要求 ≥6 个月） | 核对 audit 日志落盘/轮转策略，不足则补 | 工作量小 |

### 7.3 密评流程

选机构 → 方案设计评审（本文档 §7 作为输入）→ 现场测评 → 整改 → 报告。**硬件密码设备采购周期通常 4–8 周，须在 M0 启动**。

## 8. 实施计划

> 工作量为粗估（人周），以评审后的详细排期为准。数据库线为关键路径。

| 阶段 | 内容 | 交付物 / 验收 | 估工 | 依赖 |
| --- | --- | --- | --- | --- |
| M0 决策与基线 | 锁定 OS/DB/中间件版本；取得金仓/openGauss/DM8 介质与许可；密评机构预沟通；§10 决策项闭环 | 版本锁定表、CI 国产库容器就绪 | 1–2 周 | — |
| M1 门面 + 可移植化 + 金仓 | `DbPool/DbRow/DbTransaction` 门面；213 处替换；SQL 方言中立化；金仓全量回归 | `sql/kingbase` 全链；金仓环境功能全绿；AGENTS.md 工作流更新 | 4–6 周 | M0 |
| M2 openGauss | 认证参数固化、全量回归、压测 | `sql/opengauss` 全链；openGauss 环境功能全绿 | 1–2 周 | M1 |
| M3 达梦 | ODBC 适配器、类型映射、`sql-translate` 工具、`0001` 流水线 + checksum 比对、全量回归 | `sql/dm` 全链；DM8 环境功能全绿 | 6–10 周 | M1（可与 M2 并行） |
| M4 双架构与离线交付 | 双 target CI、tarball/容器双形态、`cargo vendor`、安装脚本、OS 认证清单 | 四组 OS×CPU 安装验收通过 | 2–3 周 | 无（与 M1 并行） |
| M5 TLCP 与密钥管理 | Tengine+铜锁网关、主密钥文件、SDF provider（视决策）、JWT SM2（视口径） | 国密 HTTPS 双证书打通；密钥轮换演练一次 | 2–4 周 | M0（硬件到位） |
| M6 联调与测评支撑 | §4.7 全矩阵 CI、四库 × 双架构交叉回归、密评现场支撑 | 测试报告、密评整改闭环 | 2–3 周 | M1–M5 |

串行关键路径约 **4–5 个月**（M0→M1→M3→M6），并行充分时可压缩到 3.5–4 个月。

## 9. 风险清单

| # | 风险 | 影响 | 缓解 |
| --- | --- | --- | --- |
| R1 | 达梦无官方 Rust 驱动，ODBC 驱动质量/性能未知 | M3 延期或性能不达标 | M0 期间先做 ODBC spike（建表+CRUD+并发压测）；备选 DPI FFI；最坏情况达梦支持期后置一期（需业务确认） |
| R2 | `0001_initial.sql` 34.6 万行翻译正确性 | 基线数据损坏 | 行数 + 抽样 checksum 自动比对，禁止目检；比对不过即阻断 |
| R3 | openGauss sha256 认证与 sqlx 不兼容 | 连不上 | 安装文档强制 `password_encryption_type=1/0`；CI 固化该配置 |
| R4 | 四套迁移链长期同步成本 | 每次变更高 | 翻译工具固化进 CI；AGENTS.md 工作流约束"四目录同交" |
| R5 | jsonb 列上的唯一约束/查询在达梦退化为 TEXT（0025 唯一值约束等） | 功能差异 | M3 专项清单逐条评审，功能降级登记并客户确认 |
| R6 | 密评硬件（密码卡/加密机）采购与选型周期 | M5 停等 | M0 启动采购；软件主密钥方案先行兜底 |
| R7 | 金仓/openGauss 容器在 CI 的授权条款 | CI 无法跑真库 | 与厂商确认开发者条款；不行则物理测试机常驻跑批 |
| R8 | aarch64 原生构建机资源 | M4 延迟 | 提前申请鲲鹏/飞腾测试机；备选 cross-rs 交叉编译 |
| R9 | 版本矩阵爆炸（每库多版本） | 测试不可收敛 | 每个数据库锁定一个受支持版本，写进版本锁定表 |

## 10. 待决策项（需在 M0 闭环）

1. **达梦排期**：M3 风险最高。是否接受"金仓/openGauss 首发、达梦 二期"？若不接受，M0 必须完成 ODBC spike 以锁死技术路线。
2. **密评目标级别**：二级还是三级（决定 §7.2 #2/#5 是否必须上硬件）。
3. **密码设备选型**：SDF 密码卡（PCIe，单机）还是加密机（网络，集群共享）？预算与厂商。
4. **Redis 是否必须目录内**（TongRDS）还是开源 Redis 可接受。
5. **交付形态优先级**：离线 tarball 与容器双形态都要，还是以某一种为先。
6. **各数据库具体版本锁定**（金仓 V8R6 小版本、openGauss 5/6、DM8 build 号）。

---

## 附：与现有文档的关系

- 本方案不改变 AGENTS.md 的迁移纪律（不可改已发布迁移、幂等、`test-database-migrations.sh` 验收）；M1 起将其扩展为四方言目录。
- 部署文档（AGENTS.md「New Server Startup」）在 M4 后按 OS/DB 维度拆分为分册。
