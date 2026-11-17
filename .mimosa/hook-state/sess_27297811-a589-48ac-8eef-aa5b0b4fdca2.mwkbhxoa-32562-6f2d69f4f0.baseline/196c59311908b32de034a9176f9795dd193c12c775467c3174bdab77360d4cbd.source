//! Sealed-secret management for the xinchuang crypto upgrade (plan §4.4.2).
//!
//! Layers on top of the raw v2 SM4-CBC primitives in [`crate`]:
//!
//! * `enc:sm4:v3:` — authenticated sealing with **key versioning**: format
//!   version, algorithm id, key version, random IV, ciphertext and an
//!   HMAC-SM3 tag (encrypt-then-MAC), so tampering is detectable and key
//!   rotation is possible.
//! * [`MasterKeyRing`] — data keys derived per version from an independently
//!   protected master key file (0600), separated from `JWT_SECRET`.
//! * [`SealingSecrets`] — the unified provider used by business code: seals
//!   with v3 when a master key ring is configured, falls back to the legacy
//!   v2 env-derived key otherwise, and opens v3/v2/plain transparently.
//!
//! v2 remains readable forever; the re-seal pass converts it to v3 in batches
//! once a master key file is present.

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

use crate::{SM4_SEALED_PREFIX, constant_time_eq, hmac_sm3, sm3_digest, sm4_open, sm4_seal};

/// Prefix of the v3 authenticated sealed format.
pub const SM4_V3_PREFIX: &str = "enc:sm4:v3:";

/// Environment variable pointing at the master key ring file.
pub const MASTER_KEY_FILE_ENV: &str = "SECRET_MASTER_KEY_FILE";

/// Tag length of the HMAC-SM3 integrity tag appended to v3 ciphertext.
const TAG_LEN: usize = 32;

/// A set of versioned 32-byte master keys; the highest version seals new data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MasterKeyRing {
    keys: Vec<(u32, [u8; 32])>,
}

impl MasterKeyRing {
    /// Build a ring from raw key bytes keyed by version. Fails when the list
    /// is empty or a version repeats.
    pub fn from_keys(keys: Vec<(u32, [u8; 32])>) -> Result<Self, GmSecretsError> {
        if keys.is_empty() {
            return Err(GmSecretsError("master key ring is empty".into()));
        }
        let mut keys = keys;
        keys.sort_by_key(|(version, _)| *version);
        for pair in keys.windows(2) {
            if pair[0].0 == pair[1].0 {
                return Err(GmSecretsError("duplicate master key version".into()));
            }
        }
        Ok(Self { keys })
    }

