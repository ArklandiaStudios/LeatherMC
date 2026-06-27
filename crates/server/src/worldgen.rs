//! World generation: turns coordinates into terrain.
//!
//! A first pass — our own value-noise terrain (rolling hills, dirt/grass over
//! stone, water at sea level), structured the way a vanilla-style generator is
//! (a height function plus a surface rule) so it can grow toward biomes and
//! density-function terrain later. This is original MIT code; Pumpkin's
//! generator is read only as a reference for the algorithms, never copied.

/// Block state ids (default states, from the generated `blocks.json`).
pub const AIR: i32 = 0;
pub const STONE: i32 = 1;
pub const GRASS_BLOCK: i32 = 9;
pub const DIRT: i32 = 10;
pub const BEDROCK: i32 = 85;
pub const WATER: i32 = 86;
pub const SAND: i32 = 118;

/// Sea level and world floor.
pub const SEA_LEVEL: i32 = 63;
const MIN_Y: i32 = -64;

/// Terrain shape: average height and how far hills rise/fall from it.
const BASE_HEIGHT: f64 = 66.0;
const AMPLITUDE: f64 = 18.0;
/// Horizontal scale of the noise (smaller = wider features).
const FREQUENCY: f64 = 0.01;
/// Fixed world seed for now (configurable later).
const SEED: i64 = 0x5EED_1A11;

/// The surface height (the y of the topmost solid block) at world `(x, z)`.
pub fn surface_height(x: i32, z: i32) -> i32 {
    let n = fbm(x as f64, z as f64);
    (BASE_HEIGHT + n * AMPLITUDE).round() as i32
}

/// The block state at world `(x, y, z)` given its column's surface `height`.
pub fn block_state(y: i32, height: i32) -> i32 {
    if y < MIN_Y {
        return AIR;
    }
    if y == MIN_Y {
        return BEDROCK;
    }
    if y > height {
        // Above the ground: water up to sea level, otherwise air.
        return if y <= SEA_LEVEL { WATER } else { AIR };
    }
    // At or below the surface.
    let underwater = height < SEA_LEVEL;
    if y >= height - 3 {
        if underwater {
            SAND
        } else if y == height {
            GRASS_BLOCK
        } else {
            DIRT
        }
    } else {
        STONE
    }
}

// --- Value noise (fractional Brownian motion) -------------------------------

/// Hashes integer lattice coords to a value in `[0, 1)`.
fn hash(x: i64, z: i64) -> f64 {
    let mut h = x
        .wrapping_mul(374_761_393)
        .wrapping_add(z.wrapping_mul(668_265_263))
        .wrapping_add(SEED) as u64;
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    h ^= h >> 16;
    (h & 0xFF_FFFF) as f64 / 0xFF_FFFF as f64
}

/// Smoothstep, for nicer interpolation between lattice points.
fn smooth(t: f64) -> f64 {
    t * t * (3.0 - 2.0 * t)
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// Value noise at `(x, z)`, in `[-1, 1]`.
fn value_noise(x: f64, z: f64) -> f64 {
    let (x0, z0) = (x.floor() as i64, z.floor() as i64);
    let (fx, fz) = (x - x0 as f64, z - z0 as f64);
    let (sx, sz) = (smooth(fx), smooth(fz));
    let a = lerp(hash(x0, z0), hash(x0 + 1, z0), sx);
    let b = lerp(hash(x0, z0 + 1), hash(x0 + 1, z0 + 1), sx);
    lerp(a, b, sz) * 2.0 - 1.0
}

/// Sums several octaves of value noise for natural-looking terrain.
fn fbm(x: f64, z: f64) -> f64 {
    let mut sum = 0.0;
    let mut amp = 1.0;
    let mut freq = FREQUENCY;
    let mut norm = 0.0;
    for _ in 0..4 {
        sum += value_noise(x * freq, z * freq) * amp;
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    sum / norm
}
