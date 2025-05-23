use std::iter;

use alloc::collections::BTreeMap;
use rand::Rng;

use crate::{
    system::auction::ValidatorWeights, testing::TestRng, Block, BlockHash, BlockV2, Digest,
    EraEndV2, EraId, ProtocolVersion, PublicKey, RewardedSignatures, Timestamp, Transaction,
    TransactionEntryPoint, TransactionTarget, AUCTION_LANE_ID, INSTALL_UPGRADE_LANE_ID,
    LARGE_WASM_LANE_ID, MEDIUM_WASM_LANE_ID, MINT_LANE_ID, SMALL_WASM_LANE_ID, U512,
};

/// A helper to build the blocks with various properties required for tests.
pub struct TestBlockV2Builder {
    parent_hash: Option<BlockHash>,
    state_root_hash: Option<Digest>,
    timestamp: Option<Timestamp>,
    era: Option<EraId>,
    height: Option<u64>,
    proposer: Option<PublicKey>,
    protocol_version: ProtocolVersion,
    txns: Vec<Transaction>,
    is_switch: Option<bool>,
    validator_weights: Option<ValidatorWeights>,
    rewarded_signatures: Option<RewardedSignatures>,
}

impl Default for TestBlockV2Builder {
    fn default() -> Self {
        Self {
            parent_hash: None,
            state_root_hash: None,
            timestamp: None,
            era: None,
            height: None,
            proposer: None,
            protocol_version: ProtocolVersion::V1_0_0,
            txns: Vec::new(),
            is_switch: None,
            validator_weights: None,
            rewarded_signatures: None,
        }
    }
}

impl TestBlockV2Builder {
    /// Creates new `TestBlockBuilder`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the parent hash for the block.
    pub fn parent_hash(self, parent_hash: BlockHash) -> Self {
        Self {
            parent_hash: Some(parent_hash),
            ..self
        }
    }

    /// Sets the state root hash for the block.
    pub fn state_root_hash(self, state_root_hash: Digest) -> Self {
        Self {
            state_root_hash: Some(state_root_hash),
            ..self
        }
    }

    /// Sets the timestamp for the block.
    pub fn timestamp(self, timestamp: Timestamp) -> Self {
        Self {
            timestamp: Some(timestamp),
            ..self
        }
    }

    /// Sets the era for the block
    pub fn era(self, era: impl Into<EraId>) -> Self {
        Self {
            era: Some(era.into()),
            ..self
        }
    }

    /// Sets the height for the block.
    pub fn height(self, height: u64) -> Self {
        Self {
            height: Some(height),
            ..self
        }
    }

    /// Sets the block proposer.
    pub fn proposer(self, proposer: PublicKey) -> Self {
        Self {
            proposer: Some(proposer),
            ..self
        }
    }

    /// Sets the protocol version for the block.
    pub fn protocol_version(self, protocol_version: ProtocolVersion) -> Self {
        Self {
            protocol_version,
            ..self
        }
    }

    /// Associates the given transactions with the created block.
    pub fn transactions<'a, I: IntoIterator<Item = &'a Transaction>>(self, txns_iter: I) -> Self {
        Self {
            txns: txns_iter.into_iter().cloned().collect(),
            ..self
        }
    }

    /// Sets the height for the block.
    pub fn rewarded_signatures(self, rewarded_signatures: RewardedSignatures) -> Self {
        Self {
            rewarded_signatures: Some(rewarded_signatures),
            ..self
        }
    }

    /// Associates a number of random transactions with the created block.
    pub fn random_transactions(mut self, count: usize, rng: &mut TestRng) -> Self {
        self.txns = iter::repeat_with(|| Transaction::random(rng))
            .take(count)
            .collect();
        self
    }

    /// Allows setting the created block to be switch block or not.
    pub fn switch_block(self, is_switch: bool) -> Self {
        Self {
            is_switch: Some(is_switch),
            ..self
        }
    }

    /// Sets the validator weights for the block.
    pub fn validator_weights(self, validator_weights: ValidatorWeights) -> Self {
        Self {
            validator_weights: Some(validator_weights),
            ..self
        }
    }

