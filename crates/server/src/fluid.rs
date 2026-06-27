//! Water flow logic.
//!
//! Water spreads from source blocks: it falls into air below, and spreads
//! sideways up to 7 blocks with a decreasing level. A cell next to two or more
//! source blocks becomes a source itself (infinite water). When a source is
//! removed, flowing water with nothing feeding it recedes. Sources never change
//! on their own, so generated oceans stay put until the terrain around them is
//! altered.

use crate::world::World;

/// Water state ids: level 0 (source) = 86, flowing 1..7 = 87..93, falling = 94.
const WATER_MIN: i32 = 86;
const WATER_MAX: i32 = 101;
const SOURCE: i32 = 86;
const FALLING: i32 = 94;
const AIR: i32 = 0;
/// Furthest a flowing level reaches from its source.
const MAX_LEVEL: i32 = 7;

fn is_water(state: i32) -> bool {
    (WATER_MIN..=WATER_MAX).contains(&state)
}

/// Water "level": 0 source, 1..7 flowing, 8 falling.
fn level(state: i32) -> i32 {
    state - WATER_MIN
}

/// The state id for a flowing water level.
fn flowing(lvl: i32) -> i32 {
    WATER_MIN + lvl
}

/// Water may occupy air or other water.
fn replaceable(state: i32) -> bool {
    state == AIR || is_water(state)
}

/// The four horizontal neighbours.
const HORIZONTAL: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];

/// The cells whose water may change after `(x, y, z)` changes: itself, the four
/// sides, and the blocks above and below.
pub fn neighbours(x: i32, y: i32, z: i32) -> [(i32, i32, i32); 6] {
    [
        (x + 1, y, z),
        (x - 1, y, z),
        (x, y, z + 1),
        (x, y, z - 1),
        (x, y + 1, z),
        (x, y - 1, z),
    ]
}

/// Computes what the water at `(x, y, z)` should become given its surroundings,
/// or `None` if it shouldn't change. Sources are left untouched.
pub fn update(world: &World, x: i32, y: i32, z: i32) -> Option<i32> {
    let current = world.block_at(x, y, z);
    if !replaceable(current) || current == SOURCE {
        return None; // solid blocks and sources don't flow
    }

    let above = world.block_at(x, y + 1, z);
    let desired = if is_water(above) {
        // Water above pours down through this cell.
        FALLING
    } else {
        // Otherwise, spread from horizontal neighbours.
        let mut sources = 0;
        let mut best = MAX_LEVEL + 1;
        for (dx, dz) in HORIZONTAL {
            let n = world.block_at(x + dx, y, z + dz);
            if is_water(n) {
                let nl = level(n);
                if nl == 0 {
                    sources += 1;
                }
                // A source/falling block feeds level 1; flowing level L feeds L+1.
                let outflow = if nl == 0 || nl >= 8 { 1 } else { nl + 1 };
                best = best.min(outflow);
            }
        }
        if sources >= 2 {
            SOURCE // two adjacent sources make a new source (infinite water)
        } else if best <= MAX_LEVEL {
            flowing(best)
        } else {
            AIR // nothing feeds it: recede
        }
    };

    if desired == current { None } else { Some(desired) }
}
