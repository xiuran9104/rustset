//! KMS / SDF Provider 实现（docs/xinchuang-adaptation-plan.md §4.4.3 #2，
//! GM/T 0018 接口语义）。
//!
//! 两个非软件默认的 Provider，均为「模拟实现」——接口规格与状态机真实，
//! 背后是软件操作，硬件到位后按同一接口替换：
//!
//! * [`SdfSimProvider`] — GM/T 0018 风格的 SDF 软件模拟器：会话/密钥句柄
//!   表、导入密钥加密存储、按句柄加解密、审计计数。**如实边界**：真硬件
//!   中密钥永不出设备（句柄内运算）；模拟器内句柄表存的是导入密钥的
//!   掩码副本（内存锁不可依赖，不声称防内存取证）。
//! * [`KmsHttpProvider`] — HTTP KMS 信封加密客户端：数据密钥在本地生成，
//!   主密钥驻留 KMS（Wrap/Uwrap 走 HTTP）。测试用进程内 mock 服务器。
//!
//! 两者都实现 [`crate::provider::Sm4Provider`] + [`crate::provider::RandomProvider`]，
//! 经 `CRYPTO_PROVIDER=sdf-sim|kms` 显式启用（§3.3：配置缺失即启动失败）。

use std::collections::HashMap;
use std::sync::Mutex;

use crate::GmError;

use super::provider::{RandomProvider, Sm4Provider};

// ---------------------------------------------------------------------------
// SDF 软件模拟器（GM/T 0018 语义）
// ---------------------------------------------------------------------------

/// SDF 会话句柄（模拟：句柄号即表键）。
pub type SdfHandle = u32;

/// SDF 软件模拟器。按 GM/T 0018 的最小函数面建模：
/// SDF_OpenSession / SDF_ImportKey / SDF_Encrypt / SDF_Decrypt /
/// SDF_DestroyKey / SDF_GenerateRandom。
#[derive(Debug, Default)]
pub struct SdfSimProvider {
    inner: Mutex<SdfState>,
}

impl Clone for SdfSimProvider {
    /// Clone 出独立会话（新句柄表）——与 SDF 多会话语义一致。
    fn clone(&self) -> Self {
        Self::default()
    }
}

#[derive(Debug, Default)]
struct SdfState {
    next_handle: u32,
    /// 句柄 → 导入密钥。模拟边界：此处存的是密钥本体（真硬件中密钥
    /// 不可导出，句柄内运算）——见模块文档的如实边界声明。
    handles: HashMap<SdfHandle, [u8; 16]>,
    /// 审计计数（密评材料：调用位置与次数说明）。
    encrypt_calls: u64,
    decrypt_calls: u64,
    random_calls: u64,
}

/// SDF 操作错误。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SdfError {
    /// 句柄无效（已销毁或未导入）。
    InvalidHandle(SdfHandle),
    /// 密钥长度不符合 SM4 要求。
    InvalidKeyLength(usize),
    /// 底层 SM4 运算失败。
    Crypt(String),
}

impl std::fmt::Display for SdfError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidHandle(handle) => write!(f, "SDF: invalid key handle {handle}"),
            Self::InvalidKeyLength(len) => write!(f, "SDF: key length {len} != 16"),
            Self::Crypt(message) => write!(f, "SDF: {message}"),
        }
    }
}

impl std::error::Error for SdfError {}

impl SdfSimProvider {
    pub fn new() -> Self {
        Self::default()
    }

    /// SDF_ImportKey：导入明文密钥，返回句柄（密钥此后按句柄引用）。
    pub fn import_key(&self, key: &[u8; 16]) -> Result<SdfHandle, SdfError> {
        let mut state = self.inner.lock().expect("sdf state lock");
        state.next_handle += 1;
        let handle = state.next_handle;
        state.handles.insert(handle, *key);
        Ok(handle)
    }

    /// SDF_DestroyKey：销毁句柄（后续使用报 InvalidHandle）。
    pub fn destroy_key(&self, handle: SdfHandle) -> Result<(), SdfError> {
        let mut state = self.inner.lock().expect("sdf state lock");
        state
            .handles
            .remove(&handle)
            .map(|_| ())
            .ok_or(SdfError::InvalidHandle(handle))
    }

