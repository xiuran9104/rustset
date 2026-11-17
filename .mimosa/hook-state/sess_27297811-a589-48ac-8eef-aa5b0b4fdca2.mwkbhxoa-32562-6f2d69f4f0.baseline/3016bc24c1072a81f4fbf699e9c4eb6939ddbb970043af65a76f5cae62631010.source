//! 密码提供方抽象（docs/xinchuang-adaptation-plan.md §4.4.3 #2）。
//!
//! 业务与密封层只面向 [`Sm4Provider`] / [`RandomProvider`] 编程；
//! 纯软件实现（libsm + OS 随机数）是非密评场景的默认，SDF 密码卡 /
//! 加密机（GM/T 0018 接口）实装时提供 trait 的硬件实现并经
//! `CRYPTO_PROVIDER` 显式启用——不支持的取值按 §3.3 启动失败，
//! 不允许静默回退到软件实现。

use crate::GmError;

/// 随机数提供方。软件实现使用 OS 熵源（getrandom）；
/// 测评要求 GM/T 0005 检测时由硬件实现取随机（§4.4.3 #5）。
pub trait RandomProvider: Send + Sync {
    fn fill_random(&self, buffer: &mut [u8]);
}

/// SM4-CBC 加解密提供方。密钥 16 字节、IV 16 字节，输出含 PKCS7 填充
/// 的完整密文（与 [`crate::sm4_seal`] 的语义一致）。
pub trait Sm4Provider: Send + Sync {
    fn encrypt_cbc(&self, key: &[u8; 16], iv: &[u8; 16], plain: &[u8]) -> Result<Vec<u8>, GmError>;
    fn decrypt_cbc(&self, key: &[u8; 16], iv: &[u8; 16], cipher: &[u8]) -> Result<Vec<u8>, GmError>;
}

/// 纯软件实现：libsm SM4-CBC + OS 随机数。
#[derive(Debug, Clone, Copy, Default)]
pub struct SoftwareProviders;

impl RandomProvider for SoftwareProviders {
    fn fill_random(&self, buffer: &mut [u8]) {
        rand::RngCore::fill_bytes(&mut rand::thread_rng(), buffer);
    }
}

impl Sm4Provider for SoftwareProviders {
    fn encrypt_cbc(&self, key: &[u8; 16], iv: &[u8; 16], plain: &[u8]) -> Result<Vec<u8>, GmError> {
        use libsm::sm4::cipher_mode::{CipherMode, Sm4CipherMode};
        let cipher = Sm4CipherMode::new(key, CipherMode::Cbc)
            .map_err(|error| GmError(format!("sm4 init: {error:?}")))?;
        cipher.encrypt(&[], plain, iv).map_err(|error| GmError(format!("sm4 encrypt: {error:?}")))
    }

    fn decrypt_cbc(&self, key: &[u8; 16], iv: &[u8; 16], cipher: &[u8]) -> Result<Vec<u8>, GmError> {
        use libsm::sm4::cipher_mode::{CipherMode, Sm4CipherMode};
        let mode = Sm4CipherMode::new(key, CipherMode::Cbc)
            .map_err(|error| GmError(format!("sm4 init: {error:?}")))?;
        mode.decrypt(&[], cipher, iv).map_err(|error| GmError(format!("sm4 decrypt: {error:?}")))
    }
}

/// 密码提供方组合（§3.3/§4.4.3 #2）。
#[derive(Debug, Clone)]
pub enum CryptoProviders {
    /// 纯软件（默认，非密评场景）。
    Software(SoftwareProviders),
    /// GM/T 0018 SDF 软件模拟器（句柄语义/审计计数真实，运算为软件——
    /// 硬件 SDF 卡到位后按同接口替换）。
    SdfSim(crate::provider_kms::SdfSimProvider),
    /// HTTP KMS 信封加密（主密钥驻留 KMS；测试可用进程内 mock）。
    Kms(crate::provider_kms::KmsHttpProvider),
}

impl CryptoProviders {
    pub fn as_sm4(&self) -> &dyn Sm4Provider {
        match self {
            Self::Software(inner) => inner,
            Self::SdfSim(inner) => inner,
            Self::Kms(inner) => inner,
        }
    }

    pub fn as_random(&self) -> &dyn RandomProvider {
        match self {
            Self::Software(inner) => inner,
            Self::SdfSim(inner) => inner,
            Self::Kms(inner) => inner,
        }
    }
}

/// `CRYPTO_PROVIDER` 取值校验与构造（§3.3：不支持的组合启动失败）。
pub fn resolve_crypto_provider(env_value: Option<&str>) -> Result<CryptoProviders, GmError> {
    match env_value.map(str::trim).filter(|value| !value.is_empty()) {
        None | Some("software") | Some("standard") => Ok(CryptoProviders::Software(SoftwareProviders)),
        Some("sdf-sim") => Ok(CryptoProviders::SdfSim(crate::provider_kms::SdfSimProvider::new())),
        Some("kms") => Ok(CryptoProviders::Kms(crate::provider_kms::KmsHttpProvider::from_env()?)),
        Some(other) => Err(GmError(format!(
            "CRYPTO_PROVIDER={other} is not supported; available: software | sdf-sim | kms"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn software_roundtrip_matches_direct_sm4() {
        let key = [7u8; 16];
        let iv = [9u8; 16];
        let plain = b"provider-trait-roundtrip";
        let cipher = SoftwareProviders.encrypt_cbc(&key, &iv, plain).unwrap();
        let opened = SoftwareProviders.decrypt_cbc(&key, &iv, &cipher).unwrap();
        assert_eq!(opened, plain);
    }

    #[test]
    fn random_fills_and_varies() {
        let mut first = [0u8; 32];
        let mut second = [0u8; 32];
        SoftwareProviders.fill_random(&mut first);
        SoftwareProviders.fill_random(&mut second);
        assert_ne!(first, second);
        assert!(first.iter().any(|byte| *byte != 0));
    }

    #[test]
    fn crypto_provider_env_is_strict() {
        assert!(resolve_crypto_provider(None).is_ok());
        assert!(resolve_crypto_provider(Some("software")).is_ok());
        assert!(resolve_crypto_provider(Some("standard")).is_ok());
        assert!(matches!(
            resolve_crypto_provider(Some("sdf-sim")),
            Ok(CryptoProviders::SdfSim(_))
        ));
        // kms 需要 KMS_ENDPOINT，缺失即失败（§3.3）
        assert!(resolve_crypto_provider(Some("kms")).is_err());
        let error = resolve_crypto_provider(Some("sdf")).unwrap_err();
        assert!(error.0.contains("not supported"), "{}", error.0);
    }
}
