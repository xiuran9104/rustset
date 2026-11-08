#!/usr/bin/env bash
# 安装认证执行器（docs/xinchuang-adaptation-plan.md §6.1 第一层 P1/P2 的
# 可重复脚本化）。
#
# 在 glibc 2.28 基线容器（almalinux:8，与银河麒麟 V10 同基线）内执行
# 离线包的 install.sh --dry-run，固化以下认证项为可重复证据：
#   1. 架构匹配预检（x86_64/aarch64）
#   2. glibc ≥ 2.28 基线预检
#   3. 端口/磁盘预检
#   4. SHA256SUMS 完整性校验
#   5. 二进制在基线 glibc 下真实可执行
#   6. 前端产物落位
#
# 真机（麒麟 V10 / 统信 UOS）认证执行同一份 install.sh --dry-run，
# 输出格式一致，证据可直接归档。
#
# 用法：
#   bash script/verify-install-cert.sh [--arch x86_64|aarch64] [--skip-build]
#
# --arch aarch64 需要宿主 binfmt 已注册（docker run --privileged
# tonistiigi/binfmt --install arm64）且宿主为多架构构建环境；未注册时
# 明确报 SKIP，不伪造证据（§1.4.6）。
set -euo pipefail
cd "$(dirname "$0")/.."

ARCH="$(uname -m | sed 's/^amd64$/x86_64/; s/^arm64$/aarch64/')"
SKIP_BUILD=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --arch) ARCH="$2"; shift 2 ;;
    --skip-build) SKIP_BUILD=1; shift ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

PKG_DIR="dist-offline"
TARBALL="${PKG_DIR}/rustset-"*"-${ARCH}.tar.gz"

echo "=== 安装认证（目标基线：almalinux:8 / glibc 2.28 / ${ARCH}）==="

# 1) 产物就绪
if [[ $SKIP_BUILD -eq 1 ]]; then
  ls $TARBALL >/dev/null 2>&1 || { echo "缺少离线包 $TARBALL（先 script/package-offline.sh）" >&2; exit 1; }
else
  echo "--- 生成离线包（--skip-build 复用现有产物）---"
  bash script/package-offline.sh --skip-build --arch "$ARCH" >/dev/null
fi
TARBALL=$(ls $TARBALL | head -1)
echo "离线包：$TARBALL"

# 2) 基线容器内执行认证
rm -rf /tmp/rustset-cert && mkdir -p /tmp/rustset-cert
# 保留原始文件名（sha256 文件引用的就是它）。
cp "$TARBALL" /tmp/rustset-cert/
( cd "$(dirname "$TARBALL")" && sha256sum "$(basename "$TARBALL")" > /tmp/rustset-cert/package.sha256 )

docker rm -f rustset-cert >/dev/null 2>&1 || true
# almalinux:8 = glibc 2.28（麒麟 V10 同基线）；--platform 按目标架构。
PLATFORM="linux/amd64"
[[ "$ARCH" == "aarch64" ]] && PLATFORM="linux/arm64"

set +e
docker run --rm --platform "$PLATFORM" \
  -v /tmp/rustset-cert:/cert \
  almalinux:8 bash -ec '
set -euo pipefail
echo "--- 基线信息 ---"
cat /etc/almalinux-release
ldd --version | head -1
uname -m
echo "--- 解包 ---"
cd /cert
sha256sum -c package.sha256
read -r _ TARBALL_NAME < package.sha256
tar xzf "$TARBALL_NAME"
PKG=$(ls -d rustset-*/ | head -1)
cd "$PKG"
echo "--- install.sh --dry-run（预检 1-6 项）---"
bash install.sh --dry-run
'
RESULT=$?
set -e
docker rm -f rustset-cert >/dev/null 2>&1 || true

if [[ "$RESULT" -ne 0 ]]; then
  echo "" >&2
  echo "认证结果：FAIL（exit=$RESULT）" >&2
  if [[ "$ARCH" == "aarch64" ]]; then
    echo "提示：aarch64 需要宿主 binfmt（docker run --privileged tonistiigi/binfmt --install arm64）；" \
         "未注册时本项报 SKIP 而非 FAIL——请在真机或注册 binfmt 后复跑。" >&2
  fi
  exit "$RESULT"
fi

echo ""
echo "认证结果：PASS（${ARCH} × almalinux:8/glibc2.28 基线，6 项全过）"
echo "真机认证：同一份 install.sh --dry-run 在目标机执行，输出可归档为验收证据。"
