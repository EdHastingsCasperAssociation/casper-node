use std::collections::BTreeMap;

use casper_storage::{
    data_access_layer::{BidsRequest, BidsResult},
    global_state::state::StateProvider,
};
use casper_types::{system::auction::BidKind, BlockHeader, EraId, PublicKey, U512};

use crate::reactor::main_reactor::tests::Nodes;

/// A set of consecutive switch blocks.
pub(crate) struct SwitchBlocks {
    pub headers: Vec<BlockHeader>,
}

impl SwitchBlocks {
    /// Collects all switch blocks of the first `era_count` eras, and asserts that they are equal
    /// in all nodes.
    pub(crate) fn collect(nodes: &Nodes, era_count: u64) -> SwitchBlocks {
        let mut headers = Vec::new();
        for era_number in 0..era_count {
            let mut header_iter = nodes.values().map(|runner| {
                let storage = runner.main_reactor().storage();
                let maybe_block = storage.read_switch_block_by_era_id(EraId::from(era_number));
                maybe_block.expect("missing switch block").take_header()
            });
            let header = header_iter.next().unwrap();
            assert_eq!(era_number, header.era_id().value());
            for other_header in header_iter {
                assert_eq!(header, other_header);
            }
            headers.push(header);
        }
        SwitchBlocks { headers }
    }

    /// Returns the list of equivocators in the given era.
    pub(crate) fn equivocators(&self, era_number: u64) -> &[PublicKey] {
        self.headers[era_number as usize]
            .maybe_equivocators()
            .expect("era end")
    }

    /// Returns the list of inactive validators in the given era.
    pub(crate) fn inactive_validators(&self, era_number: u64) -> &[PublicKey] {
        self.headers[era_number as usize]
            .maybe_inactive_validators()
            .expect("era end")
    }

    /// Returns the list of validators in the successor era.
    pub(crate) fn next_era_validators(&self, era_number: u64) -> &BTreeMap<PublicKey, U512> {
        self.headers[era_number as usize]
            .next_era_validator_weights()
            .expect("validators")
    }

    /// Returns the set of bids in the auction contract at the end of the given era.
    pub(crate) fn bids(&self, nodes: &Nodes, era_number: u64) -> Vec<BidKind> {
        let state_root_hash = *self.headers[era_number as usize].state_root_hash();
        for runner in nodes.values() {
            let request = BidsRequest::new(state_root_hash);
            let data_provider = runner.main_reactor().contract_runtime().data_access_layer();
            if let BidsResult::Success { bids } = data_provider.bids(request) {
                return bids;
            }
        }
        unreachable!("at least one node should have bids for era {}", era_number);
    }
}
