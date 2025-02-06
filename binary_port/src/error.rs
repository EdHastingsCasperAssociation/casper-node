use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Invalid command tag ({0})")]
    InvalidCommandTag(u8),
    #[error("Request too large: allowed {allowed} bytes, got {got} bytes")]
    RequestTooLarge { allowed: u32, got: u32 },
    #[error("Empty request")]
    EmptyRequest,
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    BytesRepr(#[from] casper_types::bytesrepr::Error),
}
