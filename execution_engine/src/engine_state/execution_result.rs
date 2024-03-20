//! Outcome of an `ExecutionRequest`.

use std::collections::VecDeque;

use tracing::{debug, trace};

use casper_storage::data_access_layer::BiddingResult;
use casper_types::{
    bytesrepr::FromBytes,
    contract_messages::Messages,
    execution::{Effects, ExecutionResultV2 as TypesExecutionResult, TransformKindV2, TransformV2},
    CLTyped, CLValue, Gas, Key, Motes, StoredValue, TransferAddr,
};

use super::Error;
use crate::execution::ExecError;

/// Represents the result of an execution.
#[derive(Clone, Debug)]
pub enum ExecutionResult {
    /// An error condition that happened during execution
    Failure {
        /// Error causing this `Failure` variant.
        error: Error,
        /// List of transfers that happened during execution up to the point of the failure.
        transfers: Vec<TransferAddr>,
        /// Gas consumed up to the point of the failure.
        gas: Gas,
        /// Execution effects.
        effects: Effects,
        /// Messages emitted during execution.
        messages: Messages,
    },
    /// Execution was finished successfully
    Success {
        /// List of transfers.
        transfers: Vec<TransferAddr>,
        /// Gas consumed.
        gas: Gas,
        /// Execution effects.
        effects: Effects,
        /// Messages emitted during execution.
        messages: Messages,
    },
}

impl ExecutionResult {
    /// Constructs [ExecutionResult::Failure] that has 0 cost and no effects.
    /// This is the case for failures that we can't (or don't want to) charge
    /// for, like `PreprocessingError` or `InvalidNonce`.
    pub fn precondition_failure(error: Error) -> ExecutionResult {
        ExecutionResult::Failure {
            error,
            transfers: Vec::default(),
            gas: Gas::zero(),
            effects: Effects::new(),
            messages: Vec::default(),
        }
    }

    /// Returns `true` if this is a successful variant.
    pub fn is_success(&self) -> bool {
        match self {
            ExecutionResult::Failure { .. } => false,
            ExecutionResult::Success { .. } => true,
        }
    }

    /// Returns `true` if this is a failure variant.
    pub fn is_failure(&self) -> bool {
        match self {
            ExecutionResult::Failure { .. } => true,
            ExecutionResult::Success { .. } => false,
        }
    }

    /// Returns `true` if this is a precondition failure.
    ///
    /// Precondition variant is further described as an execution failure which does not have any
    /// effects, and has a gas cost of 0.
    pub fn has_precondition_failure(&self) -> bool {
        match self {
            ExecutionResult::Failure { gas, effects, .. } => {
                *gas == Gas::zero() && effects.is_empty()
            }
            ExecutionResult::Success { .. } => false,
        }
    }

    /// Returns gas used during execution regardless of variant.
    pub fn gas(&self) -> Gas {
        match self {
            ExecutionResult::Failure { gas, .. } | ExecutionResult::Success { gas, .. } => *gas,
        }
    }

    /// Returns list of transfers regardless of variant.
    pub fn transfers(&self) -> &Vec<TransferAddr> {
        match self {
            ExecutionResult::Failure { transfers, .. } => transfers,
            ExecutionResult::Success { transfers, .. } => transfers,
        }
    }

    /// The ordered execution effects regardless of variant.
    pub fn effects(&self) -> &Effects {
        match self {
            ExecutionResult::Failure { effects, .. } | ExecutionResult::Success { effects, .. } => {
                effects
            }
        }
    }

    /// Returns a new execution result with updated gas.
    ///
    /// This method preserves the [`ExecutionResult`] variant and updates the gas field only.
    pub fn with_gas(self, gas: Gas) -> Self {
        match self {
            ExecutionResult::Failure {
                error,
                transfers,
                effects,
                messages,
                ..
            } => ExecutionResult::Failure {
                error,
                transfers,
                gas,
                effects,
                messages,
            },
            ExecutionResult::Success {
                transfers,
                effects,
                messages,
                ..
            } => ExecutionResult::Success {
                transfers,
                gas,
                effects,
                messages,
            },
        }
    }

    /// Returns a new execution result with updated transfers field.
    ///
    /// This method preserves the [`ExecutionResult`] variant and updates the
    /// `transfers` field only.
    pub fn with_transfers(self, transfers: Vec<TransferAddr>) -> Self {
        match self {
            ExecutionResult::Failure {
                error,
                gas,
                effects,
                messages,
                ..
            } => ExecutionResult::Failure {
                error,
                transfers,
                gas,
                effects,
                messages,
            },
            ExecutionResult::Success {
                gas,
                effects,
                messages,
                ..
            } => ExecutionResult::Success {
                transfers,
                gas,
                effects,
                messages,
            },
        }
    }

