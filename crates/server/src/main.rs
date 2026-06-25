//! LeatherMC server entry point.
//!
//! Milestone 1: accept TCP connections and answer the Server-List Ping (status +
//! ping/pong) so a vanilla client can see the server in its multiplayer list.
//! Login and gameplay are not implemented yet — login attempts get a friendly
//! disconnect message.

use leather_server::config::ServerConfig;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    leather_server::run(ServerConfig::default()).await
}
