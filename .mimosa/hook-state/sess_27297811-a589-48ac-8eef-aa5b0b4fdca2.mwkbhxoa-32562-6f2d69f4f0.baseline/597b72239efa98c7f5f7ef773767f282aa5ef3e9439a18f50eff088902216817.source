use axum::Router;
use tokio::net::TcpListener;
use tracing::info;

use crate::ServiceConfig;

pub async fn serve(config: ServiceConfig, router: Router) -> anyhow::Result<()> {
    let addr = config.addr()?;
    let listener = TcpListener::bind(addr).await?;

    info!(service = %config.name, address = %addr, "service listening");

    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
