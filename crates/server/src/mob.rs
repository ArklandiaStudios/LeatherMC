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

/// How a mob behaves toward the player.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Behavior {
    /// Wanders and flees when hurt (most animals).
    Passive,
    /// Wanders peacefully but chases the player once provoked (wolf, bee…).
    Neutral,
    /// Chases the player on sight (zombie, creeper…).
    Hostile,
}

/// A kind of mob and its stats. `entity_type` is the built-in protocol id from
/// `registries.json`; `max_health` is the vanilla value (in half-hearts).
pub struct MobKind {
    pub name: &'static str,
    pub entity_type: i32,
    pub max_health: f32,
    pub behavior: Behavior,
}

/// Every living mob: entity-type protocol id, vanilla max health (half-hearts)
/// and its AI behaviour. Health/behaviour are standard-vanilla approximations;
/// newer 26.2 mobs are best-effort and will be refined.
pub const ALLAY: MobKind = MobKind { name: "allay", entity_type: 2, max_health: 20.0, behavior: Behavior::Passive };
pub const ARMADILLO: MobKind = MobKind { name: "armadillo", entity_type: 4, max_health: 12.0, behavior: Behavior::Passive };
pub const AXOLOTL: MobKind = MobKind { name: "axolotl", entity_type: 7, max_health: 14.0, behavior: Behavior::Passive };
pub const BAT: MobKind = MobKind { name: "bat", entity_type: 10, max_health: 6.0, behavior: Behavior::Passive };
pub const BEE: MobKind = MobKind { name: "bee", entity_type: 11, max_health: 10.0, behavior: Behavior::Neutral };
pub const BLAZE: MobKind = MobKind { name: "blaze", entity_type: 14, max_health: 20.0, behavior: Behavior::Hostile };
pub const BOGGED: MobKind = MobKind { name: "bogged", entity_type: 16, max_health: 16.0, behavior: Behavior::Hostile };
pub const BREEZE: MobKind = MobKind { name: "breeze", entity_type: 17, max_health: 30.0, behavior: Behavior::Hostile };
pub const CAMEL: MobKind = MobKind { name: "camel", entity_type: 19, max_health: 32.0, behavior: Behavior::Passive };
pub const CAMEL_HUSK: MobKind = MobKind { name: "camel_husk", entity_type: 20, max_health: 32.0, behavior: Behavior::Neutral }; // new in 26.2
pub const CAT: MobKind = MobKind { name: "cat", entity_type: 21, max_health: 10.0, behavior: Behavior::Passive };
pub const CAVE_SPIDER: MobKind = MobKind { name: "cave_spider", entity_type: 22, max_health: 12.0, behavior: Behavior::Hostile };
pub const CHICKEN: MobKind = MobKind { name: "chicken", entity_type: 26, max_health: 4.0, behavior: Behavior::Passive };
pub const COD: MobKind = MobKind { name: "cod", entity_type: 27, max_health: 3.0, behavior: Behavior::Passive };
pub const COPPER_GOLEM: MobKind = MobKind { name: "copper_golem", entity_type: 28, max_health: 20.0, behavior: Behavior::Passive }; // new in 26.2
pub const COW: MobKind = MobKind { name: "cow", entity_type: 30, max_health: 10.0, behavior: Behavior::Passive };
pub const CREAKING: MobKind = MobKind { name: "creaking", entity_type: 31, max_health: 20.0, behavior: Behavior::Hostile }; // new in 26.2
pub const CREEPER: MobKind = MobKind { name: "creeper", entity_type: 32, max_health: 20.0, behavior: Behavior::Hostile };
pub const DOLPHIN: MobKind = MobKind { name: "dolphin", entity_type: 35, max_health: 10.0, behavior: Behavior::Neutral };
pub const DONKEY: MobKind = MobKind { name: "donkey", entity_type: 36, max_health: 15.0, behavior: Behavior::Passive };
pub const DROWNED: MobKind = MobKind { name: "drowned", entity_type: 38, max_health: 20.0, behavior: Behavior::Hostile };
pub const ELDER_GUARDIAN: MobKind = MobKind { name: "elder_guardian", entity_type: 40, max_health: 80.0, behavior: Behavior::Hostile };
pub const ENDER_DRAGON: MobKind = MobKind { name: "ender_dragon", entity_type: 43, max_health: 200.0, behavior: Behavior::Hostile };
pub const ENDERMAN: MobKind = MobKind { name: "enderman", entity_type: 41, max_health: 40.0, behavior: Behavior::Neutral };
pub const ENDERMITE: MobKind = MobKind { name: "endermite", entity_type: 42, max_health: 8.0, behavior: Behavior::Hostile };
pub const EVOKER: MobKind = MobKind { name: "evoker", entity_type: 46, max_health: 24.0, behavior: Behavior::Hostile };
pub const FOX: MobKind = MobKind { name: "fox", entity_type: 54, max_health: 10.0, behavior: Behavior::Passive };
pub const FROG: MobKind = MobKind { name: "frog", entity_type: 55, max_health: 10.0, behavior: Behavior::Passive };
pub const GHAST: MobKind = MobKind { name: "ghast", entity_type: 57, max_health: 10.0, behavior: Behavior::Hostile };
pub const GIANT: MobKind = MobKind { name: "giant", entity_type: 59, max_health: 100.0, behavior: Behavior::Hostile };
pub const GLOW_SQUID: MobKind = MobKind { name: "glow_squid", entity_type: 61, max_health: 10.0, behavior: Behavior::Passive };
pub const GOAT: MobKind = MobKind { name: "goat", entity_type: 62, max_health: 10.0, behavior: Behavior::Neutral };
pub const GUARDIAN: MobKind = MobKind { name: "guardian", entity_type: 63, max_health: 30.0, behavior: Behavior::Hostile };
pub const HAPPY_GHAST: MobKind = MobKind { name: "happy_ghast", entity_type: 58, max_health: 20.0, behavior: Behavior::Passive }; // new in 26.2
pub const HOGLIN: MobKind = MobKind { name: "hoglin", entity_type: 64, max_health: 40.0, behavior: Behavior::Hostile };
pub const HORSE: MobKind = MobKind { name: "horse", entity_type: 66, max_health: 30.0, behavior: Behavior::Passive };
pub const HUSK: MobKind = MobKind { name: "husk", entity_type: 67, max_health: 20.0, behavior: Behavior::Hostile };
pub const ILLUSIONER: MobKind = MobKind { name: "illusioner", entity_type: 68, max_health: 32.0, behavior: Behavior::Hostile };
pub const IRON_GOLEM: MobKind = MobKind { name: "iron_golem", entity_type: 70, max_health: 100.0, behavior: Behavior::Neutral };
pub const LLAMA: MobKind = MobKind { name: "llama", entity_type: 78, max_health: 30.0, behavior: Behavior::Neutral };
pub const MAGMA_CUBE: MobKind = MobKind { name: "magma_cube", entity_type: 80, max_health: 16.0, behavior: Behavior::Hostile };
pub const MOOSHROOM: MobKind = MobKind { name: "mooshroom", entity_type: 86, max_health: 10.0, behavior: Behavior::Passive };
pub const MULE: MobKind = MobKind { name: "mule", entity_type: 87, max_health: 15.0, behavior: Behavior::Passive };
pub const NAUTILUS: MobKind = MobKind { name: "nautilus", entity_type: 88, max_health: 20.0, behavior: Behavior::Neutral }; // new in 26.2
pub const OCELOT: MobKind = MobKind { name: "ocelot", entity_type: 91, max_health: 10.0, behavior: Behavior::Passive };
pub const PANDA: MobKind = MobKind { name: "panda", entity_type: 96, max_health: 20.0, behavior: Behavior::Neutral };
pub const PARCHED: MobKind = MobKind { name: "parched", entity_type: 97, max_health: 20.0, behavior: Behavior::Hostile }; // new in 26.2
pub const PARROT: MobKind = MobKind { name: "parrot", entity_type: 98, max_health: 6.0, behavior: Behavior::Passive };
pub const PHANTOM: MobKind = MobKind { name: "phantom", entity_type: 99, max_health: 20.0, behavior: Behavior::Hostile };
pub const PIG: MobKind = MobKind { name: "pig", entity_type: 100, max_health: 10.0, behavior: Behavior::Passive };
pub const PIGLIN: MobKind = MobKind { name: "piglin", entity_type: 101, max_health: 16.0, behavior: Behavior::Neutral };
pub const PIGLIN_BRUTE: MobKind = MobKind { name: "piglin_brute", entity_type: 102, max_health: 50.0, behavior: Behavior::Hostile };
pub const PILLAGER: MobKind = MobKind { name: "pillager", entity_type: 103, max_health: 24.0, behavior: Behavior::Hostile };
pub const POLAR_BEAR: MobKind = MobKind { name: "polar_bear", entity_type: 104, max_health: 30.0, behavior: Behavior::Neutral };
pub const PUFFERFISH: MobKind = MobKind { name: "pufferfish", entity_type: 107, max_health: 3.0, behavior: Behavior::Passive };
pub const RABBIT: MobKind = MobKind { name: "rabbit", entity_type: 108, max_health: 3.0, behavior: Behavior::Passive };
pub const RAVAGER: MobKind = MobKind { name: "ravager", entity_type: 109, max_health: 100.0, behavior: Behavior::Hostile };
pub const SALMON: MobKind = MobKind { name: "salmon", entity_type: 110, max_health: 3.0, behavior: Behavior::Passive };
pub const SHEEP: MobKind = MobKind { name: "sheep", entity_type: 111, max_health: 8.0, behavior: Behavior::Passive };
pub const SHULKER: MobKind = MobKind { name: "shulker", entity_type: 112, max_health: 30.0, behavior: Behavior::Hostile };
pub const SILVERFISH: MobKind = MobKind { name: "silverfish", entity_type: 114, max_health: 8.0, behavior: Behavior::Hostile };
pub const SKELETON: MobKind = MobKind { name: "skeleton", entity_type: 115, max_health: 20.0, behavior: Behavior::Hostile };
pub const SKELETON_HORSE: MobKind = MobKind { name: "skeleton_horse", entity_type: 116, max_health: 15.0, behavior: Behavior::Passive };
pub const SLIME: MobKind = MobKind { name: "slime", entity_type: 117, max_health: 16.0, behavior: Behavior::Hostile };
pub const SNIFFER: MobKind = MobKind { name: "sniffer", entity_type: 119, max_health: 14.0, behavior: Behavior::Passive };
pub const SNOW_GOLEM: MobKind = MobKind { name: "snow_golem", entity_type: 121, max_health: 4.0, behavior: Behavior::Passive };
pub const SPIDER: MobKind = MobKind { name: "spider", entity_type: 124, max_health: 16.0, behavior: Behavior::Hostile };
pub const SQUID: MobKind = MobKind { name: "squid", entity_type: 127, max_health: 10.0, behavior: Behavior::Passive };
pub const STRAY: MobKind = MobKind { name: "stray", entity_type: 128, max_health: 20.0, behavior: Behavior::Hostile };
pub const STRIDER: MobKind = MobKind { name: "strider", entity_type: 129, max_health: 20.0, behavior: Behavior::Passive };
pub const SULFUR_CUBE: MobKind = MobKind { name: "sulfur_cube", entity_type: 130, max_health: 16.0, behavior: Behavior::Hostile }; // new in 26.2
pub const TADPOLE: MobKind = MobKind { name: "tadpole", entity_type: 131, max_health: 6.0, behavior: Behavior::Passive };
pub const TRADER_LLAMA: MobKind = MobKind { name: "trader_llama", entity_type: 135, max_health: 30.0, behavior: Behavior::Neutral };
pub const TROPICAL_FISH: MobKind = MobKind { name: "tropical_fish", entity_type: 137, max_health: 3.0, behavior: Behavior::Passive };
pub const TURTLE: MobKind = MobKind { name: "turtle", entity_type: 138, max_health: 30.0, behavior: Behavior::Passive };
pub const VEX: MobKind = MobKind { name: "vex", entity_type: 139, max_health: 14.0, behavior: Behavior::Hostile };
pub const VILLAGER: MobKind = MobKind { name: "villager", entity_type: 140, max_health: 20.0, behavior: Behavior::Passive };
pub const VINDICATOR: MobKind = MobKind { name: "vindicator", entity_type: 141, max_health: 24.0, behavior: Behavior::Hostile };
pub const WANDERING_TRADER: MobKind = MobKind { name: "wandering_trader", entity_type: 142, max_health: 20.0, behavior: Behavior::Passive };
pub const WARDEN: MobKind = MobKind { name: "warden", entity_type: 143, max_health: 500.0, behavior: Behavior::Hostile };
pub const WITCH: MobKind = MobKind { name: "witch", entity_type: 145, max_health: 26.0, behavior: Behavior::Hostile };
pub const WITHER: MobKind = MobKind { name: "wither", entity_type: 146, max_health: 300.0, behavior: Behavior::Hostile };
pub const WITHER_SKELETON: MobKind = MobKind { name: "wither_skeleton", entity_type: 147, max_health: 20.0, behavior: Behavior::Hostile };
pub const WOLF: MobKind = MobKind { name: "wolf", entity_type: 149, max_health: 8.0, behavior: Behavior::Neutral };
pub const ZOGLIN: MobKind = MobKind { name: "zoglin", entity_type: 150, max_health: 40.0, behavior: Behavior::Hostile };
pub const ZOMBIE: MobKind = MobKind { name: "zombie", entity_type: 151, max_health: 20.0, behavior: Behavior::Hostile };
pub const ZOMBIE_HORSE: MobKind = MobKind { name: "zombie_horse", entity_type: 152, max_health: 15.0, behavior: Behavior::Passive };
pub const ZOMBIE_NAUTILUS: MobKind = MobKind { name: "zombie_nautilus", entity_type: 153, max_health: 20.0, behavior: Behavior::Neutral }; // new in 26.2
pub const ZOMBIE_VILLAGER: MobKind = MobKind { name: "zombie_villager", entity_type: 154, max_health: 20.0, behavior: Behavior::Hostile };
pub const ZOMBIFIED_PIGLIN: MobKind = MobKind { name: "zombified_piglin", entity_type: 155, max_health: 20.0, behavior: Behavior::Neutral };

