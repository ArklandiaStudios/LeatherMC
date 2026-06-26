//! Login state (offline mode).
//!
//! Flow we implement:
//!   1. read **Login Start** (name + UUID),
//!   2. send **Login Success** (offline: echo the UUID, no properties),
//!   3. read **Login Acknowledged** -> the connection enters Configuration,
//!   4. send a Configuration **Disconnect** with a friendly message.
//!
//! That last disconnect is, for now, the visible proof that login negotiation
//! succeeded end to end. Joining a world (the Configuration + Play states) is
//! the next bricks.
//!
//! Offline mode note: a real offline server derives the UUID from
//! `md5("OfflinePlayer:" + name)`. We echo the client-sent UUID for now; proper
//! offline UUID derivation is a later refinement.

use leather_protocol::{PacketWriter, ProtocolError, Result, read_frame, write_frame};
use tokio::net::TcpStream;

use crate::config::ServerConfig;

const PKT_LOGIN_START: i32 = 0x00;
const PKT_LOGIN_ACKNOWLEDGED: i32 = 0x03;
const PKT_LOGIN_SUCCESS: i32 = 0x02;
const PKT_CONFIG_DISCONNECT: i32 = 0x02;

/// NBT tag id for a String (TAG_String).
const TAG_STRING: u8 = 0x08;

pub async fn handle(stream: &mut TcpStream, _config: &ServerConfig) -> Result<()> {
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

    // 2. Login Success: offline mode echoes the UUID and sends no properties.
    let mut success = PacketWriter::new(PKT_LOGIN_SUCCESS);
    success.write_uuid(uuid).write_string(&name).write_varint(0);
    write_frame(stream, &success.into_body()).await?;

    // 3. Login Acknowledged -> we are now in the Configuration state.
    let mut ack = read_frame(stream).await?;
    let ack_id = ack.read_varint()?;
    if ack_id != PKT_LOGIN_ACKNOWLEDGED {
        return Err(ProtocolError::Invalid(format!(
            "expected Login Acknowledged (0x03), got {ack_id:#x}"
        )));
    }

    // 4. Configuration Disconnect with a friendly message.
    let reason = "LeatherMC: login works! Joining a world is coming next.";
    let mut disconnect = PacketWriter::new(PKT_CONFIG_DISCONNECT);
    disconnect.write_bytes(&nbt_root_string(reason));
    write_frame(stream, &disconnect.into_body()).await?;
    Ok(())
}

/// Encodes `text` as a network-NBT root String tag.
///
/// Since 1.20.2 the root NBT tag carries no name, so a String component is just
/// `[TAG_String][u16 length][utf-8 bytes]`. The vanilla client accepts a bare
/// string as a Text Component (equivalent to `{"text": text}`).
fn nbt_root_string(text: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(3 + text.len());
    out.push(TAG_STRING);
    out.extend_from_slice(&(text.len() as u16).to_be_bytes());
    out.extend_from_slice(text.as_bytes());
    out
}
