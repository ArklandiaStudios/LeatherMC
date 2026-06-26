//! Login state (offline mode).
//!
//! Flow we implement:
//!   1. read **Login Start** (name + UUID),
//!   2. send **Login Success** (offline: echo the UUID, no properties),
//!   3. read **Login Acknowledged** -> the connection enters Configuration,
//!   4. hand off to the Configuration then Play states.
//!
//! Offline mode note: a real offline server derives the UUID from
//! `md5("OfflinePlayer:" + name)`. We echo the client-sent UUID for now; proper
//! offline UUID derivation is a later refinement.

use leather_protocol::{PacketWriter, ProtocolError, Result, read_frame, write_frame};
use tokio::net::TcpStream;

use crate::config::ServerConfig;
use crate::registries::Registries;
use crate::world::World;
use crate::{configuration, play};

const PKT_LOGIN_START: i32 = 0x00;
const PKT_LOGIN_ACKNOWLEDGED: i32 = 0x03;
const PKT_LOGIN_SUCCESS: i32 = 0x02;

pub async fn handle(
    stream: &mut TcpStream,
    _config: &ServerConfig,
    registries: &Registries,
    world: &World,
) -> Result<()> {
    // 1. Login Start: name + player UUID.
    let mut start = read_frame(stream).await?;
    let id = start.read_varint()?;
    if id != PKT_LOGIN_START {
        return Err(ProtocolError::Invalid(format!(
            "expected Login Start (0x00), got {id:#x}"
        )));
    }
    let name = start.read_string()?;
    let uuid = start.read_uuid()?;
    tracing::info!("login (offline): {name} ({uuid:032x})");

    // 2. Login Success ("login_finished"). Offline mode: echo the UUID, no
    //    properties. Minecraft 26.2 (protocol 776) added a trailing `session_id`
    //    UUID after the properties array; we reuse the player UUID for it for now
    //    (it only matters for chat session signing, which we don't do yet).
    let mut success = PacketWriter::new(PKT_LOGIN_SUCCESS);
    success
        .write_uuid(uuid)
        .write_string(&name)
        .write_varint(0)
        .write_uuid(uuid);
    write_frame(stream, &success.into_body()).await?;

    // 3. Login Acknowledged -> we are now in the Configuration state.
    let mut ack = read_frame(stream).await?;
    let ack_id = ack.read_varint()?;
    if ack_id != PKT_LOGIN_ACKNOWLEDGED {
        return Err(ProtocolError::Invalid(format!(
            "expected Login Acknowledged (0x03), got {ack_id:#x}"
        )));
    }

    // 4. Configuration, then Play.
    configuration::handle(stream, registries).await?;
    play::handle(stream, registries, &name, world).await
}
