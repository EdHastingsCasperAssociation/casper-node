use std::fmt::{Debug, Display};

use derive_more::From;

use super::super::contract_runtime::{
    core::engine_state::{self, execution_result::ExecutionResults, RootNotFound},
    storage::global_state::CommitResult,
};
use crate::{
    effect::{requests::BlockExecutorRequest, Responder},
    types::{Deploy, ExecutedBlock, FinalizedBlock},
};

/// Block executor component event.
#[derive(Debug, From)]
pub enum Event {
    /// Initial request to execute a block.
    #[from]
    ExecuteBlock(BlockExecutorRequest),
    /// Received all requested deploys.
    GotBlockDeploys {
        /// Finalized block that is passed around from the original request in `Event::Request`.
        finalized_block: FinalizedBlock,
        /// Contents of deploys. All deploys are expected to be present in the storage layer.
        deploys: Vec<Deploy>,
        /// Responder passed with `Event::ExecuteBlock`.
        responder: Responder<ExecutedBlock>,
    },
    /// Contract execution result.
    ExecutedBlockDeploys {
        /// Finalized block used to request execution on.
        finalized_block: FinalizedBlock,
        /// Result of deploy execution.
        result: Result<ExecutionResults, RootNotFound>,
        /// Responder passed with `Event::ExecuteBlock`.
        responder: Responder<ExecutedBlock>,
    },
    /// Commit effects
    CommittedBlockDeploys {
        /// Finalized block committed.
        finalized_block: FinalizedBlock,
        /// Commit result for execution request.
        commit_result: Result<CommitResult, engine_state::Error>,
        /// Result of committed deploys execution.
        execution_results: ExecutionResults,
        /// Responder passed with `Event::ExecuteBlock`.
        responder: Responder<ExecutedBlock>,
    },
}

impl Display for Event {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Event::ExecuteBlock(req) => write!(f, "{}", req),
            Event::GotBlockDeploys {
                finalized_block,
                deploys,
                ..
            } => write!(
                f,
                "fetch deploys for block {} has {} deploys",
                finalized_block,
                deploys.len()
            ),
            Event::ExecutedBlockDeploys {
                finalized_block,
                result: Ok(result),
                ..
            } => write!(
                f,
                "deploys execution result {}, total results: {}",
                finalized_block,
                result.len()
            ),
            Event::ExecutedBlockDeploys {
                finalized_block,
                result: Err(_),
                ..
            } => write!(
                f,
                "deploys execution result {}, root not found",
                finalized_block
            ),
            Event::CommittedBlockDeploys {
                execution_results: results,
                ..
            } if results.is_empty() => write!(f, "commit execution effects tail"),
            Event::CommittedBlockDeploys {
                execution_results: results,
                ..
            } => write!(
                f,
                "commit execution effects remaining {} results",
                results.len()
            ),
        }
    }
}
