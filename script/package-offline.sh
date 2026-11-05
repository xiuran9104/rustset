#!/usr/bin/env bash
# 离线交付打包（docs/xinchuang-adaptation-plan.md §4.6.1、§4.2.3）。
#
# 产出 rustset-<版本>-<架构>.tar.gz：
#   rustset-gateway        后端二进制（release）
#   frontend/              Vben 前端静态产物
#   sql/                   数据库迁移链（网关启动自动执行）
#   install.sh             安装脚本（预检 CPU/OS/glibc/端口/磁盘，写 systemd，载入 env）
#   gateway.env.example    环境变量模板（安装时首装复制，不覆盖已有配置）
#   rustset-gateway.service
#   nginx-rustset.conf
#   DEPLOY.md  VERSION  SHA256SUMS
#
# 用法：
#   bash script/package-offline.sh                       # 本机架构，重新构建
#   bash script/package-offline.sh --skip-build          # 复用现有 dist-deploy 产物
#   bash script/package-offline.sh --arch aarch64        # 交叉构建产物的打包（二进制需已在 target/<triple>/release）
#   bash script/package-offline.sh --dm-odbc libdodbc.so # 附带达梦 ODBC 驱动
#
# 约束（§4.2.1）：目标环境 glibc 基线 2.28（麒麟 V10 / UOS V20）；
# x86_64 直接构建，aarch64 优先原生构建机，交叉编译仅作 CI 检查。
set -euo pipefail
cd "$(dirname "$0")/.."

ARCH="$(uname -m | sed 's/^amd64$/x86_64/; s/^arm64$/aarch64/')"
PKG_ARCH="$ARCH"
SKIP_BUILD=0
DM_ODBC=""
OUTPUT="dist-offline"
VERSION="$(grep -m1 '^version' Cargo.toml | tr -d ' "version=')"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --arch) PKG_ARCH="$2"; shift 2 ;;
    --skip-build) SKIP_BUILD=1; shift ;;
    --dm-odbc) DM_ODBC="$2"; shift 2 ;;
    --output) OUTPUT="$2"; shift 2 ;;
    --version) VERSION="$2"; shift 2 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

if [[ "$PKG_ARCH" != "$ARCH" && "$SKIP_BUILD" -eq 0 ]]; then
  echo "note: --arch $PKG_ARCH differs from host $ARCH; use --skip-build with a prebuilt cross binary" >&2
fi