/// All mob kinds, for spawning and the showcase.
pub const ALL_MOBS: &[&MobKind] = &[
    &ALLAY, &ARMADILLO, &AXOLOTL, &BAT, &BEE, &BLAZE,
    &BOGGED, &BREEZE, &CAMEL, &CAMEL_HUSK, &CAT, &CAVE_SPIDER,
    &CHICKEN, &COD, &COPPER_GOLEM, &COW, &CREAKING, &CREEPER,
    &DOLPHIN, &DONKEY, &DROWNED, &ELDER_GUARDIAN, &ENDER_DRAGON, &ENDERMAN,
    &ENDERMITE, &EVOKER, &FOX, &FROG, &GHAST, &GIANT,
    &GLOW_SQUID, &GOAT, &GUARDIAN, &HAPPY_GHAST, &HOGLIN, &HORSE,
    &HUSK, &ILLUSIONER, &IRON_GOLEM, &LLAMA, &MAGMA_CUBE, &MOOSHROOM,
    &MULE, &NAUTILUS, &OCELOT, &PANDA, &PARCHED, &PARROT,
    &PHANTOM, &PIG, &PIGLIN, &PIGLIN_BRUTE, &PILLAGER, &POLAR_BEAR,
    &PUFFERFISH, &RABBIT, &RAVAGER, &SALMON, &SHEEP, &SHULKER,
    &SILVERFISH, &SKELETON, &SKELETON_HORSE, &SLIME, &SNIFFER, &SNOW_GOLEM,
    &SPIDER, &SQUID, &STRAY, &STRIDER, &SULFUR_CUBE, &TADPOLE,
    &TRADER_LLAMA, &TROPICAL_FISH, &TURTLE, &VEX, &VILLAGER, &VINDICATOR,
    &WANDERING_TRADER, &WARDEN, &WITCH, &WITHER, &WITHER_SKELETON, &WOLF,
    &ZOGLIN, &ZOMBIE, &ZOMBIE_HORSE, &ZOMBIE_NAUTILUS, &ZOMBIE_VILLAGER, &ZOMBIFIED_PIGLIN,
];

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