    /// Parse a key ring file: one `v<version> = <64 hex chars>` entry per
    /// line, `#` comments allowed. Returns the ring plus non-fatal warnings
    /// (e.g. loose file permissions) the caller should log.
    pub fn parse(contents: &str) -> Result<(Self, Vec<String>), GmSecretsError> {
        let mut keys = Vec::new();
        let warnings = Vec::new();
        for (index, raw) in contents.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((version_part, hex_part)) = line.split_once('=') else {
                return Err(GmSecretsError(format!(
                    "master key file line {}: expected 'v<version> = <hex>'",
                    index + 1
                )));
            };
            let version = version_part
                .trim()
                .strip_prefix('v')
                .and_then(|digits| digits.parse::<u32>().ok())
                .ok_or_else(|| {
                    GmSecretsError(format!(
                        "master key file line {}: invalid version",
                        index + 1
                    ))
                })?;
            if version == 0 {
                return Err(GmSecretsError(format!(
                    "master key file line {}: version starts at 1",
                    index + 1
                )));
            }
            let key = decode_key(hex_part.trim()).ok_or_else(|| {
                GmSecretsError(format!(
                    "master key file line {}: expected 64 hex chars (32 bytes)",
                    index + 1
                ))
            })?;
            keys.push((version, key));
        }
        let ring = Self::from_keys(keys)?;
        if !ring.latest_is_v1_style() {
            // informational only; gaps in versions are allowed for rollbacks
        }
        Ok((ring, warnings))
    }

    fn latest_is_v1_style(&self) -> bool {
        self.keys.first().is_some_and(|(version, _)| *version == 1)
    }

    /// Load and parse the ring from a file, checking restrictive permissions
    /// on Unix. Warnings are returned for the caller to log.
    pub fn load(path: &std::path::Path) -> Result<(Self, Vec<String>), GmSecretsError> {
        let mut warnings = Vec::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            match std::fs::metadata(path) {
                Ok(metadata) => {
                    let mode = metadata.permissions().mode();
                    if mode & 0o077 != 0 {
                        warnings.push(format!(
                            "master key file {path:?} is group/world accessible (mode {:o}); \
                             expected 0600",
                            mode & 0o777
                        ));
                    }
                }
                Err(error) => return Err(GmSecretsError(format!("stat {path:?}: {error}"))),
            }
        }
        let contents = std::fs::read_to_string(path)
            .map_err(|error| GmSecretsError(format!("read {path:?}: {error}")))?;
        let (ring, parse_warnings) = Self::parse(&contents)?;
        warnings.extend(parse_warnings);
        Ok((ring, warnings))
    }

    /// Highest available version — seals new data.
    pub fn active_version(&self) -> u32 {
        self.keys.last().map(|(version, _)| *version).unwrap_or(1)
    }

    fn master(&self, version: u32) -> Result<&[u8; 32], GmSecretsError> {
        self.keys
            .iter()
            .find(|(candidate, _)| *candidate == version)
            .map(|(_, key)| key)
            .ok_or_else(|| GmSecretsError(format!("master key version {version} not present")))
    }

    /// SM4 data key for a version (first 16 bytes of a domain-separated SM3).
    fn enc_key(&self, version: u32) -> Result<[u8; 16], GmSecretsError> {
        let master = self.master(version)?;
        let mut input = b"rustset/seal/enc/sm4-cbc:".to_vec();
        input.extend_from_slice(&version.to_be_bytes());
        input.push(b':');
        input.extend_from_slice(master);
        let digest = sm3_digest(&input);
        let mut key = [0u8; 16];
        key.copy_from_slice(&digest[..16]);
        Ok(key)
    }

    /// HMAC-SM3 MAC key for a version (full 32-byte domain-separated SM3).
    fn mac_key(&self, version: u32) -> Result<Vec<u8>, GmSecretsError> {
        let master = self.master(version)?;
        let mut input = b"rustset/seal/mac/hmac-sm3:".to_vec();
        input.extend_from_slice(&version.to_be_bytes());
        input.push(b':');
        input.extend_from_slice(master);
        Ok(sm3_digest(&input))
    }
}

fn decode_key(hex_text: &str) -> Option<[u8; 32]> {
    let bytes = hex::decode(hex_text).ok()?;
    let mut key = [0u8; 32];
    if bytes.len() != 32 {
        return None;
    }
    key.copy_from_slice(&bytes);
    Some(key)
}

/// Errors from the sealing provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GmSecretsError(pub String);

impl std::fmt::Display for GmSecretsError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for GmSecretsError {}

/// True when the value uses the authenticated v3 format.
pub fn is_v3_sealed(value: &str) -> bool {
    value.starts_with(SM4_V3_PREFIX)
}

