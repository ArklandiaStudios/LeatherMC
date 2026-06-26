//! Play state: send Join Game + spawn position so the client enters the world
//! (an empty void for now), then keep the connection alive.

use std::time::Duration;

use leather_protocol::{PacketWriter, Result, read_frame, write_frame};
use tokio::net::TcpStream;

use crate::registries::Registries;

// Clientbound play packet ids (protocol 776).
const P_LOGIN: i32 = 49; // "Join Game"
const P_PLAYER_POSITION: i32 = 72; // "Synchronize Player Position"
const P_GAME_EVENT: i32 = 38;
const P_KEEP_ALIVE: i32 = 44;

/// Game event id: "start waiting for level chunks".
const EVENT_START_WAITING_FOR_CHUNKS: u8 = 13;

pub async fn handle(stream: &mut TcpStream, registries: &Registries) -> Result<()> {
    send_join_game(stream, registries).await?;
    send_spawn_position(stream).await?;

    // Tell the client to stop the loading screen and render the (empty) world.
    let mut event = PacketWriter::new(P_GAME_EVENT);
    event
        .write_u8(EVENT_START_WAITING_FOR_CHUNKS)
        .write_f32(0.0);
    write_frame(stream, &event.into_body()).await?;

    keep_alive_loop(stream).await
}

async fn send_join_game(stream: &mut TcpStream, registries: &Registries) -> Result<()> {
    // The dimension type is referenced by its index in the dimension_type
    // registry as we sent it.
    let dimension_type_index = registries
        .index_of("minecraft:dimension_type", "minecraft:overworld")
        .unwrap_or(0);

    let mut w = PacketWriter::new(P_LOGIN);
    w.write_i32(1); // entity id
    w.write_bool(false); // hardcore
    w.write_varint(1); // dimension names: count
    w.write_string("minecraft:overworld"); // ... the one dimension
    w.write_varint(20); // max players
    w.write_varint(10); // view distance
    w.write_varint(10); // simulated distance
    w.write_bool(false); // reduced debug info
    w.write_bool(true); // enable respawn screen
    w.write_bool(false); // limited crafting

    // Common spawn info.
    w.write_varint(dimension_type_index);
    w.write_string("minecraft:overworld"); // dimension (world) name
    w.write_i64(0); // hashed seed
    w.write_u8(1); // game mode: creative (so we float in the void)
    w.write_i8(-1); // previous game mode: none
    w.write_bool(false); // is debug world
    w.write_bool(false); // is flat world
    w.write_bool(false); // has death location
    w.write_varint(0); // portal cooldown
    w.write_varint(63); // sea level

    w.write_bool(false); // online mode (added in 26.2)
    w.write_bool(false); // enforce secure chat

    write_frame(stream, &w.into_body()).await
}

async fn send_spawn_position(stream: &mut TcpStream) -> Result<()> {
    let mut w = PacketWriter::new(P_PLAYER_POSITION);
    w.write_varint(1); // teleport id
    w.write_f64(0.0).write_f64(100.0).write_f64(0.0); // position
    w.write_f64(0.0).write_f64(0.0).write_f64(0.0); // velocity
    w.write_f32(0.0).write_f32(0.0); // yaw, pitch
    w.write_i32(0); // relative-flags bitfield (all absolute)
    write_frame(stream, &w.into_body()).await
}

/// Keeps the connection alive: pings every 10s and drains whatever the client
/// sends (teleport confirms, keep-alive replies, settings) without acting on it
/// yet.
async fn keep_alive_loop(stream: &mut TcpStream) -> Result<()> {
    let (mut reader, mut writer) = stream.split();
    let mut interval = tokio::time::interval(Duration::from_secs(10));
    interval.tick().await; // consume the immediate first tick
    let mut keep_alive_id: i64 = 1;

    loop {
        tokio::select! {
            incoming = read_frame(&mut reader) => {
                match incoming {
                    Ok(_) => {} // ignore client packets for now
                    Err(_) => return Ok(()), // client disconnected
                }
            }
            _ = interval.tick() => {
                let mut w = PacketWriter::new(P_KEEP_ALIVE);
                w.write_i64(keep_alive_id);
                keep_alive_id += 1;
                if write_frame(&mut writer, &w.into_body()).await.is_err() {
                    return Ok(());
                }
            }
        }
    }
}
