mod config;
mod event;
mod tests;

use std::{convert::Infallible, fmt::Debug};

use thiserror::Error;
use tracing::{debug, error, info};

use casper_types::{
    account::AccountHash, ContractHash, ContractPackageHash, Key, StoredValue, U512,
};

use crate::{
    components::Component,
    effect::{
        announcements::DeployAcceptorAnnouncement,
        requests::{ContractRuntimeRequest, StorageRequest},
        EffectBuilder, EffectExt, Effects, Responder,
    },
    types::{
        chainspec::DeployConfig, Block, Chainspec, Deploy, DeployConfigurationFailure, NodeId,
    },
    utils::Source,
    NodeRng,
};

use casper_execution_engine::{
    core::engine_state::{
        executable_deploy_item::{ContractIdentifier, ContractPackageIdentifier},
        BalanceRequest, ExecutableDeployItem, QueryRequest, QueryResult, MAX_PAYMENT,
    },
    shared::newtypes::Blake2bHash,
};

use casper_execution_engine::core::engine_state::executable_deploy_item::ExecutableDeployItemIdentifier;
use casper_types::gens::entry_point_access_arb;
pub(crate) use config::Config;
pub(crate) use event::Event;

#[derive(Debug, Error)]
pub(crate) enum Error {
    /// An invalid deploy was received from the client.
    #[error("block chain has no blocks")]
    InvalidBlockchain,

    /// An invalid deploy was received from the client.
    #[error("invalid deploy: {0}")]
    InvalidDeployConfiguration(DeployConfigurationFailure),

    /// An invalid deploy was received from the client.
    #[error("deploy parameter failure: {failure} at prestate_hash: {prestate_hash}")]
    InvalidDeployParameters {
        prestate_hash: Blake2bHash,
        failure: DeployParameterFailure,
    },
}

/// A representation of the way in which a deploy failed validation checks.
#[derive(Clone, DataSize, Ord, PartialOrd, Eq, PartialEq, Hash, Debug, Error)]
pub enum DeployParameterFailure {
    /// Invalid Global State Hash
    #[error("Query failed")]
    InvalidQuery { key: Key, message: String },
    /// Invalid Global State Hash
    #[error("Invalid global state")]
    InvalidGlobalStateHash,
    /// Account does not exist
    #[error("Account does not exist")]
    NonexistentAccount { account_hash: AccountHash },
    /// Nonexistent contract at hash
    #[error("Contract at {contract_hash} does not exist")]
    NonexistentContractAtHash { contract_hash: ContractHash },
    #[error("Contract named {name} does not exist in Account's NamedKeys")]
    NonexistentContractAtName { name: String },
    /// Nonexistent contract entrypoint
    #[error("Contract does not have {entry_point}")]
    NonexistentContractEntryPoint { entry_point: String },
    /// Account does not exist
    #[error("Contract Package at {contract_package_hash} does not exist")]
    NonexistentContractPackageAtHash {
        contract_package_hash: ContractPackageHash,
    },
    #[error("ContractPackage named {name} does not exist in Account's NamedKeys")]
    NonexistentContractPackageAtName { name: String },
    /// Authorization invalid
    #[error("Account authorization invalid")]
    Authorization,
    /// Invalid associated keys
    #[error("Account authorization invalid")]
    InvalidAssociatedKeys,
    /// Insufficient deploy signature weight
    #[error("Insufficient deploy signature weight")]
    InsufficientDeploySignatureWeight,
    /// Insufficient transfer payment
    #[error("Insufficient transfer payment; attempted: {attempted} plus cost: {cost} exceeds current balance: {balance}")]
    InsufficientTransferPayment {
        /// The attempted transfer amount.
        attempted: U512,
        /// The transfer cost.
        cost: U512,
        /// The account balance.
        balance: U512,
    },
    /// A deploy was sent from account with insufficient balance.
    #[error("insufficient balance")]
    InsufficientBalance,
    /// A deploy was sent from account with an unknown balance.
    #[error("unable to determine balance")]
    UnknownBalance,
}

/// A helper trait constraining `DeployAcceptor` compatible reactor events.
pub(crate) trait ReactorEventT:
    From<Event>
    + From<DeployAcceptorAnnouncement<NodeId>>
    + From<StorageRequest>
    + From<ContractRuntimeRequest>
    + Send
{
}

impl<REv> ReactorEventT for REv where
    REv: From<Event>
        + From<DeployAcceptorAnnouncement<NodeId>>
        + From<StorageRequest>
        + From<ContractRuntimeRequest>
        + Send
{
}

