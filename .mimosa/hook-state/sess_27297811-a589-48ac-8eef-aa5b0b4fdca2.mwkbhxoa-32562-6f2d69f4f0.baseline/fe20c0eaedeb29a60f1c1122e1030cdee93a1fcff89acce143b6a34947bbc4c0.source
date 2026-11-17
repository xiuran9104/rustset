use tracing_subscriber::{EnvFilter, fmt};

pub fn init_tracing(service_name: &'static str) {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    let _ = fmt()
        .with_env_filter(filter)
        .with_target(false)
        .with_thread_ids(true)
        .try_init();

    tracing::info!(service = service_name, "tracing initialized");
}
