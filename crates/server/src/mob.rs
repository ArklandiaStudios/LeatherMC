//! Mobs: the living-entity foundation of the mob engine.
//!
//! A [`Mob`] is a generic living entity of some [`MobKind`] (pig, cow, …). This
//! is the first real engine brick: mobs have health, take damage and die — no
//! more one-shot. Every mob plugs into this same model.
//!
//! What is still a placeholder (handled by later bricks): movement here is a
//! simple scripted pace (real AI + pathfinding come next), and mobs are placed
//! by hand (a conditional spawning engine comes after that).

use leather_protocol::{Nbt, PacketWriter, Result, write_frame, write_network_nbt};
use tokio::io::AsyncWrite;

// Clientbound play packet ids (protocol 776).
const P_ADD_ENTITY: i32 = 1;
const P_ANIMATE: i32 = 2;
const P_DAMAGE_EVENT: i32 = 25;
const P_ENTITY_EVENT: i32 = 34;
const P_ENTITY_POSITION_SYNC: i32 = 35;
const P_REMOVE_ENTITIES: i32 = 77;
const P_ROTATE_HEAD: i32 = 83;
const P_SET_ENTITY_DATA: i32 = 99;

/// `animate` ids and `entity_event` statuses we use.
const ANIM_CRITICAL_HIT: u8 = 4;
const EVENT_DEATH: u8 = 3;

/// Entity-metadata value type ids and base Entity indices (for the debug name).
const META_TYPE_BOOLEAN: i32 = 8;
const META_TYPE_OPTIONAL_TEXT: i32 = 6;
const META_IDX_CUSTOM_NAME: u8 = 2;
const META_IDX_CUSTOM_NAME_VISIBLE: u8 = 3;
const META_END: u8 = 0xFF;

/// Ticks the death animation plays before the entity is removed (1s at 20 TPS).
const DEATH_TICKS: u32 = 20;

/// Damage-immunity window after a hit: 10 ticks (0.5s). During it, a new hit
/// only lands if it is stronger than the one that started the window, and only
/// the difference applies — so spamming weak hits does nothing.
const IMMUNITY_TICKS: u32 = 10;

/// What a hit did to a mob.
pub enum Hit {
    /// Blocked by damage immunity (no damage, no flash).
    Blocked,
    /// Damage was applied; `died` is true if it was the killing blow.
    Damaged { died: bool },
}

/// A kind of mob and its stats. `entity_type` is the built-in protocol id from
/// `registries.json`; `max_health` is the vanilla value (in half-hearts).
pub struct MobKind {
    pub name: &'static str,
    pub entity_type: i32,
    pub max_health: f32,
}

pub const PIG: MobKind = MobKind {
    name: "pig",
    entity_type: 100,
    max_health: 10.0,
};
pub const COW: MobKind = MobKind {
    name: "cow",
    entity_type: 30,
    max_health: 10.0,
};
pub const CHICKEN: MobKind = MobKind {
    name: "chicken",
    entity_type: 26,
    max_health: 4.0,
};

/// Wander AI tuning (movement is in blocks per tick, at 20 ticks/second).
/// These are tuned by eye for now; once we model the movement-speed attribute
/// and friction, panic will derive from walk via vanilla's 1.25× modifier.
const WALK_SPEED: f64 = 0.12;
const PANIC_SPEED: f64 = 0.25;
/// How far from its home a mob strolls.
const WANDER_RADIUS: f64 = 8.0;
/// Each panic dash is a short hop this many blocks, in a random direction, so a
/// panicking mob bolts around frantically.
const FLEE_MIN_DASH: f64 = 2.0;
const FLEE_MAX_DASH: f64 = 5.0;
/// Panic lasts a random 4–12 seconds (at 20 ticks/second).
const PANIC_MIN_TICKS: u32 = 80;
const PANIC_MAX_TICKS: u32 = 240;
/// Distance at which a movement target counts as reached.
const REACH_DISTANCE: f64 = 0.4;
/// The mob walks only when its body is within this many degrees of its heading.
const ALIGN_THRESHOLD: f64 = 60.0;

