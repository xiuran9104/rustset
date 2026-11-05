#!/usr/bin/env bash
# SBOM 与交付物校验（docs/xinchuang-adaptation-plan.md §4.6.5）。
#
# 产出 <输出目录>/：
#   sbom-cargo.txt       Rust 依赖清单（cargo tree 扁平唯一清单）
#   sbom-frontend.txt    前端依赖清单（bun pm ls）
#   Cargo.lock.sha256    锁文件指纹
#   sbom-summary.txt     汇总（直接依赖数 / 总依赖数 / 生成时间 / git 版本）
#
# 若构建机装有 cargo-cyclonedx（cargo install cargo-cyclonedx），自动追加
# 标准 CycloneDX SBOM（sbom-cyclonedx.json）——漏洞与许可证扫描（§4.6.5）
# 在内网扫描环境基于该文件进行。
#
# 用法：bash script/supply-chain/generate-sbom.sh [--output sbom-out]
set -euo pipefail
cd "$(dirname "$0")/../.."

OUTPUT="sbom-out"
while [[ $# -gt 0 ]]; do
  case "$1" in
    --output) OUTPUT="$2"; shift 2 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

mkdir -p "$OUTPUT"
rm -f "$OUTPUT"/sbom-cargo.json "$OUTPUT"/sbom-cyclonedx.json  # 清理旧格式残留

echo "=== Rust 依赖清单 ==="
# 扁平唯一清单（包名+版本+来源），Dependency 图按需用 `cargo tree --dots` 补充。
cargo tree --workspace --all-features --prefix none | sort -u \
  > "$OUTPUT/sbom-cargo.txt"
sha256sum Cargo.lock > "$OUTPUT/Cargo.lock.sha256"

echo "=== 前端依赖清单 ==="
if command -v bun >/dev/null && [[ -d apps/web ]]; then
  ( cd apps/web && bun pm ls --all ) > "$OUTPUT/sbom-frontend.txt" 2>/dev/null \
    || echo "(bun pm ls 失败；内网构建时基于 bun.lock 生成)" > "$OUTPUT/sbom-frontend.txt"
else
  echo "(bun 不可用；交付机内网生成)" > "$OUTPUT/sbom-frontend.txt"
fi

echo "=== CycloneDX（如可用）==="
if cargo cyclonedx --version >/dev/null 2>&1; then
  cargo cyclonedx --all-features --format json --output-path "$OUTPUT"
  echo "CycloneDX SBOM 已生成"
else
  echo "cargo-cyclonedx 未安装（cargo install cargo-cyclonedx）；交付扫描机基于清单文件执行"
fi

DIRECT=$(grep -c '^\[\[package\]\]' Cargo.lock || true)
GIT_REV=$(git rev-parse --short HEAD 2>/dev/null || echo unknown)
cat > "$OUTPUT/sbom-summary.txt" <<EOF
generated_at=$(date -u +%Y-%m-%dT%H:%M:%SZ)
git=$GIT_REV
cargo_lock_packages=$DIRECT
rust_sbom=sbom-cargo.txt
frontend_sbom=sbom-frontend.txt
note=漏洞/许可证扫描基于本清单或 CycloneDX 文件（§4.6.5）
EOF

echo "=== 完成 ==="
ls -la "$OUTPUT"
