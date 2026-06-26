//! Loads the generated registry NBT files (produced by `leather-datagen`) that
//! the server streams to clients during the Configuration state.
//!
//! Directory layout (relative to the configured registries dir):
//!
//! ```text
//! dimension_type/overworld.nbt
//! worldgen/biome/plains.nbt
//! ...
//! ```
//!
//! A registry's id is its directory path (e.g. `minecraft:worldgen/biome`); an
//! entry's id is the file stem (e.g. `minecraft:plains`).

use std::collections::HashMap;
use std::path::Path;

use leather_protocol::PacketReader;

/// One registry entry: its id and the network-NBT blob to send verbatim.
pub struct RegistryEntry {
    pub id: String,
    pub nbt: Vec<u8>,
}

/// One registry (e.g. `minecraft:dimension_type`) and its entries, in a stable
/// order (sorted by id) so registry indices are deterministic.
pub struct Registry {
    pub id: String,
    pub entries: Vec<RegistryEntry>,
}

#[derive(Default)]
pub struct Registries {
    pub list: Vec<Registry>,
    /// Pre-encoded Update Tags packet body (id + payload), produced by
    /// `leather-datagen`. Empty if no `tags.bin` is present.
    pub tags: Vec<u8>,
    /// item id -> default block state id, for placing the held block.
    pub item_to_block: HashMap<i32, i32>,
}

impl Registries {
    /// Loads all registries under `root`. A missing directory yields an empty
    /// set (logged by the caller) rather than an error.
    pub fn load(root: &Path) -> std::io::Result<Self> {
        let mut files: Vec<(String, String, Vec<u8>)> = Vec::new();
        if root.is_dir() {
            collect(root, root, &mut files)?;
        }
        let tags = std::fs::read(root.join("tags.bin")).unwrap_or_default();
        let item_to_block = load_item_to_block(root);

        // Group by registry id.
        let mut by_registry: std::collections::BTreeMap<String, Vec<RegistryEntry>> =
            std::collections::BTreeMap::new();
        for (registry, entry, nbt) in files {
            by_registry
                .entry(format!("minecraft:{registry}"))
                .or_default()
                .push(RegistryEntry {
                    id: format!("minecraft:{entry}"),
                    nbt,
                });
        }

        let mut list: Vec<Registry> = by_registry
            .into_iter()
            .map(|(id, mut entries)| {
                entries.sort_by(|a, b| a.id.cmp(&b.id));
                Registry { id, entries }
            })
            .collect();
        list.sort_by(|a, b| a.id.cmp(&b.id));

        Ok(Self {
            list,
            tags,
            item_to_block,
        })
    }

    /// Total entry count, for logging.
    pub fn entry_count(&self) -> usize {
        self.list.iter().map(|r| r.entries.len()).sum()
    }

    /// Index of `entry_id` within the named registry, as sent to the client.
    /// Returns `None` if absent.
    pub fn index_of(&self, registry_id: &str, entry_id: &str) -> Option<i32> {
        let registry = self.list.iter().find(|r| r.id == registry_id)?;
        registry
            .entries
            .iter()
            .position(|e| e.id == entry_id)
            .map(|i| i as i32)
    }
}

/// Loads the `item_to_block.bin` table (flat VarInt pairs item_id, state_id).
fn load_item_to_block(root: &Path) -> HashMap<i32, i32> {
    let mut map = HashMap::new();
    let Ok(bytes) = std::fs::read(root.join("item_to_block.bin")) else {
        return map;
    };
    let mut reader = PacketReader::new(bytes);
    while let (Ok(item), Ok(state)) = (reader.read_varint(), reader.read_varint()) {
        map.insert(item, state);
    }
    map
}

/// Recursively collects `*.nbt` files, recording `(registry_dir, file_stem, bytes)`
/// where `registry_dir` is the path relative to `root` using `/` separators.
fn collect(
    root: &Path,
    dir: &Path,
    out: &mut Vec<(String, String, Vec<u8>)>,
) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect(root, &path, out)?;
        } else if path.extension().is_some_and(|e| e == "nbt") {
            let registry = path
                .parent()
                .and_then(|p| p.strip_prefix(root).ok())
                .map(|p| p.to_string_lossy().replace('\\', "/"))
                .unwrap_or_default();
            let stem = path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned();
            let bytes = std::fs::read(&path)?;
            out.push((registry, stem, bytes));
        }
    }
    Ok(())
}