/// Rotation model, matching vanilla (`LivingEntity::tickHeadTurn`): the head
/// turns toward the heading at up to `HEAD_TURN` per tick, the body eases toward
/// the head by `BODY_FOLLOW` of the gap, and the head may never stray more than
/// `MAX_HEAD_YAW` from the body. So the head leads each turn and the body follows.
const HEAD_TURN: f64 = 40.0;
const BODY_FOLLOW: f64 = 0.5;
const MAX_HEAD_YAW: f64 = 75.0;
/// How fast an idle mob's head drifts back to face its body.
const IDLE_HEAD_RELAX: f64 = 8.0;

/// A living entity in the world.
pub struct Mob {
    kind: &'static MobKind,
    entity_id: i32,
    uuid: u128,
    x: f64,
    y: f64,
    z: f64,
    /// Current movement target (x, z), if the mob is heading somewhere.
    target: Option<(f64, f64)>,
    /// Ticks to wait before picking the next stroll target.
    idle_ticks: u32,
    /// Ticks of panic (fleeing fast) left after being hurt.
    panic_ticks: u32,
    /// Per-mob PRNG state (xorshift), for wander/idle decisions.
    rng: u64,
    /// Current body and head headings, in degrees, eased toward the target.
    body_yaw: f64,
    head_yaw: f64,
    /// Current health, in half-hearts. The mob dies when it reaches zero.
    health: f32,
    /// Damage-immunity ticks left, and the damage of the hit that started the
    /// current immunity window (for the "only stronger hits land" rule).
    invulnerable: u32,
    last_damage: f32,
    /// `Some(ticks_left)` while the death animation plays, then removal. `None`
    /// while alive.
    death_timer: Option<u32>,
}

impl Mob {
    /// A mob of `kind` with the given entity id, at `(x, z)`, facing east and at
    /// full health. The entity id must be unique and distinct from the player (1).
    pub fn new(kind: &'static MobKind, entity_id: i32, x: f64, z: f64) -> Self {
        Self {
            kind,
            entity_id,
            // Distinct, stable per-entity UUID derived from the id.
            uuid: 0x1ea7_e12c_0000_0000_0000_0000_0000_0000 + entity_id as u128,
            x,
            y: 64.0,
            z,
            target: None,
            idle_ticks: 0,
            panic_ticks: 0,
            // Seed the PRNG from the id so each mob wanders differently (nonzero).
            rng: 0x9E37_79B9_7F4A_7C15 ^ (entity_id as u64).wrapping_mul(0x2545_F491_4F6C_DD1D),
            body_yaw: -90.0, // east
            head_yaw: -90.0,
            health: kind.max_health,
            invulnerable: 0,
            last_damage: 0.0,
            death_timer: None,
        }
    }

    /// A small mixed herd in parallel lanes, staggered so they don't pace in
    /// lockstep — enough to show the engine handles several kinds at once. Entity
    /// ids start at 2 (the player is 1).
    pub fn herd() -> Vec<Mob> {
        vec![
            Mob::new(&PIG, 2, 5.0, -3.0),
            Mob::new(&COW, 3, 5.0, 0.0),
            Mob::new(&CHICKEN, 4, 5.0, 3.0),
        ]
    }

    /// This mob's entity id, for matching serverbound packets that target it.
    pub fn id(&self) -> i32 {
        self.entity_id
    }

    /// Whether the mob is playing its death animation (and so can't be hit).
    pub fn is_dying(&self) -> bool {
        self.death_timer.is_some()
    }

    /// Applies `amount` of damage, honouring the damage-immunity window: during
    /// immunity a hit lands only if it's stronger than the one that started the
    /// window, and only the surplus applies. Returns what happened.
    pub fn take_damage(&mut self, amount: f32) -> Hit {
        let effective = if self.invulnerable > 0 {
            if amount <= self.last_damage {
                return Hit::Blocked; // immune to this (weaker-or-equal) hit
            }
            let surplus = amount - self.last_damage;
            self.last_damage = amount; // raise the bar; timer keeps running
            surplus
        } else {
            self.last_damage = amount;
            self.invulnerable = IMMUNITY_TICKS;
            amount
        };

        self.health -= effective;
        if self.health <= 0.0 {
            self.health = 0.0;
            tracing::info!("{} (entity {}) died", self.kind.name, self.entity_id);
            Hit::Damaged { died: true }
        } else {
            Hit::Damaged { died: false }
        }
    }

