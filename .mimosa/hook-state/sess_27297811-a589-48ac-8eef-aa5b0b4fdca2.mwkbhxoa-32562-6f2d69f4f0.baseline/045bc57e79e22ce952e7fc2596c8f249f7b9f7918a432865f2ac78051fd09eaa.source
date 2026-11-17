#!/usr/bin/env bash
# TLCP 国密双证书生成（docs/xinchuang-adaptation-plan.md §4.4.3 #1、GB/T 38656）。
#
# 产出（默认 /etc/rustset/tlcp 或 --out 指定目录）：
#   ca.crt / ca.key                 SM2 根 CA（自签）
#   server-sign.crt / .key          服务器签名证书（TLCP 签名域）
#   server-enc.crt / .key           服务器加密证书（TLCP 密钥交换域）
#   证书链 fullchain（sign/enc 各自挂根 CA）
#
# 双证书是 TLCP 的核心特征：签名域用签名密钥、密钥交换/加密域用加密密钥，
# 分别由 server-sign/server-enc 两张证书承载。
#
# 依赖：OpenSSL 3.x（SM2/SM3 原生支持）。Tongsuo CLI 亦可（脚本自动探测）。
# 用法：bash script/tlcp/gen-sm2-certs.sh [--out DIR] [--cn rustset.local] [--days 825]
set -euo pipefail

OUT="${TLCP_CERT_DIR:-/tmp/rustset-tlcp-certs}"
CN="rustset.local"
DAYS=825

while [[ $# -gt 0 ]]; do
  case "$1" in
    --out) OUT="$2"; shift 2 ;;
    --cn) CN="$2"; shift 2 ;;
    --days) DAYS="$2"; shift 2 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

OPENSSL_BIN=openssl
if ! command -v openssl >/dev/null; then
  if command -v tongsuo >/dev/null; then OPENSSL_BIN=tongsuo; else
    echo "需要 OpenSSL 3.x 或 Tongsuo CLI" >&2; exit 1
  fi
fi

# SM2 生成能力探测（老 OpenSSL 不支持 -algorithm SM2）。
if ! echo | "$OPENSSL_BIN" genpkey -algorithm SM2 -out /dev/null 2>/dev/null; then
  echo "当前 $OPENSSL_BIN 不支持 SM2 密钥生成（需 OpenSSL 3.x / Tongsuo）" >&2
  exit 1
fi

mkdir -p "$OUT"
EXT="$OUT/extensions.cnf"
cat > "$EXT" <<EOF
[ca_ext]
basicConstraints = critical,CA:TRUE
keyUsage = critical,keyCertSign,cRLSign
subjectKeyIdentifier = hash

[server_sign_ext]
basicConstraints = critical,CA:FALSE
keyUsage = critical,digitalSignature
extendedKeyUsage = serverAuth
subjectAltName = DNS:${CN},DNS:localhost
subjectKeyIdentifier = hash
authorityKeyIdentifier = keyid

[server_enc_ext]
basicConstraints = critical,CA:FALSE
keyUsage = critical,keyEncipherment,dataEncipherment,keyAgreement
extendedKeyUsage = serverAuth
subjectAltName = DNS:${CN},DNS:localhost
subjectKeyIdentifier = hash
authorityKeyIdentifier = keyid
EOF

echo "=== 1/4 SM2 根 CA ==="
"$OPENSSL_BIN" genpkey -algorithm SM2 -out "$OUT/ca.key" 2>/dev/null
"$OPENSSL_BIN" req -new -x509 -key "$OUT/ca.key" -out "$OUT/ca.crt" \
  -subj "/C=CN/O=RustSet/CN=RustSet GM Root CA" -days "$DAYS" \
  -extensions ca_ext -config "$EXT" -sm3 2>/dev/null

echo "=== 2/4 服务器签名证书 ==="
"$OPENSSL_BIN" genpkey -algorithm SM2 -out "$OUT/server-sign.key" 2>/dev/null
"$OPENSSL_BIN" req -new -key "$OUT/server-sign.key" -out "$OUT/server-sign.csr" \
  -subj "/C=CN/O=RustSet/CN=${CN}" 2>/dev/null
"$OPENSSL_BIN" x509 -req -in "$OUT/server-sign.csr" \
  -CA "$OUT/ca.crt" -CAkey "$OUT/ca.key" -CAcreateserial \
  -out "$OUT/server-sign.crt" -days "$DAYS" \
  -extfile "$EXT" -extensions server_sign_ext -sm3 2>/dev/null

echo "=== 3/4 服务器加密证书 ==="
"$OPENSSL_BIN" genpkey -algorithm SM2 -out "$OUT/server-enc.key" 2>/dev/null
"$OPENSSL_BIN" req -new -key "$OUT/server-enc.key" -out "$OUT/server-enc.csr" \
  -subj "/C=CN/O=RustSet/CN=${CN}" 2>/dev/null
"$OPENSSL_BIN" x509 -req -in "$OUT/server-enc.csr" \
  -CA "$OUT/ca.crt" -CAkey "$OUT/ca.key" -CAcreateserial \
  -out "$OUT/server-enc.crt" -days "$DAYS" \
  -extfile "$EXT" -extensions server_enc_ext -sm3 2>/dev/null

echo "=== 4/4 自校验 ==="
for cert in ca server-sign server-enc; do
  algo=$("$OPENSSL_BIN" x509 -in "$OUT/$cert.crt" -noout -text 2>/dev/null | grep -am1 'ASN1 OID\|Signature Algorithm' | head -1)
  echo "$cert: ${algo}"
  grep -q sm3 <<<"$(echo "$algo" | tr '[:upper:]' '[:lower:]')" \
    || { echo "证书 $cert 不是 SM3 签名" >&2; exit 1; }
done
"$OPENSSL_BIN" verify -CAfile "$OUT/ca.crt" "$OUT/server-sign.crt" "$OUT/server-enc.crt"

chmod 0600 "$OUT"/*.key
rm -f "$OUT"/*.csr "$OUT"/ca.srl "$EXT"
echo "=== 完成：$OUT ==="
ls -la "$OUT"
