//! Definition of all the possible outcomes of the operation on an `EngineState` instance.
use datasize::DataSize;
use thiserror::Error;

use casper_storage::{system::transfer::TransferError, tracking_copy::TrackingCopyError};
use casper_types::{bytesrepr, system::mint, ApiError, Digest, Key, ProtocolVersion};

use super::InvalidRequest;
use crate::{
    execution::ExecError,
    runtime::{stack, PreprocessingError},
};

/// Engine state errors.
#[derive(Clone, Error, Debug)]
#[non_exhaustive]
pub enum Error {
    /// Specified state root hash is not found.
    #[error("Root not found: {0}")]
    RootNotFound(Digest),
    /// Protocol version used in the deploy is invalid.
    #[error("Invalid protocol version: {0}")]
    InvalidProtocolVersion(ProtocolVersion),
    /// WASM preprocessing error.
    #[error("Wasm preprocessing error: {0}")]
    WasmPreprocessing(#[from] PreprocessingError),
    /// Contract execution error.
    #[error(transparent)]
    Exec(ExecError),
    /// Serialization/deserialization error.
    #[error("Bytesrepr error: {0}")]
    Bytesrepr(String),
    /// Mint error.
    #[error("Mint error: {0}")]
    Mint(String),
    /// Invalid key variant.
    #[error("Unsupported key type: {0}")]
    InvalidKeyVariant(Key),
    /// An attempt to push to the runtime stack while already at the maximum height.
    #[error("Runtime stack overflow")]
    RuntimeStackOverflow,
    /// Storage error.
    #[error("Tracking copy error: {0}")]
    TrackingCopy(TrackingCopyError),
    /// Native transfer error.
    #[error("Transfer error: {0}")]
    Transfer(TransferError),
    /// Could not derive a valid item to execute.
    #[error("Invalid executable item: {0}")]
    InvalidExecutableItem(#[from] InvalidRequest),
}

impl Error {
    /// Creates an [`enum@Error`] instance of an [`Error::Exec`] variant with an API
    /// error-compatible object.
    ///
    /// This method should be used only by native code that has to mimic logic of a WASM executed
    /// code.
    pub fn reverter(api_error: impl Into<ApiError>) -> Error {
        Error::Exec(ExecError::Revert(api_error.into()))
    }
}

impl From<TransferError> for Error {
    fn from(err: TransferError) -> Self {
        Error::Transfer(err)
    }
}

impl From<ExecError> for Error {
    fn from(error: ExecError) -> Self {
        match error {
            ExecError::WasmPreprocessing(preprocessing_error) => {
                Error::WasmPreprocessing(preprocessing_error)
            }
            _ => Error::Exec(error),
        }
    }
}

impl From<bytesrepr::Error> for Error {
    fn from(error: bytesrepr::Error) -> Self {
        Error::Bytesrepr(format!("{}", error))
    }
}

impl From<mint::Error> for Error {
    fn from(error: mint::Error) -> Self {
        Error::Mint(format!("{}", error))
    }
}

impl From<stack::RuntimeStackOverflow> for Error {
    fn from(_: stack::RuntimeStackOverflow) -> Self {
        Self::RuntimeStackOverflow
    }
}

impl From<TrackingCopyError> for Error {
    fn from(e: TrackingCopyError) -> Self {
        Error::TrackingCopy(e)
    }
}

impl DataSize for Error {
    const IS_DYNAMIC: bool = true;

    const STATIC_HEAP_SIZE: usize = 0;

    #[inline]
    fn estimate_heap_size(&self) -> usize {
        12 // TODO: replace with some actual estimation depending on the variant
    }
}
