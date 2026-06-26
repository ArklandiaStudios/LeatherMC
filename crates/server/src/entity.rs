//! Entities: spawn a mob and move it so the player can see a living, moving
//! entity. This is the second step of the "entities / mobs" brick (after a
//! static spawn): the mob paces back and forth, driven by the server, and turns
//! around smoothly at each end (head leading, body following).

use leather_protocol::{PacketWriter, Result, write_frame};
use tokio::io::AsyncWrite;

/// Clientbound play packet ids (protocol 776).
const P_ADD_ENTITY: i32 = 1;
const P_ENTITY_POSITION_SYNC: i32 = 35;
const P_ROTATE_HEAD: i32 = 83;

/// `entity_type` registry id of a pig. These ids are built-in (the registry is
/// not data-driven), so we use the canonical protocol id from `registries.json`.
const ENTITY_TYPE_PIG: i32 = 100;

/// How far the mob may stray from spawn along X, and how far it steps per tick
/// (at 20 ticks/second, like vanilla, so the motion looks right).
const MIN_X: f64 = 3.0;
const MAX_X: f64 = 8.0;
const STEP: f64 = 0.15;

/// Rotation model, matching vanilla (`LivingEntity::tickHeadTurn`):
/// the head turns toward the heading at up to `HEAD_TURN` per tick; the body
/// eases toward the head by `BODY_FOLLOW` of the gap each tick; and the head may
/// never stray more than `MAX_HEAD_YAW` from the body (beyond that the body is
/// dragged along). So the head leads the turn and the body swings to follow.
const HEAD_TURN: f64 = 40.0;
const BODY_FOLLOW: f64 = 0.5;
const MAX_HEAD_YAW: f64 = 75.0;

/// The mob walks only once its head is roughly facing where it's going, so it
/// turns toward the new direction at each end before setting off again.
const ALIGN_THRESHOLD: f64 = 45.0;

/// A simple, server-driven mob that paces back and forth along the X axis,
/// turning around smoothly at each end — just enough to exercise spawning,
/// movement and rotation on a real client.
pub struct DemoMob {
    entity_id: i32,
    uuid: u128,
    x: f64,
    y: f64,
    z: f64,
    /// Walking direction along X: `+1.0` (east) or `-1.0` (west).
    dir: f64,
    /// Current body and head headings, in degrees, eased toward the target each
    /// tick so turns are gradual rather than instant.
    body_yaw: f64,
    head_yaw: f64,
}

impl DemoMob {
    /// A pig spawned a few blocks east of the world spawn, already facing east.
    /// Its entity id must be unique and distinct from the player's own id (1).
    pub fn pig() -> Self {
        Self {
            entity_id: 2,
            uuid: 0x1ea7_e12c_0000_0000_0000_0000_0000_0001,
            x: MIN_X,
            y: 64.0,
            z: 0.0,
            dir: 1.0,
            body_yaw: -90.0, // east
            head_yaw: -90.0,
        }
    }

    /// The heading the mob currently wants to face, in degrees (clockwise from
    /// south): east (+x) = -90°, west (-x) = +90°.
    fn target_yaw(&self) -> f64 {
        if self.dir > 0.0 { -90.0 } else { 90.0 }
    }

    /// Sends the `add_entity` packet that makes this mob appear, already facing
    /// its starting direction (both body and head).
    pub async fn spawn<W: AsyncWrite + Unpin>(&self, writer: &mut W) -> Result<()> {
        let yaw = yaw_to_angle(self.body_yaw);
        let mut w = PacketWriter::new(P_ADD_ENTITY);
        w.write_varint(self.entity_id);
        w.write_uuid(self.uuid);
        w.write_varint(ENTITY_TYPE_PIG);
        w.write_f64(self.x).write_f64(self.y).write_f64(self.z);
        // Velocity, compact bit-packed form added in 1.21.9 (right after the
        // position): a zero velocity is a single 0x00 header byte.
        w.write_u8(0);
        w.write_u8(0); // pitch (angle: 1/256 of a full turn)
        w.write_u8(yaw); // yaw (body)
        w.write_u8(yaw); // head yaw
        w.write_varint(0); // data (entity-type-specific; unused for a plain mob)
        write_frame(writer, &w.into_body()).await
    }

    /// Advances the mob one tick: ease its head and body toward the target
    /// heading, walk only once roughly aligned, and turn around at the ends.
    /// Then send the new position (carrying the body yaw) and head rotation.
    ///
    /// An entity has two yaws: the body yaw (carried by the position sync) and
    /// the head yaw (a separate `rotate_head` packet). Both must be sent or the
    /// mob's body and head face different ways.
    pub async fn tick<W: AsyncWrite + Unpin>(&mut self, writer: &mut W) -> Result<()> {
        let target = self.target_yaw();

        // Head turns toward the heading, fast but rate-limited.
        self.head_yaw = approach_angle(self.head_yaw, target, HEAD_TURN);
        // Body eases toward the head...
        self.body_yaw += angle_diff(self.body_yaw, self.head_yaw) * BODY_FOLLOW;
        // ...but the head may not stray more than 75° from the body; past that,
        // drag the body along so the head stays "on the shoulders".
        let off = angle_diff(self.body_yaw, self.head_yaw);
        if off.abs() > MAX_HEAD_YAW {
            self.body_yaw = self.head_yaw - MAX_HEAD_YAW * off.signum();
        }

        // Walk only when the head is roughly facing where we're going, so the
        // pig turns toward the new direction at each end before walking back.
        if angle_diff(self.head_yaw, target).abs() < ALIGN_THRESHOLD {
            self.x += STEP * self.dir;
            if self.x >= MAX_X {
                self.x = MAX_X;
                self.dir = -1.0;
            } else if self.x <= MIN_X {
                self.x = MIN_X;
                self.dir = 1.0;
            }
        }

        let mut sync = PacketWriter::new(P_ENTITY_POSITION_SYNC);
        sync.write_varint(self.entity_id);
        sync.write_f64(self.x).write_f64(self.y).write_f64(self.z);
        sync.write_f64(0.0).write_f64(0.0).write_f64(0.0); // velocity (delta)
        sync.write_f32(self.body_yaw as f32).write_f32(0.0); // yaw (body), pitch
        sync.write_bool(true); // on_ground
        write_frame(writer, &sync.into_body()).await?;

        let mut head = PacketWriter::new(P_ROTATE_HEAD);
        head.write_varint(self.entity_id);
        head.write_u8(yaw_to_angle(self.head_yaw));
        write_frame(writer, &head.into_body()).await
    }
}

/// Shortest signed angular difference `target - current`, wrapped to (-180, 180].
fn angle_diff(current: f64, target: f64) -> f64 {
    let mut d = (target - current).rem_euclid(360.0);
    if d > 180.0 {
        d -= 360.0;
    }
    d
}

/// Moves `current` toward `target` by at most `max_step` degrees (shortest way).
fn approach_angle(current: f64, target: f64, max_step: f64) -> f64 {
    let d = angle_diff(current, target);
    if d.abs() <= max_step {
        target
    } else {
        current + max_step * d.signum()
    }
}

/// Converts a yaw in degrees to a protocol angle byte (256 units = 360°).
fn yaw_to_angle(deg: f64) -> u8 {
    (deg / 360.0 * 256.0).round() as i32 as u8
}