/// The `DeployAcceptor` is the component which handles all new `Deploy`s immediately after they're
/// received by this node, regardless of whether they were provided by a peer or a client.
///
/// It validates a new `Deploy` as far as possible, stores it if valid, then announces the newly-
/// accepted `Deploy`.
#[derive(Debug)]
pub struct DeployAcceptor {
    chain_name: String,
    deploy_config: DeployConfig,
    verify_accounts: bool,
}

impl DeployAcceptor {
    pub(crate) fn new(config: Config, chainspec: &Chainspec) -> Self {
        DeployAcceptor {
            chain_name: chainspec.network_config.name.clone(),
            deploy_config: chainspec.deploy_config,
            verify_accounts: config.verify_accounts(),
        }
    }

    /// Handles receiving a new `Deploy` from a peer or client.
    /// In the case of a peer, there should be no responder and the variant should be `None`
    /// In the case of a client, there should be a responder to communicate the validity of the
    /// deploy and the variant will be `Some`
    fn accept<REv: ReactorEventT>(
        &mut self,
        effect_builder: EffectBuilder<REv>,
        deploy: Box<Deploy>,
        source: Source<NodeId>,
        maybe_responder: Option<Responder<Result<(), Error>>>,
    ) -> Effects<Event> {
        let mut effects = Effects::new();
        let mut cloned_deploy = deploy.clone();
        let acceptable_result =
            cloned_deploy.is_config_compliant(&self.chain_name, &self.deploy_config);
        // checks chainspec values
        // DOES NOT check cryptographic security
        if let Err(error) = acceptable_result {
            return effect_builder
                .immediately()
                .event(move |_| Event::InvalidDeployResult {
                    deploy,
                    source,
                    error: Error::InvalidDeployConfiguration(error),
                    maybe_responder,
                });
        }

        let block = {
            match effect_builder.get_highest_block_from_storage().await {
                Some(block) => block,
                None => {
                    // this should be unreachable per current design of the system
                    if let Some(responder) = maybe_responder {
                        effects.extend(responder.respond(Err(Error::InvalidBlockchain)).ignore());
                    }
                    return effects;
                }
            }
        };

        let prestate_hash = block.state_root_hash().into();

        let valid_account_result =
            self.is_valid_account(effect_builder, prestate_hash, cloned_deploy);
        // many EE preconditions
        // DOES NOT check cryptographic security
        if let Err(failure) = valid_account_result {
            return effect_builder
                .immediately()
                .event(move |_| Event::InvalidDeployResult {
                    deploy,
                    source,
                    error: Error::InvalidDeployParameters {
                        prestate_hash: prestate_hash,
                        failure,
                    },
                    maybe_responder,
                });
        }

        if let Err(failure) =
            self.is_valid_executable_deploy_item(effect_builder, prestate_hash, deploy.payment())
        {
            return effect_builder
                .immediately()
                .event(move |_| Event::InvalidDeployResult {
                    deploy,
                    source,
                    error: Error::InvalidDeployParameters {
                        prestate_hash: prestate_hash,
                        failure,
                    },
                    maybe_responder,
                });
        }

        if let Err(failure) =
            self.is_valid_executable_deploy_item(effect_builder, prestate_hash, deploy.session())
        {
            return effect_builder
                .immediately()
                .event(move |_| Event::InvalidDeployResult {
                    deploy,
                    source,
                    error: Error::InvalidDeployParameters {
                        prestate_hash: prestate_hash,
                        failure,
                    },
                    maybe_responder,
                });
        }

        let account_key = account_hash.into();

        // if received from client, check to see if account exists and has at least
        // enough motes to cover the penalty payment (aka has at least the minimum account balance)
        // NEVER CHECK THIS for deploys received from other nodes
        let verified_account = {
            if source.from_client() {
                effect_builder.is_verified_account(account_key).await
            } else {
                Some(true)
            }
        };

        match verified_account {
            None => {
                // The client has submitted an invalid deploy.
                // Return an error message to the RPC component via the responder.
                info! {
                    "Received deploy from invalid account using {}", account_key
                };
                if let Some(responder) = maybe_responder {
                    effects.extend(responder.respond(Err(Error::InvalidAccount)).ignore());
                }
                return effects;
            }

            Some(false) => {
                info! {
                    "Received deploy from account {} that does not have minimum balance required", account_key
                };
                // The client has submitted a deploy from an account that does not have minimum
                // balance required. Return an error message to the RPC component via the responder.
                if let Some(responder) = maybe_responder {
                    effects.extend(responder.respond(Err(Error::InsufficientBalance)).ignore());
                }
                return effects;
            }

            Some(true) => {
                // noop; can proceed
            }
        }

        // check cryptographic signature(s) on the deploy
        // do this last as this is computationally expensive and there is
        // no reason to do it if the deploy is invalid
        if let Err(deploy_configuration_failure) = cloned_deploy.is_valid() {
            // The client has submitted a deploy with one or more invalid signatures.
            // Return an error to the RPC component via the responder.
            return effect_builder
                .immediately()
                .event(move |_| Event::InvalidDeployResult {
                    deploy,
                    source,
                    error: Error::InvalidDeployConfiguration(deploy_configuration_failure),
                    maybe_responder,
                });
        }

        let is_new = effect_builder.put_deploy_to_storage(deploy.clone()).await;
        if is_new {
            effects.extend(
                effect_builder
                    .announce_new_deploy_accepted(deploy, source.clone())
                    .ignore(),
            );
        }

        // success
        if let Some(responder) = maybe_responder {
            effects.extend(responder.respond(Ok(())).ignore());
        }
        effects
    }