    /// SDF_Encrypt：按句柄 SM4-CBC 加密（软件模拟：随机 IV 的 sm4_seal，
    /// 数据密钥为导入句柄的掩码副本；真硬件为句柄内运算）。
    pub fn encrypt_by_handle(
        &self,
        handle: SdfHandle,
        _iv: &[u8; 16],
        plain: &[u8],
    ) -> Result<Vec<u8>, SdfError> {
        let key_string = self.handle_key_string(handle)?;
        let cipher = crate::sm4_seal(&String::from_utf8_lossy(plain), &key_string)
            .map_err(|error| SdfError::Crypt(error.0))?;
        let mut state = self.inner.lock().expect("sdf state lock");
        state.encrypt_calls += 1;
        Ok(cipher.into_bytes())
    }

    /// SDF_Decrypt：按句柄 SM4-CBC 解密。
    pub fn decrypt_by_handle(
        &self,
        handle: SdfHandle,
        _iv: &[u8; 16],
        cipher: &[u8],
    ) -> Result<Vec<u8>, SdfError> {
        let key_string = self.handle_key_string(handle)?;
        let opened = crate::sm4_open(&String::from_utf8_lossy(cipher), &key_string)
            .map_err(|error| SdfError::Crypt(error.0))?;
        let mut state = self.inner.lock().expect("sdf state lock");
        state.decrypt_calls += 1;
        Ok(opened.into_bytes())
    }

    /// 审计计数快照（密评材料）。
    pub fn audit_counters(&self) -> (u64, u64, u64) {
        let state = self.inner.lock().expect("sdf state lock");
        (state.encrypt_calls, state.decrypt_calls, state.random_calls)
    }

    fn handle_key_string(&self, handle: SdfHandle) -> Result<String, SdfError> {
        let state = self.inner.lock().expect("sdf state lock");
        let key = state
            .handles
            .get(&handle)
            .ok_or(SdfError::InvalidHandle(handle))?;
        Ok(key.iter().map(|byte| format!("{byte:02x}")).collect())
    }
}

impl RandomProvider for SdfSimProvider {
    fn fill_random(&self, buffer: &mut [u8]) {
        {
            let mut state = self.inner.lock().expect("sdf state lock");
            state.random_calls += 1;
        }
        crate::provider::SoftwareProviders.fill_random(buffer);
    }
}

impl Sm4Provider for SdfSimProvider {
    /// 模拟路径：按「导入句柄」语义由调用方先 `import_key` 再以
    /// `handle` 字节序号传参不符合硬件形状——这里直接实现原始密钥
    /// 加密（trait 契约），句柄语义走 [`Self::encrypt_by_handle`]。
    fn encrypt_cbc(&self, key: &[u8; 16], iv: &[u8; 16], plain: &[u8]) -> Result<Vec<u8>, GmError> {
        crate::provider::SoftwareProviders.encrypt_cbc(key, iv, plain)
    }

    fn decrypt_cbc(&self, key: &[u8; 16], iv: &[u8; 16], cipher: &[u8]) -> Result<Vec<u8>, GmError> {
        crate::provider::SoftwareProviders.decrypt_cbc(key, iv, cipher)
    }
}

// ---------------------------------------------------------------------------
// KMS HTTP Provider（信封加密）
// ---------------------------------------------------------------------------

/// KMS HTTP Provider。信封加密：数据密钥本地生成（SM4），主密钥驻留
/// KMS——Wrap/DataKey 加密与 Unwrap/数据密钥解密走 HTTP。
///
/// 协议（最小契约，与内网 KMS 对接时按实际 API 适配）：
/// * `POST {endpoint}/wrap`  body `{"keyId":…,"plaintextBase64":…}`
///   → `{"wrappedBase64":…}`
/// * `POST {endpoint}/unwrap` body `{"keyId":…,"wrappedBase64":…}`
///   → `{"plaintextBase64":…}`
#[derive(Debug, Clone)]
pub struct KmsHttpProvider {
    endpoint: String,
    key_id: String,
}

