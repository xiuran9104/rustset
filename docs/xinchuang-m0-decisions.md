# M0 决策记录（docs/xinchuang-adaptation-plan.md §5.4）

| 文档项 | 内容 |
| --- | --- |
| 性质 | M0 六项决策记录 + 需求追踪矩阵 |
| 编制日期 | 2026-11-06（确认 2026-11-08） |
| 状态标记 | ✅ 已确认（项目负责人委托按建议稿执行；后续变更走范围变更流程） |

## 一、六项决策建议

### 1. 达梦排期（R1）— ✅ 已确认

| 选项 | 影响 |
| --- | --- |
| A. 金仓/openGauss 首发、达梦二期 | 上线最快；已验证的 openGauss 链可直接投产 |
| B. 达梦同批交付 | M0 必须完成 ODBC spike（需厂商介质 + `libdodbc.so`），串行关键路径 +6~10 周 |

**建议**：A。openGauss 全链已真机验证（迁移/合同/数据比对），首发风险最低；达梦在介质到位后按 spike→适配器→流水线顺序补齐。

### 2. 密评目标级别（§4.4.3 #2/#5）— ✅ 已确认（随等保三级）

| 级别 | 硬件要求 | 影响项 |
| --- | --- | --- |
| 二级 | 无强制 | 软件侧已全部就绪（v3 密文、主密钥环、Provider 抽象） |
| 三级 | SDF 密码卡/加密机必配 | `CRYPTO_PROVIDER=sdf`（trait 已预留）；随机数走 GM/T 0005 检测 |

**建议**：随等保定级走（通常三级）。软件侧已按三级预备，硬件到位后只需实现一个 trait。

### 3. 密码设备选型 — ✅ 已确认（依赖 #2：单机 SDF 卡起步，集群换加密机）

SDF 密码卡（PCIe，单机）vs 加密机（网络，集群共享）。**建议**：单机部署形态选 SDF 卡，多实例集群选加密机；预算与厂商在 #2 确认后采购（周期 4–8 周，§4.4.4）。

### 4. Redis 是否必须目录内 — ✅ 已确认（TongRDS 优先，开源 Redis 保留为开发默认）

代码零改动适配 TongRDS（`REDIS_URL` 换址 + §4.3.1 语义合同回归）。**建议**：采购清单允许时优先 TongRDS；开源 Redis 保留为开发默认。

### 5. 交付形态优先级 — ✅ 已确认（tarball 主交付，容器增值形态）

两条链路均已可执行：离线 tarball（`package-offline.sh`，实测）+ 容器多阶段镜像（`build-images.sh`，运行冒烟通过）。**建议**：tarball 为主交付物（信创现场无容器平台时唯一可用），容器为增值形态。

### 6. 数据库版本锁定 — ✅ 已确认（附实测基线）

| 数据库 | 建议锁定版本 | 实测状态 |
| --- | --- | --- |
| openGauss | 7.0.0-RC2（或 M0 确认的 5.0/6.0 GA） | ✅ 7.0.0-RC2 容器全链验证 |
| 金仓 KingbaseES | V8R6 最新小版本 | 待介质 |
| 达梦 DM8 | 待介质 | 待 ODBC spike |
| PostgreSQL | 18（开发基线） | ✅ 持续验证 |

## 二、需求追踪矩阵（§10.1）