/// Seal with the ring's active key version:
/// `enc:sm4:v3:<version>:<base64(iv || ciphertext || hmac-sm3 tag)>`.
///
/// The tag is computed over `version || iv || ciphertext` with the version's
/// MAC key (encrypt-then-MAC); opening verifies it before any decryption.
pub fn seal_v3(plain: &str, ring: &MasterKeyRing) -> Result<String, GmSecretsError> {
    if plain.is_empty() {
        return Ok(String::new());
    }
    let version = ring.active_version();
    let enc_key = ring.enc_key(version)?;
    let mac_key = ring.mac_key(version)?;

    let mut iv = [0u8; 16];
    rand::RngCore::fill_bytes(&mut rand::thread_rng(), &mut iv);
    let cipher =
        libsm::sm4::cipher_mode::Sm4CipherMode::new(&enc_key, libsm::sm4::cipher_mode::CipherMode::Cbc)
            .map_err(|error| GmSecretsError(format!("sm4 init: {error:?}")))?;
    let padded = pkcs7_pad(plain.as_bytes(), 16);
    let ciphertext = cipher
        .encrypt(&[], &padded, &iv)
        .map_err(|error| GmSecretsError(format!("sm4 encrypt: {error:?}")))?;

    let mut mac_input = version.to_be_bytes().to_vec();
    mac_input.extend_from_slice(&iv);
    mac_input.extend_from_slice(&ciphertext);
    let tag = hmac_sm3(&mac_key, &mac_input);

    let mut blob = iv.to_vec();
    blob.extend_from_slice(&ciphertext);
    blob.extend_from_slice(&tag);
    Ok(format!("{SM4_V3_PREFIX}{version}:{}", BASE64.encode(blob)))
}

/// Open a v3-sealed value. Verifies the HMAC-SM3 tag (constant-time) before
/// decrypting; unknown key versions and any tampering are hard errors.
pub fn open_v3(sealed: &str, ring: &MasterKeyRing) -> Result<String, GmSecretsError> {
    let Some(encoded) = sealed.strip_prefix(SM4_V3_PREFIX) else {
        return Err(GmSecretsError("value is not v3-sealed".into()));
    };
    let Some((version_part, blob_part)) = encoded.split_once(':') else {
        return Err(GmSecretsError("v3 sealed value is missing its key version".into()));
    };
    let version = version_part
        .parse::<u32>()
        .map_err(|_| GmSecretsError("v3 sealed value has an invalid key version".into()))?;
    let blob = BASE64
        .decode(blob_part)
        .map_err(|_| GmSecretsError("v3 sealed value is not valid base64".into()))?;
    if blob.len() < 16 + 16 + TAG_LEN || !(blob.len() - TAG_LEN).is_multiple_of(16) {
        return Err(GmSecretsError("v3 sealed value has invalid length".into()));
    }
    let body = &blob[..blob.len() - TAG_LEN];
    let tag = &blob[blob.len() - TAG_LEN..];
    let mac_key = ring.mac_key(version)?;
    let mut mac_input = version.to_be_bytes().to_vec();
    mac_input.extend_from_slice(body);
    let expected = hmac_sm3(&mac_key, &mac_input);
    if !constant_time_eq(&expected, tag) {
        return Err(GmSecretsError(
            "v3 sealed value failed integrity verification".into(),
        ));
    }
    let (iv, ciphertext) = body.split_at(16);
    let enc_key = ring.enc_key(version)?;
    let cipher =
        libsm::sm4::cipher_mode::Sm4CipherMode::new(&enc_key, libsm::sm4::cipher_mode::CipherMode::Cbc)
            .map_err(|error| GmSecretsError(format!("sm4 init: {error:?}")))?;
    let padded = cipher
        .decrypt(&[], ciphertext, iv)
        .map_err(|error| GmSecretsError(format!("sm4 decrypt: {error:?}")))?;
    let plain = pkcs7_unpad(&padded)
        .ok_or_else(|| GmSecretsError("v3 padding check failed".into()))?;
    String::from_utf8(plain).map_err(|_| GmSecretsError("v3 plaintext is not valid UTF-8".into()))
}

/// The unified sealing provider: master-key v3 when available, legacy v2
/// env-derived key otherwise, transparent open for v3/v2/plain.
#[derive(Debug, Clone)]
pub struct SealingSecrets {
    ring: Option<MasterKeyRing>,
    legacy_secret: String,
}

