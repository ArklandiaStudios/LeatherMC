//! Builds the Configuration-state **Update Tags** packet from the vanilla tag
//! JSON files.
//!
//! A tag is a named list of registry entries (e.g. `minecraft:infiniburn_overworld`
//! in `minecraft:block`). On the wire each entry is a numeric index into its
//! registry:
//!
//! - built-in registries (block, item, entity_type, …) use the ids from
//!   `registries.json`;
//! - the registries we stream ourselves (dimension_type, biome, …) use the
//!   index of the entry in our **sorted** entry list — the same order the server
//!   sends, so the indices line up.
//!
//! Tag values may reference other tags (`#minecraft:other`); those are expanded
//! recursively. Tags for registries we don't know how to index are skipped.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::io::{Cursor, Read};
use std::path::Path;

use anyhow::{Context, Result};
use leather_protocol::PacketWriter;
use serde_json::Value;
use zip::ZipArchive;

const PKT_UPDATE_TAGS: i32 = 13;

/// Reads the tag JSONs from `inner`, resolves them to numeric indices, and
/// writes the full Update Tags packet body to `<out>/tags.bin`. Returns the
/// number of tags written.
pub fn write_tags(
    inner: &mut ZipArchive<Cursor<Vec<u8>>>,
    out: &Path,
    registries_json_path: &str,
    dynamic_entries: &BTreeMap<String, Vec<String>>,
) -> Result<usize> {
    let index = build_index(registries_json_path, dynamic_entries)?;

    // Known registry paths (without the "minecraft:" prefix), longest first so
    // multi-segment ids like `worldgen/biome` win over shorter prefixes.
    let mut registry_paths: Vec<String> = index.keys().map(|k| strip_ns(k).to_string()).collect();
    registry_paths.sort_by_key(|p| std::cmp::Reverse(p.len()));

    // raw[registry_id][tag_id] = list of JSON values.
    let mut raw: BTreeMap<String, BTreeMap<String, Vec<Value>>> = BTreeMap::new();
    for i in 0..inner.len() {
        let mut entry = inner.by_index(i)?;
        let name = entry.name().to_string();
        let Some(rest) = name.strip_prefix("data/minecraft/tags/") else {
            continue;
        };
        let Some(rest) = rest.strip_suffix(".json") else {
            continue;
        };
        let Some((registry, tag)) = split_registry(rest, &registry_paths) else {
            continue; // a registry we don't index (worldgen/structure, …)
        };

        let mut json = String::new();
        entry.read_to_string(&mut json)?;
        let value: Value =
            serde_json::from_str(&json).with_context(|| format!("parsing {name}"))?;
        let values = value
            .get("values")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        raw.entry(registry).or_default().insert(tag, values);
    }

    // Resolve and encode.
    let mut writer = PacketWriter::new(PKT_UPDATE_TAGS);
    let registries: Vec<&String> = raw.keys().collect();
    writer.write_varint(registries.len() as i32);

    let mut tag_count = 0usize;
    for registry in registries {
        let tags = &raw[registry];
        let lookup = &index[registry];
        writer.write_string(registry);
        writer.write_varint(tags.len() as i32);
        for tag in tags.keys() {
            let mut cache = HashMap::new();
            let mut stack = HashSet::new();
            let indices = resolve(tags, lookup, tag, &mut cache, &mut stack);
            writer.write_string(tag);
            writer.write_varint(indices.len() as i32);
            for idx in indices {
                writer.write_varint(idx);
            }
            tag_count += 1;
        }
    }

    std::fs::create_dir_all(out)?;
    std::fs::write(out.join("tags.bin"), writer.into_body())?;
    Ok(tag_count)
}

/// Builds `registry_id -> (entry_id -> index)` for every registry we can index.
fn build_index(
    registries_json_path: &str,
    dynamic_entries: &BTreeMap<String, Vec<String>>,
) -> Result<HashMap<String, HashMap<String, i32>>> {
    let text = std::fs::read_to_string(registries_json_path)
        .with_context(|| format!("reading {registries_json_path}"))?;
    let json: Value = serde_json::from_str(&text)?;

    let mut index: HashMap<String, HashMap<String, i32>> = HashMap::new();

    // Built-in registries from registries.json.
    if let Some(obj) = json.as_object() {
        for (registry, body) in obj {
            if let Some(entries) = body.get("entries").and_then(Value::as_object) {
                let map = entries
                    .iter()
                    .filter_map(|(id, e)| {
                        e.get("protocol_id")
                            .and_then(Value::as_i64)
                            .map(|n| (id.clone(), n as i32))
                    })
                    .collect();
                index.insert(registry.clone(), map);
            }
        }
    }

    // Our streamed registries: index = position in the sorted entry list.
    for (registry, ids) in dynamic_entries {
        let map = ids
            .iter()
            .enumerate()
            .map(|(i, id)| (id.clone(), i as i32))
            .collect();
        index.insert(registry.clone(), map);
    }

    Ok(index)
}

/// Resolves a tag to its entry indices, expanding `#tag` references.
fn resolve(
    tags: &BTreeMap<String, Vec<Value>>,
    lookup: &HashMap<String, i32>,
    tag: &str,
    cache: &mut HashMap<String, Vec<i32>>,
    stack: &mut HashSet<String>,
) -> Vec<i32> {
    if let Some(cached) = cache.get(tag) {
        return cached.clone();
    }
    if !stack.insert(tag.to_string()) {
        return Vec::new(); // cycle guard
    }

    let mut out = Vec::new();
    let mut seen = HashSet::new();
    if let Some(values) = tags.get(tag) {
        for value in values {
            let entry = match value {
                Value::String(s) => Some(s.as_str()),
                Value::Object(o) => o.get("id").and_then(Value::as_str),
                _ => None,
            };
            let Some(entry) = entry else { continue };

            if let Some(referenced) = entry.strip_prefix('#') {
                for idx in resolve(tags, lookup, referenced, cache, stack) {
                    if seen.insert(idx) {
                        out.push(idx);
                    }
                }
            } else if let Some(&idx) = lookup.get(entry)
                && seen.insert(idx)
            {
                out.push(idx);
            }
        }
    }

    stack.remove(tag);
    cache.insert(tag.to_string(), out.clone());
    out
}

/// Splits `regpath/tag` into `(registry_id, tag_id)` using the known registry
/// paths (longest match first).
fn split_registry(rest: &str, registry_paths: &[String]) -> Option<(String, String)> {
    for path in registry_paths {
        if let Some(tag) = rest.strip_prefix(path).and_then(|r| r.strip_prefix('/')) {
            return Some((format!("minecraft:{path}"), format!("minecraft:{tag}")));
        }
    }
    None
}

fn strip_ns(id: &str) -> &str {
    id.strip_prefix("minecraft:").unwrap_or(id)
}
