use crate::global_state::error::Error as GlobalStateError;

/// Request to flush state.
pub struct FlushRequest {}

impl FlushRequest {
    /// Returns a new instance of FlushRequest.
    pub fn new() -> Self {
        FlushRequest {}
    }
}

impl Default for FlushRequest {
    fn default() -> Self {
        FlushRequest::new()
    }
}

/// Represents a result of a `flush` request.
pub enum FlushResult {
    /// Manual sync is disabled in config settings.
    ManualSyncDisabled,
    /// Successfully flushed.
    Success,
    /// Failed to flush.
    Failure(GlobalStateError),
}

impl FlushResult {
    /// Flush succeeded
    pub fn flushed(&self) -> bool {
        matches!(self, FlushResult::Success)
    }

    /// Transforms flush result to global state error, if relevant.
    pub fn as_error(self) -> Result<(), GlobalStateError> {
        match self {
            FlushResult::ManualSyncDisabled | FlushResult::Success => Ok(()),
            FlushResult::Failure(gse) => Err(gse),
        }
    }
}
