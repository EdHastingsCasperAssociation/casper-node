use crate::types::{BlockHash, BlockHeader};
use casper_types::EraId;
use itertools::Itertools;
use std::collections::btree_map::Entry;
use std::collections::hash_map::Entry as HashEntry;
use std::collections::{BTreeMap, HashMap};

#[derive(Debug, Copy, Clone, PartialEq)]
pub(crate) enum BlockChainEntry {
    Vacant {
        block_height: u64,
    },
    Proposed {
        block_height: u64,
    },
    Finalized {
        block_height: u64,
    },
    Incomplete {
        block_hash: BlockHash,
        block_height: u64,
    },
    Complete {
        block_height: u64,
        block_hash: BlockHash,
        era_id: EraId,
        is_switch_block: bool,
    },
}

#[allow(unused)]
impl BlockChainEntry {
    /// Create a vacant item
    pub(crate) fn vacant(block_height: u64) -> Self {
        BlockChainEntry::Vacant { block_height }
    }

    /// Create a new proposed item.
    pub(crate) fn new_proposed(block_height: u64) -> Self {
        BlockChainEntry::Proposed { block_height }
    }

    /// Create a new finalize item.
    pub(crate) fn new_finalized(block_height: u64) -> Self {
        BlockChainEntry::Finalized { block_height }
    }

    /// Create a new incomplete item.
    pub(crate) fn new_incomplete(block_height: u64, block_hash: &BlockHash) -> Self {
        BlockChainEntry::Incomplete {
            block_height,
            block_hash: *block_hash,
        }
    }

    /// Create a new complete item.
    pub(crate) fn new_complete(block_header: &BlockHeader) -> Self {
        BlockChainEntry::Complete {
            block_height: block_header.height(),
            block_hash: block_header.block_hash(),
            era_id: block_header.era_id(),
            is_switch_block: block_header.is_switch_block(),
        }
    }

    /// Get block height from item
    pub(crate) fn block_height(&self) -> u64 {
        match self {
            BlockChainEntry::Vacant { block_height }
            | BlockChainEntry::Proposed { block_height }
            | BlockChainEntry::Finalized { block_height }
            | BlockChainEntry::Incomplete { block_height, .. }
            | BlockChainEntry::Complete { block_height, .. } => *block_height,
        }
    }

    /// Get block hash from item, if present.
    pub(crate) fn block_hash(&self) -> Option<BlockHash> {
        match self {
            BlockChainEntry::Vacant { .. }
            | BlockChainEntry::Proposed { .. }
            | BlockChainEntry::Finalized { .. } => None,
            BlockChainEntry::Incomplete { block_hash, .. }
            | BlockChainEntry::Complete { block_hash, .. } => Some(*block_hash),
        }
    }

    /// Get era id from item, if present.
    pub(crate) fn era_id(&self) -> Option<EraId> {
        match self {
            BlockChainEntry::Vacant { .. }
            | BlockChainEntry::Proposed { .. }
            | BlockChainEntry::Finalized { .. }
            | BlockChainEntry::Incomplete { .. } => None,
            BlockChainEntry::Complete { era_id, .. } => Some(*era_id),
        }
    }

    /// Is this instance a switch block?
    pub(crate) fn is_switch_block(&self) -> Option<bool> {
        match self {
            BlockChainEntry::Vacant { .. }
            | BlockChainEntry::Proposed { .. }
            | BlockChainEntry::Finalized { .. }
            | BlockChainEntry::Incomplete { .. } => None,
            BlockChainEntry::Complete {
                is_switch_block, ..
            } => Some(*is_switch_block),
        }
    }

    /// Is this instance complete a switch block?
    pub(crate) fn is_complete_switch_block(&self) -> bool {
        match self {
            BlockChainEntry::Vacant { .. }
            | BlockChainEntry::Proposed { .. }
            | BlockChainEntry::Finalized { .. }
            | BlockChainEntry::Incomplete { .. } => false,
            BlockChainEntry::Complete {
                is_switch_block, ..
            } => *is_switch_block,
        }
    }

    /// Is this instance the proposed variant?
    pub(crate) fn is_proposed(&self) -> bool {
        matches!(self, BlockChainEntry::Proposed { .. })
    }

    /// Is this instance the finalized variant?
    pub(crate) fn is_finalized(&self) -> bool {
        matches!(self, BlockChainEntry::Finalized { .. })
    }

    /// Is this instance the incomplete variant?
    pub(crate) fn is_incomplete(&self) -> bool {
        matches!(self, BlockChainEntry::Incomplete { .. })
    }

