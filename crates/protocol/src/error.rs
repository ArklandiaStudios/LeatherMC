use thiserror::Error;

/// Errors that can occur while reading or writing protocol frames.
#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("VarInt is too big (more than 5 bytes)")]
    VarIntTooBig,

    #[error("packet body is too large: {0} bytes")]
    FrameTooLarge(usize),

    #[error("unexpected end of packet buffer")]
    UnexpectedEof,

    #[error("invalid UTF-8 string: {0}")]
    InvalidString(#[from] std::string::FromUtf8Error),

    #[error("invalid packet: {0}")]
    Invalid(String),
}

pub type Result<T> = std::result::Result<T, ProtocolError>;
