use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::{self, Display, Formatter},
};

use datasize::DataSize;
use itertools::Itertools;
use thiserror::Error;
use tracing::error;

use casper_types::{
    Approval, Gas, PublicKey, RewardedSignatures, Timestamp, TransactionConfig, TransactionHash,
    AUCTION_LANE_ID, INSTALL_UPGRADE_LANE_ID, MINT_LANE_ID, U512,
};

use super::{BlockPayload, TransactionFootprint, VariantMismatch};

#[derive(Debug, Error)]
pub(crate) enum AddError {
    #[error("would exceed maximum count for the category per block")]
    Count(u8),
    #[error("would exceed maximum approval count per block")]
    ApprovalCount,
    #[error("would exceed maximum gas per block")]
    GasLimit,
    #[error("would exceed maximum block size")]
    BlockSize,
    #[error("duplicate deploy or transaction")]
    Duplicate,
    #[error("deploy or transaction has expired")]
    Expired,
    #[error(transparent)]
    VariantMismatch(#[from] VariantMismatch),
    #[error("transaction has excessive ttl")]
    ExcessiveTtl,
    #[error("transaction is future dated")]
    FutureDatedDeploy,
}

/// A block that is still being added to. It keeps track of and enforces block limits.
#[derive(Clone, Eq, PartialEq, DataSize, Debug)]
pub(crate) struct AppendableBlock {
    transaction_config: TransactionConfig,
    current_gas_price: u8,
    transactions: BTreeMap<TransactionHash, TransactionFootprint>,
    timestamp: Timestamp,
}

impl AppendableBlock {
    /// Creates an empty `AppendableBlock`.
    pub(crate) fn new(
        transaction_config: TransactionConfig,
        current_gas_price: u8,
        timestamp: Timestamp,
    ) -> Self {
        AppendableBlock {
            transaction_config,
            current_gas_price,
            transactions: BTreeMap::new(),
            timestamp,
        }
    }

    /// Attempt to append transaction to block.
    pub(crate) fn add_transaction(
        &mut self,
        footprint: &TransactionFootprint,
    ) -> Result<(), AddError> {
        if self
            .transactions
            .keys()
            .contains(&footprint.transaction_hash)
        {
            return Err(AddError::Duplicate);
        }
        if footprint.ttl > self.transaction_config.max_ttl {
            return Err(AddError::ExcessiveTtl);
        }
        if footprint.timestamp > self.timestamp {
            return Err(AddError::FutureDatedDeploy);
        }
        let expires = footprint.timestamp.saturating_add(footprint.ttl);
        if expires < self.timestamp {
            return Err(AddError::Expired);
        }
        let lane_id = footprint.lane_id;
        let limit = self
            .transaction_config
            .transaction_v1_config
            .get_max_transaction_count(lane_id);
        // check total count by category
        let count = self
            .transactions
            .iter()
            .filter(|(_, item)| item.lane_id == lane_id)
            .count();
        if count.checked_add(1).ok_or(AddError::Count(lane_id))? > limit as usize {
            return Err(AddError::Count(lane_id));
        }
        // check total gas
        let gas_limit: U512 = self
            .transactions
            .values()
            .map(|item| item.gas_limit.value())
            .sum();
        if gas_limit
            .checked_add(footprint.gas_limit.value())
            .ok_or(AddError::GasLimit)?
            > U512::from(self.transaction_config.block_gas_limit)
        {
            return Err(AddError::GasLimit);
        }
        // check total byte size
        let size: usize = self
            .transactions
            .values()
            .map(|item| item.size_estimate)
            .sum();
        if size
            .checked_add(footprint.size_estimate)
            .ok_or(AddError::BlockSize)?
            > self.transaction_config.max_block_size as usize
        {
            return Err(AddError::BlockSize);
        }
        // check total approvals
        let count: usize = self
            .transactions
            .values()
            .map(|item| item.approvals_count())
            .sum();
        if count
            .checked_add(footprint.approvals_count())
            .ok_or(AddError::ApprovalCount)?
            > self.transaction_config.block_max_approval_count as usize
        {
            return Err(AddError::ApprovalCount);
        }
        self.transactions
            .insert(footprint.transaction_hash, footprint.clone());
        Ok(())
    }