    /// TEMPORARY debug: shows the mob's current/max health as a floating name, so
    /// damage is visible while we build combat. Remove once a real HUD exists.
    pub async fn update_name<W: AsyncWrite + Unpin>(&self, writer: &mut W) -> Result<()> {
        let label = format!("{:.0} / {:.0}", self.health, self.kind.max_health);
        let mut name_nbt = Vec::new();
        write_network_nbt(&mut name_nbt, &Nbt::String(label));

        let mut w = PacketWriter::new(P_SET_ENTITY_DATA);
        w.write_varint(self.entity_id);
        w.write_u8(META_IDX_CUSTOM_NAME);
        w.write_varint(META_TYPE_OPTIONAL_TEXT);
        w.write_bool(true);
        w.write_bytes(&name_nbt);
        w.write_u8(META_IDX_CUSTOM_NAME_VISIBLE);
        w.write_varint(META_TYPE_BOOLEAN);
        w.write_bool(true);
        w.write_u8(META_END);
        write_frame(writer, &w.into_body()).await
    }

    /// Shows the critical-hit star particles on this mob.
    pub async fn play_crit<W: AsyncWrite + Unpin>(&self, writer: &mut W) -> Result<()> {
        let mut w = PacketWriter::new(P_ANIMATE);
        w.write_varint(self.entity_id);
        w.write_u8(ANIM_CRITICAL_HIT);
        write_frame(writer, &w.into_body()).await
    }

    /// Begins the death sequence: tell the client to play the death animation and
    /// start the removal countdown. The mob stops moving and can't be hit again.
    pub async fn start_dying<W: AsyncWrite + Unpin>(&mut self, writer: &mut W) -> Result<()> {
        self.death_timer = Some(DEATH_TICKS);
        let mut w = PacketWriter::new(P_ENTITY_EVENT);
        w.write_i32(self.entity_id); // entity_event uses a plain Int id
        w.write_u8(EVENT_DEATH);
        write_frame(writer, &w.into_body()).await
    }

    /// Sends the `add_entity` packet that makes this mob appear, facing east.
    pub async fn spawn<W: AsyncWrite + Unpin>(&self, writer: &mut W) -> Result<()> {
        let yaw = yaw_to_angle(self.body_yaw);
        let mut w = PacketWriter::new(P_ADD_ENTITY);
        w.write_varint(self.entity_id);
        w.write_uuid(self.uuid);
        w.write_varint(self.kind.entity_type);
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

    /// Plays the hurt reaction (red flash + hurt sound) by telling the client
    /// this entity took damage of `damage_type_index` (an index into the
    /// damage_type registry we sent). No cause entity and no knockback source.
    pub async fn hurt<W: AsyncWrite + Unpin>(
        &self,
        writer: &mut W,
        damage_type_index: i32,
    ) -> Result<()> {
        let mut w = PacketWriter::new(P_DAMAGE_EVENT);
        w.write_varint(self.entity_id);
        w.write_varint(damage_type_index);
        w.write_varint(0); // source cause entity: none
        w.write_varint(0); // source direct entity: none
        w.write_bool(false); // no source position
        write_frame(writer, &w.into_body()).await
    }

    /// Removes this entity from the client (it disappears).
    pub async fn despawn<W: AsyncWrite + Unpin>(&self, writer: &mut W) -> Result<()> {
        let mut w = PacketWriter::new(P_REMOVE_ENTITIES);
        w.write_varint(1); // count
        w.write_varint(self.entity_id);
        write_frame(writer, &w.into_body()).await
    }

    /// Reacts to being hurt: panic for a random 4–12 seconds (vanilla-ish
    /// PanicGoal), bolting around in random directions the whole time.
    pub fn panic(&mut self) {
        let span = PANIC_MAX_TICKS - PANIC_MIN_TICKS;
        self.panic_ticks = PANIC_MIN_TICKS + (self.rand01() * span as f64) as u32;
        self.pick_flee_target();
    }

    /// Next PRNG value (xorshift64).
    fn next_rng(&mut self) -> u64 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.rng = x;
        x
    }

