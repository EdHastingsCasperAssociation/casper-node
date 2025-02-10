use crate::{data_access_layer::BalanceIdentifier, tracking_copy::TrackingCopyError};
use casper_types::{Digest, ProtocolVersion, URefAddr};

/// Represents a balance identifier purse request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BalanceIdentifierPurseRequest {
    state_hash: Digest,
    protocol_version: ProtocolVersion,
    identifier: BalanceIdentifier,
}

impl BalanceIdentifierPurseRequest {
    /// Creates a new [`BalanceIdentifierPurseRequest`].
    pub fn new(
        state_hash: Digest,
        protocol_version: ProtocolVersion,
        identifier: BalanceIdentifier,
    ) -> Self {
        BalanceIdentifierPurseRequest {
            state_hash,
            protocol_version,
            identifier,
        }
    }

    /// Returns a state hash.
    pub fn state_hash(&self) -> Digest {
        self.state_hash
    }

    /// Protocol version.
    pub fn protocol_version(&self) -> ProtocolVersion {
        self.protocol_version
    }

    /// Returns the identifier [`BalanceIdentifier`].
    pub fn identifier(&self) -> &BalanceIdentifier {
        &self.identifier
    }
}

/// Result enum that represents all possible outcomes of a balance request.
#[derive(Debug, Clone)]
pub enum BalanceIdentifierPurseResult {
    /// Returned if a passed state root hash is not found.
    RootNotFound,
    /// A query returned a balance.
    Success {
        /// The purse address.
        purse_addr: URefAddr,
    },
    /// Failure.
    Failure(TrackingCopyError),
}

impl BalanceIdentifierPurseResult {
    /// Returns the purse address for a [`BalanceIdentifierPurseResult::Success`] variant.
    pub fn purse_addr(&self) -> Option<URefAddr> {
        match self {
            BalanceIdentifierPurseResult::Success { purse_addr, .. } => Some(*purse_addr),
            _ => None,
        }
    }

    /// Was the balance request successful?
    pub fn is_success(&self) -> bool {
        match self {
            BalanceIdentifierPurseResult::RootNotFound
            | BalanceIdentifierPurseResult::Failure(_) => false,
            BalanceIdentifierPurseResult::Success { .. } => true,
        }
    }

    /// Tracking copy error, if any.
    pub fn error(&self) -> Option<&TrackingCopyError> {
        match self {
            BalanceIdentifierPurseResult::RootNotFound
            | BalanceIdentifierPurseResult::Success { .. } => None,
            BalanceIdentifierPurseResult::Failure(err) => Some(err),
        }
    }
}

impl From<TrackingCopyError> for BalanceIdentifierPurseResult {
    fn from(tce: TrackingCopyError) -> Self {
        BalanceIdentifierPurseResult::Failure(tce)
    }
}