    /// Returns a new execution result with updated execution effects.
    ///
    /// This method preserves the [`ExecutionResult`] variant and updates the
    /// `effects` field only.
    pub fn with_effects(self, effects: Effects) -> Self {
        match self {
            ExecutionResult::Failure {
                error,
                transfers,
                gas,
                effects: _,
                messages,
            } => ExecutionResult::Failure {
                error,
                transfers,
                gas,
                effects,
                messages,
            },
            ExecutionResult::Success {
                transfers,
                gas,
                effects: _,
                messages,
            } => ExecutionResult::Success {
                transfers,
                gas,
                effects,
                messages,
            },
        }
    }

    /// Returns error value, if possible.
    ///
    /// Returns a reference to a wrapped [`Error`] instance if the object is a failure
    /// variant.
    pub fn as_error(&self) -> Option<&Error> {
        match self {
            ExecutionResult::Failure { error, .. } => Some(error),
            ExecutionResult::Success { .. } => None,
        }
    }

    /// Consumes [`ExecutionResult`] instance and optionally returns [`Error`] instance for
    /// [`ExecutionResult::Failure`] variant.
    pub fn take_error(self) -> Option<Error> {
        match self {
            ExecutionResult::Failure { error, .. } => Some(error),
            ExecutionResult::Success { .. } => None,
        }
    }

    /// Checks the transfer status of a payment code.
    ///
    /// This method converts the gas cost of the execution result into motes using supplied
    /// `gas_price`, and then a check is made to ensure that user deposited enough funds in the
    /// payment purse (in motes) to cover the execution of a payment code.
    ///
    /// Returns `None` if user deposited enough funds in payment purse and the execution result was
    /// a success variant, otherwise a wrapped [`ForcedTransferResult`] that indicates an error
    /// condition.
    pub fn check_forced_transfer(
        &self,
        payment_purse_balance: Motes,
        gas_price: u64,
    ) -> Option<ForcedTransferResult> {
        let payment_result_cost = match Motes::from_gas(self.gas(), gas_price) {
            Some(cost) => cost,
            None => return Some(ForcedTransferResult::GasConversionOverflow),
        };
        // payment_code_spec_3_b_ii: if (balance of handle payment pay purse) < (gas spent during
        // payment code execution) * gas_price, no session
        let insufficient_balance_to_continue = payment_purse_balance < payment_result_cost;

        match self {
            ExecutionResult::Success { .. } if insufficient_balance_to_continue => {
                // payment_code_spec_4: insufficient payment
                Some(ForcedTransferResult::InsufficientPayment)
            }
            ExecutionResult::Success { .. } => {
                // payment_code_spec_3_b_ii: continue execution
                None
            }
            ExecutionResult::Failure { .. } => {
                // payment_code_spec_3_a: report payment error in the deploy response
                Some(ForcedTransferResult::PaymentFailure)
            }
        }
    }

    /// Creates a new payment code error.
    ///
    /// The method below creates an [`ExecutionResult`] with precomputed effects of a
    /// "finalize_payment".
    ///
    /// The effects that are produced as part of this process would subract `max_payment_cost` from
    /// account's main purse, and add `max_payment_cost` to proposer account's balance.
    pub fn new_payment_code_error(
        error: Error,
        max_payment_cost: Motes,
        account_main_purse_balance: Motes,
        gas: Gas,
        account_main_purse_balance_key: Key,
        proposer_main_purse_balance_key: Key,
    ) -> Result<ExecutionResult, Error> {
        let new_balance = account_main_purse_balance
            .checked_sub(max_payment_cost)
            .ok_or(Error::InsufficientPayment)?;
        let new_balance_value =
            StoredValue::CLValue(CLValue::from_t(new_balance.value()).map_err(ExecError::from)?);
        let mut effects = Effects::new();
        effects.push(TransformV2::new(
            account_main_purse_balance_key.normalize(),
            TransformKindV2::Write(new_balance_value),
        ));
        effects.push(TransformV2::new(
            proposer_main_purse_balance_key.normalize(),
            TransformKindV2::AddUInt512(max_payment_cost.value()),
        ));
        let transfers = Vec::default();
        Ok(ExecutionResult::Failure {
            error,
            effects,
            transfers,
            gas,
            messages: Vec::default(),
        })
    }