    /// Is this instance the complete variant?
    pub(crate) fn is_complete(&self) -> bool {
        matches!(self, BlockChainEntry::Complete { .. })
    }
}

#[derive(Debug, Default)]
pub(crate) struct BlockChain {
    chain: BTreeMap<u64, BlockChainEntry>,
    index: HashMap<BlockHash, u64>,
}

#[allow(unused)]
impl BlockChain {
    /// Instantiate.
    pub(crate) fn new() -> Self {
        BlockChain::default()
    }

    /// Register a block that is proposed.
    pub(crate) fn register_proposed(&mut self, block_height: u64) {
        self.register(BlockChainEntry::new_proposed(block_height));
    }

    /// Register a block that is finalized.
    pub(crate) fn register_finalized(&mut self, block_height: u64) {
        self.register(BlockChainEntry::new_finalized(block_height));
    }

    /// Register a block that is not yet complete.
    pub(crate) fn register_incomplete(&mut self, block_height: u64, block_hash: &BlockHash) {
        self.register(BlockChainEntry::new_incomplete(block_height, block_hash));
    }

    /// Register a block that has been marked complete.
    pub(crate) fn register_complete(&mut self, block_header: &BlockHeader) {
        self.register(BlockChainEntry::new_complete(block_header));
    }

    /// Returns entry by height, if present.
    pub(crate) fn by_height(&self, block_height: u64) -> BlockChainEntry {
        *self
            .chain
            .get(&block_height)
            .unwrap_or(&BlockChainEntry::vacant(block_height))
    }

    /// Return entry by hash, if present.
    pub(crate) fn by_hash(&self, block_hash: &BlockHash) -> Option<BlockChainEntry> {
        if let Some(height) = self.index.get(block_hash) {
            return Some(self.by_height(*height));
        }
        None
    }

    /// Returns entry of child, if present.
    pub(crate) fn by_parent(&self, parent_block_hash: &BlockHash) -> Option<BlockChainEntry> {
        if let Some(height) = self.index.get(parent_block_hash) {
            return Some(self.by_height(height + 1));
        }
        None
    }

    /// Is block at height incomplete?
    pub(crate) fn is_incomplete(&self, block_height: u64) -> bool {
        self.chain
            .get(&block_height)
            .map(|b| b.is_incomplete())
            .unwrap_or(false)
    }

    /// Is block at height complete?
    pub(crate) fn is_complete(&self, block_height: u64) -> bool {
        self.chain
            .get(&block_height)
            .map(|b| b.is_complete())
            .unwrap_or(false)
    }

    /// Is block at height a switch block?
    pub(crate) fn is_switch_block(&self, block_height: u64) -> Option<bool> {
        self.chain
            .get(&block_height)
            .map(|b| b.is_switch_block())
            .unwrap_or(None)
    }

    /// Returns the lowest entry (by block height) where the predicate is true, if any.
    pub(crate) fn lowest<F>(&self, predicate: F) -> Option<&BlockChainEntry>
    where
        F: Fn(&BlockChainEntry) -> bool,
    {
        self.chain
            .values()
            .filter(|x| predicate(x))
            .min_by(|x, y| x.block_height().cmp(&y.block_height()))
    }

    /// Returns the highest entry (by block height) where the predicate is true, if any.
    pub(crate) fn highest<F>(&self, predicate: F) -> Option<&BlockChainEntry>
    where
        F: Fn(&BlockChainEntry) -> bool,
    {
        self.chain
            .values()
            .filter(|x| predicate(x))
            .max_by(|x, y| x.block_height().cmp(&y.block_height()))
    }

    /// Returns the lowest switch block entry, if any.
    pub(crate) fn lowest_switch_block(&self) -> Option<&BlockChainEntry> {
        self.chain
            .values()
            .filter(|x| x.is_complete() && x.is_switch_block().unwrap_or(false))
            .min_by(|x, y| x.block_height().cmp(&y.block_height()))
    }

    /// Returns the highest switch block entry, if any.
    pub(crate) fn highest_switch_block(&self) -> Option<&BlockChainEntry> {
        self.chain
            .values()
            .filter(|x| x.is_complete() && x.is_switch_block().unwrap_or(false))
            .max_by(|x, y| x.block_height().cmp(&y.block_height()))
    }

    /// Returns the highest entry (by block height) where the predicate is true, if any.
    pub(crate) fn all_by<F>(&self, predicate: F) -> Vec<&BlockChainEntry>
    where
        F: Fn(&BlockChainEntry) -> bool,
    {
        self.chain.values().filter(|x| predicate(x)).collect_vec()
    }

