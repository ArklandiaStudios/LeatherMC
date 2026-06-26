//! Play state: put the player in a flat world and stream chunks around them as
//! they move, so the world feels endless (no visible edge).

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use leather_protocol::{Nbt, PacketWriter, Result, read_frame, write_frame, write_network_nbt};
use tokio::io::AsyncWrite;
use tokio::net::TcpStream;

use crate::chunk::flat_chunk;
use crate::registries::Registries;
use crate::world::World;

// Clientbound play packet ids (protocol 776).
const P_LOGIN: i32 = 49; // "Join Game"
const P_PLAYER_POSITION: i32 = 72; // "Synchronize Player Position"
const P_GAME_EVENT: i32 = 38;
const P_KEEP_ALIVE: i32 = 44;
const P_SET_CENTER_CHUNK: i32 = 94;
const P_CHUNK_BATCH_START: i32 = 12;
const P_CHUNK_BATCH_FINISHED: i32 = 11;
const P_SYSTEM_CHAT: i32 = 121;
const P_BLOCK_UPDATE: i32 = 8;
const P_BLOCK_CHANGED_ACK: i32 = 4;

// Serverbound play packet ids we care about.
const S_MOVE_POS: i32 = 30;
const S_MOVE_POS_ROT: i32 = 31;
const S_CHAT: i32 = 9;
const S_PLAYER_ACTION: i32 = 41; // digging
const S_USE_ITEM_ON: i32 = 66; // placing
const S_SET_CARRIED_ITEM: i32 = 53; // selected hotbar slot
const S_SET_CREATIVE_SLOT: i32 = 56; // creative inventory edit

const STATE_AIR: i32 = 0;

/// Player-inventory container slot of hotbar slot 0 (hotbar is slots 36..=44).
const HOTBAR_OFFSET: i32 = 36;

/// Game event id: "start waiting for level chunks".
const EVENT_START_WAITING_FOR_CHUNKS: u8 = 13;

/// View distance we announce to the client.
const VIEW_DISTANCE: i32 = 8;

/// Chunk radius we actually send. The client only renders a chunk whose
/// neighbours are loaded, so we send one extra ring beyond the view distance —
/// otherwise the outermost visible chunks have blocks but no rendered faces.
const SEND_RADIUS: i32 = VIEW_DISTANCE + 1;

