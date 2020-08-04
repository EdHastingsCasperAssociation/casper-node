//! Block executor component.
mod event;

use std::fmt::Debug;

use rand::Rng;
use tracing::{debug, error, trace, warn};

use casperlabs_types::ProtocolVersion;

use BlockExecutorRequest::ExecuteFinalizedBlock as Payload;

use super::{
    contract_runtime::{
        core::engine_state::{
            execute_request::ExecuteRequest,
            execution_result::{ExecutionResult, ExecutionResults},
        },
        storage::global_state::CommitResult,
    },
    storage::Storage,
};
use crate::{
    components::Component,
    crypto::hash::Digest,
    effect::{
        requests::{BlockExecutorRequest, ContractRuntimeRequest, StorageRequest},
        EffectBuilder, EffectExt, Effects, Responder,
    },
    types::{Deploy, ExecutedBlock, FinalizedBlock, Timestamp},
};

pub use event::Event;

/// A helper trait constraining `BlockExecutor` compatible reactor events.
pub trait ReactorEvent:
    From<Event> + From<StorageRequest<Storage>> + From<ContractRuntimeRequest> + Send
{
}

impl<REv> ReactorEvent for REv where
    REv: From<Event> + From<StorageRequest<Storage>> + From<ContractRuntimeRequest> + Send
{
}

/// The block executor context.
#[derive(Debug)]
pub(crate) struct BlockExecutor {
    // NOTE: Currently state hash is tracked here from genesis onward.
    // TODO: implement more robust approach that allows node stop / restart
    /// Current post state hash.
    post_state_hash: Digest,
}

impl BlockExecutor {
    pub(crate) fn new(post_state_hash: Digest) -> Self {
        BlockExecutor { post_state_hash }
    }

    /// Attempt to retrieve all of the deploys in the block from storage and then execute them.
    fn execute<REv: ReactorEvent>(
        &self,
        effect_builder: EffectBuilder<REv>,
        finalized_block: FinalizedBlock,
        responder: Responder<ExecutedBlock>,
    ) -> Effects<Event> {
        if finalized_block.is_empty() {
            // Early response & exit if no deploys are in block.
            let executed_block = ExecutedBlock {
                finalized_block,
                post_state_hash: self.post_state_hash,
            };
            return responder.respond(executed_block).ignore();
        }

        let deploy_hashes = finalized_block.deploy_hashes().into();

        // Get all deploys in the finalized block (in insertion order).
        effect_builder
            .get_deploys_from_storage(deploy_hashes)
            .event(move |result| Event::GotBlockDeploys {
                finalized_block,
                deploys: result
                    .into_iter()
                    // Assumes all deploys are present
                    .map(|maybe_deploy| {
                        maybe_deploy.expect("deploy is expected to exist in the storage")
                    })
                    .collect(),
                responder,
            })
    }

    /// Execute all of the deploys.
    fn execute_deploys<REv: ReactorEvent>(
        &self,
        effect_builder: EffectBuilder<REv>,
        finalized_block: FinalizedBlock,
        deploys: Vec<Deploy>,
        responder: Responder<ExecutedBlock>,
    ) -> Effects<Event> {
        let execute_request = self.create_execute_request_from(deploys);
        effect_builder
            .request_execute(execute_request)
            .event(move |result| Event::ExecutedBlockDeploys {
                finalized_block,
                result,
                responder,
            })
    }

    /// Creates new `ExecuteRequest` from a list of deploys.
    fn create_execute_request_from(&self, deploys: Vec<Deploy>) -> ExecuteRequest {
        let deploy_items = deploys
            .into_iter()
            .map(|deploy| Ok(deploy.into()))
            .collect();

        ExecuteRequest::new(
            self.post_state_hash,
            // TODO: Use `BlockContext`'s timestamp as part of NDRS-175
            Timestamp::now().millis(),
            deploy_items,
            ProtocolVersion::V1_0_0,
        )
    }

    /// Consumes execution results and dispatches appropriate events.
    fn process_execution_results<REv: ReactorEvent>(
        &self,
        effect_builder: EffectBuilder<REv>,
        finalized_block: FinalizedBlock,
        mut execution_results: ExecutionResults,
        responder: Responder<ExecutedBlock>,
    ) -> Effects<Event> {
        let effect = match execution_results.pop_front() {
            Some(ExecutionResult::Success { effect, cost }) => {
                debug!(?effect, %cost, "execution succeeded");
                effect
            }
            Some(ExecutionResult::Failure {
                error,
                effect,
                cost,
            }) => {
                error!(?error, ?effect, %cost, "execution failure");
                effect
            }
            None => {
                // We processed all executed deploys.
                let executed_block = ExecutedBlock {
                    finalized_block,
                    post_state_hash: self.post_state_hash,
                };
                trace!(?executed_block, "all execution results processed");
                return responder.respond(executed_block).ignore();
            }
        };

        // There's something more to process.
        effect_builder
            .request_commit(
                // TODO: protocol_version should be provided by the caller, NOT hardcoded:
                ProtocolVersion::V1_0_0,
                self.post_state_hash,
                effect.transforms,
            )
            .event(|commit_result| Event::CommittedBlockDeploys {
                finalized_block,
                commit_result,
                execution_results,
                responder,
            })
    }
}

/// The BlockExecutor component.
impl<REv> Component<REv> for BlockExecutor
where
    REv: ReactorEvent,
{
    type Event = Event;

    fn handle_event<R: Rng + ?Sized>(
        &mut self,
        effect_builder: EffectBuilder<REv>,
        _rng: &mut R,
        event: Self::Event,
    ) -> Effects<Self::Event> {
        match event {
            Event::ExecuteBlock(Payload {
                finalized_block,
                responder,
            }) => {
                debug!(?finalized_block, "execute block");
                self.execute(effect_builder, finalized_block, responder)
            }

            Event::GotBlockDeploys {
                finalized_block,
                deploys,
                responder,
            } => {
                trace!(total = %deploys.len(), ?deploys, "fetched deploys");
                self.execute_deploys(effect_builder, finalized_block, deploys, responder)
            }

            Event::ExecutedBlockDeploys {
                finalized_block,
                result,
                responder,
            } => {
                trace!(?finalized_block, ?result, "deploys execution result");
                match result {
                    Ok(execution_results) => self.process_execution_results(
                        effect_builder,
                        finalized_block,
                        execution_results,
                        responder,
                    ),
                    Err(_) => {
                        // NOTE: A given state is expected to exist
                        panic!("root not found");
                    }
                }
            }

            Event::CommittedBlockDeploys {
                finalized_block,
                commit_result,
                execution_results,
                responder,
            } => {
                match commit_result {
                    Ok(CommitResult::Success {
                        state_root,
                        bonded_validators,
                    }) => {
                        debug!(?state_root, ?bonded_validators, "commit succeeded");
                        // Update current post state hash as this will be used for next commit.
                        self.post_state_hash = state_root;
                    }
                    Ok(result) => warn!(?result, "commit succeeded in unexpected state"),
                    Err(error) => {
                        error!(?error, "commit failed");
                        // Shut down to avoid improper operations due to invalid state.
                        panic!("unable to commit");
                    }
                }

                self.process_execution_results(
                    effect_builder,
                    finalized_block,
                    execution_results,
                    responder,
                )
            }
        }
    }
}
