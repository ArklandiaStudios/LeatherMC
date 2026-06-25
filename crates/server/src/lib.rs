//! LeatherMC server library.
//!
//! The networking and connection logic lives here so it can be exercised by
//! integration tests (and, later, embedded elsewhere). The `leathermc` binary
//! is a thin wrapper around [`run`].

#![deny(unsafe_code)]

pub mod config;
pub mod connection;
pub mod status;

use std::sync::Arc;

use tokio::net::TcpListener;

use config::ServerConfig;

/// Binds the listener and serves connections forever.
pub async fn run(config: ServerConfig) -> anyhow::Result<()> {
    let config = Arc::new(config);
    let addr = config.bind_address();

    let listener = TcpListener::bind(&addr).await?;
    tracing::info!("LeatherMC (Rust) listening on {addr}");
    tracing::info!("MOTD: {}", config.motd);

    serve(listener, config).await
}

/// Accept loop, factored out so tests can supply their own listener.
pub async fn serve(listener: TcpListener, config: Arc<ServerConfig>) -> anyhow::Result<()> {
    loop {
        let (socket, peer) = match listener.accept().await {
            Ok(pair) => pair,
            Err(err) => {
                tracing::warn!("accept failed: {err}");
                continue;
            }
        };

        let config = Arc::clone(&config);
        tokio::spawn(async move {
            socket.set_nodelay(true).ok();
            if let Err(err) = connection::handle(socket, config).await {
                tracing::debug!("connection from {peer} closed: {err}");
            }
        });
    }
}
