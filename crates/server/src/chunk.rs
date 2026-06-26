//! Encodes a minimal flat chunk for the Play state.
//!
//! To stay simple we keep every section *uniform* (a single block type), which
//! makes the paletted container trivial: bits-per-entry 0, one palette value,
//! and no packed data. The lower sections are solid stone (the ground), the
//! rest is air. Full sky light is sent so the surface renders lit.

use leather_protocol::{PacketWriter, write_varint};

const PKT_LEVEL_CHUNK: i32 = 45;

/// Overworld is 384 blocks tall (min_y -64) -> 24 sections of 16.
const SECTION_COUNT: usize = 24;
/// Sections 0..8 cover y -64..63: a solid stone floor with its surface at y 63.
const STONE_SECTIONS: usize = 8;

const STATE_AIR: i32 = 0;
const STATE_STONE: i32 = 1;
const BLOCKS_PER_SECTION: i16 = 16 * 16 * 16;

/// Light data is 2048 bytes per section (4096 nibbles). 0xFF = full (15).
const LIGHT_ARRAY_LEN: usize = 2048;

/// Builds the full `level_chunk_with_light` packet body for one flat chunk.
pub fn flat_chunk(chunk_x: i32, chunk_z: i32, biome_index: i32) -> Vec<u8> {
    let mut w = PacketWriter::new(PKT_LEVEL_CHUNK);
    w.write_i32(chunk_x);
    w.write_i32(chunk_z);

    // Heightmaps (1.21.5+): a 3-entry map (world_surface=1, motion_blocking=4,
    // motion_blocking_no_leaves=5), each a packed long array. Zeros are accepted.
    w.write_varint(3);
    for index in [1, 4, 5] {
        w.write_varint(index);
        w.write_varint(37);
        for _ in 0..37 {
            w.write_i64(0);
        }
    }

    // Block + biome sections, built into a length-prefixed buffer.
    let mut sections = Vec::new();
    for s in 0..SECTION_COUNT {
        let is_stone = s < STONE_SECTIONS;
        let block_count = if is_stone { BLOCKS_PER_SECTION } else { 0 };
        let state = if is_stone { STATE_STONE } else { STATE_AIR };

        sections.extend_from_slice(&block_count.to_be_bytes()); // non-air count
        sections.extend_from_slice(&0i16.to_be_bytes()); // liquid count (26.1+)

        // Block palette: single value.
        sections.push(0); // bits per entry
        write_varint(&mut sections, state);

        // Biome palette: single value.
        sections.push(0);
        write_varint(&mut sections, biome_index);
    }
    w.write_varint(sections.len() as i32);
    w.write_bytes(&sections);

    // Block entities: none.
    w.write_varint(0);

    write_light(&mut w);
    w.into_body()
}

/// Writes the light section: full sky light for every world section, no block
/// light. Masks cover bits 0..=SECTION_COUNT+1 (the extra bits are the
/// always-empty below- and above-world sections).
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

    write_bitset(w, sky_mask); // sky light mask
    write_bitset(w, 0); // block light mask
    write_bitset(w, sky_empty); // empty sky light mask
    write_bitset(w, block_empty); // empty block light mask

    // Sky light arrays, one per set bit in the sky mask.
    w.write_varint(SECTION_COUNT as i32);
    let full = [0xFFu8; LIGHT_ARRAY_LEN];
    for _ in 0..SECTION_COUNT {
        w.write_varint(LIGHT_ARRAY_LEN as i32);
        w.write_bytes(&full);
    }

    // Block light arrays: none.
    w.write_varint(0);
}

/// A BitSet on the wire: a length-prefixed array of longs (here always one).
fn write_bitset(w: &mut PacketWriter, mask: i64) {
    w.write_varint(1);
    w.write_i64(mask);
}