case "$OUTPUT" in
  /*) STAGE_ROOT="$OUTPUT" ;;
  *) STAGE_ROOT="$(pwd)/${OUTPUT}" ;;
esac
STAGE="${STAGE_ROOT}/rustset-${VERSION}-${PKG_ARCH}"
rm -rf "$STAGE"
mkdir -p "$STAGE_ROOT" "$STAGE"

if [[ "$SKIP_BUILD" -eq 1 ]]; then
  if [[ ! -d dist-deploy ]]; then
    echo "error: --skip-build requires an existing dist-deploy/ (run script/deploy.sh first)" >&2
    exit 1
  fi
  cp -r dist-deploy/. "$STAGE/"
else
  echo "=== 构建 + 组装（script/deploy.sh）==="
  bash script/deploy.sh "$STAGE"
fi

# 达梦 ODBC 驱动（§4.1.6.5：进入离线包清单，安装脚本配置 odbcinst.ini）
if [[ -n "$DM_ODBC" ]]; then
  mkdir -p "$STAGE/lib"
  cp "$DM_ODBC" "$STAGE/lib/"
fi

cat > "$STAGE/VERSION" <<EOF
version=${VERSION}
arch=${PKG_ARCH}
built_at=$(date -u +%Y-%m-%dT%H:%M:%SZ)
git=$(git rev-parse --short HEAD 2>/dev/null || echo unknown)
EOF

# 安装脚本：预检通过前不修改系统（§4.2.3.3）；升级/卸载不删除数据与备份。
cat > "$STAGE/install.sh" <<'INSTALL'
#!/usr/bin/env bash
# RustSet 离线安装脚本（信创方案 §4.6.1）。预检失败立即退出，不修改系统。
set -euo pipefail

DEST=/opt/rustset
ENV_FILE=/etc/rustset/gateway.env
UNIT=/etc/systemd/system/rustset-gateway.service
PKG_DIR="$(cd "$(dirname "$0")" && pwd)"

[[ "$(id -u)" -eq 0 ]] || { echo "需要 root 运行"; exit 1; }

case "${1:-install}" in
  uninstall)
    systemctl stop rustset-gateway 2>/dev/null || true
    systemctl disable rustset-gateway 2>/dev/null || true
    rm -f "$UNIT" /opt/rustset/rustset-gateway
    systemctl daemon-reload
    echo "已卸载服务与二进制；保留 ${ENV_FILE}、数据库与 ${DEST}/frontend（证据与回退需要）"
    exit 0
    ;;
  install) ;;
  *) echo "usage: install.sh [install|uninstall]"; exit 2 ;;
esac

echo "=== 预检（§4.2.3.3）==="
PKG_ARCH="$(grep '^arch=' "$PKG_DIR/VERSION" | cut -d= -f2)"
HOST_ARCH="$(uname -m | sed 's/^amd64$/x86_64/; s/^arm64$/aarch64/')"
[[ "$PKG_ARCH" == "$HOST_ARCH" ]] || { echo "架构不匹配：包 ${PKG_ARCH} vs 主机 ${HOST_ARCH}"; exit 1; }

command -v systemctl >/dev/null || { echo "需要 systemd"; exit 1; }
systemctl is-system-running >/dev/null 2>&1 || true

GLIBC_MAJOR="$(ldd --version 2>/dev/null | head -1 | grep -oE '[0-9]+\.[0-9]+$' | cut -d. -f1)"
GLIBC_MINOR="$(ldd --version 2>/dev/null | head -1 | grep -oE '[0-9]+\.[0-9]+$' | cut -d. -f2)"
if [[ "${GLIBC_MAJOR:-0}" -lt 2 || ( "${GLIBC_MAJOR:-0}" -eq 2 && "${GLIBC_MINOR:-0}" -lt 28 ) ]]; then
  echo "glibc 低于 2.28 基线（麒麟 V10 / UOS V20），继续安装可能无法运行"; exit 1
fi

if ss -ltn 2>/dev/null | grep -q ':8080 '; then
  echo "端口 8080 已被占用；先调整 gateway.env 的 GATEWAY_PORT 或停用占用进程"; exit 1
fi

AVAILABLE_MB="$(df -Pm /opt | awk 'NR==2 {print $4}')"
[[ "${AVAILABLE_MB:-0}" -ge 2048 ]] || { echo "磁盘空间不足 2GiB（剩余 ${AVAILABLE_MB}MiB）"; exit 1; }
echo "预检通过（arch=${HOST_ARCH} glibc=${GLIBC_MAJOR}.${GLIBC_MINOR} disk=${AVAILABLE_MB}MiB）"

echo "=== 安装 ==="
mkdir -p "$DEST" /etc/rustset
install -m 0755 "$PKG_DIR/rustset-gateway" "$DEST/rustset-gateway"
mkdir -p "$DEST/frontend"
cp -r "$PKG_DIR/frontend/." "$DEST/frontend/"
if [[ -d "$PKG_DIR/lib" ]]; then
  mkdir -p "$DEST/lib"
  cp -r "$PKG_DIR/lib/." "$DEST/lib/"
  echo "注意：达梦 ODBC 驱动已复制到 ${DEST}/lib；请配置 /etc/odbcinst.ini 指向该路径"
fi

if [[ ! -f "$ENV_FILE" ]]; then
  install -m 0600 "$PKG_DIR/gateway.env.example" "$ENV_FILE"
  echo "已写入 ${ENV_FILE}：修改全部 CHANGE_ME 后重启服务"
else
  echo "保留既有 ${ENV_FILE}（不覆盖；对照 gateway.env.example 检查新增变量）"
fi

install -m 0644 "$PKG_DIR/rustset-gateway.service" "$UNIT"
systemctl daemon-reload
if [[ "${1:-install}" == "install" ]]; then
  systemctl enable --now rustset-gateway || echo "服务未启动成功：检查 journalctl -u rustset-gateway"
fi

echo "=== 校验 ==="
( cd "$PKG_DIR" && sha256sum -c SHA256SUMS --quiet ) && echo "文件完整性校验通过"
curl -fsS -m 5 http://127.0.0.1:8080/health >/dev/null 2>&1 && echo "健康检查通过" \
  || echo "健康检查未就绪（可能仍在执行迁移）：curl http://127.0.0.1:8080/health"
INSTALL
chmod +x "$STAGE/install.sh"

echo "=== SHA256 清单 ==="
( cd "$STAGE" && find . -type f ! -name SHA256SUMS -print0 | sort -z \
    | xargs -0 sha256sum > SHA256SUMS )

TARBALL="${STAGE_ROOT}/rustset-${VERSION}-${PKG_ARCH}.tar.gz"
tar -czf "$TARBALL" -C "$STAGE_ROOT" "rustset-${VERSION}-${PKG_ARCH}"
( cd "$STAGE_ROOT" && sha256sum "rustset-${VERSION}-${PKG_ARCH}.tar.gz" \
    > "rustset-${VERSION}-${PKG_ARCH}.tar.gz.sha256" )

echo "=== 完成 ==="
echo "包：${TARBALL}"
cat "${TARBALL}.sha256"
du -h "$TARBALL" | cut -f1 | xargs echo "大小："
