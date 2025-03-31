//! Error code for signaling error while processing a host function.
//!
//! API inspired by `std::io::Error` and `std::io::ErrorKind` but somewhat more memory efficient.

#[derive(Debug, Default, PartialEq)]
#[non_exhaustive]
#[repr(u32)]
pub enum CommonResult {
    #[default]
    Success = 0,
    /// An entity was not found, often a missing key in the global state.
    NotFound = 1,
    /// Data not valid for the operation were encountered.
    ///
    /// As an example this could be a malformed parameter that does not contain a valid UTF-8.
    InvalidData = 2,
    /// The input to the host function was invalid.
    InvalidInput = 3,
    /// The topic is too long.
    TopicTooLong = 4,
    /// Too many topics.
    TooManyTopics = 5,
    /// The payload is too long.
    PayloadTooLong = 6,
    /// The message topic is full and cannot accept new messages.
    MessageTopicFull = 7,
    /// The maximum number of messages emitted per block was exceeded when trying to emit a
    /// message.
    MaxMessagesPerBlockExceeded = 8,
    /// Internal error (for example, failed to acquire a lock)
    Internal = 9,
    /// An error code not covered by the other variants.
    Other(u32),
}

pub const HOST_ERROR_SUCCESS: u32 = 0;
pub const HOST_ERROR_NOT_FOUND: u32 = 1;
pub const HOST_ERROR_INVALID_DATA: u32 = 2;
pub const HOST_ERROR_INVALID_INPUT: u32 = 3;
pub const HOST_ERROR_TOPIC_TOO_LONG: u32 = 4;
pub const HOST_ERROR_TOO_MANY_TOPICS: u32 = 5;
pub const HOST_ERROR_PAYLOAD_TOO_LONG: u32 = 6;
pub const HOST_ERROR_MESSAGE_TOPIC_FULL: u32 = 7;
pub const HOST_ERROR_MAX_MESSAGES_PER_BLOCK_EXCEEDED: u32 = 8;
pub const HOST_ERROR_INTERNAL: u32 = 9;

impl From<u32> for CommonResult {
    fn from(value: u32) -> Self {
        match value {
            HOST_ERROR_SUCCESS => Self::Success,
            HOST_ERROR_NOT_FOUND => Self::NotFound,
            HOST_ERROR_INVALID_DATA => Self::InvalidData,
            HOST_ERROR_INVALID_INPUT => Self::InvalidInput,
            HOST_ERROR_TOPIC_TOO_LONG => Self::TopicTooLong,
            HOST_ERROR_TOO_MANY_TOPICS => Self::TooManyTopics,
            HOST_ERROR_PAYLOAD_TOO_LONG => Self::PayloadTooLong,
            HOST_ERROR_MESSAGE_TOPIC_FULL => Self::MessageTopicFull,
            HOST_ERROR_MAX_MESSAGES_PER_BLOCK_EXCEEDED => Self::MaxMessagesPerBlockExceeded,
            HOST_ERROR_INTERNAL => Self::Internal,
            other => Self::Other(other),
        }
    }
}

pub fn result_from_code(code: u32) -> Result<(), CommonResult> {
    match code {
        HOST_ERROR_SUCCESS => Ok(()),
        other => Err(CommonResult::from(other)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_u32_not_found() {
        let error = CommonResult::from(HOST_ERROR_NOT_FOUND);
        assert_eq!(error, CommonResult::NotFound);
    }

    #[test]
    fn test_from_u32_invalid_data() {
        let error = CommonResult::from(HOST_ERROR_INVALID_DATA);
        assert_eq!(error, CommonResult::InvalidData);
    }

    #[test]
    fn test_from_u32_invalid_input() {
        let error = CommonResult::from(HOST_ERROR_INVALID_INPUT);
        assert_eq!(error, CommonResult::InvalidInput);
    }

    #[test]
    fn test_from_u32_other() {
        let error = CommonResult::from(4);
        assert_eq!(error, CommonResult::Other(4));
    }
}