    fn is_valid_account(
        &mut self,
        effect_builder: EffectBuilder<REv>,
        prestate_hash: Blake2bHash,
        deploy: Box<Deploy>,
    ) -> Result<(), DeployParameterFailure> {
        let account_hash = deploy.header().account().to_account_hash();
        let query_key = account_hash.into();
        let query_request = QueryRequest::new(prestate_hash, query_key, vec![]);
        let query_result = effect_builder.query_global_state(query_request).await;

        match query_result {
            Ok(QueryResult::Success { value, .. }) => {
                if let StoredValue::Account(account) = *value {
                    // check account here
                    let authorization_keys = deploy.approvals().into();
                    if account.can_authorize(authorization_keys) == false {
                        return Err(DeployParameterFailure::InvalidAssociatedKeys);
                    }
                    if account.can_deploy_with(authorization_keys) == false {
                        return Err(DeployParameterFailure::InsufficientDeploySignatureWeight);
                    }
                    let balance_request = BalanceRequest::new(prestate_hash, account.main_purse());
                    if let Ok(balance_result) = effect_builder.get_balance(balance_request).await {
                        match balance_result.motes() {
                            Some(motes) => {
                                if motes < &*MAX_PAYMENT {
                                    return Err(DeployParameterFailure::InsufficientBalance);
                                }
                            }
                            None => {
                                return Err(DeployParameterFailure::UnknownBalance);
                            }
                        }
                    }
                    return Ok(());
                }
                return Err(DeployParameterFailure::NonexistentAccount { account_hash });
            }
            Ok(QueryResult::RootNotFound) => {
                return Err(DeployParameterFailure::InvalidGlobalStateHash);
            }
            Ok(query_result) => {
                return Err(DeployParameterFailure::InvalidQuery {
                    key: query_key,
                    message: query_result.to_string(),
                });
            }
            Err(error) => {
                return Err(DeployParameterFailure::InvalidQuery {
                    key: query_key,
                    message: error.to_string(),
                });
            }
        }
    }

    fn is_valid_executable_deploy_item(
        &self,
        effect_builder: EffectBuilder<REv>,
        prestate_hash: Blake2bHash,
        executable_deploy_item: &ExecutableDeployItem,
    ) -> Result<(), DeployParameterFailure> {
        match executable_deploy_item.identifier() {
            ExecutableDeployItemIdentifier::Module | ExecutableDeployItemIdentifier::Transfer => {
                Ok(())
            }
            ExecutableDeployItemIdentifier::Contract(contract_identifier) => {
                let query_request = match contract_identifier.clone() {
                    ContractIdentifier::Name(name) => {
                        let account_hash = deploy.header().account().to_account_hash();
                        let query_key = account_hash.into();
                        let path = vec![name];
                        QueryRequest::new(prestate_hash, query_key, path)
                    }
                    ContractIdentifier::Hash(contract_hash) => {
                        let query_key = contract_hash.into();
                        let path = vec![];
                        QueryRequest::new(prestate_hash, query_key, path)
                    }
                };
                let entry_point = executable_deploy_item.entry_point_name().to_string();
                self.is_valid_contract(
                    effect_builder,
                    entry_point,
                    query_request,
                    contract_identifier,
                )
            }
            ExecutableDeployItemIdentifier::Package(contract_package_identifier) => {
                let query_request = match contract_package_identifier.clone() {
                    ContractIdentifier::Name(name) => {
                        let account_hash = deploy.header().account().to_account_hash();
                        let query_key = account_hash.into();
                        let path = vec![name];
                        QueryRequest::new(prestate_hash, query_key, path)
                    }
                    ContractIdentifier::Hash(contract_hash) => {
                        let query_key = contract_hash.into();
                        let path = vec![];
                        QueryRequest::new(prestate_hash, query_key, path)
                    }
                };
                self.is_valid_contract_package(
                    effect_builder,
                    entry_point,
                    query_request,
                    contract_package_identifier,
                )
            }
        }
    }

