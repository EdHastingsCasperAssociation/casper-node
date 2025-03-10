use casper_types::{addressable_entity::MessageTopics, Digest, EntityAddr};

use crate::tracking_copy::TrackingCopyError;

/// Request for a message topics.
pub struct MessageTopicsRequest {
    state_hash: Digest,
    entity_addr: EntityAddr,
}

impl MessageTopicsRequest {
    /// Creates new request object.
    pub fn new(state_hash: Digest, entity_addr: EntityAddr) -> Self {
        Self {
            state_hash,
            entity_addr,
        }
    }

    /// Returns state root hash.
    pub fn state_hash(&self) -> Digest {
        self.state_hash
    }

    /// Returns the hash addr.
    pub fn entity_addr(&self) -> EntityAddr {
        self.entity_addr
    }
}

/// Result of a global state query request.
#[derive(Debug)]
pub enum MessageTopicsResult {
    /// Invalid state root hash.
    RootNotFound,
    /// Successful query.
    Success {
        /// Stored value under a path.
        message_topics: MessageTopics,
    },
    /// Tracking Copy Error
    Failure(TrackingCopyError),
}
