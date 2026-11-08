# 达梦 ODBC spike（M0 清单第 3 项，2026-11-08 PASS）

结论：**达梦 Rust ODBC 路线成立**——unixODBC + 厂商 `libdodbc.so` +
`odbc-api` 直连 DM8，建表/CRUD/并发压测（4×25）/故障注入全部通过。

## 运行

```bash
bash script/dm-odbc-spike/run.sh   # 全自动：容器内 isql 连通性 → Rust 编译 → spike 执行
```

前置：DM8 容器（`sizx/dm8`，SYSDBA/Dameng2026）、`rust:bookworm` 镜像。
Cargo.lock 未入库（spike 独立 crate，不进工作区依赖树）。

## 实测要点（复刻必读）

| 坑 | 处置 |
|---|---|
| `libdodbc.so` 依赖 `libdmdpi`/`libdmfldr`/`libdmcrypt` 同目录库 | 整个 bin 目录 `.so` 全拷 |
| 驱动按固定路径找加密模块（-70089 Encryption module failed to load） | 容器内复刻 `/home/dmdba/dmdbms/bin` 目录结构 |
| `libodbcinst.so` 未版本化引用 | 符号链接到 unixODBC 的 `.so.2` |
| 达梦口令含 `@` 撞连接串解析 | 口令避开 `@` |
| odbc-api 8 的 `CursorRow` 只有 `get_data`/`get_text` 手动缓冲 API | `get_text(1, &mut Vec<u8>)` 取标量 |
| 达梦把 `""` 视为标识符 | SQL 字面量一律单引号 |

## 与生产适配器（M3）的关系

spike 证明技术路线后，M3 生产适配器按同一路线实现 `DbPool` 等价面
（连接池/事务/行读取），spike 的驱动目录复刻逻辑进安装脚本
（`install.sh` 配置 `odbcinst.ini` + 库路径）。
