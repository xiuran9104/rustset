//! 离线密钥管理子命令（信创方案 §4.4.2）：
//!
//! * `rustset-gateway gen-master-key <path> [--force]` — 生成 32 字节主密钥
//!   钥匙环文件（0600），作为 `SECRET_MASTER_KEY_FILE` 的输入；
//! * `rustset-gateway reseal-secrets [--batch N]` — 用统一注册表把历史
//!   `enc:v1:` / `enc:sm4:v2:` 密文批量重加密为 v3，分批、幂等、可重跑，
//!   失败行保留原值并计数。

use anyhow::{Context, bail};
use rustset_framework_database::{DatabaseConfig, connect, migrate_with_flavor};
use rustset_framework_gm::secrets::{MASTER_KEY_FILE_ENV, SealingSecrets};

/// `gen-master-key`：生成 v1 主密钥并写盘。
pub fn gen_master_key(args: &[String]) -> anyhow::Result<()> {
    let Some(path) = args.iter().find(|arg| !arg.starts_with("--")) else {
        bail!("usage: rustset-gateway gen-master-key <path> [--force]");
    };
    let force = args.iter().any(|arg| arg == "--force");
    let target = std::path::Path::new(path);
    if target.exists() && !force {
        bail!("refusing to overwrite {path:?}; pass --force to replace it");
    }

    let mut key = [0u8; 32];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut key);
    let hex_key: String = key.iter().map(|byte| format!("{byte:02x}")).collect();
    let contents = format!(
        "# RustSet sealing master key ring (信创方案 §4.4.2)\n\
         # Format: v<version> = <64 hex chars>; the highest version seals new data.\n\
         # Keep permissions 0600 and back the file up under key-management controls.\n\
         v1 = {hex_key}\n"
    );
    std::fs::write(target, contents).with_context(|| format!("write {path:?}"))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(target, std::fs::Permissions::from_mode(0o600))
            .with_context(|| format!("chmod 0600 {path:?}"))?;
    }
    println!("master key ring written to {path:?} (v1, mode 0600)");
    println!(
        "next: set {MASTER_KEY_FILE_ENV}={path}, restart the gateway, then run \
         `reseal-secrets` to convert surviving v1/v2 values to v3"
    );
    Ok(())
}

/// `reseal-secrets`：批量把 v1/v2 密文升级到 v3（需要先配置主密钥文件）。
pub async fn reseal_secrets(args: &[String]) -> anyhow::Result<()> {
    let batch = args
        .windows(2)
        .find(|pair| pair[0] == "--batch")
        .and_then(|pair| pair[1].parse::<i64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(200);
    let provider = SealingSecrets::from_env().map_err(|error| anyhow::anyhow!("{error}"))?;
    if !provider.uses_master_key() {
        bail!(
            "no master key ring configured; set {MASTER_KEY_FILE_ENV} first — \
             v2 values stay untouched otherwise"
        );
    }
    println!(
        "master key ring active (v{}); re-sealing in batches of {batch}",
        provider.active_key_version().unwrap_or(0)
    );
    let database_config = DatabaseConfig::from_env()?;
    let database = connect(&database_config).await?;
    migrate_with_flavor(&database, database_config.flavor).await?;
    let (converted, skipped) =
        rustset_system_server::bootstrap::reseal_sealed_columns(&database, &provider, batch)
            .await?;
    println!("re-sealed {converted} value(s) to enc:sm4:v3");
    if skipped > 0 {
        println!(
            "skipped {skipped} undecodable value(s); verify the historical \
             SECRET_ENCRYPTION_KEY / JWT_SECRET and rerun"
        );
    }
    Ok(())
}