    /// Creates a `BlockPayload` with the `AppendableBlock`s transactions and transfers, and the
    /// given random bit and accusations.
    pub(crate) fn into_block_payload(
        self,
        accusations: Vec<PublicKey>,
        rewarded_signatures: RewardedSignatures,
        random_bit: bool,
    ) -> BlockPayload {
        let AppendableBlock {
            transactions: footprints,
            current_gas_price: price,
            ..
        } = self;

        fn collate(
            lane: u8,
            collater: &mut BTreeMap<u8, Vec<(TransactionHash, BTreeSet<Approval>)>>,
            items: &BTreeMap<TransactionHash, TransactionFootprint>,
        ) {
            let mut ret = vec![];
            for (x, y) in items.iter().filter(|(_, y)| y.lane_id == lane) {
                ret.push((*x, y.approvals.clone()));
            }
            if !ret.is_empty() {
                collater.insert(lane, ret);
            }
        }

        let mut transactions = BTreeMap::new();
        collate(MINT_LANE_ID, &mut transactions, &footprints);
        collate(AUCTION_LANE_ID, &mut transactions, &footprints);
        collate(INSTALL_UPGRADE_LANE_ID, &mut transactions, &footprints);
        for lane_id in self
            .transaction_config
            .transaction_v1_config
            .wasm_lanes()
            .iter()
            .map(|lane| lane.id())
        {
            collate(lane_id, &mut transactions, &footprints);
        }

        BlockPayload::new(
            transactions,
            accusations,
            rewarded_signatures,
            random_bit,
            price,
        )
    }

    pub(crate) fn timestamp(&self) -> Timestamp {
        self.timestamp
    }

    fn category_lane(&self, lane: u8) -> usize {
        self.transactions
            .iter()
            .filter(|(_, f)| f.lane_id == lane)
            .count()
    }

    #[cfg(test)]
    pub fn transaction_count(&self) -> usize {
        self.transactions.len()
    }
}

impl Display for AppendableBlock {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        let total_count = self.transactions.len();
        let mint_count = self.category_lane(MINT_LANE_ID);
        let auction_count = self.category_lane(AUCTION_LANE_ID);
        let install_upgrade_count = self.category_lane(INSTALL_UPGRADE_LANE_ID);
        let wasm_count = total_count - mint_count - auction_count - install_upgrade_count;
        let total_gas_limit: Gas = self
            .transactions
            .values()
            .map(|f| f.gas_limit)
            .try_fold(Gas::new(0), |acc, gas| acc.checked_add(gas))
            .unwrap_or(Gas::MAX);
        let total_approvals_count: usize = self
            .transactions
            .values()
            .map(|f| f.approvals_count())
            .sum();
        let total_size_estimate: usize = self.transactions.values().map(|f| f.size_estimate).sum();

        write!(
            formatter,
            "AppendableBlock(timestamp-{}:
                mint: {mint_count}, \
                auction: {auction_count}, \
                install_upgrade: {install_upgrade_count}, \
                wasm: {wasm_count}, \
                total count: {total_count}, \
                approvals: {total_approvals_count}, \
                gas: {total_gas_limit}, \
                size: {total_size_estimate})",
            self.timestamp,
        )
    }
}

#[cfg(test)]
mod tests {
    use casper_types::{testing::TestRng, SingleBlockRewardedSignatures, TimeDiff};

    use crate::testing::LARGE_WASM_LANE_ID;

    use super::*;
    use std::collections::HashSet;

    impl AppendableBlock {
        pub(crate) fn transaction_hashes(&self) -> HashSet<TransactionHash> {
            self.transactions.keys().copied().collect()
        }
    }

    #[test]
    pub fn should_build_block_payload_from_all_transactions() {
        let mut test_rng = TestRng::new();
        let mut appendable_block = AppendableBlock::new(
            TransactionConfig::default(),
            0,
            Timestamp::now() + TimeDiff::from_millis(15000),
        );
        let transfer_footprint = TransactionFootprint::random_of_lane(MINT_LANE_ID, &mut test_rng);
        let auction_footprint =
            TransactionFootprint::random_of_lane(AUCTION_LANE_ID, &mut test_rng);
        let install_upgrade_footprint =
            TransactionFootprint::random_of_lane(INSTALL_UPGRADE_LANE_ID, &mut test_rng);
        let large_wasm_footprint =
            TransactionFootprint::random_of_lane(LARGE_WASM_LANE_ID, &mut test_rng);
        let signatures = RewardedSignatures::new(vec![SingleBlockRewardedSignatures::random(
            &mut test_rng,
            2,
        )]);
        appendable_block
            .add_transaction(&transfer_footprint)
            .unwrap();
        appendable_block
            .add_transaction(&auction_footprint)
            .unwrap();
        appendable_block
            .add_transaction(&install_upgrade_footprint)
            .unwrap();
        appendable_block
            .add_transaction(&large_wasm_footprint)
            .unwrap();
        let block_payload = appendable_block.into_block_payload(vec![], signatures.clone(), false);
        let transaction_hashes: BTreeSet<TransactionHash> =
            block_payload.all_transaction_hashes().collect();
        assert!(transaction_hashes.contains(&transfer_footprint.transaction_hash));
        assert!(transaction_hashes.contains(&auction_footprint.transaction_hash));
        assert!(transaction_hashes.contains(&install_upgrade_footprint.transaction_hash));
        assert!(transaction_hashes.contains(&large_wasm_footprint.transaction_hash));
        assert_eq!(transaction_hashes.len(), 4);
        assert_eq!(*block_payload.rewarded_signatures(), signatures);
    }
}
