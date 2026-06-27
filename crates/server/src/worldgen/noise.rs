//! Vanilla-compatible noise: improved Perlin, octave Perlin, and normal noise.
//!
//! These reproduce the maths vanilla uses so that, seeded identically, they
//! produce identical values — the basis for matching terrain. Our own code; the
//! algorithm (improved Perlin / fBm) is public.

use super::rng::WorldgenRandom;

/// Gradient vectors for improved Perlin noise.
const GRADIENT: [[i32; 3]; 16] = [
    [1, 1, 0],
    [-1, 1, 0],
    [1, -1, 0],
    [-1, -1, 0],
    [1, 0, 1],
    [-1, 0, 1],
    [1, 0, -1],
    [-1, 0, -1],
    [0, 1, 1],
    [0, -1, 1],
    [0, 1, -1],
    [0, -1, -1],
    [1, 1, 0],
    [0, -1, 1],
    [-1, 1, 0],
    [0, -1, -1],
];

fn grad_dot(hash: i32, x: f64, y: f64, z: f64) -> f64 {
    let g = GRADIENT[(hash & 15) as usize];
    g[0] as f64 * x + g[1] as f64 * y + g[2] as f64 * z
}

/// Perlin's quintic fade curve.
fn smoothstep(t: f64) -> f64 {
    t * t * t * (t * (t * 6.0 - 15.0) + 10.0)
}

fn lerp(t: f64, a: f64, b: f64) -> f64 {
    a + t * (b - a)
}

#[allow(clippy::too_many_arguments)]
fn lerp3(
    tx: f64,
    ty: f64,
    tz: f64,
    v000: f64,
    v100: f64,
    v010: f64,
    v110: f64,
    v001: f64,
    v101: f64,
    v011: f64,
    v111: f64,
) -> f64 {
    let x00 = lerp(tx, v000, v100);
    let x10 = lerp(tx, v010, v110);
    let x01 = lerp(tx, v001, v101);
    let x11 = lerp(tx, v011, v111);
    let y0 = lerp(ty, x00, x10);
    let y1 = lerp(ty, x01, x11);
    lerp(tz, y0, y1)
}

/// Keeps coordinates in a range where f64 stays precise (vanilla's `Mth.wrap`).
fn wrap(value: f64) -> f64 {
    value - (value / 3.3554432e7 + 0.5).floor() * 3.3554432e7
}

/// A single octave of improved Perlin noise.
pub struct ImprovedNoise {
    p: [u8; 256],
    xo: f64,
    yo: f64,
    zo: f64,
}

impl ImprovedNoise {
    pub fn new(random: &mut WorldgenRandom) -> Self {
        let xo = random.next_double() * 256.0;
        let yo = random.next_double() * 256.0;
        let zo = random.next_double() * 256.0;
        let mut p = [0u8; 256];
        for (i, slot) in p.iter_mut().enumerate() {
            *slot = i as u8;
        }
        for i in 0..256 {
            let j = random.next_int_bound((256 - i) as i32) as usize;
            p.swap(i, i + j);
        }
        Self { p, xo, yo, zo }
    }

    fn perm(&self, i: i32) -> i32 {
        self.p[(i & 0xFF) as usize] as i32
    }

    pub fn noise(&self, x: f64, y: f64, z: f64) -> f64 {
        let dx = x + self.xo;
        let dy = y + self.yo;
        let dz = z + self.zo;
        let ix = dx.floor() as i32;
        let iy = dy.floor() as i32;
        let iz = dz.floor() as i32;
        let fx = dx - ix as f64;
        let fy = dy - iy as f64;
        let fz = dz - iz as f64;
        self.sample(ix, iy, iz, fx, fy, fz)
    }

    fn sample(&self, gx: i32, gy: i32, gz: i32, dx: f64, dy: f64, dz: f64) -> f64 {
        let a = self.perm(gx);
        let b = self.perm(gx + 1);
        let aa = self.perm(a + gy);
        let ab = self.perm(a + gy + 1);
        let ba = self.perm(b + gy);
        let bb = self.perm(b + gy + 1);

        let v000 = grad_dot(self.perm(aa + gz), dx, dy, dz);
        let v100 = grad_dot(self.perm(ba + gz), dx - 1.0, dy, dz);
        let v010 = grad_dot(self.perm(ab + gz), dx, dy - 1.0, dz);
        let v110 = grad_dot(self.perm(bb + gz), dx - 1.0, dy - 1.0, dz);
        let v001 = grad_dot(self.perm(aa + gz + 1), dx, dy, dz - 1.0);
        let v101 = grad_dot(self.perm(ba + gz + 1), dx - 1.0, dy, dz - 1.0);
        let v011 = grad_dot(self.perm(ab + gz + 1), dx, dy - 1.0, dz - 1.0);
        let v111 = grad_dot(self.perm(bb + gz + 1), dx - 1.0, dy - 1.0, dz - 1.0);

        let u = smoothstep(dx);
        let v = smoothstep(dy);
        let w = smoothstep(dz);
        lerp3(u, v, w, v000, v100, v010, v110, v001, v101, v011, v111)
    }
}

