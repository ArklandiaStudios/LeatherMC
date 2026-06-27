//! The noise registry: builds a seeded [`NormalNoise`] for every noise the
//! generator references, from Mojang's extracted `noise/*.json` parameters.
//!
//! Vanilla seeds each noise by hashing its id from one positional factory forked
//! off the world seed, so this reproduces that exactly: `from_seed(seed)` ->
//! `fork_positional()` -> `from_hash_of("minecraft:<id>")` -> `NormalNoise`.

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;

use super::noise::NormalNoise;
use super::rng::WorldgenRandom;

/// Mojang's `NoiseParameters` JSON: a first octave and per-octave amplitudes.
#[derive(Deserialize)]
struct NoiseParameters {
    #[serde(rename = "firstOctave")]
    first_octave: i32,
    amplitudes: Vec<f64>,
}

/// All noises, keyed by id (e.g. `"minecraft:continentalness"`).
#[derive(Default)]
pub struct Noises {
    map: HashMap<String, NormalNoise>,
}

impl Noises {
    /// Loads every `*.json` under `noise_dir` and builds its seeded normal noise.
    pub fn load(noise_dir: &Path, seed: i64) -> std::io::Result<Self> {
        let mut base = WorldgenRandom::from_seed(seed);
        let factory = base.fork_positional();

        let mut files = Vec::new();
        if noise_dir.is_dir() {
            collect_json(noise_dir, noise_dir, &mut files)?;
        }

        let mut map = HashMap::new();
        for (id, path) in files {
            let text = std::fs::read_to_string(&path)?;
            let Ok(params) = serde_json::from_str::<NoiseParameters>(&text) else {
                continue;
            };
            let key = format!("minecraft:{id}");
            let mut random = factory.from_hash_of(&key);
            let noise = NormalNoise::create(&mut random, params.first_octave, &params.amplitudes);
            map.insert(key, noise);
        }
        Ok(Self { map })
    }

    pub fn len(&self) -> usize {
        self.map.len()
    }

    pub fn is_empty(&self) -> bool {
        self.map.is_empty()
    }

    /// The noise registered under `id` (e.g. `"minecraft:erosion"`).
    pub fn get(&self, id: &str) -> Option<&NormalNoise> {
        self.map.get(id)
    }
}

/// Recursively collects `*.json` files, returning `(id, path)` where `id` is the
/// path under `root` without the `.json` suffix (e.g. `nether/temperature`).
fn collect_json(root: &Path, dir: &Path, out: &mut Vec<(String, std::path::PathBuf)>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect_json(root, &path, out)?;
        } else if path.extension().is_some_and(|e| e == "json") {
            let id = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .with_extension("")
                .to_string_lossy()
                .replace('\\', "/");
            out.push((id, path));
        }
    }
    Ok(())
}
