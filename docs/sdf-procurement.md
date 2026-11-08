# SDF 密码卡采购需求书（密评整改，M0 决策 #2/#3 落地）

依据：docs/xinchuang-adaptation-plan.md §4.4.3 #2/#5、GB/T 39786-2021（三级）、
GM/T 0018（SDF 接口）、docs/xinchuang-m0-decisions.md（已确认决策）。

## 1. 采购标的

| 项 | 要求 |
| --- | --- |
| 产品形态 | PCIe 密码卡（SDF 接口），单机部署起步；后续集群共享可换加密机（网络型） |
| 资质 | **商用密码产品认证证书**（对应 GB/T 0028 检测），提供证书复印件与可查编号 |
| 支持算法 | SM2（签名/加解密）、SM3（摘要/HMAC）、SM4（ECB/CBC，≥一个句柄并发流） |
| 密钥管理 | 密钥不出卡（句柄内运算）；支持内部生成、外部导入（含明文导入会话权限控制）、备份恢复 |
| 接口 | GM/T 0018-2012 C 接口（SDF_OpenSession / SDF_ImportKey / SDF_Encrypt / SDF_Decrypt / SDF_GenerateRandom 等标准命名），提供 Linux x86_64 + aarch64 驱动库（.so）与头文件 |
| 随机数 | 通过 GM/T 0005 随机数检测（对应 §4.4.3 #5：测评要求随机数出自硬件） |
| 操作系统 | 银河麒麟 V10、统信 UOS V20 驱动可用（供应商书面承诺，最好列内核版本清单） |
| 并发 | ≥4 会话并发（对应任务 worker 并发），标称 TPS 与实测报告一并提供 |
| 供货 | 驱动库、示例代码、接口文档、质保与升级渠道 |

## 2. 与平台的接入方式（已预留）

平台侧接口已锁定（`crates/framework/gm/src/provider.rs`）：

```rust
// CRYPTO_PROVIDER=sdf 启用（当前 sdf-sim 为软件模拟，硬件到位后替换）
Sm4Provider  { encrypt_cbc / decrypt_cbc }
RandomProvider { fill_random }
```

集成清单（供应商配合项）：
1. 提供 `libsdf.so` 与头文件 → 平台以 FFI 绑定实现 `Sm4Provider`/`RandomProvider`
2. 会话/句柄生命周期示例代码
3. 密评现场：SDF 卡调用位置说明（对应平台 `SealingSecrets` 密封路径）

## 3. 验收

1. 平台以 `CRYPTO_PROVIDER=sdf` 启动，密封/解密/随机数全链路走卡
2. GM/T 0005 随机数检测报告（测评机构出具）
3. 并发压测达标的实测记录
4. 密钥备份恢复演练一次

## 4. 流程与周期

- 选型 → 采购 → 到货：**4–8 周**（M0 已确认启动；建议在金仓/达梦真机收尾期并行推进）
- 到货后平台集成预估：1–2 周（接口已预留，`provider.rs` 的 SdfSimProvider 即替换点）
- 密评送测：与整改后的平台版本一起送测
