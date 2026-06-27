//! Projectiles: server-driven flying entities (arrows, fireballs) that ranged
//! mobs shoot at the player. Like mobs, we spawn the entity and then stream its
//! position every tick, so we don't need to encode packed velocity on the wire.

use leather_protocol::{PacketWriter, Result, write_frame};
use tokio::io::AsyncWrite;

const P_ADD_ENTITY: i32 = 1;
const P_ENTITY_POSITION_SYNC: i32 = 35;
const P_REMOVE_ENTITIES: i32 = 77;

const ARROW: i32 = 6;
const SMALL_FIREBALL: i32 = 118;
const SPLASH_POTION: i32 = 105;
const WIND_CHARGE: i32 = 144;
const SHULKER_BULLET: i32 = 113;
const LLAMA_SPIT: i32 = 79;
const WITHER_SKULL: i32 = 148;

/// The flat world's surface; a projectile dies when it hits the ground.
const GROUND_Y: f64 = 64.0;
/// How close to the player (its body centre) counts as a hit.
const HIT_RADIUS: f64 = 1.3;
/// Projectiles expire after this many ticks if they hit nothing (10s).
const MAX_LIFE: u32 = 200;

/// What a projectile did this tick.
pub enum Flight {
    /// Still in the air.
    Flying,
    /// Hit the player for this much damage (and should be removed).
    Hit(f32),
    /// Hit the ground or expired (should be removed).
    Gone,
}

pub struct Projectile {
    entity_id: i32,
    entity_type: i32,
    x: f64,
    y: f64,
    z: f64,
    vx: f64,
    vy: f64,
    vz: f64,
    gravity: f64,
    damage: f32,
    life: u32,
}

impl Projectile {
    /// A projectile fired from `from` toward `to` (both `(x, y, z)`).
    fn aimed(
        entity_id: i32,
        entity_type: i32,
        from: (f64, f64, f64),
        to: (f64, f64, f64),
        speed: f64,
        gravity: f64,
        damage: f32,
    ) -> Self {
        let (dx, dy, dz) = (to.0 - from.0, to.1 - from.1, to.2 - from.2);
        let len = (dx * dx + dy * dy + dz * dz).sqrt().max(0.001);
        Self {
            entity_id,
            entity_type,
            x: from.0,
            y: from.1,
            z: from.2,
            vx: dx / len * speed,
            vy: dy / len * speed,
            vz: dz / len * speed,
            gravity,
            damage,
            life: MAX_LIFE,
        }
    }

    /// An arrow: fast, affected by gravity.
    pub fn arrow(id: i32, from: (f64, f64, f64), to: (f64, f64, f64)) -> Self {
        Self::aimed(id, ARROW, from, to, 2.0, 0.05, 4.0)
    }

    /// A small fireball: slower, flies straight (no gravity).
    pub fn fireball(id: i32, from: (f64, f64, f64), to: (f64, f64, f64)) -> Self {
        Self::aimed(id, SMALL_FIREBALL, from, to, 1.2, 0.0, 5.0)
    }

    /// A witch's thrown splash potion: lobbed, affected by gravity.
    pub fn potion(id: i32, from: (f64, f64, f64), to: (f64, f64, f64)) -> Self {
        Self::aimed(id, SPLASH_POTION, from, to, 1.1, 0.05, 6.0)
    }

    /// A breeze's wind charge: straight, mostly knockback (light damage).
    pub fn wind_charge(id: i32, from: (f64, f64, f64), to: (f64, f64, f64)) -> Self {
        Self::aimed(id, WIND_CHARGE, from, to, 1.2, 0.0, 1.0)
    }

    /// A shulker bullet: slow (homing in vanilla; straight here for now).
    pub fn shulker_bullet(id: i32, from: (f64, f64, f64), to: (f64, f64, f64)) -> Self {
        Self::aimed(id, SHULKER_BULLET, from, to, 0.8, 0.0, 4.0)
    }

    /// A llama's spit: light, affected by gravity.
    pub fn llama_spit(id: i32, from: (f64, f64, f64), to: (f64, f64, f64)) -> Self {
        Self::aimed(id, LLAMA_SPIT, from, to, 1.2, 0.05, 1.0)
    }

    /// A wither skull: flies straight.
    pub fn wither_skull(id: i32, from: (f64, f64, f64), to: (f64, f64, f64)) -> Self {
        Self::aimed(id, WITHER_SKULL, from, to, 1.0, 0.0, 5.0)
    }

    pub fn id(&self) -> i32 {
        self.entity_id
    }

    /// Sends the `add_entity` packet (spawned with zero packed velocity; we drive
    /// its motion ourselves via position sync).
    pub async fn spawn<W: AsyncWrite + Unpin>(&self, writer: &mut W) -> Result<()> {
        let mut w = PacketWriter::new(P_ADD_ENTITY);
        w.write_varint(self.entity_id);
        w.write_uuid(0xA440_0000_0000_0000_0000_0000_0000_0000 + self.entity_id as u128);
        w.write_varint(self.entity_type);
        w.write_f64(self.x).write_f64(self.y).write_f64(self.z);
        w.write_u8(0); // velocity (compact zero)
        w.write_u8(0); // pitch
        w.write_u8(0); // yaw
        w.write_u8(0); // head yaw
        w.write_varint(0); // data (shooter id; 0 = none)
        write_frame(writer, &w.into_body()).await
    }

    /// Removes the projectile from the client.
    pub async fn despawn<W: AsyncWrite + Unpin>(&self, writer: &mut W) -> Result<()> {
        let mut w = PacketWriter::new(P_REMOVE_ENTITIES);
        w.write_varint(1);
        w.write_varint(self.entity_id);
        write_frame(writer, &w.into_body()).await
    }

    /// Advances the projectile: move, apply gravity, then test for a hit on the
    /// player `(px, py, pz)` (its body centre), the ground, or expiry.
    pub async fn tick<W: AsyncWrite + Unpin>(
        &mut self,
        writer: &mut W,
        px: f64,
        py: f64,
        pz: f64,
    ) -> Result<Flight> {
        self.x += self.vx;
        self.y += self.vy;
        self.z += self.vz;
        self.vy -= self.gravity;
        self.life = self.life.saturating_sub(1);

        let (dx, dy, dz) = (self.x - px, self.y - py, self.z - pz);
        if (dx * dx + dy * dy + dz * dz).sqrt() <= HIT_RADIUS {
            self.despawn(writer).await?;
            return Ok(Flight::Hit(self.damage));
        }
        if self.y <= GROUND_Y || self.life == 0 {
            self.despawn(writer).await?;
            return Ok(Flight::Gone);
        }

        let mut sync = PacketWriter::new(P_ENTITY_POSITION_SYNC);
        sync.write_varint(self.entity_id);
        sync.write_f64(self.x).write_f64(self.y).write_f64(self.z);
        sync.write_f64(0.0).write_f64(0.0).write_f64(0.0); // velocity (delta)
        sync.write_f32(0.0).write_f32(0.0); // yaw, pitch
        sync.write_bool(false); // on_ground
        write_frame(writer, &sync.into_body()).await?;
        Ok(Flight::Flying)
    }
}