impl SealingSecrets {
    /// Resolve from the environment:
    ///
    /// * `SECRET_MASTER_KEY_FILE` → v3 sealing via [`MasterKeyRing::load`]
    ///   (missing file is an error, not a silent downgrade);
    /// * legacy secret: `SECRET_ENCRYPTION_KEY`, else `JWT_SECRET`, else the
    ///   local-development fallback — kept for v2 read compatibility and as
    ///   the v2 sealing fallback when no master key is configured.
    pub fn from_env() -> Result<Self, GmSecretsError> {
        let ring = match std::env::var(MASTER_KEY_FILE_ENV) {
            Ok(path) if !path.trim().is_empty() => {
                let (ring, warnings) = MasterKeyRing::load(std::path::Path::new(path.trim()))?;
                for warning in warnings {
                    // surfaced where tracing is unavailable; callers log at startup
                    eprintln!("warning: {warning}");
                }
                Some(ring)
            }
            _ => None,
        };
        let legacy_secret = std::env::var("SECRET_ENCRYPTION_KEY")
            .or_else(|_| std::env::var("JWT_SECRET"))
            .unwrap_or_else(|_| "rustset-local-secret".to_owned());
        Ok(Self {
            ring,
            legacy_secret,
        })
    }

    /// Construct explicitly (tests and tooling).
    pub fn new(ring: Option<MasterKeyRing>, legacy_secret: &str) -> Self {
        Self {
            ring,
            legacy_secret: legacy_secret.to_owned(),
        }
    }

    /// Whether new seals use the authenticated v3 format.
    pub fn uses_master_key(&self) -> bool {
        self.ring.is_some()
    }

    /// Active v3 key version, if a master key ring is configured.
    pub fn active_key_version(&self) -> Option<u32> {
        self.ring.as_ref().map(MasterKeyRing::active_version)
    }

    /// Seal a secret: v3 when a master key is configured, v2 otherwise
    /// (matching the pre-v3 behavior exactly). Already-sealed values and
    /// empty strings pass through unchanged so update flows never nest
    /// ciphertext layers.
    pub fn seal(&self, plain: &str) -> String {
        if plain.is_empty() || is_any_sealed_format(plain) {
            return plain.to_owned();
        }
        if let Some(ring) = &self.ring
            && let Ok(sealed) = seal_v3(plain, ring)
        {
            return sealed;
        }
        sm4_seal(plain, &self.legacy_secret).unwrap_or_else(|_| plain.to_owned())
    }

    /// Open a sealed secret. v3 values fail **closed**: without the key ring
    /// or when the integrity tag does not verify, nothing is returned —
    /// tampering is detectable instead of silently tolerated. v2/plain values
    /// keep the tolerant passthrough of [`crate::sm4_open`].
    pub fn open(&self, sealed: &str) -> String {
        if sealed.is_empty() {
            return String::new();
        }
        if is_v3_sealed(sealed) {
            return self
                .ring
                .as_ref()
                .and_then(|ring| open_v3(sealed, ring).ok())
                .unwrap_or_default();
        }
        sm4_open(sealed, &self.legacy_secret).unwrap_or_else(|_| String::new())
    }

    /// Re-seal a legacy v1 or v2 value to the active format. Returns
    /// `Ok(None)` when the value needs no conversion (already v3, plain, or
    /// v2 with no master key configured — the pass is then a no-op for v2
    /// while surviving v1 rows still convert to v2, preserving the
    /// historical startup behavior).
    pub fn upgrade_sealed(&self, sealed: &str) -> Result<Option<String>, GmSecretsError> {
        if sealed.is_empty() || is_v3_sealed(sealed) {
            return Ok(None);
        }
        let plain = if sealed.starts_with(SM4_SEALED_PREFIX) {
            sm4_open(sealed, &self.legacy_secret).map_err(|error| GmSecretsError(error.0))?
        } else if sealed.starts_with(crate::LEGACY_SEALED_PREFIX) {
            crate::legacy_v1_open(sealed, &self.legacy_secret)
                .ok_or_else(|| GmSecretsError("legacy value is not decodable".into()))?
        } else {
            return Ok(None);
        };
        if let Some(ring) = &self.ring {
            seal_v3(&plain, ring).map(Some)
        } else if sealed.starts_with(crate::LEGACY_SEALED_PREFIX) {
            sm4_seal(&plain, &self.legacy_secret)
                .map(Some)
                .map_err(|error| GmSecretsError(error.0))
        } else {
            Ok(None)
        }
    }
}