    /// Builds the block.
    pub fn build(self, rng: &mut TestRng) -> BlockV2 {
        let Self {
            parent_hash,
            state_root_hash,
            timestamp,
            era,
            height,
            proposer,
            protocol_version,
            txns,
            is_switch,
            validator_weights,
            rewarded_signatures,
        } = self;

        let parent_hash = parent_hash.unwrap_or_else(|| BlockHash::new(rng.gen()));
        let parent_seed = Digest::random(rng);
        let state_root_hash = state_root_hash.unwrap_or_else(|| rng.gen());
        let random_bit = rng.gen();
        let is_switch = is_switch.unwrap_or_else(|| rng.gen_bool(0.1));
        let era_end = is_switch.then(|| gen_era_end_v2(rng, validator_weights));
        let timestamp = timestamp.unwrap_or_else(Timestamp::now);
        let era_id = era.unwrap_or(EraId::random(rng));
        let height = height.unwrap_or_else(|| era_id.value() * 10 + rng.gen_range(0..10));
        let proposer = proposer.unwrap_or_else(|| PublicKey::random(rng));

        let mut mint_hashes = vec![];
        let mut auction_hashes = vec![];
        let mut install_upgrade_hashes = vec![];
        let mut large_hashes = vec![];
        let mut medium_hashes = vec![];
        let mut small_hashes = vec![];
        for txn in txns {
            let txn_hash = txn.hash();
            let lane_id = match txn {
                Transaction::Deploy(deploy) => {
                    if deploy.is_transfer() {
                        MINT_LANE_ID
                    } else {
                        LARGE_WASM_LANE_ID
                    }
                }
                Transaction::V1(transaction_v1) => {
                    let entry_point = transaction_v1.get_transaction_entry_point().unwrap();
                    let target = transaction_v1.get_transaction_target().unwrap();
                    simplified_calculate_transaction_lane_from_values(&entry_point, &target)
                }
            };
            match lane_id {
                MINT_LANE_ID => mint_hashes.push(txn_hash),
                AUCTION_LANE_ID => auction_hashes.push(txn_hash),
                INSTALL_UPGRADE_LANE_ID => install_upgrade_hashes.push(txn_hash),
                LARGE_WASM_LANE_ID => large_hashes.push(txn_hash),
                MEDIUM_WASM_LANE_ID => medium_hashes.push(txn_hash),
                SMALL_WASM_LANE_ID => small_hashes.push(txn_hash),
                _ => panic!("Invalid lane id"),
            }
        }
        let transactions = {
            let mut ret = BTreeMap::new();
            ret.insert(MINT_LANE_ID, mint_hashes);
            ret.insert(AUCTION_LANE_ID, auction_hashes);
            ret.insert(INSTALL_UPGRADE_LANE_ID, install_upgrade_hashes);
            ret.insert(LARGE_WASM_LANE_ID, large_hashes);
            ret.insert(MEDIUM_WASM_LANE_ID, medium_hashes);
            ret.insert(SMALL_WASM_LANE_ID, small_hashes);
            ret
        };
        let rewarded_signatures = rewarded_signatures.unwrap_or_default();
        let current_gas_price: u8 = 1;
        let last_switch_block_hash = BlockHash::new(Digest::from([8; Digest::LENGTH]));
        BlockV2::new(
            parent_hash,
            parent_seed,
            state_root_hash,
            random_bit,
            era_end,
            timestamp,
            era_id,
            height,
            protocol_version,
            proposer,
            transactions,
            rewarded_signatures,
            current_gas_price,
            Some(last_switch_block_hash),
        )
    }

    /// Builds the block as a versioned block.
    pub fn build_versioned(self, rng: &mut TestRng) -> Block {
        self.build(rng).into()
    }

    /// Builds a block that is invalid.
    pub fn build_invalid(self, rng: &mut TestRng) -> BlockV2 {
        self.build(rng).make_invalid(rng)
    }
}

