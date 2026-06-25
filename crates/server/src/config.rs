//! Minimal server configuration.
//!
//! For now these are sensible defaults. A real TOML config file (the chosen
//! format for this project) is a later milestone; this struct is what it will
//! deserialize into.

pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub motd: String,
    pub max_players: i32,
    /// Version name shown in the client's server list.
    pub version_name: String,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: "0.0.0.0".to_string(),
            port: 25565,
            motd: "LeatherMC - Rust server (alpha)".to_string(),
            max_players: 20,
            version_name: "LeatherMC 26.2".to_string(),
        }
    }
}

impl ServerConfig {
    pub fn bind_address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}