/// Whether the value already carries any sealed-value prefix (v1/v2/v3).
fn is_any_sealed_format(value: &str) -> bool {
    is_v3_sealed(value)
        || value.starts_with(SM4_SEALED_PREFIX)
        || value.starts_with(crate::LEGACY_SEALED_PREFIX)
}

fn pkcs7_pad(data: &[u8], block: usize) -> Vec<u8> {
    let padding = block - (data.len() % block);
    let mut padded = data.to_vec();
    padded.extend(std::iter::repeat_n(padding as u8, padding));
    padded
}

fn pkcs7_unpad(data: &[u8]) -> Option<Vec<u8>> {
    let last = *data.last()? as usize;
    if last == 0 || last > 16 || last > data.len() {
        return None;
    }
    let (body, padding) = data.split_at(data.len() - last);
    (padding.iter().all(|byte| *byte as usize == last)).then(|| body.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ring(version: u32, seed: u8) -> MasterKeyRing {
        let key = [seed; 32];
        MasterKeyRing::from_keys(vec![(version, key)]).unwrap()
    }

    fn parse_ring(contents: &str) -> MasterKeyRing {
        MasterKeyRing::parse(contents).unwrap().0
    }

    #[test]
    fn ring_parse_and_versions() {
        let contents = "# comment\nv2 = deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef\nv1 = 1111111111111111111111111111111111111111111111111111111111111111\n";
        let ring = parse_ring(contents);
        assert_eq!(ring.active_version(), 2);
        assert!(ring.master(1).is_ok());
        assert!(ring.master(3).is_err());
        assert!(MasterKeyRing::parse("").is_err());
        assert!(MasterKeyRing::parse("v1 = not-hex").is_err());
        assert!(MasterKeyRing::parse("v0 = 1111111111111111111111111111111111111111111111111111111111111111").is_err());
    }

    #[test]
    fn v3_roundtrip_and_tamper_detection() {
        let ring = ring(1, 7);
        for plain in ["short", "包含中文的机密", "a".repeat(100).as_str()] {
            let sealed = seal_v3(plain, &ring).unwrap();
            assert!(is_v3_sealed(&sealed));
            assert!(sealed.starts_with("enc:sm4:v3:1:"));
            assert_ne!(sealed, plain);
            assert_eq!(open_v3(&sealed, &ring).unwrap(), plain);
        }
        // random IV: two seals differ
        let first = seal_v3("same", &ring).unwrap();
        let second = seal_v3("same", &ring).unwrap();
        assert_ne!(first, second);
    }

    #[test]
    fn v3_tampering_fails() {
        let ring = ring(1, 7);
        let sealed = seal_v3("sensitive", &ring).unwrap();
        // flip a ciphertext byte (inside base64 blob)
        let encoded = sealed.strip_prefix(SM4_V3_PREFIX).unwrap();
        let (version, blob) = encoded.split_once(':').unwrap();
        let mut bytes = BASE64.decode(blob).unwrap();
        let mid = bytes.len() / 2;
        bytes[mid] ^= 0x01;
        let tampered = format!("{SM4_V3_PREFIX}{version}:{}", BASE64.encode(bytes));
        let error = open_v3(&tampered, &ring).unwrap_err();
        assert!(error.0.contains("integrity"));
        // flip a tag byte
        let mut bytes = BASE64.decode(blob).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 0x01;
        let tampered = format!("{SM4_V3_PREFIX}{version}:{}", BASE64.encode(bytes));
        assert!(open_v3(&tampered, &ring).is_err());
    }

    #[test]
    fn v3_unknown_key_version_is_rejected() {
        let old = ring(1, 7);
        let sealed = seal_v3("secret", &old).unwrap();
        let rotated = ring(2, 9);
        assert_eq!(open_v3(&sealed, &old).unwrap(), "secret");
        let error = open_v3(&sealed, &rotated).unwrap_err();
        assert!(error.0.contains("version 1 not present"));
    }

    #[test]
    fn provider_seals_v3_when_ring_present() {
        let ring = ring(1, 3);
        let provider = SealingSecrets::new(Some(ring), "legacy-secret");
        assert!(provider.uses_master_key());
        let sealed = provider.seal("top-secret");
        assert!(is_v3_sealed(&sealed));
        assert_eq!(provider.open(&sealed), "top-secret");
        assert_eq!(provider.open("plain-value"), "plain-value");
        assert_eq!(provider.open(""), "");
    }

    #[test]
    fn provider_seals_v2_without_ring_and_reads_both() {
        let provider = SealingSecrets::new(None, "legacy-secret");
        let v2 = provider.seal("legacy-value");
        assert!(v2.starts_with(SM4_SEALED_PREFIX));

        // a provider WITH a ring still reads v2 values sealed by the legacy key
        let upgraded = SealingSecrets::new(Some(ring(1, 3)), "legacy-secret");
        assert_eq!(upgraded.open(&v2), "legacy-value");

        // and upgrade_sealed converts v2 -> v3, is idempotent, and leaves
        // plain values alone
        let converted = upgraded.upgrade_sealed(&v2).unwrap().unwrap();
        assert!(is_v3_sealed(&converted));
        assert_eq!(upgraded.open(&converted), "legacy-value");
        assert!(upgraded.upgrade_sealed(&converted).unwrap().is_none());
        assert!(upgraded.upgrade_sealed("plain").unwrap().is_none());
    }

    #[test]
    fn upgrade_without_ring_preserves_historical_behavior() {
        let provider = SealingSecrets::new(None, "legacy-secret");
        // v2 stays v2 (no master key configured)
        let v2 = provider.seal("value");
        assert!(provider.upgrade_sealed(&v2).unwrap().is_none());
        // surviving v1 rows still convert to v2
        let v1 = crate::legacy_sealed_for_test("legacy-plain", "legacy-secret");
        let converted = provider.upgrade_sealed(&v1).unwrap().unwrap();
        assert!(converted.starts_with(SM4_SEALED_PREFIX));
        assert_eq!(provider.open(&converted), "legacy-plain");
    }

    #[test]
    fn seal_never_nests_and_v3_open_fails_closed() {
        let provider = SealingSecrets::new(Some(ring(1, 5)), "legacy-secret");
        let sealed = provider.seal("secret-value");
        assert_eq!(provider.seal(&sealed), sealed); // no double sealing
        // v3 value cannot be opened without the ring (fail closed, no leak)
        let ringless = SealingSecrets::new(None, "legacy-secret");
        assert_eq!(ringless.open(&sealed), "");
        // tampered v3 also fails closed
        let tampered = format!("{sealed}x");
        assert_eq!(provider.open(&tampered), "");
    }

    #[test]
    fn key_derivation_is_domain_separated() {
        let ring = MasterKeyRing::from_keys(vec![(1, [1u8; 32])]).unwrap();
        let enc1 = ring.enc_key(1).unwrap();
        let mac1 = ring.mac_key(1).unwrap();
        let enc2 = MasterKeyRing::from_keys(vec![(2, [1u8; 32])]).unwrap().enc_key(2).unwrap();
        // enc and mac keys derive from different domains; versions differ
        assert_ne!(&enc1[..], mac1.as_slice());
        assert_ne!(enc1, enc2);
        // matches the documented derivation (SM3(domain||version||master))
        let mut input = b"rustset/seal/enc/sm4-cbc:".to_vec();
        input.extend_from_slice(&1u32.to_be_bytes());
        input.push(b':');
        input.extend_from_slice(&[1u8; 32]);
        let digest = crate::sm3_digest(&input);
        assert_eq!(&enc1[..], &digest[..16]);
    }
}
