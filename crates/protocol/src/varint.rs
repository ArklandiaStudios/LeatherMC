//! VarInt encoding — the variable-length 32-bit integer used everywhere in the
//! Minecraft protocol (packet length, packet id, enum fields, string lengths…).
//!
//! Each byte carries 7 bits of payload; the top bit ("continue bit") signals
//! that more bytes follow. A VarInt is at most 5 bytes long.

use tokio::io::{AsyncRead, AsyncReadExt};

use crate::error::{ProtocolError, Result};

const SEGMENT_BITS: u8 = 0x7F;
const CONTINUE_BIT: u8 = 0x80;

/// Reads a VarInt asynchronously from a stream (one byte at a time).
pub async fn read_varint<R: AsyncRead + Unpin>(reader: &mut R) -> Result<i32> {
    let mut value: i32 = 0;
    let mut position: u32 = 0;

    loop {
        let byte = reader.read_u8().await?;
        value |= ((byte & SEGMENT_BITS) as i32) << position;

        if byte & CONTINUE_BIT == 0 {
            break;
        }

        position += 7;
        if position >= 32 {
            return Err(ProtocolError::VarIntTooBig);
        }
    }

    Ok(value)
}

/// Appends a VarInt to a byte buffer.
pub fn write_varint(buf: &mut Vec<u8>, mut value: i32) {
    loop {
        if value & !(SEGMENT_BITS as i32) == 0 {
            buf.push(value as u8);
            return;
        }
        buf.push((value as u8 & SEGMENT_BITS) | CONTINUE_BIT);
        // Logical (unsigned) shift so the sign bit doesn't smear.
        value = ((value as u32) >> 7) as i32;
    }
}

/// Number of bytes a given value occupies when encoded as a VarInt.
pub fn varint_len(value: i32) -> usize {
    let mut v = value as u32;
    let mut len = 1;
    while v & !(SEGMENT_BITS as u32) != 0 {
        v >>= 7;
        len += 1;
    }
    len
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_known_values() {
        // (value, expected encoding) from the protocol reference.
        let cases: &[(i32, &[u8])] = &[
            (0, &[0x00]),
            (1, &[0x01]),
            (127, &[0x7f]),
            (128, &[0x80, 0x01]),
            (255, &[0xff, 0x01]),
            (25565, &[0xdd, 0xc7, 0x01]),
            (2147483647, &[0xff, 0xff, 0xff, 0xff, 0x07]),
            (-1, &[0xff, 0xff, 0xff, 0xff, 0x0f]),
        ];

        for (value, expected) in cases {
            let mut buf = Vec::new();
            write_varint(&mut buf, *value);
            assert_eq!(&buf, expected, "encoding {value}");
            assert_eq!(varint_len(*value), expected.len(), "len {value}");
        }
    }

    #[tokio::test]
    async fn async_read_matches_write() {
        for value in [0, 1, 127, 128, 25565, i32::MAX, -1] {
            let mut buf = Vec::new();
            write_varint(&mut buf, value);
            let mut slice = &buf[..];
            let got = read_varint(&mut slice).await.unwrap();
            assert_eq!(got, value);
        }
    }
}
