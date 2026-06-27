//! LeatherMC server library.
//!
//! The networking and connection logic lives here so it can be exercised by
//! integration tests (and, later, embedded elsewhere). The `leathermc` binary
//! is a thin wrapper around [`run`].

#![deny(unsafe_code)]

pub mod chunk;
pub mod config;
pub mod configuration;
pub mod connection;
pub mod login;
pub mod mob;
pub mod projectile;
pub mod play;
pub mod registries;
pub mod status;
pub mod world;

use std::sync::Arc;
use std::time::Duration;

use tokio::net::TcpListener;

use config::ServerConfig;
use registries::Registries;
use world::World;

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
    // Registries are loaded once and shared across connections. A missing
    // directory is non-fatal here (ping/login still work); the world join will
    // be incomplete until `leather-datagen` has populated it.
    let registries = Arc::new(Registries::load(&config.registries_dir).unwrap_or_default());
    if registries.list.is_empty() {
        tracing::warn!(
            "no registries loaded from {} — run leather-datagen to enable world join",
            config.registries_dir.display()
        );
    } else {
        tracing::info!(
            "loaded {} registries ({} entries)",
            registries.list.len(),
            registries.entry_count()
        );
    }

    // Shared world; loaded from disk and saved periodically.
    let world = Arc::new(World::load(&config.world_file));
    tracing::info!("world: {} edited blocks", world.block_count());
    {
        let world = Arc::clone(&world);
        let path = config.world_file.clone();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(Duration::from_secs(30));
            interval.tick().await; // skip the immediate tick
            loop {
                interval.tick().await;
                if let Err(err) = world.save(&path) {
                    tracing::warn!("world save failed: {err}");
                }
            }
        });
    }

    loop {
        let (socket, peer) = match listener.accept().await {
            Ok(pair) => pair,
            Err(err) => {
                tracing::warn!("accept failed: {err}");
                continue;
            }
        };

        let config = Arc::clone(&config);
        let registries = Arc::clone(&registries);
        let world = Arc::clone(&world);
        tokio::spawn(async move {
            socket.set_nodelay(true).ok();
            if let Err(err) = connection::handle(socket, config, registries, world).await {
                tracing::debug!("connection from {peer} closed: {err}");
            }
        });
    }
}
