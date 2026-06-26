//! Entities: spawn a mob into the world so the player can see it.
//!
//! This is the first, deliberately small step of the "entities / mobs" brick.
//! We send a single `add_entity` packet for a static mob near spawn. Its default
//! appearance is rendered by the client without any metadata, so this is enough
//! to make a living entity visible. Movement and metadata come later.

use leather_protocol::{PacketWriter, Result, write_frame};
use tokio::io::AsyncWrite;

/// Clientbound play packet id (protocol 776): "Spawn Entity".
const P_ADD_ENTITY: i32 = 1;

/// `entity_type` registry id of a pig. These ids are built-in (the registry is
/// not data-driven), so we use the canonical protocol id from `registries.json`.
const ENTITY_TYPE_PIG: i32 = 100;

/// Sends an `add_entity` packet spawning a mob of `entity_type` at `(x, y, z)`.
///
/// `entity_id` must be unique and distinct from the player's own entity id.
/// Rotation, head yaw and velocity are left at zero (a still, forward-facing mob).
async fn spawn_entity<W: AsyncWrite + Unpin>(
    writer: &mut W,
    entity_id: i32,
    uuid: u128,
    entity_type: i32,
    x: f64,
    y: f64,
    z: f64,
) -> Result<()> {
    let mut w = PacketWriter::new(P_ADD_ENTITY);
    w.write_varint(entity_id);
    w.write_uuid(uuid);
    w.write_varint(entity_type);
    w.write_f64(x).write_f64(y).write_f64(z);
    // Velocity, in the compact bit-packed form added in 1.21.9 (placed right
    // after the position). A zero velocity has scale factor 0, which encodes as a
    // single 0x00 header byte (no quantised components, no extension). Non-zero
    // velocity (the full LpVector3d packing) will come with entity movement.
    w.write_u8(0);
    w.write_u8(0); // pitch (angle: 1/256 of a full turn)
    w.write_u8(0); // yaw
    w.write_u8(0); // head yaw
    w.write_varint(0); // data (entity-type-specific; unused for a plain mob)
    write_frame(writer, &w.into_body()).await
}

/// Spawns the demo mob: one static pig a few blocks from the spawn point, so the
/// player sees an entity as soon as they join.
pub async fn spawn_demo_mob<W: AsyncWrite + Unpin>(writer: &mut W) -> Result<()> {
    const PIG_ENTITY_ID: i32 = 2; // player is entity id 1
    const PIG_UUID: u128 = 0x1ea7_e12c_0000_0000_0000_0000_0000_0001;
    spawn_entity(writer, PIG_ENTITY_ID, PIG_UUID, ENTITY_TYPE_PIG, 3.0, 64.0, 0.0).await
}
