#!/usr/bin/env bash
# 容器镜像构建：linux/amd64 + linux/arm64 双 manifest（§4.6.2）。
#
# 多阶段构建（script/docker/Dockerfile.gateway）：编译在镜像内完成，
# 构建与运行同底座，宿主 glibc 无关；前端 dist 由宿主构建机提供一份
# （§4.2.2，双架构共用）。aarch64 建议 aarch64 原生构建机执行本脚本，
# 本机 QEMU 模拟编译 Rust 极慢，仅作 CI 检查。
#
# 用法：
#   bash script/build-images.sh --tag registry.example.com/rustset-gateway:1.0.0 \
#        [--dist apps/web/apps/web-antd/dist] [--arch amd64|arm64|both] [--push]
#
# --push 时推送各架构 tag 并用 `buildx imagetools create` 合成双架构
# manifest list（编排侧再加节点亲和性，防止 ARM 节点误拉 amd64 镜像）。
# 不带 --push 只做本机构建与运行冒烟。
set -euo pipefail
cd "$(dirname "$0")/.."

TAG=""
DIST="apps/web/apps/web-antd/dist"
ARCH="amd64"
PUSH=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --tag) TAG="$2"; shift 2 ;;
    --dist) DIST="$2"; shift 2 ;;
    --arch) ARCH="$2"; shift 2 ;;
    --push) PUSH=1; shift ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

[[ -n "$TAG" ]] || { echo "需要 --tag <registry/image:tag>" >&2; exit 1; }
[[ -f "$DIST/index.html" ]] || { echo "前端产物缺失：$DIST（先 cd apps/web && bun run build:antd）" >&2; exit 1; }

case "$ARCH" in
  amd64|arm64|both) ;;
  *) echo "--arch 仅支持 amd64|arm64|both" >&2; exit 2 ;;
esac

build_one() {
  local arch="$1"
  echo "=== 构建 linux/${arch}：${TAG}-${arch}（镜像内编译，arm64 QEMU 较慢）==="
  docker build -f script/docker/Dockerfile.gateway \
    --platform "linux/${arch}" \
    --build-arg DIST="$DIST" \
    -t "${TAG}-${arch}" .
}

# 运行冒烟：不带 DATABASE_URL 启动，网关必须输出配置错误并退出——
# 证明二进制在容器 glibc 下真实可执行（动态链接、入口点、非 root 用户）。
smoke_one() {
  local arch="$1"
  echo "=== 运行冒烟：linux/${arch} ==="
  local output
  output="$(docker run --rm "${TAG}-${arch}" 2>&1 || true)"
  if grep -q "DATABASE_URL is required" <<<"$output"; then
    echo "冒烟通过：二进制在容器内正常运行"
  else
    echo "冒烟失败：未看到预期的配置错误输出" >&2
    echo "$output" >&2
    return 1
  fi
}

failed=0
build_and_smoke() {
  local arch="$1"
  build_one "$arch" && smoke_one "$arch" || failed=1
}
case "$ARCH" in
  amd64) build_and_smoke amd64 ;;
  arm64) build_and_smoke arm64 ;;
  both) build_and_smoke amd64; build_and_smoke arm64 ;;
esac
[[ "$failed" -eq 0 ]] || exit 1

if [[ "$PUSH" -eq 1 ]]; then
  echo "=== 推送并合成双架构 manifest ==="
  local_tags=()
  [[ "$ARCH" == "amd64" || "$ARCH" == "both" ]] && local_tags+=("${TAG}-amd64")
  [[ "$ARCH" == "arm64" || "$ARCH" == "both" ]] && local_tags+=("${TAG}-arm64")
  for tag in "${local_tags[@]}"; do docker push "$tag"; done
  docker buildx imagetools create -t "$TAG" "${local_tags[@]}"
  docker buildx imagetools inspect "$TAG"
else
  echo "本机构建完成（未推送）：${TAG}-${ARCH}"
  echo "双架构 manifest 需 --push（推送后用 buildx imagetools create 合成）"
fi
