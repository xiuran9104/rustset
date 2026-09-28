use aws_sdk_s3::{
    Client,
    config::{Credentials, Region},
    primitives::ByteStream,
    presigning::PresigningConfig,
};
use std::{env, time::Duration};

#[derive(Clone)]
pub struct ObjectStorage {
    client: Client,
    bucket: String,
}

pub(crate) enum ObjectReadError {
    NotFound,
    Unavailable(String),
}

impl ObjectStorage {
    pub fn from_env() -> Result<Self, String> {
        let endpoint = env::var("RUSTFS_ENDPOINT")
            .unwrap_or_else(|_| "http://127.0.0.1:9000".to_owned());
        let local_endpoint = endpoint.starts_with("http://127.0.0.1:")
            || endpoint.starts_with("http://localhost:")
            || endpoint.starts_with("https://127.0.0.1:")
            || endpoint.starts_with("https://localhost:");
        let access_key = env::var("RUSTFS_ACCESS_KEY").ok();
        let secret_key = env::var("RUSTFS_SECRET_KEY").ok();
        if !local_endpoint && (access_key.is_none() || secret_key.is_none()) {
            return Err("RUSTFS_ACCESS_KEY and RUSTFS_SECRET_KEY are required for non-local endpoints".to_owned());
        }
        let using_local_defaults = access_key.is_none() || secret_key.is_none();
        let access_key = access_key.unwrap_or_else(|| "rustset".to_owned());
        let secret_key = secret_key.unwrap_or_else(|| "rustset_password".to_owned());
        if using_local_defaults {
            tracing::warn!("using development RustFS credentials; set RUSTFS_ACCESS_KEY and RUSTFS_SECRET_KEY outside local development");
        }
        let region = env::var("RUSTFS_REGION").unwrap_or_else(|_| "us-east-1".to_owned());
        let bucket = env::var("RUSTFS_BUCKET").unwrap_or_else(|_| "rustset".to_owned());
        let credentials = Credentials::new(access_key, secret_key, None, None, "rustfs");
        let config = aws_sdk_s3::Config::builder()
            .region(Region::new(region))
            .credentials_provider(credentials)
            .endpoint_url(endpoint)
            .force_path_style(true)
            .build();

        Ok(Self {
            client: Client::from_conf(config),
            bucket,
        })
    }

    pub async fn ensure_bucket(&self) -> Result<(), String> {
        let mut last_error = String::new();
        for attempt in 0..30 {
            if self
                .client
                .head_bucket()
                .bucket(&self.bucket)
                .send()
                .await
                .is_ok()
            {
                return Ok(());
            }

            match self
                .client
                .create_bucket()
                .bucket(&self.bucket)
                .send()
                .await
            {
                Ok(_) => return Ok(()),
                Err(error) => last_error = format!("failed to create RustFS bucket: {error}"),
            }
            if attempt < 29 {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            }
        }
        Err(last_error)
    }

    pub(crate) async fn put(
        &self,
        key: &str,
        content_type: &str,
        bytes: Vec<u8>,
    ) -> Result<(), String> {
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .content_type(content_type)
            .body(ByteStream::from(bytes))
            .send()
            .await
            .map(|_| ())
            .map_err(|error| format!("failed to write object to RustFS: {error}"))
    }

    pub(crate) async fn get(&self, key: &str) -> Result<Vec<u8>, ObjectReadError> {
        let result = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map_err(|error| {
                if error
                    .as_service_error()
                    .is_some_and(|service_error| service_error.is_no_such_key())
                {
                    ObjectReadError::NotFound
                } else {
                    ObjectReadError::Unavailable(format!(
                        "failed to read object from RustFS: {error}"
                    ))
                }
            })?;
        result
            .body
            .collect()
            .await
            .map(|body| body.into_bytes().to_vec())
            .map_err(|error| {
                ObjectReadError::Unavailable(format!("failed to read RustFS response body: {error}"))
            })
    }

    pub(crate) async fn delete(&self, key: &str) -> Result<(), String> {
        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(key)
            .send()
            .await
            .map(|_| ())
            .map_err(|error| format!("failed to delete object from RustFS: {error}"))
    }

    pub(crate) async fn presign_put(&self, key: &str, size: i64) -> Result<String, String> {
        let config = PresigningConfig::expires_in(Duration::from_secs(900))
            .map_err(|error| format!("failed to configure upload URL: {error}"))?;
        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(key)
            .content_length(size)
            .presigned(config)
            .await
            .map(|request| request.uri().to_string())
            .map_err(|error| format!("failed to create RustFS upload URL: {error}"))
    }
}
