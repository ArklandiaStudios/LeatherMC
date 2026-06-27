//! Mojang's world-generation RNG, reimplemented in Rust (our own code).
//!
//! Since 1.18 vanilla seeds its generator with **Xoroshiro128++**, and derives
//! the 128-bit state from a 64-bit world seed via a fixed mixing function. To
//! generate the *same* world as vanilla for a given seed, this bit-for-bit
//! behaviour has to match. The algorithm is public (it's a well-known PRNG);
//! only the constants and step order are reproduced here, not any Mojang code.

const GOLDEN_RATIO: u64 = 0x9E37_79B9_7F4A_7C15;
const SILVER_RATIO: u64 = 0x6A09_E667_F3BC_C909;

/// Stafford variant 13 mix (used by `upgrade_seed`).
fn mix_stafford13(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Expands a 64-bit seed into the 128-bit Xoroshiro state, as vanilla does.
pub fn upgrade_seed(seed: i64) -> (u64, u64) {
    let lo = (seed as u64) ^ SILVER_RATIO;
    let hi = lo.wrapping_add(GOLDEN_RATIO);
    (mix_stafford13(lo), mix_stafford13(hi))
}

/// The Xoroshiro128++ generator.
#[derive(Clone)]
pub struct Xoroshiro {
    lo: u64,
    hi: u64,
}

impl Xoroshiro {
    pub fn new(lo: u64, hi: u64) -> Self {
        // The all-zero state is invalid; vanilla substitutes the ratio constants.
        if lo == 0 && hi == 0 {
            Self {
                lo: GOLDEN_RATIO,
                hi: SILVER_RATIO,
            }
        } else {
            Self { lo, hi }
        }
    }

    /// The next 64-bit output, advancing the state (the `++` scrambler).
    pub fn next_long(&mut self) -> i64 {
        let l = self.lo;
        let m = self.hi;
        let result = (l.wrapping_add(m)).rotate_left(17).wrapping_add(l);
        let m = m ^ l;
        self.lo = l.rotate_left(49) ^ m ^ (m << 21);
        self.hi = m.rotate_left(28);
        result as i64
    }
}

/// A vanilla `XoroshiroRandomSource`: a Xoroshiro plus the standard integer and
/// double helpers, matching vanilla's bit layout.
#[derive(Clone)]
pub struct WorldgenRandom {
    inner: Xoroshiro,
}

impl WorldgenRandom {
    /// Seeds from a 64-bit world seed (via the 128-bit upgrade).
    pub fn from_seed(seed: i64) -> Self {
        let (lo, hi) = upgrade_seed(seed);
        Self {
            inner: Xoroshiro::new(lo, hi),
        }
    }

    /// Constructs directly from a 128-bit state (used by positional seeding).
    pub fn from_state(lo: u64, hi: u64) -> Self {
        Self {
            inner: Xoroshiro::new(lo, hi),
        }
    }

    pub fn next_long(&mut self) -> i64 {
        self.inner.next_long()
    }

    /// The top `bits` bits of the next output.
    fn next_bits(&mut self, bits: u32) -> u64 {
        (self.inner.next_long() as u64) >> (64 - bits)
    }

    /// A full 32-bit int (the low 32 bits of the next output).
    pub fn next_int(&mut self) -> i32 {
        self.inner.next_long() as i32
    }

    /// A uniform int in `[0, bound)`, using vanilla's (Lemire-style) algorithm.
    pub fn next_int_bound(&mut self, bound: i32) -> i32 {
        debug_assert!(bound > 0);
        let bound = bound as u64;
        let mut l = (self.next_int() as u32) as u64;
        let mut m = l * bound;
        let mut n = m & 0xFFFF_FFFF;
        if n < bound {
            let threshold = (bound.wrapping_neg() % bound) & 0xFFFF_FFFF;
            while n < threshold {
                l = (self.next_int() as u32) as u64;
                m = l * bound;
                n = m & 0xFFFF_FFFF;
            }
        }
        (m >> 32) as i32
    }

    /// A double in `[0, 1)`.
    pub fn next_double(&mut self) -> f64 {
        self.next_bits(53) as f64 * 1.110_223_024_625_156_5e-16 // 2^-53
    }

    /// Forks a positional factory (used to seed each noise by name).
    pub fn fork_positional(&mut self) -> PositionalFactory {
        PositionalFactory {
            lo: self.gen_next(),
            hi: self.gen_next(),
        }
    }

    fn gen_next(&mut self) -> u64 {
        self.inner.next_long() as u64
    }
}

/// Vanilla's `XoroshiroPositionalRandomFactory`: derives a fresh random source
/// from a name by MD5-hashing it and XOR-ing into the factory's state.
pub struct PositionalFactory {
    lo: u64,
    hi: u64,
}

impl PositionalFactory {
    /// A random source seeded from `name` (e.g. `"minecraft:octave_-7"`).
    pub fn from_hash_of(&self, name: &str) -> WorldgenRandom {
        let hash = super::md5::md5(name.as_bytes());
        let l = u64::from_be_bytes(hash[0..8].try_into().unwrap());
        let m = u64::from_be_bytes(hash[8..16].try_into().unwrap());
        WorldgenRandom::from_state(l ^ self.lo, m ^ self.hi)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_for_a_seed() {
        let mut a = WorldgenRandom::from_seed(12345);
        let mut b = WorldgenRandom::from_seed(12345);
        for _ in 0..100 {
            assert_eq!(a.next_long(), b.next_long());
        }
    }

    #[test]
    fn different_seeds_diverge() {
        let mut a = WorldgenRandom::from_seed(1);
        let mut b = WorldgenRandom::from_seed(2);
        assert_ne!(a.next_long(), b.next_long());
    }

    #[test]
    fn next_double_in_unit_range() {
        let mut r = WorldgenRandom::from_seed(42);
        for _ in 0..1000 {
            let d = r.next_double();
            assert!((0.0..1.0).contains(&d));
        }
    }

    #[test]
    fn next_int_bound_in_range() {
        let mut r = WorldgenRandom::from_seed(7);
        for _ in 0..1000 {
            let v = r.next_int_bound(256);
            assert!((0..256).contains(&v));
        }
    }
}