    /// Returns the highest entry (by block height) where the predicate is true, if any.
    pub(crate) fn lowest_sequence<F>(&self, predicate: F) -> Vec<BlockChainEntry>
    where
        F: Fn(&BlockChainEntry) -> bool,
    {
        match self
            .chain
            .values()
            .filter(|x| predicate(x))
            .min_by(|x, y| x.block_height().cmp(&y.block_height()))
        {
            None => {
                vec![]
            }
            Some(entry) => {
                let mut ret = vec![*entry];
                let mut idx = entry.block_height() + 1;
                loop {
                    let item = self.by_height(idx);
                    if predicate(&item) {
                        ret.push(item);
                        idx += 1;
                    } else {
                        break;
                    }
                }
                ret
            }
        }
    }

    /// Returns the highest entry (by block height) where the predicate is true, if any.
    pub(crate) fn highest_sequence<F>(&self, predicate: F) -> Vec<&BlockChainEntry>
    where
        F: Fn(&BlockChainEntry) -> bool,
    {
        let mut ret = vec![];

        for height in self.chain.keys().into_iter().rev() {
            match self.chain.get(height) {
                None => {
                    break;
                }
                Some(item) => {
                    if predicate(item) {
                        ret.push(item);
                        continue;
                    }
                    if !ret.is_empty() {
                        // sequence is broken
                        break;
                    }
                    // still seeking start of sequence
                }
            }
        }
        ret
    }

