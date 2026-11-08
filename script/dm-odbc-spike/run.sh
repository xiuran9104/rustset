#!/usr/bin/env bash
# 达梦 ODBC spike 可重复执行（docs/xinchuang-adaptation-plan.md §4.1.6、
# M0 清单第 3 项——2026-11-08 已 PASS，技术路线锁定：unixODBC + 厂商
# libdodbc.so + odbc-api）。
#
# 前提：script/dm-odbc-spike（spike crate）、DM8 容器（sizx/dm8，
# SYSDBA 口令 Dameng2026、无 @ 字符）、rust:bookworm 镜像。
#
# 实测要点（复刻必读）：
# * libdodbc.so 依赖 libdmdpi/libdmfldr/libdmcrypt 同目录库——整个 bin
#   目录的 .so 都要带上；
# * 驱动按固定路径 /home/dmdba/dmdbms/bin 找加密模块——spike 容器内必须
#   复刻该目录结构（仅 LD_LIBRARY_PATH 不够）；
# * libodbcinst.so 需要手工符号链接到 unixODBC 的带版本实体；
# * 达梦口令不能含 @（撞 disql/连接串解析）。
#
# 用法：bash script/dm-odbc-spike/run.sh
set -euo pipefail
cd "$(dirname "$0")/../.."

CONTAINER_DM="dm-clean"
CONTAINER_BUILD="dm-spike-build"
CONTAINER_RUN="dm-spike-run"
PORT="${DM_PORT:-5238}"

echo "=== 1/5 DM8 就绪 ==="
docker start "$CONTAINER_DM" >/dev/null 2>&1 || true
for _ in $(seq 1 40); do
  docker exec -u dmdba "$CONTAINER_DM" bash -lc \
    'echo "SELECT 1;" | LD_LIBRARY_PATH=/home/dmdba/dmdbms/bin /home/dmdba/dmdbms/bin/disql SYSDBA/Dameng2026@localhost:5236' \
    2>/dev/null | grep -q 'used time' && break
  sleep 3
done

echo "=== 2/5 unixODBC + 驱动（容器内 isql 连通性）==="
docker exec "$CONTAINER_DM" bash -c 'command -v isql >/dev/null || yum install -y unixODBC' >/dev/null
docker exec "$CONTAINER_DM" bash -c '
cat > /etc/odbcinst.ini << "EOS"
[DM8 ODBC DRIVER]
Driver = /home/dmdba/dmdbms/bin/libdodbc.so
EOS
mkdir -p /opt/dmodbc && cp /home/dmdba/dmdbms/bin/*.so /opt/dmodbc/'
docker exec dm-clean bash -c 'isql -v rustset-dm SYSDBA Dameng2026 "SELECT 1 FROM DUAL;"' 2>&1 | grep -aq 'Connected' \
  || { echo "容器内 isql 连通失败" >&2; exit 1; }
echo "容器内 isql 连通 OK"

echo "=== 3/5 Rust spike 编译（unixodbc-dev 工具链）==="
rm -f /tmp/dm-odbc-spike.tar.gz
tar czf /tmp/dm-odbc-spike.tar.gz -C script/dm-odbc-spike Cargo.toml src
docker rm -f "$CONTAINER_BUILD" >/dev/null 2>&1 || true
docker run --name "$CONTAINER_BUILD" -v /tmp/dm-odbc-spike.tar.gz:/src.tar.gz rust:bookworm bash -c '
apt-get update -qq >/dev/null 2>&1 && apt-get install -y -qq unixodbc-dev >/dev/null 2>&1
mkdir /spike && tar xzf /src.tar.gz -C /spike
cd /spike && cargo build --release'
docker cp "$CONTAINER_BUILD:/spike/target/release/dm-odbc-spike" /tmp/dm-odbc-spike-bin

echo "=== 4/5 复刻驱动目录 + 执行 spike ==="
rm -rf /tmp/dmodbc2 && docker cp "$CONTAINER_DM:/opt/dmodbc" /tmp/dmodbc2
docker rm -f "$CONTAINER_RUN" >/dev/null 2>&1 || true
docker run -d --name "$CONTAINER_RUN" --network container:"$CONTAINER_DM" \
  -v /tmp/dm-odbc-spike-bin:/spike-bin:ro \
  -v /tmp/dmodbc2:/opt/dmodbc:ro \
  debian:bookworm sleep 900 >/dev/null
docker exec "$CONTAINER_RUN" bash -c '
apt-get update -qq >/dev/null 2>&1 && apt-get install -y -qq unixodbc >/dev/null 2>&1
cp /opt/dmodbc/*.so /usr/local/lib/ 2>/dev/null || true
mkdir -p /home/dmdba/dmdbms/bin
cp /opt/dmodbc/*.so /home/dmdba/dmdbms/bin/
ln -sf /usr/lib/x86_64-linux-gnu/libodbcinst.so.2 /home/dmdba/dmdbms/bin/libodbcinst.so
cat > /etc/odbcinst.ini << "EOS"
[DM8 ODBC DRIVER]
Driver = /home/dmdba/dmdbms/bin/libdodbc.so
EOS
ldconfig >/dev/null 2>&1 || true
export DM_SERVER=127.0.0.1 DM_PORT=5236 DM_PWD=Dameng2026
/spike-bin'

echo "=== 5/5 清理 ==="
docker rm -f "$CONTAINER_RUN" >/dev/null
docker rm -f "$CONTAINER_BUILD" >/dev/null
echo "SPIKE 完成（结论：达梦 Rust ODBC 路线成立，M3 适配器以此为据）"