/// Chase AI: hostiles target the player within this range; chasing runs at this
/// speed and stops at melee range. Neutrals chase while angry (after provoked).
const DETECT_RANGE: f64 = 16.0;
const CHASE_SPEED: f64 = 0.23;
const STOP_DISTANCE: f64 = 1.6;
/// How long a provoked neutral mob stays angry and chases (ticks ≈ 10s).
const AGGRO_TICKS: u32 = 200;

/// Mob melee: an aggressive mob within reach hits the player for this much, this
/// often (a single default for now; per-mob attack values come later).
const MOB_ATTACK_DAMAGE: f32 = 3.0;
const MOB_ATTACK_INTERVAL: u32 = 20; // ticks between hits (~1s)
const MOB_REACH: f64 = STOP_DISTANCE + 0.4;

/// Knockback when hit: a shove away from the attacker plus a small upward pop
/// (blocks per tick), then gravity and ground friction bring it to rest.
const KNOCKBACK_H: f64 = 0.4;
const KNOCKBACK_V: f64 = 0.4;
const GRAVITY: f64 = 0.08;
/// Horizontal velocity retained per tick (ground friction): knockback slides out.
const FRICTION: f64 = 0.6;
/// The flat world's surface: mobs rest here and land back on it.
const GROUND_Y: f64 = 64.0;
/// Below this horizontal speed (and on the ground) the mob is "settled" again.
const SETTLED_SPEED: f64 = 0.05;

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
    /// Velocity (blocks/tick), used for knockback; decays via gravity + friction.
    vx: f64,
    vy: f64,
    vz: f64,
    /// Current movement target (x, z), if the mob is heading somewhere.
    target: Option<(f64, f64)>,
    /// Ticks to wait before picking the next stroll target.
    idle_ticks: u32,
    /// Ticks of panic (fleeing fast) left after being hurt.
    panic_ticks: u32,
    /// Ticks a provoked neutral mob stays angry and chases the player.
    anger_ticks: u32,
    /// Ticks until this mob can melee the player again.
    attack_cooldown: u32,
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
            y: GROUND_Y,
            z,
            vx: 0.0,
            vy: 0.0,
            vz: 0.0,
            target: None,
            idle_ticks: 0,
            panic_ticks: 0,
            anger_ticks: 0,
            attack_cooldown: 0,
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

    /// One of every mob kind, laid out in a grid east of spawn so they can all be
    /// seen at once. Entity ids start at 2 (the player is 1). Temporary showcase
    /// until the spawning engine drives population.
    pub fn showcase() -> Vec<Mob> {
        const PER_ROW: usize = 10;
        const SPACING: f64 = 3.0;
        ALL_MOBS
            .iter()
            .enumerate()
            .map(|(i, kind)| {
                let col = (i % PER_ROW) as f64;
                let row = (i / PER_ROW) as f64;
                let x = 4.0 + col * SPACING;
                let z = -(PER_ROW as f64) * SPACING / 2.0 + row * SPACING;
                Mob::new(kind, 2 + i as i32, x, z)
            })
            .collect()
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

    /// Applies knockback away from `(from_x, from_z)`: a horizontal shove plus a
    /// small upward pop. Gravity and friction in `tick` bring it back to rest.
    pub fn knockback(&mut self, from_x: f64, from_z: f64) {
        let (dx, dz) = (self.x - from_x, self.z - from_z);
        let len = (dx * dx + dz * dz).sqrt().max(0.001);
        self.vx = dx / len * KNOCKBACK_H;
        self.vz = dz / len * KNOCKBACK_H;
        self.vy = KNOCKBACK_V;
    }

    /// Reacts to being hurt according to the mob's behaviour: passive mobs panic
    /// and flee; neutral mobs get angry and chase; hostile mobs already chase.
    pub fn provoke(&mut self) {
        match self.kind.behavior {
            Behavior::Passive => {
                let span = PANIC_MAX_TICKS - PANIC_MIN_TICKS;
                self.panic_ticks = PANIC_MIN_TICKS + (self.rand01() * span as f64) as u32;
                self.pick_flee_target();
            }
            Behavior::Neutral | Behavior::Hostile => {
                self.anger_ticks = AGGRO_TICKS;
            }
        }
    }

    /// If this mob is aggressive, within reach of the player `(px, pz)` and its
    /// attack has recharged, returns the melee damage to deal (and starts the
    /// cooldown). Otherwise `None`.
    pub fn melee_damage(&mut self, px: f64, pz: f64) -> Option<f32> {
        let aggressive = match self.kind.behavior {
            Behavior::Hostile => true,
            Behavior::Neutral => self.anger_ticks > 0,
            Behavior::Passive => false,
        };
        if !aggressive || self.is_dying() || self.attack_cooldown > 0 {
            return None;
        }
        if (self.x - px).hypot(self.z - pz) <= MOB_REACH {
            self.attack_cooldown = MOB_ATTACK_INTERVAL;
            Some(MOB_ATTACK_DAMAGE)
        } else {
            None
        }
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

    /// Turns toward `heading` (degrees): the head leads, the body eases to follow,
    /// and the head stays within 75° of the body (vanilla-style).
    fn face(&mut self, heading: f64) {
        self.head_yaw = approach_angle(self.head_yaw, heading, HEAD_TURN);
        self.body_yaw += angle_diff(self.body_yaw, self.head_yaw) * BODY_FOLLOW;
        let off = angle_diff(self.body_yaw, self.head_yaw);
        if off.abs() > MAX_HEAD_YAW {
            self.body_yaw = self.head_yaw - MAX_HEAD_YAW * off.signum();
        }
    }

    /// Picks the next flee point while panicking: a short dash in a fully random
    /// direction, so the mob bolts around frantically rather than in one line.
    fn pick_flee_target(&mut self) {
        let angle = self.rand01() * std::f64::consts::TAU;
        let dist = FLEE_MIN_DASH + self.rand01() * (FLEE_MAX_DASH - FLEE_MIN_DASH);
        self.target = Some((self.x + angle.cos() * dist, self.z + angle.sin() * dist));
    }

    /// Advances the mob one tick: knockback physics, then AI (chase the player,
    /// flee in panic, or stroll), then send its position and head rotation. `px`/
    /// `pz` are the player's position. Returns `true` once the mob should be
    /// dropped (death animation finished). Position sync avoids rounding drift.
    pub async fn tick<W: AsyncWrite + Unpin>(
        &mut self,
        writer: &mut W,
        px: f64,
        pz: f64,
    ) -> Result<bool> {
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
        if self.attack_cooldown > 0 {
            self.attack_cooldown -= 1;
        }
        let panicking = self.panic_ticks > 0;
        if panicking {
            self.panic_ticks -= 1;
        }

        // Knockback physics: slide with the current velocity, fall under gravity,
        // land on the ground, and lose horizontal speed to friction.
        self.x += self.vx;
        self.y += self.vy;
        self.z += self.vz;
        self.vy -= GRAVITY;
        if self.y <= GROUND_Y {
            self.y = GROUND_Y;
            if self.vy < 0.0 {
                self.vy = 0.0;
            }
        }
        self.vx *= FRICTION;
        self.vz *= FRICTION;
        // While still being shoved (airborne or sliding) the mob can't steer.
        let knocked = self.y > GROUND_Y || self.vx.hypot(self.vz) > SETTLED_SPEED;
        if knocked {
            return self.send_position(writer).await.map(|()| false);
        }

        // Choose a goal. Chasing (hostile in range, or an angry neutral) beats
        // panicking (passive fleeing), which beats strolling.
        let dist_player = (self.x - px).hypot(self.z - pz);
        if self.anger_ticks > 0 {
            self.anger_ticks -= 1;
        }
        let chasing = match self.kind.behavior {
            Behavior::Hostile => dist_player <= DETECT_RANGE,
            Behavior::Neutral => self.anger_ticks > 0 && dist_player <= DETECT_RANGE,
            Behavior::Passive => false,
        };

        let speed = if chasing {
            self.target = Some((px, pz));
            CHASE_SPEED
        } else if panicking {
            if self.target.is_none() {
                self.pick_flee_target();
            }
            PANIC_SPEED
        } else {
            if self.target.is_none() {
                if self.idle_ticks > 0 {
                    self.idle_ticks -= 1;
                } else {
                    self.pick_wander_target();
                }
            }
            WALK_SPEED
        };

        // Steer toward the target and move when roughly facing it.
        if let Some((tx, tz)) = self.target {
            let (dx, dz) = (tx - self.x, tz - self.z);
            let dist = (dx * dx + dz * dz).sqrt();
            // Chasers stop at melee range; others when they reach the spot.
            let stop = if chasing { STOP_DISTANCE } else { REACH_DISTANCE };
            if dist <= stop {
                if chasing {
                    // Reached the player: face them but don't overrun.
                    self.face(-dx.atan2(dz).to_degrees());
                } else {
                    self.target = None;
                    if !panicking {
                        self.idle_ticks = 20 + (self.rand01() * 60.0) as u32;
                    }
                }
            } else {
                // Yaw toward the movement direction (0° = +z south, -90° = +x east).
                let heading = -dx.atan2(dz).to_degrees();
                self.face(heading);
                if angle_diff(self.body_yaw, heading).abs() < ALIGN_THRESHOLD {
                    let step = speed.min(dist);
                    self.x += dx / dist * step;
                    self.z += dz / dist * step;
                }
            }
        } else {
            // Idle: let the head drift back to face the body.
            self.head_yaw = approach_angle(self.head_yaw, self.body_yaw, IDLE_HEAD_RELAX);
        }

        self.send_position(writer).await?;
        Ok(false)
    }

    /// Sends the mob's current absolute position (and body/head yaw) to the client.
    async fn send_position<W: AsyncWrite + Unpin>(&self, writer: &mut W) -> Result<()> {
        let mut sync = PacketWriter::new(P_ENTITY_POSITION_SYNC);
        sync.write_varint(self.entity_id);
        sync.write_f64(self.x).write_f64(self.y).write_f64(self.z);
        sync.write_f64(0.0).write_f64(0.0).write_f64(0.0); // velocity (delta)
        sync.write_f32(self.body_yaw as f32).write_f32(0.0); // yaw (body), pitch
        sync.write_bool(self.y <= GROUND_Y); // on_ground
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