| 方案条目 | 代码/交付物 | 测试 | 状态 |
| --- | --- | --- | --- |
| §4.1.1 门面 | `crates/framework/database`（DbPool/DbTransaction/DbQueryBuilder/DbError/BindIn） | 合同套件 + 145 workspace 测试 | ✅ |
| §4.1.2/4.1.3 SQL 可移植化 | 运行时 SQL 全量改写 | 迁移+合同套件（PG/og 双库） | ✅ |
| §4.1.4 金仓 | `sql/kingbase` 链（仅 NND 翻译）+ migrator 接线 | 迁移 59 + 合同 7/7 + R2 比对 真机全绿（社区许可镜像） | ✅（企业镜像覆盖可选） |
| §4.1.5 openGauss | `sql/opengauss` 59 文件翻译链 | 迁移+合同+幂等重跑 全绿 | ✅ 真机 |
| §4.1.6 达梦 | `sql-translate` 规则引擎/评审报告；流水线脚本 | og 侧 R2 端到端通过；DM 侧待介质 | ⏳ 介质 |
| §4.1.7 事务锁合同 | `tests/contract.rs` | PG 7/7、og 7/7（2 项 og 差异已登记） | ✅ |
| §4.1.8 迁移基础设施 | 四方言目录 + 一致性测试 + CI 矩阵 | CI（门禁变量待开） | ✅ |
| §4.2.1 双架构 | `.cargo/config.toml` + cross + **基线构建容器**（`Dockerfile.build-baseline`，glibc 2.28 内编译） | aarch64 构建实测 + x86_64 基线二进制**安装认证 PASS**（`verify-install-cert.sh`，almalinux:8 六项） | ✅（真机安装待环境；binfmt 未注册，arm64 容器模拟 SKIP） |
| §4.2.3 安装改造 | `package-offline.sh` + `install.sh` | 预检/打包实测 | ✅（OS 认证待环境） |
| §4.3 消息队列 | `framework/mq` JetStream 总线 + 语义合同套件（6/6） | 合同测试实测（compose NATS --js） | ✅（国产 MQ 替换按同套件验收） |
| §4.3 缓存 | compose/Redis 零改动 | 语义合同待测 | ⏳ TongRDS 选型 |
| §4.4.2/4.4.3 #3 密钥管理 | v3 格式 + 钥匙环 + re-seal + CLI | 21 gm 测试 + 端到端 | ✅ |
| §4.4.3 #2 硬件密码 | `Sm4Provider`/`RandomProvider` trait + CRYPTO_PROVIDER 门禁 | 单测 | ✅（硬件待采购） |
| §4.4.3 #1/#7 TLCP | SM2 双证书脚本 + 铜锁 NTLS 验证镜像 + Tengine 配置 + 端到端握手验证（NTLSv1.1 协商 + 链校验 OK） | `verify-ntls.sh` 真机握手 PASS | ✅ 软件侧（浏览器互操作待真机） |
| §4.4.3 #6 审计 | 留存 190 天 + SM3 行摘要（迁移 0059） | 单测 + 迁移断言 | ✅ |
| §4.5 浏览器/WPS | browserslist 基线 + 兼容声明 | 前端构建通过；截图回归待真机 | ⏳ 环境 |
| §4.6.1 离线 tarball | `package-offline.sh` | 打包实测（31M） | ✅ |
| §4.6.2 容器 | 多阶段 Dockerfile + buildx | 构建 + 运行冒烟实测 | ✅ |
| §4.6.3/4.6.5 供应链 | vendor 快照 + SBOM 脚本 | vendor 425M 离线验证、SBOM 461 包 | ✅ |
| §4.7 备份恢复 | PG + 国产库备份脚本 + RustFS 清单 | dump/restore 容器实测 | ✅（国产库客户端待环境） |
| §7 切换/回退 | 流水线 + 备份 + install.sh 回退点 | 演练待环境 | ⏳ |

## 三、立即可执行的采购/申请清单

1. ~~金仓 V8R6 介质~~（社区许可镜像已验证，`--db kingbase` 一键回归；企业许可镜像可选覆盖 `KINGBASE_IMAGE`）
1. ~~达梦 DM8 介质~~（社区镜像 sizx/dm8 已真机连通，翻译链 0002–0059 已执行并留存错误清单；`--db dm`/ODBC 适配器收敛待 Rust 侧接入）
1. SDF 密码卡（按 #2/#3 采购，接口已以 trait 预留：`CRYPTO_PROVIDER` 门禁）
1. Tengine + 铜锁 + SM2 双证书（TLCP 配置与验证脚本已就绪，证书到位即部署）
1. 麒麟 V10 / 统信 UOS / aarch64 真机（安装认证脚本已就绪，真机执行即出证据）
2. aarch64 原生测试机（鲲鹏/飞腾）——交叉构建已验证，缺真机运行认证
3. 麒麟 V10 / 统信 UOS 环境各一台（P1–P4 安装认证）
4. SDF 密码卡或加密机（若密评三级，4–8 周周期）
5. Tengine + 铜锁 + SM2 双证书（TLCP 项）