/// Octave Perlin noise: several improved-noise octaves with per-octave amplitudes.
pub struct PerlinNoise {
    octaves: Vec<Option<ImprovedNoise>>,
    amplitudes: Vec<f64>,
    lowest_freq_input_factor: f64,
    lowest_freq_value_factor: f64,
}

impl PerlinNoise {
    /// Builds octaves for `amplitudes`, seeding each by name as vanilla does.
    pub fn create(random: &mut WorldgenRandom, first_octave: i32, amplitudes: &[f64]) -> Self {
        let n = amplitudes.len();
        let factory = random.fork_positional();
        let mut octaves = Vec::with_capacity(n);
        for (p, &amp) in amplitudes.iter().enumerate() {
            if amp != 0.0 {
                let q = first_octave + p as i32;
                let mut nr = factory.from_hash_of(&format!("minecraft:octave_{q}"));
                octaves.push(Some(ImprovedNoise::new(&mut nr)));
            } else {
                octaves.push(None);
            }
        }
        let lowest_freq_input_factor = 2f64.powi(first_octave);
        let lowest_freq_value_factor =
            2f64.powi(n as i32 - 1) / (2f64.powi(n as i32) - 1.0);
        Self {
            octaves,
            amplitudes: amplitudes.to_vec(),
            lowest_freq_input_factor,
            lowest_freq_value_factor,
        }
    }

    pub fn get_value(&self, x: f64, y: f64, z: f64) -> f64 {
        let mut value = 0.0;
        let mut input_factor = self.lowest_freq_input_factor;
        let mut value_factor = self.lowest_freq_value_factor;
        for (i, octave) in self.octaves.iter().enumerate() {
            if let Some(noise) = octave {
                let v = noise.noise(
                    wrap(x * input_factor),
                    wrap(y * input_factor),
                    wrap(z * input_factor),
                );
                value += self.amplitudes[i] * v * value_factor;
            }
            input_factor *= 2.0;
            value_factor /= 2.0;
        }
        value
    }
}

/// Normal noise: two octave-Perlin noises combined, scaled to ~unit deviation.
/// This is what density functions reference via the `noise/*.json` parameters.
pub struct NormalNoise {
    first: PerlinNoise,
    second: PerlinNoise,
    value_factor: f64,
}

/// Offset between the two component noises (vanilla constant).
const INPUT_FACTOR: f64 = 1.018_126_888_217_522_7;

impl NormalNoise {
    pub fn create(random: &mut WorldgenRandom, first_octave: i32, amplitudes: &[f64]) -> Self {
        let first = PerlinNoise::create(random, first_octave, amplitudes);
        let second = PerlinNoise::create(random, first_octave, amplitudes);

        let mut min = i32::MAX;
        let mut max = i32::MIN;
        for (k, &amp) in amplitudes.iter().enumerate() {
            if amp != 0.0 {
                min = min.min(k as i32);
                max = max.max(k as i32);
            }
        }
        let value_factor = 0.166_666_666_666_666_66 / expected_deviation(max - min);
        Self {
            first,
            second,
            value_factor,
        }
    }

    pub fn get_value(&self, x: f64, y: f64, z: f64) -> f64 {
        let a = self.first.get_value(x, y, z);
        let b = self
            .second
            .get_value(x * INPUT_FACTOR, y * INPUT_FACTOR, z * INPUT_FACTOR);
        (a + b) * self.value_factor
    }
}

fn expected_deviation(octaves: i32) -> f64 {
    0.1 * (1.0 + 1.0 / (octaves + 1) as f64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn improved_noise_is_deterministic() {
        let mut r1 = WorldgenRandom::from_seed(123);
        let mut r2 = WorldgenRandom::from_seed(123);
        let a = ImprovedNoise::new(&mut r1);
        let b = ImprovedNoise::new(&mut r2);
        assert_eq!(a.noise(1.5, 2.5, 3.5), b.noise(1.5, 2.5, 3.5));
    }

    #[test]
    fn normal_noise_in_reasonable_range() {
        let mut r = WorldgenRandom::from_seed(987);
        let noise = NormalNoise::create(&mut r, -7, &[1.0, 1.0, 1.0, 1.0]);
        for i in 0..2000 {
            let v = noise.get_value(i as f64 * 0.1, 0.0, i as f64 * 0.07);
            assert!(v.abs() < 4.0, "value out of expected range: {v}");
        }
    }
}
