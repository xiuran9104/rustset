use std::{env, net::SocketAddr};

#[derive(Debug, Clone)]
pub struct ServiceConfig {
    pub name: String,
    pub host: String,
    pub port: u16,
}

impl ServiceConfig {
    pub fn from_env(service_name: impl Into<String>, default_port: u16) -> Self {
        let name = service_name.into();
        let env_prefix = name.to_uppercase().replace('-', "_");
        let host = env::var(format!("{env_prefix}_HOST")).unwrap_or_else(|_| "0.0.0.0".to_string());
        let port = env::var(format!("{env_prefix}_PORT"))
            .ok()
            .and_then(|value| value.parse::<u16>().ok())
            .unwrap_or(default_port);

        Self { name, host, port }
    }

    pub fn addr(&self) -> anyhow::Result<SocketAddr> {
        format!("{}:{}", self.host, self.port)
            .parse()
            .map_err(|error| anyhow::anyhow!("invalid bind address for {}: {error}", self.name))
    }
}