    /// Returns a wrapped `ret` by consuming object.
    pub(crate) fn take_with_ret<T: FromBytes + CLTyped>(self, ret: T) -> (Option<T>, Self) {
        (Some(ret), self)
    }

    /// Returns a self and has a return type compatible with [`ExecutionResult::take_with_ret`].
    pub(crate) fn take_without_ret<T: FromBytes + CLTyped>(self) -> (Option<T>, Self) {
        (None, self)
    }

    /// Converts a bidding result to an execution result.
    pub fn from_bidding_result(bidding_result: BiddingResult, gas: Gas) -> Option<Self> {
        match bidding_result {
            BiddingResult::RootNotFound => None,
            BiddingResult::Success { effects, .. } => Some(ExecutionResult::Success {
                transfers: vec![],
                gas,
                effects,
                messages: Messages::default(),
            }),
            BiddingResult::Failure(te) => {
                Some(ExecutionResult::Failure {
                    error: Error::TrackingCopy(te),
                    transfers: vec![],
                    gas,
                    effects: Effects::default(), // currently not returning effects on failure
                    messages: Messages::default(),
                })
            }
        }
    }

    /// Should charge for wasm errors?
    pub(crate) fn should_charge_for_errors_in_wasm(&self) -> bool {
        match self {
            ExecutionResult::Failure {
                error,
                transfers: _,
                gas: _,
                effects: _,
                messages: _,
            } => match error {
                Error::Exec(err) => matches!(
                    err,
                    ExecError::WasmPreprocessing(_) | ExecError::UnsupportedWasmStart
                ),
                Error::WasmPreprocessing(_) => true,
                _ => false,
            },
            ExecutionResult::Success { .. } => false,
        }
    }

    /// Logs execution results.
    pub fn log_execution_result(&self, preamble: &'static str) {
        trace!("{}: {:?}", preamble, self);
        match self {
            ExecutionResult::Success {
                transfers,
                gas,
                effects,
                messages,
            } => {
                debug!(
                    %gas,
                    transfer_count = %transfers.len(),
                    transforms_count = %effects.len(),
                    messages_count = %messages.len(),
                    "{}: execution success",
                    preamble
                );
            }
            ExecutionResult::Failure {
                error,
                transfers,
                gas,
                effects,
                messages,
            } => {
                debug!(
                    %error,
                    %gas,
                    transfer_count = %transfers.len(),
                    transforms_count = %effects.len(),
                    messages_count = %messages.len(),
                    "{}: execution failure",
                    preamble
                );
            }
        }
    }
}

/// A type alias that represents multiple execution results.
pub type ExecutionResults = VecDeque<ExecutionResult>;

/// Indicates the outcome of a transfer payment check.
pub enum ForcedTransferResult {
    /// Payment code ran out of gas during execution
    InsufficientPayment,
    /// Gas conversion overflow
    GasConversionOverflow,
    /// Payment code execution resulted in an error
    PaymentFailure,
}

/// A versioned execution result and the messages produced by that execution.
#[derive(Debug)]
pub struct ExecutionResultAndMessages {
    /// Execution result
    pub execution_result: TypesExecutionResult,
    /// Messages emitted during execution
    pub messages: Messages,
}

impl From<ExecutionResult> for ExecutionResultAndMessages {
    fn from(execution_result: ExecutionResult) -> Self {
        match execution_result {
            ExecutionResult::Success {
                transfers,
                gas,
                effects,
                messages,
            } => ExecutionResultAndMessages {
                execution_result: TypesExecutionResult::Success {
                    effects,
                    transfers,
                    gas,
                },
                messages,
            },
            ExecutionResult::Failure {
                error,
                transfers,
                gas,
                effects,
                messages,
            } => ExecutionResultAndMessages {
                execution_result: TypesExecutionResult::Failure {
                    effects,
                    transfers,
                    gas,
                    error_message: error.to_string(),
                },
                messages,
            },
        }
    }
}

/// Represents error conditions of an execution result builder.
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ExecutionResultBuilderError {
    /// Missing a payment execution result.
    MissingPaymentExecutionResult,
    /// Missing a session execution result.
    MissingSessionExecutionResult,
    /// Missing a finalize execution result.
    MissingFinalizeExecutionResult,
}

