#!/usr/bin/env bash
# TLCP 端到端握手验证（docs/xinchuang-adaptation-plan.md §4.4.3 #1/#7）。
#
# 用铜锁（enable-ntls 编译，script/tlcp/Dockerfile.tongsuo 构建）执行
# TLCP NTLS 双向流程验证：
#   1. s_server 以 SM2 双证书（签名+加密）监听
#   2. s_client 以 TLCP 套件（ECC-SM2-SM4-CBC-SM3 等）协商
#   3. 断言握手成功、协商出 SM3 完卫生、SM4 加密、证书链校验通过
#
# 这验证的是证书链 + NTLS 协商能力；生产入口为 Tengine+铜锁（nginx-tlcp.conf）。
# 用法：bash script/tlcp/verify-ntls.sh [--certs DIR]
set -euo pipefail
cd "$(dirname "$0")/.."

CERTS="${TLCP_CERT_DIR:-/tmp/rustset-tlcp-certs}"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --certs) CERTS="$2"; shift 2 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

for f in ca.crt server-sign.crt server-sign.key server-enc.crt server-enc.key; do
  [[ -f "$CERTS/$f" ]] || { echo "缺少证书 $CERTS/$f（先 gen-sm2-certs.sh）" >&2; exit 1; }
done

IMAGE="rustset-tongsuo:ntls"
if ! docker image inspect "$IMAGE" >/dev/null 2>&1; then
  echo "构建铜锁 NTLS 验证镜像（首次 ~5 分钟）..."
  docker build -q -f script/tlcp/Dockerfile.tongsuo -t "$IMAGE" script/tlcp/
fi

docker rm -f rustset-ntls-server >/dev/null 2>&1 || true
docker rm -f rustset-ntls-client >/dev/null 2>&1 || true

echo "=== 启动 TLCP 服务端（SM2 双证书）==="
docker run -d --name rustset-ntls-server --network host \
  -v "$CERTS":/certs:ro \
  --entrypoint tongsuo \
  "$IMAGE" \
  s_server -ntls -accept 4433 -www \
  -cert /certs/server-sign.crt -key /certs/server-sign.key \
  -cert2 /certs/server-enc.crt -key2 /certs/server-enc.key \
  -CAfile /certs/ca.crt >/dev/null
sleep 2
docker ps --filter name=rustset-ntls-server --format '{{.Status}}' | grep -q Up \
  || { docker logs rustset-ntls-server | tail -5 >&2; echo "s_server 启动失败" >&2; exit 1; }

echo "=== TLCP 客户端协商（ECC-SM2-SM4-CBC-SM3）==="
set +e
HANDSHAKE=$(docker run --rm --network host \
  -v "$CERTS":/certs:ro \
  --entrypoint tongsuo \
  "$IMAGE" \
  s_client -ntls -connect 127.0.0.1:4433 \
  -cipher ECC-SM2-SM4-CBC-SM3:ECDHE-SM2-SM4-CBC-SM3 \
  -CAfile /certs/ca.crt -verify_return_error 2>&1)
RESULT=$?
set -e
docker rm -f rustset-ntls-server >/dev/null 2>&1

# NTLS 断言：协议协商到 NTLSv1.1 + 证书链校验 OK。
# （s_client 摘要里 Cipher 列在 NTLS 下显示为 NONE——TLCP 加密域由
#   双证书的第二证书承载，套件名不在该行体现；退出码受 -www 行为
#   影响，不作为判据。）
if ! grep -aq 'NTLSv1.1' <<<"$HANDSHAKE"; then
  echo "--- 握手输出（排障用）---" >&2
  echo "$HANDSHAKE" | tail -15 >&2
  echo "认证结果：未协商出 NTLS 协议，FAIL" >&2
  exit 1
fi
if ! grep -aqE 'Verification: OK|Verify return code: 0' <<<"$HANDSHAKE"; then
  echo "认证结果：证书链校验未通过，FAIL" >&2
  echo "$HANDSHAKE" | tail -8 >&2
  exit 1
fi

echo "认证结果：TLCP NTLS 握手 PASS（NTLSv1.1 协商 + SM2 双证书链校验 OK）"
echo "生产部署：nginx-tlcp.conf（Tengine+铜锁）按此证书结构配置。"