pub async fn handle(
    stream: &mut TcpStream,
    registries: &Registries,
    name: &str,
    world: &World,
) -> Result<()> {
    send_join_game(stream, registries).await?;
    send_spawn_position(stream).await?;

    // Tell the client to wait for chunks (shows the loading progress).
    let mut event = PacketWriter::new(P_GAME_EVENT);
    event
        .write_u8(EVENT_START_WAITING_FOR_CHUNKS)
        .write_f32(0.0);
    write_frame(stream, &event.into_body()).await?;

    let biome = registries
        .index_of("minecraft:worldgen/biome", "minecraft:plains")
        .unwrap_or(0);

    let (mut reader, mut writer) = stream.split();
    let mut loaded: HashSet<(i32, i32)> = HashSet::new();
    let (mut center_x, mut center_z) = (0, 0);
    load_around(&mut writer, center_x, center_z, &mut loaded, biome, world).await?;

    // Track the creative hotbar so we can place the block the player holds.
    let mut inventory: HashMap<i32, i32> = HashMap::new(); // container slot -> item id
    let mut selected: i32 = 0; // hotbar index 0..=8

    let mut interval = tokio::time::interval(Duration::from_secs(10));
    interval.tick().await; // consume the immediate first tick
    let mut keep_alive_id: i64 = 1;

    loop {
        tokio::select! {
            incoming = read_frame(&mut reader) => {
                let mut frame = match incoming {
                    Ok(f) => f,
                    Err(_) => return Ok(()), // client disconnected
                };
                let Ok(id) = frame.read_varint() else { continue };
                match id {
                    // Stream new chunks when the player crosses into a new chunk.
                    S_MOVE_POS | S_MOVE_POS_ROT => {
                        if let (Ok(x), Ok(_y), Ok(z)) =
                            (frame.read_f64(), frame.read_f64(), frame.read_f64())
                        {
                            let (cx, cz) = chunk_of(x, z);
                            if cx != center_x || cz != center_z {
                                center_x = cx;
                                center_z = cz;
                                load_around(&mut writer, cx, cz, &mut loaded, biome, world).await?;
                            }
                        }
                    }
                    // Echo chat back to the player as a system message.
                    S_CHAT => {
                        if let Ok(message) = frame.read_string() {
                            tracing::info!("<{name}> {message}");
                            let line = format!("<{name}> {message}");
                            send_system_chat(&mut writer, &line).await?;
                        }
                    }
                    // Break a block (status 0 = creative instant, 2 = survival finish).
                    S_PLAYER_ACTION => {
                        if let (Ok(status), Ok(pos), Ok(_face), Ok(seq)) = (
                            frame.read_varint(),
                            frame.read_i64(),
                            frame.read_u8(),
                            frame.read_varint(),
                        ) && (status == 0 || status == 2)
                        {
                            let (x, y, z) = decode_position(pos);
                            world.set_block(x, y, z, STATE_AIR);
                            send_block_update(&mut writer, pos, STATE_AIR).await?;
                            send_block_ack(&mut writer, seq).await?;
                        }
                    }
                    // Place the held block on the clicked face.
                    S_USE_ITEM_ON => {
                        if let (Ok(_hand), Ok(pos), Ok(face)) =
                            (frame.read_varint(), frame.read_i64(), frame.read_varint())
                        {
                            // Skip cursor (3 f32), inside_block and world_border bools.
                            let _ = (frame.read_f32(), frame.read_f32(), frame.read_f32());
                            let _ = (frame.read_u8(), frame.read_u8());
                            if let Ok(seq) = frame.read_varint() {
                                let held = inventory
                                    .get(&(HOTBAR_OFFSET + selected))
                                    .and_then(|item| registries.item_to_block.get(item))
                                    .copied();
                                if let Some(state) = held {
                                    let (tx, ty, tz) = offset_block(pos, face);
                                    world.set_block(tx, ty, tz, state);
                                    send_block_update(&mut writer, encode_position(tx, ty, tz), state)
                                        .await?;
                                }
                                send_block_ack(&mut writer, seq).await?;
                            }
                        }
                    }
                    // Track the selected hotbar slot.
                    S_SET_CARRIED_ITEM => {
                        if let Ok(slot) = frame.read_u16() {
                            selected = i32::from(slot);
                        }
                    }
                    // Track creative inventory edits (slot + item stack).
                    S_SET_CREATIVE_SLOT => {
                        if let Ok(slot) = frame.read_u16() {
                            let slot = i32::from(slot);
                            match frame.read_varint() {
                                Ok(count) if count > 0 => {
                                    if let Ok(item) = frame.read_varint() {
                                        inventory.insert(slot, item);
                                    }
                                }
                                Ok(_) => {
                                    inventory.remove(&slot);
                                }
                                Err(_) => {}
                            }
                        }
                    }
                    _ => {}
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

/// The chunk coordinates containing world position `(x, z)`.
fn chunk_of(x: f64, z: f64) -> (i32, i32) {
    (
        (x.floor() as i32).div_euclid(16),
        (z.floor() as i32).div_euclid(16),
    )
}

/// Re-centres the client's chunk cache on `(cx, cz)` and sends any chunks within
/// `SEND_RADIUS` that haven't been sent yet, as one batch.
async fn load_around<W: AsyncWrite + Unpin>(
    writer: &mut W,
    cx: i32,
    cz: i32,
    loaded: &mut HashSet<(i32, i32)>,
    biome: i32,
    world: &World,
) -> Result<()> {
    let mut center = PacketWriter::new(P_SET_CENTER_CHUNK);
    center.write_varint(cx).write_varint(cz);
    write_frame(writer, &center.into_body()).await?;

    let mut new_chunks = Vec::new();
    for x in (cx - SEND_RADIUS)..=(cx + SEND_RADIUS) {
        for z in (cz - SEND_RADIUS)..=(cz + SEND_RADIUS) {
            if loaded.insert((x, z)) {
                new_chunks.push((x, z));
            }
        }
    }
    if new_chunks.is_empty() {
        return Ok(());
    }

    write_frame(writer, &PacketWriter::new(P_CHUNK_BATCH_START).into_body()).await?;
    for (x, z) in &new_chunks {
        let edits = world.chunk_edits(*x, *z);
        write_frame(writer, &flat_chunk(*x, *z, biome, &edits)).await?;
    }
    let mut finished = PacketWriter::new(P_CHUNK_BATCH_FINISHED);
    finished.write_varint(new_chunks.len() as i32);
    write_frame(writer, &finished.into_body()).await
}

/// Tells the client a block changed to `state` at the packed `position`.
async fn send_block_update<W: AsyncWrite + Unpin>(
    writer: &mut W,
    position: i64,
    state: i32,
) -> Result<()> {
    let mut w = PacketWriter::new(P_BLOCK_UPDATE);
    w.write_i64(position).write_varint(state);
    write_frame(writer, &w.into_body()).await
}

/// Confirms a client action `sequence` so the client keeps its prediction.
async fn send_block_ack<W: AsyncWrite + Unpin>(writer: &mut W, sequence: i32) -> Result<()> {
    let mut w = PacketWriter::new(P_BLOCK_CHANGED_ACK);
    w.write_varint(sequence);
    write_frame(writer, &w.into_body()).await
}

/// Decodes a packed block position into `(x, y, z)`.
fn decode_position(packed: i64) -> (i32, i32, i32) {
    (
        (packed >> 38) as i32,
        (packed << 52 >> 52) as i32,
        (packed << 26 >> 38) as i32,
    )
}

/// Packs `(x, y, z)` into the protocol's block-position long.
fn encode_position(x: i32, y: i32, z: i32) -> i64 {
    ((x as i64 & 0x3FF_FFFF) << 38) | ((z as i64 & 0x3FF_FFFF) << 12) | (y as i64 & 0xFFF)
}

/// The block one step along `face` from a packed position (the placement target).
fn offset_block(packed: i64, face: i32) -> (i32, i32, i32) {
    let (mut x, mut y, mut z) = decode_position(packed);
    match face {
        0 => y -= 1, // bottom
        1 => y += 1, // top
        2 => z -= 1, // north
        3 => z += 1, // south
        4 => x -= 1, // west
        5 => x += 1, // east
        _ => {}
    }
    (x, y, z)
}

/// Sends an (unsigned) system chat message — simpler than signed player chat.
async fn send_system_chat<W: AsyncWrite + Unpin>(writer: &mut W, text: &str) -> Result<()> {
    let mut nbt = Vec::new();
    write_network_nbt(&mut nbt, &Nbt::String(text.to_string()));

    let mut w = PacketWriter::new(P_SYSTEM_CHAT);
    w.write_bytes(&nbt);
    w.write_bool(false); // overlay: false = chat box (not the action bar)
    write_frame(writer, &w.into_body()).await
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
    w.write_varint(VIEW_DISTANCE); // view distance
    w.write_varint(VIEW_DISTANCE); // simulated distance
    w.write_bool(false); // reduced debug info
    w.write_bool(true); // enable respawn screen
    w.write_bool(false); // limited crafting

    // Common spawn info.
    w.write_varint(dimension_type_index);
    w.write_string("minecraft:overworld"); // dimension (world) name
    w.write_i64(0); // hashed seed
    w.write_u8(1); // game mode: creative (so we can fly)
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
    w.write_f64(0.0).write_f64(64.0).write_f64(0.0); // position: on the stone floor
    w.write_f64(0.0).write_f64(0.0).write_f64(0.0); // velocity
    w.write_f32(0.0).write_f32(0.0); // yaw, pitch
    w.write_i32(0); // relative-flags bitfield (all absolute)
    write_frame(stream, &w.into_body()).await
}
