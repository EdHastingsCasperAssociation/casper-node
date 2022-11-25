use crate::types::BlockHash;

#[derive(Debug)]
pub(crate) enum SyncInstruction {
    Leap {
        block_hash: BlockHash,
    },
    BlockExec {
        block_hash: BlockHash,
        block_height: u64,
        next_block_hash: Option<BlockHash>,
    },
    BlockSync {
        block_hash: BlockHash,
    },
    CaughtUp,
}
