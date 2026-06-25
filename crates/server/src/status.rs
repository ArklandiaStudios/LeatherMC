//! Builds the JSON payload for the Status Response packet (the data shown in
//! the multiplayer server list: version, player count, MOTD).

use serde::Serialize;

use crate::config::ServerConfig;

#[derive(Serialize)]
struct StatusResponse<'a> {
    version: Version<'a>,
    players: Players,
    description: Description<'a>,
}

#[derive(Serialize)]
struct Version<'a> {
    name: &'a str,
    protocol: i32,
}

#[derive(Serialize)]
struct Players {
    max: i32,
    online: i32,
    sample: Vec<()>,
}

#[derive(Serialize)]
struct Description<'a> {
    text: &'a str,
}

/// Serializes the status JSON.
///
/// We echo back the protocol number the client sent in its handshake, so the
/// server always shows as "compatible" regardless of the exact protocol number
/// of the target Minecraft version. Once login is implemented we'll pin a real
/// protocol number and reject mismatches.
pub fn build_status_json(config: &ServerConfig, client_protocol: i32) -> String {
    let response = StatusResponse {
        version: Version {
            name: &config.version_name,
            protocol: client_protocol,
        },
        players: Players {
            max: config.max_players,
            online: 0,
            sample: Vec::new(),
        },
        description: Description { text: &config.motd },
    };

    serde_json::to_string(&response).expect("status JSON is always serializable")
}
