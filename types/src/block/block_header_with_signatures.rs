use core::fmt::{self, Display, Formatter};
#[cfg(feature = "std")]
use std::error::Error as StdError;

#[cfg(feature = "datasize")]
use datasize::DataSize;
#[cfg(any(feature = "std", test))]
use serde::{Deserialize, Serialize};

use super::{BlockHash, BlockHeader, BlockSignatures};
use crate::EraId;
#[cfg(doc)]
use crate::Signature;

/// An error which can result from validating a [`BlockHeaderWithSignatures`].
#[derive(Copy, Clone, Eq, PartialEq, Debug)]
#[non_exhaustive]
pub enum BlockHeaderWithSignaturesValidationError {
    /// Mismatch between block hash in [`BlockHeader`] and [`BlockSignatures`].
    BlockHashMismatch {
        /// The block hash in the `BlockHeader`.
        block_hash_in_header: BlockHash,
        /// The block hash in the `BlockSignatures`.
        block_hash_in_signatures: BlockHash,
    },
    /// Mismatch between era ID in [`BlockHeader`] and [`BlockSignatures`].
    EraIdMismatch {
        /// The era ID in the `BlockHeader`.
        era_id_in_header: EraId,
        /// The era ID in the `BlockSignatures`.
        era_id_in_signatures: EraId,
    },
}

impl Display for BlockHeaderWithSignaturesValidationError {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        match self {
            BlockHeaderWithSignaturesValidationError::BlockHashMismatch {
                block_hash_in_header: expected,
                block_hash_in_signatures: actual,
            } => {
                write!(
                    formatter,
                    "block hash mismatch - header: {expected}, signatures: {actual}",
                )
            }
            BlockHeaderWithSignaturesValidationError::EraIdMismatch {
                era_id_in_header: expected,
                era_id_in_signatures: actual,
            } => {
                write!(
                    formatter,
                    "era id mismatch - header: {expected}, signatures: {actual}",
                )
            }
        }
    }
}

#[cfg(feature = "std")]
impl StdError for BlockHeaderWithSignaturesValidationError {}

/// A block header and collection of signatures of a given block.
#[derive(Clone, Eq, PartialEq, Debug)]
#[cfg_attr(any(feature = "std", test), derive(Serialize, Deserialize))]
#[cfg_attr(feature = "datasize", derive(DataSize))]
pub struct BlockHeaderWithSignatures {
    block_header: BlockHeader,
    block_signatures: BlockSignatures,
}

impl BlockHeaderWithSignatures {
    /// Returns a new `BlockHeaderWithSignatures`.
    pub fn new(block_header: BlockHeader, block_signatures: BlockSignatures) -> Self {
        BlockHeaderWithSignatures {
            block_header,
            block_signatures,
        }
    }

    /// Returns the block header.
    pub fn block_header(&self) -> &BlockHeader {
        &self.block_header
    }

    /// Returns the block signatures.
    pub fn block_signatures(&self) -> &BlockSignatures {
        &self.block_signatures
    }

    /// Returns `Ok` if and only if the block hash and era ID in the `BlockHeader` are identical to
    /// those in the `BlockSignatures`.
    ///
    /// Note that no cryptographic verification of the contained signatures is performed.  For this,
    /// see [`BlockSignatures::is_verified`].
    pub fn is_valid(&self) -> Result<(), BlockHeaderWithSignaturesValidationError> {
        if self.block_header.block_hash() != *self.block_signatures.block_hash() {
            return Err(
                BlockHeaderWithSignaturesValidationError::BlockHashMismatch {
                    block_hash_in_header: self.block_header.block_hash(),
                    block_hash_in_signatures: *self.block_signatures.block_hash(),
                },
            );
        }
        if self.block_header.era_id() != self.block_signatures.era_id() {
            return Err(BlockHeaderWithSignaturesValidationError::EraIdMismatch {
                era_id_in_header: self.block_header.era_id(),
                era_id_in_signatures: self.block_signatures.era_id(),
            });
        }
        Ok(())
    }

    /// Sets the era ID contained in `block_signatures` to its max value, rendering it and hence
    /// `self` invalid (assuming the relevant era ID for this `BlockHeaderWithSignatures` wasn't
    /// already the max value).
    #[cfg(any(feature = "testing", test))]
    pub fn invalidate_era(&mut self) {
        self.block_signatures.invalidate_era()
    }

    /// Replaces the signature field of the last `block_signatures` entry with the `System` variant
    /// of [`crate::crypto::Signature`], rendering that entry invalid.
    ///
    /// Note that [`Self::is_valid`] will be unaffected by this as it only checks for equality in
    /// the block hash and era ID of the header and signatures; no cryptographic verification is
    /// performed.
    #[cfg(any(feature = "testing", test))]
    pub fn invalidate_last_signature(&mut self) {
        self.block_signatures.invalidate_last_signature()
    }
}

impl Display for BlockHeaderWithSignatures {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "{}, and {}", self.block_header, self.block_signatures)
    }
}