// A simplified way of calculating transaction lanes. It doesn't take
// into consideration the size of the transaction against the chainspec
// and doesn't take `additional_computation_factor` into consideration.
// This is only used for tests purposes.
fn simplified_calculate_transaction_lane_from_values(
    entry_point: &TransactionEntryPoint,
    target: &TransactionTarget,
) -> u8 {
    match target {
        TransactionTarget::Native => match entry_point {
            TransactionEntryPoint::Transfer | TransactionEntryPoint::Burn => MINT_LANE_ID,
            TransactionEntryPoint::AddBid
            | TransactionEntryPoint::WithdrawBid
            | TransactionEntryPoint::Delegate
            | TransactionEntryPoint::Undelegate
            | TransactionEntryPoint::Redelegate
            | TransactionEntryPoint::ActivateBid
            | TransactionEntryPoint::ChangeBidPublicKey
            | TransactionEntryPoint::AddReservations
            | TransactionEntryPoint::CancelReservations => AUCTION_LANE_ID,
            TransactionEntryPoint::Call => panic!("EntryPointCannotBeCall"),
            TransactionEntryPoint::Custom(_) => panic!("EntryPointCannotBeCustom"),
        },
        TransactionTarget::Stored { .. } => match entry_point {
            TransactionEntryPoint::Custom(_) => LARGE_WASM_LANE_ID,
            TransactionEntryPoint::Call
            | TransactionEntryPoint::Transfer
            | TransactionEntryPoint::Burn
            | TransactionEntryPoint::AddBid
            | TransactionEntryPoint::WithdrawBid
            | TransactionEntryPoint::Delegate
            | TransactionEntryPoint::Undelegate
            | TransactionEntryPoint::Redelegate
            | TransactionEntryPoint::ActivateBid
            | TransactionEntryPoint::ChangeBidPublicKey
            | TransactionEntryPoint::AddReservations
            | TransactionEntryPoint::CancelReservations => {
                panic!("EntryPointMustBeCustom")
            }
        },
        TransactionTarget::Session {
            is_install_upgrade, ..
        } => match entry_point {
            TransactionEntryPoint::Call => {
                if *is_install_upgrade {
                    INSTALL_UPGRADE_LANE_ID
                } else {
                    LARGE_WASM_LANE_ID
                }
            }
            TransactionEntryPoint::Custom(_)
            | TransactionEntryPoint::Transfer
            | TransactionEntryPoint::Burn
            | TransactionEntryPoint::AddBid
            | TransactionEntryPoint::WithdrawBid
            | TransactionEntryPoint::Delegate
            | TransactionEntryPoint::Undelegate
            | TransactionEntryPoint::Redelegate
            | TransactionEntryPoint::ActivateBid
            | TransactionEntryPoint::ChangeBidPublicKey
            | TransactionEntryPoint::AddReservations
            | TransactionEntryPoint::CancelReservations => {
                panic!("EntryPointMustBeCall")
            }
        },
    }
}

fn gen_era_end_v2(
    rng: &mut TestRng,
    validator_weights: Option<BTreeMap<PublicKey, U512>>,
) -> EraEndV2 {
    let equivocators_count = rng.gen_range(0..5);
    let rewards_count = rng.gen_range(0..5);
    let inactive_count = rng.gen_range(0..5);
    let next_era_validator_weights = validator_weights.unwrap_or_else(|| {
        (1..6)
            .map(|i| (PublicKey::random(rng), U512::from(i)))
            .take(6)
            .collect()
    });
    let equivocators = iter::repeat_with(|| PublicKey::random(rng))
        .take(equivocators_count)
        .collect();
    let rewards = iter::repeat_with(|| {
        let pub_key = PublicKey::random(rng);
        let mut rewards = vec![U512::from(rng.gen_range(1..=1_000_000_000 + 1))];
        if rng.gen_bool(0.2) {
            rewards.push(U512::from(rng.gen_range(1..=1_000_000_000 + 1)));
        };
        (pub_key, rewards)
    })
    .take(rewards_count)
    .collect();
    let inactive_validators = iter::repeat_with(|| PublicKey::random(rng))
        .take(inactive_count)
        .collect();

    EraEndV2::new(
        equivocators,
        inactive_validators,
        next_era_validator_weights,
        rewards,
        1u8,
    )
}
