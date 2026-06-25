//! Minecraft wire-protocol primitives for LeatherMC.
//!
//! This crate is deliberately small and version-agnostic. It only knows how to
//! read and write the *framing* of the Minecraft protocol — VarInts, length
//! prefixes, and the primitive field types that appear in every packet since
//! the modern protocol (Netty rewrite). Packet *meaning* lives in the server
//! crate, which builds on top of these primitives.

#![deny(unsafe_code)]

mod error;
mod packet;
mod varint;

pub use error::{ProtocolError, Result};
pub use packet::{PacketReader, PacketWriter, read_frame, write_frame};
pub use varint::{read_varint, varint_len, write_varint};

/// Connection states of the handshake state machine.
///
/// A fresh TCP connection starts in `Handshake`; the first packet tells us
/// whether the client wants `Status` (server-list ping) or `Login`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Handshake,
    Status,
    Login,
}

impl State {
    /// Maps the `next_state` field of a handshake packet to a [`State`].
    pub fn from_next_state(value: i32) -> Result<Self> {
        match value {
            1 => Ok(State::Status),
            2 => Ok(State::Login),
            other => Err(ProtocolError::Invalid(format!(
                "unknown next_state in handshake: {other}"
            ))),
        }
    }
}