    /// A pseudo-random float in `[0, 1)`.
    fn rand01(&mut self) -> f64 {
        (self.next_rng() >> 11) as f64 / (1u64 << 53) as f64
    }

    /// Picks a random stroll target near the mob's current position (so it
    /// wanders wherever it is, never drifting back to its spawn).
    fn pick_wander_target(&mut self) {
        let angle = self.rand01() * std::f64::consts::TAU;
        let r = self.rand01() * WANDER_RADIUS;
        self.target = Some((self.x + angle.cos() * r, self.z + angle.sin() * r));
    }

    /// Picks the next flee point while panicking: a short dash in a fully random
    /// direction, so the mob bolts around frantically rather than in one line.
    fn pick_flee_target(&mut self) {
        let angle = self.rand01() * std::f64::consts::TAU;
        let dist = FLEE_MIN_DASH + self.rand01() * (FLEE_MAX_DASH - FLEE_MIN_DASH);
        self.target = Some((self.x + angle.cos() * dist, self.z + angle.sin() * dist));
    }

    /// Advances the mob one tick. While dying, counts down and then removes the
    /// entity; otherwise paces (placeholder) and sends its position and head
    /// rotation. Returns `true` once the mob should be dropped (death finished).
    /// Using position sync (absolute) avoids rounding drift.
    pub async fn tick<W: AsyncWrite + Unpin>(&mut self, writer: &mut W) -> Result<bool> {
        // Dying: hold still, run out the animation, then despawn.
        if let Some(left) = self.death_timer {
            if left == 0 {
                self.despawn(writer).await?;
                return Ok(true);
            }
            self.death_timer = Some(left - 1);
            return Ok(false);
        }

        if self.invulnerable > 0 {
            self.invulnerable -= 1;
        }
        let panicking = self.panic_ticks > 0;
        if panicking {
            self.panic_ticks -= 1;
        }

        // Decide where to go. While panicking, keep running: pick a fresh flee
        // point ahead whenever the last is reached. Otherwise stroll, with an idle
        // pause between targets. Panic ends only when its timer runs out.
        if panicking {
            if self.target.is_none() {
                self.pick_flee_target();
            }
        } else if self.target.is_none() {
            if self.idle_ticks > 0 {
                self.idle_ticks -= 1;
            } else {
                self.pick_wander_target();
            }
        }

        // Steer toward the target (if any) and move when roughly facing it.
        if let Some((tx, tz)) = self.target {
            let (dx, dz) = (tx - self.x, tz - self.z);
            let dist = (dx * dx + dz * dz).sqrt();
            if dist < REACH_DISTANCE {
                self.target = None;
                // Strolling pauses between targets; panic immediately re-aims.
                if !panicking {
                    self.idle_ticks = 20 + (self.rand01() * 60.0) as u32;
                }
            } else {
                // Yaw toward the movement direction (0° = +z south, -90° = +x east).
                let heading = -dx.atan2(dz).to_degrees();
                self.head_yaw = approach_angle(self.head_yaw, heading, HEAD_TURN);
                self.body_yaw += angle_diff(self.body_yaw, self.head_yaw) * BODY_FOLLOW;
                let off = angle_diff(self.body_yaw, self.head_yaw);
                if off.abs() > MAX_HEAD_YAW {
                    self.body_yaw = self.head_yaw - MAX_HEAD_YAW * off.signum();
                }
                // Walk once roughly facing the way we're going.
                if angle_diff(self.body_yaw, heading).abs() < ALIGN_THRESHOLD {
                    let speed = if panicking { PANIC_SPEED } else { WALK_SPEED };
                    let step = speed.min(dist);
                    self.x += dx / dist * step;
                    self.z += dz / dist * step;
                }
            }
        } else {
            // Idle: let the head drift back to face the body.
            self.head_yaw = approach_angle(self.head_yaw, self.body_yaw, IDLE_HEAD_RELAX);
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
        write_frame(writer, &head.into_body()).await?;

        Ok(false)
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
