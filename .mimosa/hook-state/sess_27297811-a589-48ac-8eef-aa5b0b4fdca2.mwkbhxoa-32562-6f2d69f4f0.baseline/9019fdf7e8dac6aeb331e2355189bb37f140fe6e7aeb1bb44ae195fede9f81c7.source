#!/usr/bin/env bash
# 依赖内网化：cargo vendor + 源替换配置（docs/xinchuang-adaptation-plan.md §4.6.3）。
#
# 在有外网的构建机上执行一次，产出 vendor/ 快照与离线源配置；离线环境
# 用同一快照构建（cargo check --offline 验证）。vendor/ 快照随发布介质
# 交付（~数百 MB），**不进入 git**（.gitignore 已排除）。
#
# 用法：
#   bash script/supply-chain/vendor-deps.sh            # 生成 vendor/ + 启用源替换
#   bash script/supply-chain/vendor-deps.sh --restore  # 恢复 config 模板（开发机用）
set -euo pipefail
cd "$(dirname "$0")/../.."

CONFIG=".cargo/config.toml"
MARKER_BEGIN="# >>> vendor source replacement (managed by script/supply-chain/vendor-deps.sh)"
MARKER_END="# <<< vendor source replacement"

restore_config() {
  python3 - "$CONFIG" <<'PY'
import sys, pathlib
path = pathlib.Path(sys.argv[1])
text = path.read_text()
begin = text.find("# >>> vendor source replacement")
end = text.find("# <<< vendor source replacement")
if begin >= 0 and end > begin:
    path.write_text(text[:begin].rstrip() + "\n")
    print("vendor source replacement removed")
else:
    print("no vendor block found; nothing to restore")
PY
}

if [[ "${1:-}" == "--restore" ]]; then
  restore_config
  rm -rf vendor
  echo "vendor/ 已删除，恢复开发机在线构建"
  exit 0
fi

command -v cargo >/dev/null || { echo "需要 cargo" >&2; exit 1; }

echo "=== cargo vendor（全量依赖快照）==="
# vendor 输出会打印需要写入 config 的替换片段；我们用受控模板重写，
# 保证 --offline 与 [source.crates-io] 的禁用顺序正确。
cargo vendor vendor > /tmp/vendor-config-snippet.toml

echo "=== 写入离线源替换 ==="
restore_config >/dev/null
# 先写临时文件再原子替换（避免读写同一文件）。
TMP_CONFIG="$(mktemp)"
{
  cat "$CONFIG"
  echo ""
  echo "$MARKER_BEGIN"
  echo "# 由 vendor-deps.sh 生成；--restore 可移除。离线构建验证：cargo check --offline"
  cat /tmp/vendor-config-snippet.toml
  echo "offline = true"
  echo "$MARKER_END"
} > "$TMP_CONFIG"
mv "$TMP_CONFIG" "$CONFIG"

echo "=== 离线构建验证 ==="
cargo check --offline -p rustset-gateway >/dev/null
echo "vendor 快照就绪：$(du -sh vendor | cut -f1)，离线构建验证通过"
echo "交付：vendor/ 与 .cargo/config.toml 一起放入发布介质（不入 git）"
