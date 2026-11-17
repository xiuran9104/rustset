use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD as BASE64URL};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{CurrentUser, SecurityConfig, SecurityError};

/// JWS-style header carried verbatim in the token's first segment. The only
/// supported algorithm is HMAC-SM3 (国密); RS/HS256 tokens are not accepted.
const TOKEN_HEADER: &str = "{\"alg\":\"HMAC-SM3\",\"typ\":\"JWT\"}";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub iss: String,
    pub aud: String,
    pub iat: i64,
    pub exp: i64,
    pub jti: String,
    pub user: CurrentUser,
}

#[derive(Clone)]
pub struct TokenService {
    signing_secret: Vec<u8>,
    config: SecurityConfig,
}

impl TokenService {
    pub fn new(config: SecurityConfig) -> Self {
        Self {
            signing_secret: config.jwt_secret.as_bytes().to_vec(),
            config,
        }
    }

    /// Issue an HMAC-SM3 signed compact token:
    /// `base64url(header).base64url(claims).base64url(hmac_sm3(header.claims))`.
    pub fn issue_access_token(&self, user: CurrentUser) -> Result<String, SecurityError> {
        let issued_at = Utc::now().timestamp();
        let claims = Claims {
            sub: user.user_id.clone(),
            iss: self.config.issuer.clone(),
            aud: self.config.audience.clone(),
            iat: issued_at,
            exp: issued_at + self.config.access_token_ttl.as_secs() as i64,
            jti: Uuid::new_v4().to_string(),
            user,
        };
        let payload =
            serde_json::to_string(&claims).map_err(|_| SecurityError::InvalidCredentials)?;
        let signing_input = format!(
            "{}.{}",
            BASE64URL.encode(TOKEN_HEADER.as_bytes()),
            BASE64URL.encode(payload.as_bytes())
        );
        let signature =
            rustset_framework_gm::hmac_sm3(&self.signing_secret, signing_input.as_bytes());
        Ok(format!("{signing_input}.{}", BASE64URL.encode(signature)))
    }

    pub fn access_token_ttl_seconds(&self) -> u64 {
        self.config.access_token_ttl.as_secs()
    }

    pub fn verify_access_token(&self, token: &str) -> Result<Claims, SecurityError> {
        let invalid = || SecurityError::InvalidCredentials;
        let mut segments = token.split('.');
        let (Some(header), Some(payload), Some(signature)) =
            (segments.next(), segments.next(), segments.next())
        else {
            return Err(invalid());
        };
        if segments.next().is_some() {
            return Err(invalid());
        }
        let header_bytes = BASE64URL.decode(header).map_err(|_| invalid())?;
        if header_bytes != TOKEN_HEADER.as_bytes() {
            return Err(invalid());
        }
        let signing_input = format!("{header}.{payload}");
        let expected =
            rustset_framework_gm::hmac_sm3(&self.signing_secret, signing_input.as_bytes());
        let provided = BASE64URL.decode(signature).map_err(|_| invalid())?;
        if expected.len() != provided.len()
            || expected
                .iter()
                .zip(provided.iter())
                .fold(0u8, |difference, (left, right)| difference | (left ^ right))
                != 0
        {
            return Err(invalid());
        }
        let payload_bytes = BASE64URL.decode(payload).map_err(|_| invalid())?;
        let claims: Claims = serde_json::from_slice(&payload_bytes).map_err(|_| invalid())?;
        if claims.sub.is_empty()
            || claims.iss != self.config.issuer
            || claims.aud != self.config.audience
            || claims.exp <= Utc::now().timestamp()
        {
            return Err(invalid());
        }
        Ok(claims)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::{CurrentUser, DataScope, Permission, PermissionSet, SecurityConfig};

    use super::TokenService;

    fn user() -> CurrentUser {
        CurrentUser {
            user_id: "user-1".into(),
            username: "admin".into(),
            tenant_id: Some("tenant-1".into()),
            role_codes: vec!["admin".into()],
            permissions: PermissionSet::new([Permission::new("system:*:read").unwrap()]),
            data_scope: DataScope::All,
        }
    }

    fn service() -> TokenService {
        let config = SecurityConfig::new(
            "a-secret-with-at-least-thirty-two-bytes",
            "issuer",
            "audience",
            Duration::from_secs(60),
        )
        .unwrap();
        TokenService::new(config)
    }

    #[test]
    fn issues_and_verifies_an_access_token() {
        let service = service();
        let token = service.issue_access_token(user()).unwrap();
        let claims = service.verify_access_token(&token).unwrap();

        assert_eq!(claims.sub, "user-1");
        assert_eq!(claims.user.username, "admin");
    }

    #[test]
    fn rejects_tampered_tokens() {
        let service = service();
        let token = service.issue_access_token(user()).unwrap();
        let mut segments: Vec<&str> = token.split('.').collect();
        segments[1] = "eyJzdWIiOiJvdGhlciJ9";
        let tampered = segments.join(".");
        assert!(service.verify_access_token(&tampered).is_err());

        let mut wrong_key = service.clone();
        wrong_key.signing_secret = b"another-secret-with-at-least-thirty-tw".to_vec();
        assert!(wrong_key.verify_access_token(&token).is_err());
        assert!(service.verify_access_token("not-a-token").is_err());
        assert!(
            service
                .verify_access_token("eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.e30.Zm9v")
                .is_err()
        );
    }
}
