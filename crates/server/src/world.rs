//! The world's block edits, shared across connections and saved to disk.
//!
//! Only *changes* to the flat base world are stored (placed/broken blocks),
//! keyed by chunk so a chunk can fetch its edits cheaply. The on-disk format is
//! a flat sequence of VarInt quadruples `(x, y, z, state)`.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

use leather_protocol::{PacketReader, write_varint};

/// Global block position.
pub type BlockPos = (i32, i32, i32);
/// One chunk's block edits (global pos -> state id).
pub type ChunkEdits = HashMap<BlockPos, i32>;

#[derive(Default)]
pub struct World {
    edits: Mutex<HashMap<(i32, i32), ChunkEdits>>,
}

impl World {
    /// Records a block change.
    pub fn set_block(&self, x: i32, y: i32, z: i32, state: i32) {
        let chunk = (x.div_euclid(16), z.div_euclid(16));
        self.edits
            .lock()
            .unwrap()
            .entry(chunk)
            .or_default()
            .insert((x, y, z), state);
    }

    /// Snapshot of a chunk's edits (global pos -> state).
    pub fn chunk_edits(&self, cx: i32, cz: i32) -> ChunkEdits {
        self.edits
            .lock()
            .unwrap()
            .get(&(cx, cz))
            .cloned()
            .unwrap_or_default()
    }

    /// Total number of edited blocks (for logging).
    pub fn block_count(&self) -> usize {
        self.edits.lock().unwrap().values().map(HashMap::len).sum()
    }

    /// Loads edits from `path`; a missing file yields an empty world.
    pub fn load(path: &Path) -> Self {
        let world = Self::default();
        if let Ok(bytes) = std::fs::read(path) {
            let mut reader = PacketReader::new(bytes);
            while let (Ok(x), Ok(y), Ok(z), Ok(state)) = (
                reader.read_varint(),
                reader.read_varint(),
                reader.read_varint(),
                reader.read_varint(),
            ) {
                world.set_block(x, y, z, state);
            }
        }
        world
    }

    /// Saves all edits to `path`.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let mut buf = Vec::new();
        for chunk_map in self.edits.lock().unwrap().values() {
            for (&(x, y, z), &state) in chunk_map {
                write_varint(&mut buf, x);
                write_varint(&mut buf, y);
                write_varint(&mut buf, z);
                write_varint(&mut buf, state);
            }
        }
        std::fs::write(path, buf)
    }
}
