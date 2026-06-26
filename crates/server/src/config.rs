//! Minimal server configuration.
//!
//! For now these are sensible defaults. A real TOML config file (the chosen
//! format for this project) is a later milestone; this struct is what it will
//! deserialize into.

use std::path::PathBuf;

pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub motd: String,
    pub max_players: i32,
    /// Version name shown in the client's server list.
    pub version_name: String,
    /// Directory of generated registry NBT files (produced by `leather-datagen`).
    pub registries_dir: PathBuf,
    /// File the world's block edits are saved to / loaded from.
    pub world_file: PathBuf,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 25565,
            motd: "LeatherMC - Rust server (alpha)".to_string(),
            max_players: 20,
            version_name: "LeatherMC 26.2".to_string(),
            registries_dir: PathBuf::from("registries"),
            world_file: PathBuf::from("world.bin"),
        }
    }
}

impl ServerConfig {
    pub fn bind_address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}
