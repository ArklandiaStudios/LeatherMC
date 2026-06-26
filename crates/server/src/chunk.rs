//! Encodes a flat chunk, applying any block edits from the world.
//!
//! Sections with no edits stay *uniform* (a single-value palette, no data).
//! Edited sections use a real paletted container: an indirect palette when few
//! distinct blocks (4–8 bits/entry), or a direct palette otherwise.

use std::collections::HashMap;

use leather_protocol::{PacketWriter, write_varint};

const PKT_LEVEL_CHUNK: i32 = 45;

/// Overworld is 384 blocks tall (min_y -64) -> 24 sections of 16.
const SECTION_COUNT: usize = 24;
const MIN_Y: i32 = -64;
/// Sections up to y=63 are solid stone (the floor); above is air.
const STONE_TOP_Y: i32 = 63;

const STATE_AIR: i32 = 0;
const STATE_STONE: i32 = 1;
/// Bits per entry for a direct palette (ceil(log2) of the block-state count).
const DIRECT_BITS: u8 = 15;

/// Light data is 2048 bytes per section (4096 nibbles). 0xFF = full (15).
const LIGHT_ARRAY_LEN: usize = 2048;

/// Builds the full `level_chunk_with_light` packet body for one chunk,
/// applying `edits` (global block pos -> state) belonging to this chunk.
pub fn flat_chunk(
    chunk_x: i32,
    chunk_z: i32,
    biome_index: i32,
    edits: &HashMap<(i32, i32, i32), i32>,
) -> Vec<u8> {
    let mut w = PacketWriter::new(PKT_LEVEL_CHUNK);
    w.write_i32(chunk_x);
    w.write_i32(chunk_z);

    // Heightmaps (1.21.5+): 3 entries, each a packed long array. Zeros are accepted.
    w.write_varint(3);
    for index in [1, 4, 5] {
        w.write_varint(index);
        w.write_varint(37);
        for _ in 0..37 {
            w.write_i64(0);
        }
    }

    let mut sections = Vec::new();
    for s in 0..SECTION_COUNT {
        let section_min_y = MIN_Y + (s as i32) * 16;
        let base = if section_min_y + 15 <= STONE_TOP_Y {
            STATE_STONE
        } else {
            STATE_AIR
        };

        // Block edits inside this section, as (local index, state).
        let mut local: Vec<(usize, i32)> = Vec::new();
        for (&(x, y, z), &state) in edits {
            if y >= section_min_y && y < section_min_y + 16 {
                let lx = x.rem_euclid(16) as usize;
                let lz = z.rem_euclid(16) as usize;
                let ly = (y - section_min_y) as usize;
                local.push(((ly * 16 + lz) * 16 + lx, state)); // YZX order
            }
        }

        let (block_container, non_air) = if local.is_empty() {
            let non_air = if base == STATE_AIR { 0 } else { 4096 };
            (single_value_container(base), non_air)
        } else {
            let mut blocks = vec![base; 4096];
            for (idx, state) in local {
                blocks[idx] = state;
            }
            encode_block_section(&blocks)
        };

        sections.extend_from_slice(&(non_air as i16).to_be_bytes()); // non-air count
        sections.extend_from_slice(&0i16.to_be_bytes()); // liquid count (26.1+)
        sections.extend_from_slice(&block_container);
        sections.extend_from_slice(&single_value_container(biome_index)); // biome
    }
    w.write_varint(sections.len() as i32);
    w.write_bytes(&sections);

    w.write_varint(0); // block entities

    write_light(&mut w);
    w.into_body()
}

/// A single-value paletted container: 0 bits per entry + one palette value.
fn single_value_container(value: i32) -> Vec<u8> {
    let mut out = vec![0u8];
    write_varint(&mut out, value);
    out
}

/// Encodes a 4096-block section's paletted container, returning (bytes, non-air count).
fn encode_block_section(blocks: &[i32]) -> (Vec<u8>, i32) {
    let non_air = blocks.iter().filter(|&&b| b != STATE_AIR).count() as i32;

    // Build the palette in first-seen order.
    let mut palette: Vec<i32> = Vec::new();
    let mut index_of: HashMap<i32, usize> = HashMap::new();
    for &b in blocks {
        index_of.entry(b).or_insert_with(|| {
            palette.push(b);
            palette.len() - 1
        });
    }

    if palette.len() == 1 {
        return (single_value_container(palette[0]), non_air);
    }

    let mut out = Vec::new();
    let palette_bits = bits_for(palette.len());
    if palette_bits <= 8 {
        let bits = palette_bits.max(4); // blocks use at least 4 bits indirect
        out.push(bits);
        write_varint(&mut out, palette.len() as i32);
        for &state in &palette {
            write_varint(&mut out, state);
        }
        let indices: Vec<i32> = blocks.iter().map(|b| index_of[b] as i32).collect();
        pack_into(&mut out, &indices, bits);
    } else {
        out.push(DIRECT_BITS);
        pack_into(&mut out, blocks, DIRECT_BITS);
    }
    (out, non_air)
}

/// Bits needed to index `len` palette entries.
fn bits_for(len: usize) -> u8 {
    let mut bits = 0u8;
    while (1usize << bits) < len {
        bits += 1;
    }
    bits
}

/// Packs `values` into big-endian longs, `bits` per entry, without straddling
/// longs (the vanilla "compacted" layout).
fn pack_into(out: &mut Vec<u8>, values: &[i32], bits: u8) {
    let bits = bits as usize;
    let per_long = 64 / bits;
    let mask = (1u64 << bits) - 1;
    let num_longs = values.len().div_ceil(per_long);

    let mut longs = vec![0u64; num_longs];
    for (i, &v) in values.iter().enumerate() {
        let long_index = i / per_long;
        let offset = (i % per_long) * bits;
        longs[long_index] |= (v as u64 & mask) << offset;
    }
    for long in longs {
        out.extend_from_slice(&(long as i64).to_be_bytes());
    }
}

/// Writes the light section: full sky light for every world section, no block
/// light.
fn write_light(w: &mut PacketWriter) {
    let mut sky_mask: i64 = 0;
    for i in 1..=SECTION_COUNT {
        sky_mask |= 1 << i;
    }
    let sky_empty: i64 = (1 << 0) | (1 << (SECTION_COUNT + 1));
    let mut block_empty: i64 = 0;
    for i in 0..=(SECTION_COUNT + 1) {
        block_empty |= 1 << i;
    }

    write_bitset(w, sky_mask);
    write_bitset(w, 0);
    write_bitset(w, sky_empty);
    write_bitset(w, block_empty);

    w.write_varint(SECTION_COUNT as i32);
    let full = [0xFFu8; LIGHT_ARRAY_LEN];
    for _ in 0..SECTION_COUNT {
        w.write_varint(LIGHT_ARRAY_LEN as i32);
        w.write_bytes(&full);
    }
    w.write_varint(0); // block light arrays
}

/// A BitSet on the wire: a length-prefixed array of longs (here always one).
fn write_bitset(w: &mut PacketWriter, mask: i64) {
    w.write_varint(1);
    w.write_i64(mask);
}
