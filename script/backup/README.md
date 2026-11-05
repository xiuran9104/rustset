# 备份与恢复（docs/xinchuang-adaptation-plan.md §4.7）

| 脚本 | 用途 |
| --- | --- |
| `postgres-backup.sh` | pg_dump 自定义格式备份 + SHA256 + 轮转（默认保留 14 份） |
| `postgres-restore.sh` | 校验完整性 → 确认覆盖 → `pg_restore --clean --if-exists --jobs=N` → 抽查 |
| `rustfs-manifest.sh` | 对象存储清单 CSV（key/size/etag），恢复时 `--verify` 逐对象校验 |

## 同一恢复集原则（§4.7.1）

数据库与对象存储必须取自同一恢复点，不拼接不同时间点的产物：

```bash
DATABASE_URL=postgres://… bash script/backup/postgres-backup.sh --dir /var/backups/rustset
bash script/backup/rustfs-manifest.sh --endpoint http://127.0.0.1:9000 \
     --bucket rustset --output /var/backups/rustset/rustset-<STAMP>.objects.csv
```

两个产物放在同一目录并一起归档；跨介质传递时校验 `.sha256`。

## 恢复演练清单（§4.7.4，§6.6 每种数据库至少一次空环境演练）

1. 停止写入：`systemctl stop rustset-gateway`
2. 准备空目标库（演练机或目标实例，不覆盖生产库）
3. `postgres-restore.sh --dump … --url …`（先校验 SHA256，损坏即拒绝恢复）
4. `rustfs-manifest.sh --verify <objects.csv>` 校验对象存在与大小
5. 启动服务（网关自动 reconciliation 迁移链），登录 + 业务抽查 + 文件下载
6. 记录演练结论：恢复耗时、校验结果、差异清单与处置

## 密钥与证书（§4.7.3）

- 密封主密钥文件（`SECRET_MASTER_KEY_FILE`）单独受控备份，**禁止以明文进入普通备份目录**；丢失主密钥 = v3 密文永久不可解，备份策略与轮换（`gen-master-key` → `reseal-secrets`）一起演练。
- TLS 证书与私钥分别备份；恢复后核对有效期与证书链。
- 备份目录权限 0700，备份文件不含明文密钥（数据库内为密封密文）。
