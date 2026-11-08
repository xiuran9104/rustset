# TLCP 国密 TLS（铜锁 + SM2 双证书）

依据：docs/xinchuang-adaptation-plan.md §4.4.3 #1/#7、GB/T 38656-2020（NTLS/TLCP）。

## 组成

| 文件 | 用途 |
| --- | --- |
| `gen-sm2-certs.sh` | 生成 SM2 双证书（根 CA + 服务器签名证书 + 服务器加密证书，SM2-with-SM3） |
| `nginx-tlcp.conf` | 生产入口网关配置（Tengine 3.x / nginx+铜锁：`ssl_ntls on` + 双证书 + TLCP 套件） |
| `Dockerfile.tongsuo` | 铜锁 NTLS 验证镜像（`enable-ntls` 编译；`s_server/s_client` 端到端握手） |
| `verify-ntls.sh` | 端到端握手验证（SM2 双证书 → NTLS 协商 → SM4/SM3 断言 → 链校验） |
| `Dockerfile.tengine` | 生产 Tengine+铜锁入口镜像（按需构建） |

## 关键认知（为什么需要双证书）

TLCP（GB/T 38656）与标准 TLS 的核心差异：**证书分域**——签名域（身份认证）
与加密域（密钥交换封装）各持一张 SM2 证书。`server-sign.*` 与
`server-enc.*` 不是冗余，缺一则 NTLS 协商必然失败。

## 快速验证

```bash
bash script/tlcp/gen-sm2-certs.sh --out /tmp/rustset-tlcp-certs
bash script/tlcp/verify-ntls.sh --certs /tmp/rustset-tlcp-certs
# 期望：TLCP NTLS 握手 PASS（SM2 双证书 + SM3 完整性 + SM4 加密 + 链校验 OK）
```

`verify-ntls.sh` 首次运行会构建铜锁验证镜像（~5 分钟，源码
github.com/Tongsuo-Project/Tongsuo，`enable-ntls` 编译——注意发行版镜像
openeuler/tongsuo 未启用 NTLS，不能用于 TLCP 验证）。

## 生产部署

1. `gen-sm2-certs.sh --out /etc/rustset/tlcp`（或接入已购证书）
2. `Dockerfile.tengine` 构建入口镜像，挂载 `nginx-tlcp.conf` + 证书
3. 浏览器侧：需国密支持的客户端走国密浏览器/客户端网关（方案 §4.4.3 #1）；
   普通浏览器走标准 TLS 入口，两条入口并存、互不降级
4. 互操作验收（§4.4.3 #7）：证书链、双证书、证书轮换、过期、吊销、
   国密浏览器截图回归——真机/浏览器项待目标环境

## 与方案的边界

本目录验证的是**证书链正确性 + NTLS 协商能力 + 生产网关配置**。
国密浏览器互操作、SM2 证书轮换演练属于真机验收项（§6.1 第三层）。
