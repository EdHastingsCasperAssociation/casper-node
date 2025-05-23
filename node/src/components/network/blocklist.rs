//! Blocklisting support.
//!
//! Blocked peers are prevented from interacting with the node through a variety of means.

use std::fmt::{self, Display, Formatter};

use casper_types::{Digest, EraId};
use datasize::DataSize;
use serde::Serialize;

use crate::{
    components::{block_accumulator, fetcher::Tag},
    types::InvalidProposalError,
};

/// Reasons why a peer was blocked.
#[derive(DataSize, Debug, Serialize)]
pub(crate) enum BlocklistJustification {
    /// Peer sent incorrect item.
    SentBadItem { tag: Tag },
    /// Peer sent an item which failed validation.
    SentInvalidItem { tag: Tag, error_msg: String },
    /// A finality signature that was sent is invalid.
    SentBadFinalitySignature {
        /// Error reported by block accumulator.
        #[serde(skip_serializing)]
        #[data_size(skip)]
        error: block_accumulator::Error,
    },
    /// A block that was sent is invalid.
    SentBadBlock {
        /// Error reported by block accumulator.
        #[serde(skip_serializing)]
        #[data_size(skip)]
        error: block_accumulator::Error,
    },
    /// An invalid proposal was received.
    SentInvalidProposal {
        /// The era for which the invalid value was destined.
        era: EraId,
        /// The specific error.
        #[serde(skip_serializing)]
        error: Box<InvalidProposalError>,
    },
    /// Too many unasked or expired pongs were sent by the peer.
    #[allow(dead_code)] // Disabled as per 1.5.5 for stability reasons.
    PongLimitExceeded,
    /// Peer misbehaved during consensus and is blocked for it.
    BadConsensusBehavior,
    /// Peer is on the wrong network.
    WrongNetwork {
        /// The network name reported by the peer.
        peer_network_name: String,
    },
    /// Peer presented the wrong chainspec hash.
    WrongChainspecHash {
        /// The chainspec hash reported by the peer.
        peer_chainspec_hash: Digest,
    },
    /// Peer did not present a chainspec hash.
    MissingChainspecHash,
    /// Peer is considered dishonest.
    DishonestPeer,
    /// Peer sent too many finality signatures.
    SentTooManyFinalitySignatures { max_allowed: u32 },
    /// This is used when the forced network instability logic is turned
    /// on. For testing purposes only - not used in normal operation.
    FlakyNetworkForcedMode,
}

impl Display for BlocklistJustification {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            BlocklistJustification::SentBadItem { tag } => {
                write!(f, "sent a {} we couldn't parse", tag)
            }
            BlocklistJustification::SentInvalidItem { tag, error_msg } => {
                write!(f, "sent a {} which failed validation ({})", tag, error_msg)
            }
            BlocklistJustification::SentBadFinalitySignature { error } => write!(
                f,
                "sent a finality signature that is invalid or unexpected ({})",
                error
            ),
            BlocklistJustification::SentInvalidProposal { era, error } => {
                write!(f, "sent an invalid proposal in {} ({:?})", era, error)
            }
            BlocklistJustification::PongLimitExceeded => {
                f.write_str("wrote too many expired or invalid pongs")
            }
            BlocklistJustification::BadConsensusBehavior => {
                f.write_str("sent invalid data in consensus")
            }
            BlocklistJustification::WrongNetwork { peer_network_name } => write!(
                f,
                "reported to be on the wrong network ({:?})",
                peer_network_name
            ),
            BlocklistJustification::WrongChainspecHash {
                peer_chainspec_hash,
            } => write!(
                f,
                "reported a mismatched chainspec hash ({})",
                peer_chainspec_hash
            ),
            BlocklistJustification::MissingChainspecHash => {
                f.write_str("sent handshake without chainspec hash")
            }
            BlocklistJustification::SentBadBlock { error } => {
                write!(f, "sent a block that is invalid or unexpected ({})", error)
            }
            BlocklistJustification::DishonestPeer => f.write_str("dishonest peer"),
            BlocklistJustification::SentTooManyFinalitySignatures { max_allowed } => write!(
                f,
                "sent too many finality signatures: maximum {max_allowed} signatures are allowed"
            ),
            BlocklistJustification::FlakyNetworkForcedMode => {
                write!(f, "forced a block in flaky network mode")
            }
        }
    }
}
