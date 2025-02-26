//! Error code for signaling error while processing a host function.
//!
//! API inspired by `std::io::Error` and `std::io::ErrorKind` but somewhat more memory efficient.

#[derive(Debug, PartialEq)]
#[non_exhaustive]
pub enum Error {
    /// An entity was not found, often a missing key in the global state.
    NotFound,
    /// Data not valid for the operation were encountered.
    ///
    /// As an example this could be a malformed parameter that does not contain a valid UTF-8.
    InvalidData,
    /// The input to the host function was invalid.
    InvalidInput,
    /// The topic is too long.
    TopicTooLong,
    /// Too many topics.
    TooManyTopics,
    /// The payload is too long.
    PayloadTooLong,
    /// The message topic is full and cannot accept new messages.
    MessageTopicFull,
    /// The maximum number of messages emitted per block was exceeded when trying to emit a
    /// message.
    MaxMessagesPerBlockExceeded,
    /// An error code not covered by the other variants.
    Other(i32),
}

pub const HOST_ERROR_SUCCEED: i32 = 0;
pub const HOST_ERROR_NOT_FOUND: i32 = 1;
pub const HOST_ERROR_INVALID_DATA: i32 = 2;
pub const HOST_ERROR_INVALID_INPUT: i32 = 3;
pub const HOST_ERROR_TOPIC_TOO_LONG: i32 = 4;
pub const HOST_ERROR_TOO_MANY_TOPICS: i32 = 5;
pub const HOST_ERROR_PAYLOAD_TOO_LONG: i32 = 6;
pub const HOST_ERROR_MESSAGE_TOPIC_FULL: i32 = 7;
pub const HOST_ERROR_MAX_MESSAGES_PER_BLOCK_EXCEEDED: i32 = 8;

impl From<i32> for Error {
    fn from(value: i32) -> Self {
        match value {
            HOST_ERROR_NOT_FOUND => Error::NotFound,
            HOST_ERROR_INVALID_DATA => Error::InvalidData,
            HOST_ERROR_INVALID_INPUT => Error::InvalidInput,
            HOST_ERROR_TOPIC_TOO_LONG => Error::TopicTooLong,
            HOST_ERROR_TOO_MANY_TOPICS => Error::TooManyTopics,
            HOST_ERROR_PAYLOAD_TOO_LONG => Error::PayloadTooLong,
            HOST_ERROR_MESSAGE_TOPIC_FULL => Error::MessageTopicFull,
            HOST_ERROR_MAX_MESSAGES_PER_BLOCK_EXCEEDED => Error::MaxMessagesPerBlockExceeded,
            other => Error::Other(other),
        }
    }
}

pub fn result_from_code(code: i32) -> Result<(), Error> {
    match code {
        HOST_ERROR_SUCCEED => Ok(()),
        other => Err(Error::from(other)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_i32_not_found() {
        let error = Error::from(HOST_ERROR_NOT_FOUND);
        assert_eq!(error, Error::NotFound);
    }

    #[test]
    fn test_from_i32_invalid_data() {
        let error = Error::from(HOST_ERROR_INVALID_DATA);
        assert_eq!(error, Error::InvalidData);
    }

    #[test]
    fn test_from_i32_invalid_input() {
        let error = Error::from(HOST_ERROR_INVALID_INPUT);
        assert_eq!(error, Error::InvalidInput);
    }

    #[test]
    fn test_from_i32_other() {
        let error = Error::from(4);
        assert_eq!(error, Error::Other(4));
    }
}