/// Builder object that will construct a final [`ExecutionResult`] given payment, session and
/// finalize [`ExecutionResult`]s.
#[derive(Default)]
pub struct ExecutionResultBuilder {
    payment_execution_result: Option<ExecutionResult>,
    session_execution_result: Option<ExecutionResult>,
    finalize_execution_result: Option<ExecutionResult>,
}

impl ExecutionResultBuilder {
    /// Creates new execution result builder.
    pub fn new() -> ExecutionResultBuilder {
        ExecutionResultBuilder::default()
    }

    /// Sets a payment execution result.
    pub fn set_payment_execution_result(&mut self, payment_result: ExecutionResult) -> &mut Self {
        self.payment_execution_result = Some(payment_result);
        self
    }

    /// Sets a session execution result.
    pub fn set_session_execution_result(
        &mut self,
        session_execution_result: ExecutionResult,
    ) -> &mut ExecutionResultBuilder {
        self.session_execution_result = Some(session_execution_result);
        self
    }

    /// Sets a finalize execution result.
    pub fn set_finalize_execution_result(
        &mut self,
        finalize_execution_result: ExecutionResult,
    ) -> &mut ExecutionResultBuilder {
        self.finalize_execution_result = Some(finalize_execution_result);
        self
    }

    /// Calculates the total gas cost of the execution result.
    ///
    /// Takes a payment execution result, and a session execution result and returns a sum. If
    /// either a payment or session code is not specified then a 0 is used.
    pub fn gas_used(&self) -> Gas {
        let payment_gas = self
            .payment_execution_result
            .as_ref()
            .map(ExecutionResult::gas)
            .unwrap_or_default();
        let session_gas = self
            .session_execution_result
            .as_ref()
            .map(ExecutionResult::gas)
            .unwrap_or_default();
        // TODO: Make sure this code isn't in production, as, even though it's highly unlikely
        // to happen, an integer overflow would be silently ignored in release builds.
        // NOTE: This code should have been removed in the fix of #1968, where arithmetic
        // operations on the Gas type were disabled.
        payment_gas + session_gas
    }

    /// Returns transfers from a session's execution result.
    ///
    /// If the session's execution result is not supplied then an empty [`Vec`] is returned.
    pub fn transfers(&self) -> Vec<TransferAddr> {
        self.session_execution_result
            .as_ref()
            .map(ExecutionResult::transfers)
            .cloned()
            .unwrap_or_default()
    }

    /// Builds a final [`ExecutionResult`] based on session result, payment result and a
    /// finalization result.
    pub fn build(self) -> Result<ExecutionResult, ExecutionResultBuilderError> {
        let mut error: Option<Error> = None;
        let mut transfers = self.transfers();
        let gas = self.gas_used();

        let (mut all_effects, mut all_messages) = match self.payment_execution_result {
            Some(result @ ExecutionResult::Failure { .. }) => return Ok(result),
            Some(ExecutionResult::Success {
                effects, messages, ..
            }) => (effects, messages),
            None => return Err(ExecutionResultBuilderError::MissingPaymentExecutionResult),
        };

        // session_code_spec_3: only include session exec effects if there is no session
        // exec error
        match self.session_execution_result {
            Some(ExecutionResult::Failure {
                error: session_error,
                transfers: session_transfers,
                effects: _,
                gas: _,
                messages,
            }) => {
                error = Some(session_error);
                transfers = session_transfers;
                all_messages.extend(messages);
            }
            Some(ExecutionResult::Success {
                effects, messages, ..
            }) => {
                all_effects.append(effects);
                all_messages.extend(messages);
            }
            None => return Err(ExecutionResultBuilderError::MissingSessionExecutionResult),
        };

        match self.finalize_execution_result {
            Some(ExecutionResult::Failure { .. }) => {
                // payment_code_spec_5_a: Finalization Error should only ever be raised here
                return Ok(ExecutionResult::precondition_failure(Error::Finalization));
            }
            Some(ExecutionResult::Success {
                effects, messages, ..
            }) => {
                all_effects.append(effects);
                all_messages.extend(messages);
            }
            None => return Err(ExecutionResultBuilderError::MissingFinalizeExecutionResult),
        }

        match error {
            None => Ok(ExecutionResult::Success {
                transfers,
                gas,
                effects: all_effects,
                messages: all_messages,
            }),
            Some(error) => Ok(ExecutionResult::Failure {
                error,
                transfers,
                gas,
                effects: all_effects,
                messages: all_messages,
            }),
        }
    }
}
