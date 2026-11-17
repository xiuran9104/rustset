use std::{env, fmt, time::Duration};

const MIN_SECRET_BYTES: usize = 32;

#[derive(Debug, Clone)]
pub struct SecurityConfig {
    pub jwt_secret: String,
    pub issuer: String,
    pub audience: String,
    pub access_token_ttl: Duration,
}

impl SecurityConfig {
    pub fn from_env() -> Result<Self, SecurityConfigError> {
        let jwt_secret = env::var("JWT_SECRET").map_err(|_| SecurityConfigError::MissingSecret)?;
        let issuer = env::var("JWT_ISSUER").unwrap_or_else(|_| "rustset".into());
        let audience = env::var("JWT_AUDIENCE").unwrap_or_else(|_| "rustset-api".into());
        let ttl_seconds = env::var("JWT_ACCESS_TOKEN_TTL_SECONDS")
            .ok()
            .map(|value| value.parse::<u64>())
            .transpose()
            .map_err(|_| SecurityConfigError::InvalidTtl)?
            .unwrap_or(900);

        Self::new(
            jwt_secret,
            issuer,
            audience,
            Duration::from_secs(ttl_seconds),
        )
    }

    pub fn new(
        jwt_secret: impl Into<String>,
        issuer: impl Into<String>,
        audience: impl Into<String>,
        access_token_ttl: Duration,
    ) -> Result<Self, SecurityConfigError> {
        let jwt_secret = jwt_secret.into();
        if jwt_secret.len() < MIN_SECRET_BYTES {
            return Err(SecurityConfigError::WeakSecret);
        }
        if access_token_ttl.is_zero() {
            return Err(SecurityConfigError::InvalidTtl);
        }

        Ok(Self {
            jwt_secret,
            issuer: issuer.into(),
            audience: audience.into(),
            access_token_ttl,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityConfigError {
    MissingSecret,
    WeakSecret,
    InvalidTtl,
}

impl fmt::Display for SecurityConfigError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::MissingSecret => "JWT_SECRET is required",
            Self::WeakSecret => "JWT_SECRET must contain at least 32 bytes",
            Self::InvalidTtl => "JWT_ACCESS_TOKEN_TTL_SECONDS must be a positive integer",
        })
    }
}

impl std::error::Error for SecurityConfigError {}
