//! Per-connection state machine: handshake -> (status | login).

use std::sync::Arc;

use leather_protocol::{PacketWriter, ProtocolError, Result, State, read_frame, write_frame};
use tokio::net::TcpStream;

use crate::config::ServerConfig;
use crate::login;
use crate::status::build_status_json;

// Packet ids (uncompressed, current protocol).
const PKT_HANDSHAKE: i32 = 0x00;
const PKT_STATUS_REQUEST: i32 = 0x00;
const PKT_PING_REQUEST: i32 = 0x01;
const PKT_STATUS_RESPONSE: i32 = 0x00;
const PKT_PONG_RESPONSE: i32 = 0x01;

/// Drives a single client connection until it closes.
pub async fn handle(mut stream: TcpStream, config: Arc<ServerConfig>) -> Result<()> {
    // --- Handshake -----------------------------------------------------------
    let mut handshake = read_frame(&mut stream).await?;
    let packet_id = handshake.read_varint()?;
    if packet_id != PKT_HANDSHAKE {
        return Err(ProtocolError::Invalid(format!(
            "expected handshake (0x00), got {packet_id:#x}"
        )));
    }

    let client_protocol = handshake.read_varint()?;
    let _server_address = handshake.read_string()?;
    let _server_port = handshake.read_u16()?;
    let next_state = handshake.read_varint()?;

    match State::from_next_state(next_state)? {
        State::Status => handle_status(&mut stream, &config, client_protocol).await,
        State::Login => login::handle(&mut stream, &config).await,
        State::Handshake => Ok(()), // unreachable: next_state is never 0
    }
}

/// Server-list ping: answer Status Request with JSON, then echo Ping/Pong.
async fn handle_status(
    stream: &mut TcpStream,
    config: &ServerConfig,
    client_protocol: i32,
) -> Result<()> {
    loop {
        let mut frame = match read_frame(stream).await {
            Ok(f) => f,
            // Client closes the socket after the pong; that's normal.
            Err(ProtocolError::Io(_)) => return Ok(()),
            Err(err) => return Err(err),
        };

        let packet_id = frame.read_varint()?;
        match packet_id {
            PKT_STATUS_REQUEST => {
                let json = build_status_json(config, client_protocol);
                let mut writer = PacketWriter::new(PKT_STATUS_RESPONSE);
                writer.write_string(&json);
                write_frame(stream, &writer.into_body()).await?;
            }
            PKT_PING_REQUEST => {
                // Echo the client's timestamp back verbatim so it can measure RTT.
                let payload = frame.read_i64()?;
                let mut writer = PacketWriter::new(PKT_PONG_RESPONSE);
                writer.write_i64(payload);
                write_frame(stream, &writer.into_body()).await?;
                return Ok(());
            }
            other => {
                return Err(ProtocolError::Invalid(format!(
                    "unexpected status packet {other:#x}"
                )));
            }
        }
    }
}