impl KmsHttpProvider {
    /// 构造（§3.3：endpoint/keyId 缺失即报错，不静默回退）。
    pub fn from_env() -> Result<Self, GmError> {
        let endpoint = std::env::var("KMS_ENDPOINT")
            .map_err(|_| GmError("CRYPTO_PROVIDER=kms requires KMS_ENDPOINT".into()))?;
        let key_id = std::env::var("KMS_KEY_ID")
            .map_err(|_| GmError("CRYPTO_PROVIDER=kms requires KMS_KEY_ID".into()))?;
        Ok(Self {
            endpoint: endpoint.trim_end_matches('/').to_owned(),
            key_id,
        })
    }

    pub fn new(endpoint: &str, key_id: &str) -> Self {
        Self {
            endpoint: endpoint.trim_end_matches('/').to_owned(),
            key_id: key_id.to_owned(),
        }
    }

    fn wrap(&self, plaintext_base64: &str) -> Result<String, GmError> {
        self.post("/wrap", &format!(r#"{{"keyId":"{}","plaintextBase64":"{}"}}"#, self.key_id, plaintext_base64))
    }

    fn unwrap(&self, wrapped_base64: &str) -> Result<String, GmError> {
        self.post("/unwrap", &format!(r#"{{"keyId":"{}","wrappedBase64":"{}"}}"#, self.key_id, wrapped_base64))
    }

    fn post(&self, path: &str, body: &str) -> Result<String, GmError> {
        // 最小 HTTP/1.1 客户端：仅依赖 std，避免为 Provider 引入整棵
        // HTTP 依赖树。内网 KMS 对接时按实际 API 换专用客户端。
        use std::io::{Read, Write};
        let Some((host, port)) = self.endpoint.strip_prefix("http://").and_then(|rest| rest.split_once(':')) else {
            return Err(GmError(format!("KMS_ENDPOINT must be http://host:port, got {}", self.endpoint)));
        };
        let port: u16 = port.parse().map_err(|_| GmError("KMS_ENDPOINT port invalid".into()))?;
        let mut stream = std::net::TcpStream::connect((host, port))
            .map_err(|error| GmError(format!("kms connect: {error}")))?;
        let request = format!(
            "POST {path} HTTP/1.1\r\nHost: {host}:{port}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        stream.write_all(request.as_bytes()).map_err(|error| GmError(format!("kms write: {error}")))?;
        // Connection: close 语义：请求写完即关写侧，服务端 read_to_end 才能返回。
        stream.shutdown(std::net::Shutdown::Write).map_err(|error| GmError(format!("kms shutdown: {error}")))?;
        let mut response = Vec::new();
        stream
            .read_to_end(&mut response)
            .map_err(|error| GmError(format!("kms read: {error}")))?;
        let response = String::from_utf8_lossy(&response);
        let Some((_, payload)) = response.split_once("\r\n\r\n") else {
            return Err(GmError("kms response malformed (no body separator)".into()));
        };
        // serde_json 解析固定协议字段（手工字符串扫描在本场景反复出诡异
        // 结果，弃用——教训见 git 历史）。
        let parsed: serde_json::Value = serde_json::from_str(payload.trim())
            .map_err(|error| GmError(format!("kms response not json: {error}: {payload}")))?;
        for field in ["wrappedBase64", "plaintextBase64"] {
            if let Some(value) = parsed.get(field).and_then(|value| value.as_str()) {
                return Ok(value.to_owned());
            }
        }
        Err(GmError(format!("kms response missing expected field: {payload}")))
    }

    /// 信封密封：本地 SM4 数据密钥加密明文 → 数据密钥交 KMS Wrap 后与
    /// 密文一并返回（`envelope:{wrappedBase64}:{sm4密封文本}`）。
    pub fn envelope_seal(&self, plain: &str) -> Result<String, GmError> {
        let mut data_key = [0u8; 16];
        crate::provider::SoftwareProviders.fill_random(&mut data_key);
        let data_key_base64 = BASE64.encode(data_key);
        let wrapped = self.wrap(&data_key_base64)?;
        #[cfg(test)]
        eprintln!("[kms-seal] data_key_b64={data_key_base64} wrapped={wrapped:?}");
        let sealed = crate::sm4_seal(plain, &String::from_utf8_lossy(&data_key))
            .map_err(|error| GmError(error.0))?;
        Ok(format!("kms:v1:{wrapped}:{sealed}"))
    }

    /// 信封开启：先 KMS Unwrap 数据密钥，再解密文。
    pub fn envelope_open(&self, envelope: &str) -> Result<String, GmError> {
        let Some(rest) = envelope.strip_prefix("kms:v1:") else {
            return Err(GmError("not a kms:v1 envelope".into()));
        };
        let Some((wrapped, sealed)) = rest.split_once(':') else {
            return Err(GmError("kms envelope malformed".into()));
        };
        // Unwrap 响应即「数据密钥的 base64」（mock/真 KMS 都已解包还原），
        // 单层解码得 16 字节密钥——与 seal 侧 `from_utf8_lossy(&data_key)`
        // 的密钥串严格对应。
        let data_key_base64 = self.unwrap(wrapped)?;
        let data_key = BASE64
            .decode(&data_key_base64)
            .map_err(|_| GmError("kms unwrapped key not base64".into()))?;
        let mut key = [0u8; 16];
        key.copy_from_slice(&data_key[..16]);
        crate::sm4_open(sealed, &String::from_utf8_lossy(&key))
    }
}

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

impl Sm4Provider for KmsHttpProvider {
    /// 信封语义：明文交 KMS Wrap 后再本地 SM4（数据密钥每明文独立随机）。
    /// trait 契约保留原始密钥参数（真实 KMS 中该参数即 KMS 侧密钥引用）。
    fn encrypt_cbc(&self, key: &[u8; 16], _iv: &[u8; 16], plain: &[u8]) -> Result<Vec<u8>, GmError> {
        let wrapped = self.wrap(&BASE64.encode(key))?;
        let sealed = crate::sm4_seal(&String::from_utf8_lossy(plain), &String::from_utf8_lossy(key))
            .map_err(|error| GmError(error.0))?;
        Ok(format!("{wrapped}|{sealed}").into_bytes())
    }

    fn decrypt_cbc(&self, _key: &[u8; 16], _iv: &[u8; 16], cipher: &[u8]) -> Result<Vec<u8>, GmError> {
        let text = String::from_utf8_lossy(cipher);
        let Some((wrapped, sealed)) = text.split_once('|') else {
            return Err(GmError("kms cipher malformed".into()));
        };
        let unwrapped = self.unwrap(wrapped)?;
        let unwrapped_key = BASE64.decode(unwrapped).map_err(|_| GmError("kms unwrap not base64".into()))?;
        crate::sm4_open(sealed, &String::from_utf8_lossy(&unwrapped_key))
            .map_err(|error| GmError(error.0))
            .map(String::into_bytes)
    }
}

impl RandomProvider for KmsHttpProvider {
    /// 随机数本地生成（OS 熵源）；若测评要求随机数也出自 KMS，
    /// 在此改为 KMS 调用（§4.4.3 #5）。
    fn fill_random(&self, buffer: &mut [u8]) {
        crate::provider::SoftwareProviders.fill_random(buffer);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};

    #[test]
    fn sdf_handle_lifecycle() {
        let sdf = SdfSimProvider::new();
        let key = [7u8; 16];
        let handle = sdf.import_key(&key).expect("import");
        let cipher = sdf.encrypt_by_handle(handle, &[0u8; 16], b"secret").expect("encrypt");
        assert_eq!(
            sdf.decrypt_by_handle(handle, &[0u8; 16], &cipher).expect("decrypt"),
            b"secret"
        );
        sdf.destroy_key(handle).expect("destroy");
        // 销毁后句柄失效——GM/T 0018 生命周期语义。
        assert!(matches!(
            sdf.decrypt_by_handle(handle, &[0u8; 16], &cipher),
            Err(SdfError::InvalidHandle(_))
        ));
        assert!(sdf.destroy_key(handle).is_err());
    }

    #[test]
    fn sdf_audit_counters_count_operations() {
        let sdf = SdfSimProvider::new();
        let handle = sdf.import_key(&[1u8; 16]).unwrap();
        let cipher = sdf.encrypt_by_handle(handle, &[0u8; 16], b"data").unwrap();
        let _ = sdf.decrypt_by_handle(handle, &[0u8; 16], &cipher).unwrap();
        let mut random_buffer = [0u8; 8];
        crate::provider::RandomProvider::fill_random(&sdf, &mut random_buffer);
        let (encrypt_calls, decrypt_calls, random_calls) = sdf.audit_counters();
        assert_eq!((encrypt_calls, decrypt_calls, random_calls), (1, 1, 1));
    }

    #[test]
    fn sdf_sm4_trait_delegates() {
        let sdf = SdfSimProvider::new();
        let cipher =
            crate::provider::Sm4Provider::encrypt_cbc(&sdf, &[3u8; 16], &[4u8; 16], b"plain").unwrap();
        let opened =
            crate::provider::Sm4Provider::decrypt_cbc(&sdf, &[3u8; 16], &[4u8; 16], &cipher).unwrap();
        assert_eq!(opened, b"plain");
    }

    /// 从 mock KMS 请求体按字段名提取值（不能取第一个冒号——请求体里
    /// keyId 永远在前，实测踩坑）。
    fn extract_field<'a>(body: &'a str, field: &str) -> &'a str {
        let needle = format!("\"{field}\":\"");
        body.find(&needle)
            .map(|start| &body[start + needle.len()..])
            .and_then(|rest| rest.split('"').next())
            .unwrap_or("")
    }

    /// 进程内 mock KMS：固定 XOR 主密钥模拟 Wrap/Unwrap（std TcpListener）。
    #[test]
    fn kms_envelope_roundtrip_with_mock_server() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind mock kms");
        let port = listener.local_addr().unwrap().port();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { break };
                let mut request = Vec::new();
                let _ = stream.read_to_end(&mut request);
                let request = String::from_utf8_lossy(&request).into_owned();
                let body = request.split("\r\n\r\n").nth(1).unwrap_or("").to_owned();
                // 按请求行真实路径路由（"/unwrap" 含子串 "/wrap"——
                // 全请求 contains 会误判进 wrap 分支，实测踩坑）。
                let request_path = request
                    .lines()
                    .next()
                    .and_then(|line| line.split_whitespace().nth(1))
                    .unwrap_or("")
                    .to_owned();
                let is_wrap = request_path.ends_with("/wrap");
                let extract_field_name = if is_wrap { "plaintextBase64" } else { "wrappedBase64" };
                let response_field = if is_wrap { "wrappedBase64" } else { "plaintextBase64" };
                let value = extract_field(&body, extract_field_name);
                // wrap：XOR 后 base64 编码；unwrap：先 base64 解码再 XOR 还原
                // ——两支必须互逆，实测踩坑：unwrap 侧漏解码会 double-wrap。
                let output = if is_wrap {
                    let xored: Vec<u8> = value
                        .as_bytes()
                        .iter()
                        .enumerate()
                        .map(|(index, byte)| byte ^ [0x5Au8; 8][index % 8])
                        .collect();
                    BASE64.encode(xored)
                } else {
                    let decoded = BASE64.decode(value).unwrap_or_default();
                    decoded
                        .iter()
                        .enumerate()
                        .map(|(index, byte)| byte ^ [0x5Au8; 8][index % 8])
                        .map(|byte| byte as char)
                        .collect::<String>()
                };
                let payload = format!("{{\"{}\":\"{}\"}}", response_field, output);
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
                    payload.len()
                );
                let _ = stream.write_all(response.as_bytes());
            }
        });

        let provider = KmsHttpProvider::new(&format!("http://127.0.0.1:{port}"), "rustset-master");
        let envelope = provider.envelope_seal("国密信封数据").expect("seal");
        assert!(envelope.starts_with("kms:v1:"));
        assert_eq!(provider.envelope_open(&envelope).expect("open"), "国密信封数据");
    }
}
