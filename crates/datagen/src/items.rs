//! Builds an item-id -> default-block-state-id table, so the server can place
//! the block a player is holding.
//!
//! An item maps to a block when a block shares its id (e.g. `minecraft:stone`).
//! Output is `item_to_block.bin`: a flat sequence of VarInt pairs
//! `(item_id, block_state_id)` the server loads into a map.

use std::collections::HashMap;
use std::path::Path;

use anyhow::{Context, Result};
use leather_protocol::write_varint;
use serde_json::Value;

pub fn write_item_blocks(registries_json_path: &str, out: &Path) -> Result<usize> {
    let registries: Value = read_json(registries_json_path)?;
    // blocks.json sits next to registries.json in the data generator output.
    let blocks_path = Path::new(registries_json_path).with_file_name("blocks.json");
    let blocks: Value = read_json(&blocks_path.to_string_lossy())?;

    // block id -> default state id
    let mut default_state: HashMap<&str, i64> = HashMap::new();
    if let Some(obj) = blocks.as_object() {
        for (name, body) in obj {
            if let Some(states) = body.get("states").and_then(Value::as_array) {
                let id = states
                    .iter()
                    .find(|s| s.get("default").and_then(Value::as_bool).unwrap_or(false))
                    .or_else(|| states.first())
                    .and_then(|s| s.get("id"))
                    .and_then(Value::as_i64);
                if let Some(id) = id {
                    default_state.insert(name.as_str(), id);
                }
            }
        }
    }

    let mut buf = Vec::new();
    let mut count = 0;
    if let Some(items) = registries
        .get("minecraft:item")
        .and_then(|i| i.get("entries"))
        .and_then(Value::as_object)
    {
        for (name, entry) in items {
            if let (Some(&state), Some(item_id)) = (
                default_state.get(name.as_str()),
                entry.get("protocol_id").and_then(Value::as_i64),
            ) {
                write_varint(&mut buf, item_id as i32);
                write_varint(&mut buf, state as i32);
                count += 1;
            }
        }
    }

    std::fs::write(out.join("item_to_block.bin"), &buf)?;
    Ok(count)
}

fn read_json(path: &str) -> Result<Value> {
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {path}"))?;
    Ok(serde_json::from_str(&text)?)
}
