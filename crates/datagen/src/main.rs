//! `leather-datagen` — extracts vanilla registry data from a Mojang server jar
//! and writes it as the NBT files (and tags) LeatherMC loads at runtime.
//!
//! Usage:
//!
//! ```text
//! leather-datagen <path/to/server.jar> <output-dir> [registries.json]
//! ```
//!
//! `registries.json` is produced by the vanilla data generator
//! (`java -DbundlerMainClass=net.minecraft.data.Main -jar server.jar --reports`,
//! then `generated/reports/registries.json`). It supplies the numeric ids of the
//! built-in registries (block, item, …) needed to encode tags. Without it, only
//! registry NBT is written (no tags).
//!
//! The vanilla data is Mojang's; it is intentionally **not** committed to this
//! repository. Each server operator runs this tool once against their own jar.

#![deny(unsafe_code)]

mod convert;
mod tags;

use std::collections::BTreeMap;
use std::io::{Cursor, Read};
use std::path::Path;

use anyhow::{Context, Result, bail};
use leather_protocol::write_network_nbt;
use zip::ZipArchive;

use convert::json_to_nbt;

/// The registries the server sends to clients during the Configuration state,
/// for Minecraft 26.2 (protocol 776). Slashes are kept (e.g. `worldgen/biome`).
pub const SYNCED_REGISTRIES: &[&str] = &[
    "banner_pattern",
    "cat_sound_variant",
    "cat_variant",
    "chat_type",
    "chicken_sound_variant",
    "chicken_variant",
    "cow_sound_variant",
    "cow_variant",
    "damage_type",
    "dialog",
    "dimension_type",
    "enchantment",
    "frog_variant",
    "instrument",
    "jukebox_song",
    "painting_variant",
    "pig_sound_variant",
    "pig_variant",
    "sulfur_cube_archetype",
    "test_environment",
    "test_instance",
    "timeline",
    "trim_material",
    "trim_pattern",
    "wolf_sound_variant",
    "wolf_variant",
    "world_clock",
    "worldgen/biome",
    "zombie_nautilus_variant",
];

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let (jar, out) = match (args.next(), args.next()) {
        (Some(jar), Some(out)) => (jar, out),
        _ => bail!("usage: leather-datagen <server.jar> <output-dir> [registries.json]"),
    };
    let registries_json = args.next();

    let mut inner = open_inner_jar(&jar).context("opening the bundled server jar")?;
    let out = Path::new(&out);

    // `entries` maps each synced registry id -> its entry ids, sorted, so we and
    // the server agree on the index a tag refers to.
    let (count, entries) = convert_registries(&mut inner, out)?;
    println!("wrote {count} registry entries");

    match registries_json {
        Some(path) => {
            let n = tags::write_tags(&mut inner, out, &path, &entries)
                .context("building Update Tags")?;
            println!("wrote {n} tags");
        }
        None => println!("no registries.json given — skipping tags"),
    }

    println!("done: {}", out.display());
    Ok(())
}

/// Opens the Mojang bundler jar and returns the inner `server-<version>.jar`
/// (which holds the datapack) as an in-memory zip archive.
fn open_inner_jar(jar: &str) -> Result<ZipArchive<Cursor<Vec<u8>>>> {
    let file = std::fs::File::open(jar).with_context(|| format!("opening {jar}"))?;
    let mut bundler = ZipArchive::new(file)?;

    // META-INF/versions.list lines: "<sha>\t<version>\t<relative path>".
    let mut list = String::new();
    bundler
        .by_name("META-INF/versions.list")
        .context("not a bundled Mojang server jar (missing versions.list)")?
        .read_to_string(&mut list)?;
    let rel = list
        .lines()
        .next()
        .and_then(|line| line.split('\t').nth(2))
        .context("could not parse versions.list")?;

    let inner_name = format!("META-INF/versions/{rel}");
    let mut inner_bytes = Vec::new();
    bundler
        .by_name(&inner_name)
        .with_context(|| format!("missing inner jar {inner_name}"))?
        .read_to_end(&mut inner_bytes)?;

    Ok(ZipArchive::new(Cursor::new(inner_bytes))?)
}

/// Converts every synced registry's JSON entries to NBT files under `out`.
/// Returns the total count and, per registry id, the sorted list of entry ids.
fn convert_registries(
    inner: &mut ZipArchive<Cursor<Vec<u8>>>,
    out: &Path,
) -> Result<(usize, BTreeMap<String, Vec<String>>)> {
    let mut total = 0usize;
    let mut entries: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for i in 0..inner.len() {
        let mut entry = inner.by_index(i)?;
        let name = entry.name().to_string();
        let Some((registry, id)) = match_registry(&name) else {
            continue;
        };

        let mut json = String::new();
        entry.read_to_string(&mut json)?;
        let value: serde_json::Value =
            serde_json::from_str(&json).with_context(|| format!("parsing {name}"))?;
        let nbt = json_to_nbt(&value)
            .with_context(|| format!("{name} is not a JSON object/value we can encode"))?;

        let mut bytes = Vec::new();
        write_network_nbt(&mut bytes, &nbt);

        let dir = out.join(registry);
        std::fs::create_dir_all(&dir)?;
        std::fs::write(dir.join(format!("{id}.nbt")), &bytes)?;
        entries
            .entry(format!("minecraft:{registry}"))
            .or_default()
            .push(format!("minecraft:{id}"));
        total += 1;
    }

    if total == 0 {
        bail!("no registry entries found — is this the right server jar?");
    }
    for ids in entries.values_mut() {
        ids.sort();
    }
    Ok((total, entries))
}

/// If `name` is a synced-registry datapack file
/// (`data/minecraft/<registry>/<id>.json`), returns `(registry, id)`.
fn match_registry(name: &str) -> Option<(&'static str, String)> {
    for registry in SYNCED_REGISTRIES {
        let prefix = format!("data/minecraft/{registry}/");
        if let Some(rest) = name.strip_prefix(&prefix)
            && let Some(id) = rest.strip_suffix(".json")
            // Skip anything in a deeper subdirectory.
            && !id.contains('/')
        {
            return Some((registry, id.to_string()));
        }
    }
    None
}