    fn register(&mut self, item: BlockChainEntry) {
        let block_height = item.block_height();
        // maintain the reverse lookup by block_hash where able
        if let Some(block_hash) = item.block_hash() {
            match self.index.entry(block_hash) {
                HashEntry::Occupied(mut entry) => {
                    let val = entry.get();
                    debug_assert!(
                        val.eq(&block_height),
                        "BlockChain: register existing block_height {} should match {}",
                        val,
                        block_height
                    );
                }
                HashEntry::Vacant(vacant) => {
                    vacant.insert(block_height);
                }
            }
        }
        /// maintain the chain representation overlay
        match self.chain.entry(block_height) {
            Entry::Vacant(vacant) => {
                vacant.insert(item);
            }
            Entry::Occupied(mut entry) => {
                let val = entry.get_mut();
                *val = item;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::components::block_chain::{BlockChain, BlockChainEntry};
    use crate::types::{Block, BlockHash};
    use casper_types::testing::TestRng;
    use casper_types::EraId;

    impl BlockChain {
        /// Register a block that has been marked complete from parts.
        pub(crate) fn register_complete_from_parts(
            &mut self,
            block_height: u64,
            block_hash: BlockHash,
            era_id: EraId,
            is_switch_block: bool,
        ) {
            let entry = BlockChainEntry::Complete {
                block_height,
                block_hash,
                era_id,
                is_switch_block,
            };
            self.register(entry);
        }
    }

    #[test]
    fn should_construct_empty() {
        let block_chain = BlockChain::new();
        assert!(block_chain.chain.is_empty(), "should ctor with empty chain");
        assert!(block_chain.index.is_empty(), "should ctor with empty index");
    }

    #[test]
    fn should_be_vacant() {
        let block_chain = BlockChain::new();
        assert_eq!(
            block_chain.by_height(0),
            BlockChainEntry::vacant(0),
            "should be vacant"
        );
    }

    #[test]
    fn should_be_proposed() {
        let mut block_chain = BlockChain::new();
        block_chain.register_proposed(0);
        assert_eq!(
            block_chain.by_height(0),
            BlockChainEntry::new_proposed(0),
            "should be proposed"
        );
    }

    #[test]
    fn should_be_finalized() {
        let mut block_chain = BlockChain::new();
        block_chain.register_finalized(0);
        assert_eq!(
            block_chain.by_height(0),
            BlockChainEntry::new_finalized(0),
            "should be finalized"
        );
    }

    #[test]
    fn should_be_incomplete() {
        let mut block_chain = BlockChain::new();
        let block_hash = crate::types::BlockHash::default();
        block_chain.register_incomplete(0, &block_hash);
        assert_eq!(
            block_chain.by_height(0),
            BlockChainEntry::new_incomplete(0, &block_hash),
            "should be incomplete"
        );
    }

    #[test]
    fn should_be_complete() {
        let mut block_chain = BlockChain::new();
        let mut rng = TestRng::new();
        let block_header = {
            let tmp = Block::random(&mut rng);
            tmp.header().clone()
        };
        block_chain.register_complete(&block_header);
        assert_eq!(
            block_chain.by_height(block_header.height()),
            BlockChainEntry::new_complete(&block_header),
            "should be complete"
        );
    }

    #[test]
    fn should_by_hash() {
        let mut block_chain = BlockChain::new();
        let mut rng = TestRng::new();
        let block_header1 = {
            let tmp = Block::random(&mut rng);
            tmp.header().clone()
        };
        block_chain.register_complete(&block_header1);
        assert_eq!(
            block_chain.by_hash(&block_header1.block_hash()),
            Some(BlockChainEntry::new_complete(&block_header1)),
            "should find complete block by hash"
        );
        let block_header2 = {
            let tmp = Block::random(&mut rng);
            tmp.header().clone()
        };
        block_chain.register_incomplete(block_header2.height(), &block_header2.block_hash());
        assert_eq!(
            block_chain.by_hash(&block_header2.block_hash()),
            Some(BlockChainEntry::new_incomplete(
                block_header2.height(),
                &block_header2.block_hash()
            )),
            "should find incomplete block by hash"
        );
    }

    #[test]
    fn should_not_by_hash() {
        let mut block_chain = BlockChain::new();
        let block_hash = crate::types::BlockHash::default();
        block_chain.register_proposed(0);
        assert_eq!(
            block_chain.by_hash(&block_hash),
            None,
            "proposed should not be indexed by hash"
        );
        block_chain.register_finalized(1);
        assert_eq!(
            block_chain.by_hash(&block_hash),
            None,
            "finalized should not be indexed by hash"
        );
    }

    #[test]
    fn should_find_low_sequence() {
        let mut block_chain = BlockChain::new();
        for height in 0..15 {
            if height >= 5 && height < 10 {
                block_chain.register_finalized(height);
                continue;
            }
            block_chain.register_proposed(height);
        }
        println!("{:?}", block_chain);
        let low = block_chain.lowest_sequence(BlockChainEntry::is_proposed);
        assert!(low.is_empty() == false, "sequence should not be empty");
        assert_eq!(
            low.iter()
                .min_by(|x, y| x.block_height().cmp(&y.block_height()))
                .expect("should have entry")
                .block_height(),
            0,
            "expected first entry by predicate"
        );
        assert_eq!(
            low.iter()
                .max_by(|x, y| x.block_height().cmp(&y.block_height()))
                .expect("should have entry")
                .block_height(),
            4,
            "expected last entry by predicate"
        );
    }

    #[test]
    fn should_find_high_sequence() {
        let mut block_chain = BlockChain::new();
        for height in 0..15 {
            if height >= 5 && height < 10 {
                block_chain.register_finalized(height);
                continue;
            }
            block_chain.register_proposed(height);
        }
        let hi = block_chain.highest_sequence(BlockChainEntry::is_proposed);
        assert!(hi.is_empty() == false, "sequence should not be empty");
        assert_eq!(
            hi.iter()
                .min_by(|x, y| x.block_height().cmp(&y.block_height()))
                .expect("should have entry")
                .block_height(),
            10,
            "expected first entry by predicate"
        );
        assert_eq!(
            hi.iter()
                .max_by(|x, y| x.block_height().cmp(&y.block_height()))
                .expect("should have entry")
                .block_height(),
            14,
            "expected last entry by predicate"
        );
    }

    #[test]
    fn should_find_switch_blocks() {
        let mut rng = TestRng::new();
        let mut block_chain = BlockChain::new();
        let mut era_id = EraId::new(0);
        let mut change_era = false;
        for height in 0..11 {
            if change_era {
                era_id = era_id.successor();
                change_era = false;
            }
            let mut is_switch_block = false;
            if height == 0 || height % 5 == 0 {
                is_switch_block = true;
                change_era = true;
            }
            block_chain.register_complete_from_parts(
                height,
                BlockHash::random(&mut rng),
                era_id,
                is_switch_block,
            );
        }
        assert_eq!(
            block_chain
                .lowest(BlockChainEntry::is_complete_switch_block)
                .expect("should have switch blocks")
                .block_height(),
            0,
            "block at height 0 should be highest switch"
        );
        assert_eq!(
            block_chain
                .highest(BlockChainEntry::is_complete_switch_block)
                .expect("should have switch blocks")
                .block_height(),
            10,
            "block at height 10 should be highest switch"
        );
        assert_eq!(
            block_chain
                .all_by(BlockChainEntry::is_complete_switch_block)
                .len(),
            3,
            "unexpected number of switch blocks"
        );
    }
}
