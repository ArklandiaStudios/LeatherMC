//! Packet framing and primitive field reading/writing.
//!
//! On the wire (uncompressed), a packet is:
//!
//! ```text
//! +----------------+------------+----------------+
//! | length: VarInt | id: VarInt | data: bytes... |
//! +----------------+------------+----------------+
//!   length = byte count of (id + data)
//! ```
//!
//! [`read_frame`] pulls one whole packet body (`id + data`) off the stream and
//! hands back a [`PacketReader`] positioned at the id. [`PacketWriter`] builds a
//! body in memory; [`write_frame`] prepends the length and flushes it.

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::error::{ProtocolError, Result};
use crate::varint::{read_varint, write_varint};

/// Hard cap on packet body size, mirroring the vanilla server's limit, to keep
/// a malicious client from making us allocate gigabytes.
const MAX_FRAME_LEN: usize = 2 * 1024 * 1024;

/// Reads one complete packet body off the stream.
///
/// Returns a [`PacketReader`] over `id + data`. The caller reads the id first.
pub async fn read_frame<R: AsyncRead + Unpin>(reader: &mut R) -> Result<PacketReader> {
    let len = read_varint(reader).await?;
    if len < 0 {
        return Err(ProtocolError::Invalid(format!(
            "negative frame length {len}"
        )));
    }
    let len = len as usize;
    if len > MAX_FRAME_LEN {
        return Err(ProtocolError::FrameTooLarge(len));
    }

    let mut buf = vec![0u8; len];
    reader.read_exact(&mut buf).await?;
    Ok(PacketReader::new(buf))
}

/// Writes one complete packet (length-prefixed) to the stream.
pub async fn write_frame<W: AsyncWrite + Unpin>(writer: &mut W, body: &[u8]) -> Result<()> {
    let mut header = Vec::with_capacity(5);
    write_varint(&mut header, body.len() as i32);
    writer.write_all(&header).await?;
    writer.write_all(body).await?;
    writer.flush().await?;
    Ok(())
}

/// Cursor over an in-memory packet body, reading primitive protocol types.
pub struct PacketReader {
    data: Vec<u8>,
    pos: usize,
}

impl PacketReader {
    pub fn new(data: Vec<u8>) -> Self {
        Self { data, pos: 0 }
    }

    fn take(&mut self, n: usize) -> Result<&[u8]> {
        if self.pos + n > self.data.len() {
            return Err(ProtocolError::UnexpectedEof);
        }
        let slice = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(slice)
    }

    pub fn read_u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }

    pub fn read_u16(&mut self) -> Result<u16> {
        let b = self.take(2)?;
        Ok(u16::from_be_bytes([b[0], b[1]]))
    }

    pub fn read_i64(&mut self) -> Result<i64> {
        let b = self.take(8)?;
        Ok(i64::from_be_bytes(b.try_into().unwrap()))
    }

    pub fn read_f64(&mut self) -> Result<f64> {
        let b = self.take(8)?;
        Ok(f64::from_be_bytes(b.try_into().unwrap()))
    }

    /// Reads a 16-byte UUID as a single `u128` (big-endian).
    pub fn read_uuid(&mut self) -> Result<u128> {
        let b = self.take(16)?;
        Ok(u128::from_be_bytes(b.try_into().unwrap()))
    }

    /// Reads a VarInt from the in-memory buffer (synchronous variant).
    pub fn read_varint(&mut self) -> Result<i32> {
        let mut value: i32 = 0;
        let mut position: u32 = 0;
        loop {
            let byte = self.read_u8()?;
            value |= ((byte & 0x7F) as i32) << position;
            if byte & 0x80 == 0 {
                break;
            }
            position += 7;
            if position >= 32 {
                return Err(ProtocolError::VarIntTooBig);
            }
        }
        Ok(value)
    }

    /// Reads a length-prefixed UTF-8 string.
    pub fn read_string(&mut self) -> Result<String> {
        let len = self.read_varint()?;
        if len < 0 {
            return Err(ProtocolError::Invalid(format!(
                "negative string length {len}"
            )));
        }
        let bytes = self.take(len as usize)?.to_vec();
        Ok(String::from_utf8(bytes)?)
    }
}

/// Builds a packet body (`id + data`) in memory.
pub struct PacketWriter {
    buf: Vec<u8>,
}

impl PacketWriter {
    /// Starts a new packet with the given id.
    pub fn new(packet_id: i32) -> Self {
        let mut buf = Vec::new();
        write_varint(&mut buf, packet_id);
        Self { buf }
    }

    pub fn write_u8(&mut self, value: u8) -> &mut Self {
        self.buf.push(value);
        self
    }

    pub fn write_bool(&mut self, value: bool) -> &mut Self {
        self.buf.push(u8::from(value));
        self
    }

    pub fn write_i8(&mut self, value: i8) -> &mut Self {
        self.buf.push(value as u8);
        self
    }

    pub fn write_i32(&mut self, value: i32) -> &mut Self {
        self.buf.extend_from_slice(&value.to_be_bytes());
        self
    }

    pub fn write_f32(&mut self, value: f32) -> &mut Self {
        self.buf.extend_from_slice(&value.to_be_bytes());
        self
    }

    pub fn write_f64(&mut self, value: f64) -> &mut Self {
        self.buf.extend_from_slice(&value.to_be_bytes());
        self
    }

    pub fn write_u16(&mut self, value: u16) -> &mut Self {
        self.buf.extend_from_slice(&value.to_be_bytes());
        self
    }

    pub fn write_i64(&mut self, value: i64) -> &mut Self {
        self.buf.extend_from_slice(&value.to_be_bytes());
        self
    }

    /// Writes a `u128` as a 16-byte UUID (big-endian).
    pub fn write_uuid(&mut self, value: u128) -> &mut Self {
        self.buf.extend_from_slice(&value.to_be_bytes());
        self
    }

    pub fn write_varint(&mut self, value: i32) -> &mut Self {
        write_varint(&mut self.buf, value);
        self
    }

    /// Appends raw bytes verbatim (e.g. a pre-encoded NBT blob).
    pub fn write_bytes(&mut self, bytes: &[u8]) -> &mut Self {
        self.buf.extend_from_slice(bytes);
        self
    }

    pub fn write_string(&mut self, value: &str) -> &mut Self {
        write_varint(&mut self.buf, value.len() as i32);
        self.buf.extend_from_slice(value.as_bytes());
        self
    }

    /// Consumes the writer, returning the finished body.
    pub fn into_body(self) -> Vec<u8> {
        self.buf
    }
}