    fn is_valid_contract(
        &self,
        effect_builder: EffectBuilder<REv>,
        entry_point: String,
        query_request: QueryRequest,
        contract_identifier: ContractIdentifier,
    ) -> Result<(), DeployParameterFailure> {
        let query_result = effect_builder.query_global_state(query_request).await;
        match query_result {
            Ok(QueryResult::Success { value, .. }) => {
                if let StoredValue::Contract(contract) = *value {
                    if contract.entry_points().has_entry_point(&entry_point) = false {
                        return Err(DeployParameterFailure::NonexistentContractEntryPoint {
                            entry_point,
                        });
                    }
                    return Ok(());
                }
                match contract_identifier {
                    ContractIdentifier::Name(name) => {
                        return Err(DeployParameterFailure::NonexistentContractAtName { name });
                    }
                    ContractIdentifier::Hash(contract_hash) => {
                        return Err(DeployParameterFailure::NonexistentContractAtHash {
                            contract_hash,
                        });
                    }
                }
            }
            Ok(QueryResult::RootNotFound) => {
                return Err(DeployParameterFailure::InvalidGlobalStateHash);
            }
            Ok(query_result) => {
                return Err(DeployParameterFailure::InvalidQuery {
                    key: query_request.key(),
                    message: query_result.to_string(),
                });
            }
            Err(error) => {
                return Err(DeployParameterFailure::InvalidQuery {
                    key: query_request.key(),
                    message: error.to_string(),
                });
            }
        }
    }

    fn is_valid_contract_package(
        &self,
        effect_builder: EffectBuilder<REv>,
        entry_point: String,
        query_request: QueryRequest,
        contract_package_identifier: ContractPackageIdentifier,
    ) -> Result<(), DeployParameterFailure> {
        let query_result = effect_builder.query_global_state(query_request).await;
        match query_result {
            Ok(QueryResult::Success { value, .. }) => {
                if let StoredValue::ContractPackage(contract_package) = *value {
                    // get the contract version of the package, then get the contract, then
                    // call is_valid_contract

                    if contract_package
                        .entry_points()
                        .has_entry_point(&entry_point) = false
                    {
                        return Err(
                            DeployParameterFailure::NonexistentContractPackageEntryPoint {
                                entry_point,
                            },
                        );
                    }
                    return Ok(());
                }
                match contract_package_identifier {
                    ContractIdentifier::Name(name) => {
                        return Err(DeployParameterFailure::NonexistentContractPackageAtName {
                            name,
                        });
                    }
                    ContractIdentifier::Hash(contract_hash) => {
                        return Err(DeployParameterFailure::NonexistentContractPackageAtHash {
                            contract_package_hash,
                        });
                    }
                }
            }
            Ok(QueryResult::RootNotFound) => {
                return Err(DeployParameterFailure::InvalidGlobalStateHash);
            }
            Ok(query_result) => {
                return Err(DeployParameterFailure::InvalidQuery {
                    key: query_request.key(),
                    message: query_result.to_string(),
                });
            }
            Err(error) => {
                return Err(DeployParameterFailure::InvalidQuery {
                    key: query_request.key(),
                    message: error.to_string(),
                });
            }
        }
    }
}

impl<REv: ReactorEventT> Component<REv> for DeployAcceptor {
    type Event = Event;
    type ConstructionError = Infallible;

    fn handle_event(
        &mut self,
        effect_builder: EffectBuilder<REv>,
        _rng: &mut NodeRng,
        event: Self::Event,
    ) -> Effects<Self::Event> {
        debug!(?event, "handling event");
        match event {
            Event::Accept {
                deploy,
                source,
                responder,
            } => self.accept(effect_builder, deploy, source, responder),
            // Event::PutToStorageResult {
            //     deploy,
            //     source,
            //     is_new,
            // } => self.handle_put_to_storage(effect_builder, deploy, source, is_new),
            Event::InvalidDeployResult {
                deploy,
                source,
                error,
                maybe_responder,
            } => {
                let mut effects = Effects::new();
                // The client has submitted a deploy with one or more invalid signatures.
                // Return an error to the RPC component via the responder.
                if let Some(responder) = maybe_responder {
                    effects.extend(responder.respond(Err(error)).ignore());
                }
                effects.extend(
                    effect_builder
                        .announce_invalid_deploy(deploy, source)
                        .ignore(),
                );
                effects
            }
        }
    }
}
