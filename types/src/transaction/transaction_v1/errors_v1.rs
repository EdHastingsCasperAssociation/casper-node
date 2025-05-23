use alloc::{boxed::Box, string::String, vec::Vec};
use core::{
    array::TryFromSliceError,
    fmt::{self, Display, Formatter},
};
#[cfg(feature = "std")]
use std::error::Error as StdError;
#[cfg(any(feature = "testing", test))]
use strum::EnumIter;

#[cfg(feature = "datasize")]
use datasize::DataSize;
use serde::Serialize;

#[cfg(doc)]
use super::TransactionV1;
use crate::{
    addressable_entity::ContractRuntimeTag, bytesrepr, crypto, CLType, DisplayIter, PricingMode,
    TimeDiff, Timestamp, TransactionEntryPoint, U512,
};

#[derive(Clone, Eq, PartialEq, Debug)]
#[cfg_attr(feature = "std", derive(Serialize))]
#[cfg_attr(feature = "datasize", derive(DataSize))]
pub enum FieldDeserializationError {
    IndexNotExists { index: u16 },
    FromBytesError { index: u16, error: bytesrepr::Error },
    LingeringBytesInField { index: u16 },
}

// This impl is provided due to a completeness test that we
// have in binary-port. It checks if all variants of this
// error have corresponding binary port error codes
#[cfg(any(feature = "testing", test))]
impl Default for FieldDeserializationError {
    fn default() -> Self {
        Self::IndexNotExists { index: 0 }
    }
}

/// Returned when a [`TransactionV1`] fails validation.
#[derive(Clone, Eq, PartialEq, Debug)]
#[cfg_attr(feature = "std", derive(Serialize))]
#[cfg_attr(feature = "datasize", derive(DataSize))]
#[non_exhaustive]
// This derive should not be removed due to a completeness
// test that we have in binary-port. It checks if all variants
// of this error have corresponding binary port error codes
#[cfg_attr(any(feature = "testing", test), derive(EnumIter))]
pub enum InvalidTransaction {
    /// Invalid chain name.
    InvalidChainName {
        /// The expected chain name.
        expected: String,
        /// The transaction's chain name.
        got: String,
    },

    /// Transaction is too large.
    ExcessiveSize(ExcessiveSizeErrorV1),

    /// Excessive time-to-live.
    ExcessiveTimeToLive {
        /// The time-to-live limit.
        max_ttl: TimeDiff,
        /// The transaction's time-to-live.
        got: TimeDiff,
    },

    /// Transaction's timestamp is in the future.
    TimestampInFuture {
        /// The node's timestamp when validating the transaction.
        validation_timestamp: Timestamp,
        /// Any configured leeway added to `validation_timestamp`.
        timestamp_leeway: TimeDiff,
        /// The transaction's timestamp.
        got: Timestamp,
    },

    /// The provided body hash does not match the actual hash of the body.
    InvalidBodyHash,

    /// The provided transaction hash does not match the actual hash of the transaction.
    InvalidTransactionHash,

    /// The transaction has no approvals.
    EmptyApprovals,

    /// Invalid approval.
    InvalidApproval {
        /// The index of the approval at fault.
        index: usize,
        /// The approval verification error.
        error: crypto::Error,
    },

    /// Excessive length of transaction's runtime args.
    ExcessiveArgsLength {
        /// The byte size limit of runtime arguments.
        max_length: usize,
        /// The length of the transaction's runtime arguments.
        got: usize,
    },

    /// The amount of approvals on the transaction exceeds the configured limit.
    ExcessiveApprovals {
        /// The chainspec limit for max_associated_keys.
        max_associated_keys: u32,
        /// Number of approvals on the transaction.
        got: u32,
    },

    /// The payment amount associated with the transaction exceeds the block gas limit.
    ExceedsBlockGasLimit {
        /// Configured block gas limit.
        block_gas_limit: u64,
        /// The transaction's calculated gas limit.
        got: Box<U512>,
    },

    /// Missing a required runtime arg.
    MissingArg {
        /// The name of the missing arg.
        arg_name: String,
    },

    /// Given runtime arg is not one of the expected types.
    UnexpectedArgType {
        /// The name of the invalid arg.
        arg_name: String,
        /// The choice of valid types for the given runtime arg.
        expected: Vec<String>,
        /// The provided type of the given runtime arg.
        got: String,
    },

    /// Failed to deserialize the given runtime arg.
    InvalidArg {
        /// The name of the invalid arg.
        arg_name: String,
        /// The deserialization error.
        error: bytesrepr::Error,
    },

    /// Insufficient transfer amount.
    InsufficientTransferAmount {
        /// The minimum transfer amount.
        minimum: u64,
        /// The attempted transfer amount.
        attempted: U512,
    },

    /// Insufficient burn amount.
    InsufficientBurnAmount {
        /// The minimum burn amount.
        minimum: u64,
        /// The attempted burn amount.
        attempted: U512,
    },

    /// The entry point for this transaction target cannot be `call`.
    EntryPointCannotBeCall,
    /// The entry point for this transaction target cannot be `TransactionEntryPoint::Custom`.
    EntryPointCannotBeCustom {
        /// The invalid entry point.
        entry_point: TransactionEntryPoint,
    },
    /// The entry point for this transaction target must be `TransactionEntryPoint::Custom`.
    EntryPointMustBeCustom {
        /// The invalid entry point.
        entry_point: TransactionEntryPoint,
    },
    /// The entry point for this transaction target must be `TransactionEntryPoint::Call`.
    EntryPointMustBeCall {
        /// The invalid entry point.
        entry_point: TransactionEntryPoint,
    },
    /// The transaction has empty module bytes.
    EmptyModuleBytes,
    /// Attempt to factor the amount over the gas_price failed.
    GasPriceConversion {
        /// The base amount.
        amount: u64,
        /// The attempted gas price.
        gas_price: u8,
    },
    /// Unable to calculate gas limit.
    UnableToCalculateGasLimit,
    /// Unable to calculate gas cost.
    UnableToCalculateGasCost,
    /// Invalid combination of pricing handling and pricing mode.
    InvalidPricingMode {
        /// The pricing mode as specified by the transaction.
        price_mode: PricingMode,
    },
    /// The transaction provided is not supported.
    InvalidTransactionLane(u8),
    /// Could not match v1 with transaction lane
    NoLaneMatch,
    /// Gas price tolerance too low.
    GasPriceToleranceTooLow {
        /// The minimum gas price tolerance.
        min_gas_price_tolerance: u8,
        /// The provided gas price tolerance.
        provided_gas_price_tolerance: u8,
    },
    /// Error when trying to deserialize one of the transactionV1 payload fields.
    CouldNotDeserializeField {
        /// Underlying reason why the deserialization failed
        error: FieldDeserializationError,
    },

    /// Unable to calculate hash for payloads transaction.
    CannotCalculateFieldsHash,

    /// The transactions field map had entries that were unexpected
    UnexpectedTransactionFieldEntries,
    /// The transaction requires named arguments.
    ExpectedNamedArguments,
    /// The transaction required bytes arguments.
    ExpectedBytesArguments,
    /// The transaction runtime is invalid.
    InvalidTransactionRuntime {
        /// The expected runtime as specified by the chainspec.
        expected: ContractRuntimeTag,
    },
    /// The transaction is missing a seed field.
    MissingSeed,
    // Pricing mode not implemented yet
    PricingModeNotSupported,
    // Invalid payment amount.
    InvalidPaymentAmount,
    /// Unexpected entry point detected.
    UnexpectedEntryPoint {
        entry_point: TransactionEntryPoint,
        lane_id: u8,
    },
    /// Could not serialize transaction
    CouldNotSerializeTransaction,

    /// Insufficient value for amount argument.
    InsufficientAmount {
        /// The attempted amount.
        attempted: U512,
    },

    /// Invalid minimum delegation amount.
    InvalidMinimumDelegationAmount {
        /// The lowest allowed amount.
        floor: u64,
        /// The attempted amount.
        attempted: u64,
    },

    /// Invalid maximum delegation amount.
    InvalidMaximumDelegationAmount {
        /// The highest allowed amount.
        ceiling: u64,
        /// The attempted amount.
        attempted: u64,
    },

    /// Invalid reserved slots.
    InvalidReservedSlots {
        /// The highest allowed amount.
        ceiling: u32,
        /// The attempted amount.
        attempted: u64,
    },

    /// Invalid delegation amount.
    InvalidDelegationAmount {
        /// The highest allowed amount.
        ceiling: u64,
        /// The attempted amount.
        attempted: U512,
    },

    // Passing TransactionInvocationTarget::ByPackageHash::version or
    // TransactionInvocationTarget::ByPackageName::version is no longer supported
    TargetingPackageVersionNotSupported,
}

impl Display for InvalidTransaction {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        match self {
            InvalidTransaction::InvalidChainName { expected, got } => {
                                        write!(
                                            formatter,
                                            "invalid chain name: expected {expected}, got {got}"
                                        )
                                    }
            InvalidTransaction::ExcessiveSize(error) => {
                                        write!(formatter, "transaction size too large: {error}")
                                    }
            InvalidTransaction::ExcessiveTimeToLive { max_ttl, got } => {
                                        write!(
                                            formatter,
                                            "time-to-live of {got} exceeds limit of {max_ttl}"
                                        )
                                    }
            InvalidTransaction::TimestampInFuture {
                                        validation_timestamp,
                                        timestamp_leeway,
                                        got,
                                    } => {
                                        write!(
                                            formatter,
                                            "timestamp of {got} is later than node's validation timestamp of \
                    {validation_timestamp} plus leeway of {timestamp_leeway}"
                                        )
                                    }
            InvalidTransaction::InvalidBodyHash => {
                                        write!(
                                            formatter,
                                            "the provided hash does not match the actual hash of the transaction body"
                                        )
                                    }
            InvalidTransaction::InvalidTransactionHash => {
                                        write!(
                                            formatter,
                                            "the provided hash does not match the actual hash of the transaction"
                                        )
                                    }
            InvalidTransaction::EmptyApprovals => {
                                        write!(formatter, "the transaction has no approvals")
                                    }
            InvalidTransaction::InvalidApproval { index, error } => {
                                        write!(
                                            formatter,
                                            "the transaction approval at index {index} is invalid: {error}"
                                        )
                                    }
            InvalidTransaction::ExcessiveArgsLength { max_length, got } => {
                                        write!(
                                            formatter,
                                            "serialized transaction runtime args of {got} bytes exceeds limit of \
                    {max_length} bytes"
                                        )
                                    }
            InvalidTransaction::ExcessiveApprovals {
                                        max_associated_keys,
                                        got,
                                    } => {
                                        write!(
                                            formatter,
                                            "number of transaction approvals {got} exceeds the maximum number of \
                    associated keys {max_associated_keys}",
                                        )
                                    }
            InvalidTransaction::ExceedsBlockGasLimit {
                                        block_gas_limit,
                                        got,
                                    } => {
                                        write!(
                                            formatter,
                                            "payment amount of {got} exceeds the block gas limit of {block_gas_limit}"
                                        )
                                    }
            InvalidTransaction::MissingArg { arg_name } => {
                                        write!(formatter, "missing required runtime argument '{arg_name}'")
                                    }
            InvalidTransaction::UnexpectedArgType {
                                        arg_name,
                                        expected,
                                        got,
                                    } => {
                                        write!(
                                            formatter,
                                            "expected type of '{arg_name}' runtime argument to be one of {}, but got {got}",
                                            DisplayIter::new(expected)
                                        )
                                    }
            InvalidTransaction::InvalidArg { arg_name, error } => {
                                        write!(formatter, "invalid runtime argument '{arg_name}': {error}")
                                    }
            InvalidTransaction::InsufficientTransferAmount { minimum, attempted } => {
                                        write!(
                                            formatter,
                                            "insufficient transfer amount; minimum: {minimum} attempted: {attempted}"
                                        )
                                    }
            InvalidTransaction::EntryPointCannotBeCall => {
                                        write!(formatter, "entry point cannot be call")
                                    }
            InvalidTransaction::EntryPointCannotBeCustom { entry_point } => {
                                        write!(formatter, "entry point cannot be custom: {entry_point}")
                                    }
            InvalidTransaction::EntryPointMustBeCustom { entry_point } => {
                                        write!(formatter, "entry point must be custom: {entry_point}")
                                    }
            InvalidTransaction::EmptyModuleBytes => {
                                        write!(formatter, "the transaction has empty module bytes")
                                    }
            InvalidTransaction::GasPriceConversion { amount, gas_price } => {
                                        write!(
                                            formatter,
                                            "failed to divide the amount {} by the gas price {}",
                                            amount, gas_price
                                        )
                                    }
            InvalidTransaction::UnableToCalculateGasLimit => {
                                        write!(formatter, "unable to calculate gas limit", )
                                    }
            InvalidTransaction::UnableToCalculateGasCost => {
                                        write!(formatter, "unable to calculate gas cost", )
                                    }
            InvalidTransaction::InvalidPricingMode { price_mode } => {
                                        write!(
                                            formatter,
                                            "received a transaction with an invalid mode {price_mode}"
                                        )
                                    }
            InvalidTransaction::InvalidTransactionLane(kind) => {
                                        write!(
                                            formatter,
                                            "received a transaction with an invalid kind {kind}"
                                        )
                                    }
            InvalidTransaction::GasPriceToleranceTooLow {
                                        min_gas_price_tolerance,
                                        provided_gas_price_tolerance,
                                    } => {
                                        write!(
                                            formatter,
                                            "received a transaction with gas price tolerance {} but this chain will only go as low as {}",
                                            provided_gas_price_tolerance, min_gas_price_tolerance
                                        )
                                    }
            InvalidTransaction::CouldNotDeserializeField { error } => {
                                        match error {
                                            FieldDeserializationError::IndexNotExists { index } => write!(
                                                formatter,
                                                "tried to deserialize a field under index {} but it is not present in the payload",
                                                index
                                            ),
                                            FieldDeserializationError::FromBytesError { index, error } => write!(
                                                formatter,
                                                "tried to deserialize a field under index {} but it failed with error: {}",
                                                index,
                                                error
                                            ),
                                            FieldDeserializationError::LingeringBytesInField { index } => write!(
                                                formatter,
                                                "tried to deserialize a field under index {} but after deserialization there were still bytes left",
                                                index,
                                            ),
                                        }
                                    }
            InvalidTransaction::CannotCalculateFieldsHash => write!(
                                        formatter,
                                        "cannot calculate a hash digest for the transaction"
                                    ),
            InvalidTransaction::EntryPointMustBeCall { entry_point } => {
                                write!(formatter, "entry point must be call: {entry_point}")
                            }
            InvalidTransaction::NoLaneMatch => write!(formatter, "Could not match any lane to the specified transaction"),
            InvalidTransaction::UnexpectedTransactionFieldEntries => write!(formatter, "There were entries in the fields map of the payload that could not be matched"),
            InvalidTransaction::ExpectedNamedArguments => {
                                        write!(formatter, "transaction requires named arguments")
                                    }
            InvalidTransaction::ExpectedBytesArguments => {
                                        write!(formatter, "transaction requires bytes arguments")
                                    }
            InvalidTransaction::InvalidTransactionRuntime { expected } => {
                                        write!(
                                            formatter,
                                            "invalid transaction runtime: expected {expected}"
                                        )
                                    }
            InvalidTransaction::MissingSeed => {
                                        write!(formatter, "missing seed for install or upgrade")
                                    }
            InvalidTransaction::PricingModeNotSupported => {
                                        write!(formatter, "Pricing mode not supported")
                                    }
            InvalidTransaction::InvalidPaymentAmount => {
                                        write!(formatter, "invalid payment amount")
                                    }
            InvalidTransaction::UnexpectedEntryPoint {
                                        entry_point, lane_id
                                    } => {
                                        write!(formatter, "unexpected entry_point {} lane_id {}", entry_point, lane_id)
                                    }
            InvalidTransaction::InsufficientBurnAmount { minimum, attempted } => {
                                        write!(formatter, "insufficient burn amount: {minimum} {attempted}")
                                    }
            InvalidTransaction::CouldNotSerializeTransaction => write!(formatter, "Could not serialize transaction."),
            InvalidTransaction::InsufficientAmount { attempted } => {
                                write!(
                                    formatter,
                                    "the value provided for the argument ({attempted}) named amount is too low.",
                                )
                            }
            InvalidTransaction::InvalidMinimumDelegationAmount { floor, attempted } => {
                                write!(
                                    formatter,
                                    "the value provided for the minimum delegation amount ({attempted}) cannot be lower than {floor}.",
                                )}
            InvalidTransaction::InvalidMaximumDelegationAmount { ceiling, attempted } => {
                                write!(
                                    formatter,
                                    "the value provided for the maximum delegation amount ({ceiling}) cannot be higher than {attempted}.",
                                )}
            InvalidTransaction::InvalidReservedSlots { ceiling, attempted } => {
                                write!(
                                    formatter,
                                    "the value provided for reserved slots ({ceiling}) cannot be higher than {attempted}.",
                                )}
            InvalidTransaction::InvalidDelegationAmount { ceiling, attempted } => {
                        write!(
                        formatter,
                        "the value provided for the delegation amount ({attempted}) cannot be higher than {ceiling}.",
                        )}
            InvalidTransaction::TargetingPackageVersionNotSupported =>  write!(formatter, "passing `version` in TransactionInvocationTarget::ByPackageHash or TransactionInvocationTarget::ByPackageName is not supported",),
        }
    }
}

impl From<ExcessiveSizeErrorV1> for InvalidTransaction {
    fn from(error: ExcessiveSizeErrorV1) -> Self {
        InvalidTransaction::ExcessiveSize(error)
    }
}

#[cfg(feature = "std")]
impl StdError for InvalidTransaction {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            InvalidTransaction::InvalidApproval { error, .. } => Some(error),
            InvalidTransaction::InvalidArg { error, .. } => Some(error),
            InvalidTransaction::InvalidChainName { .. }
            | InvalidTransaction::ExcessiveSize(_)
            | InvalidTransaction::ExcessiveTimeToLive { .. }
            | InvalidTransaction::TimestampInFuture { .. }
            | InvalidTransaction::InvalidBodyHash
            | InvalidTransaction::InvalidTransactionHash
            | InvalidTransaction::EmptyApprovals
            | InvalidTransaction::ExcessiveArgsLength { .. }
            | InvalidTransaction::ExcessiveApprovals { .. }
            | InvalidTransaction::ExceedsBlockGasLimit { .. }
            | InvalidTransaction::MissingArg { .. }
            | InvalidTransaction::UnexpectedArgType { .. }
            | InvalidTransaction::InsufficientTransferAmount { .. }
            | InvalidTransaction::EntryPointCannotBeCall
            | InvalidTransaction::EntryPointCannotBeCustom { .. }
            | InvalidTransaction::EntryPointMustBeCustom { .. }
            | InvalidTransaction::EntryPointMustBeCall { .. }
            | InvalidTransaction::EmptyModuleBytes
            | InvalidTransaction::GasPriceConversion { .. }
            | InvalidTransaction::UnableToCalculateGasLimit
            | InvalidTransaction::UnableToCalculateGasCost
            | InvalidTransaction::InvalidPricingMode { .. }
            | InvalidTransaction::GasPriceToleranceTooLow { .. }
            | InvalidTransaction::InvalidTransactionLane(_)
            | InvalidTransaction::CannotCalculateFieldsHash
            | InvalidTransaction::NoLaneMatch
            | InvalidTransaction::UnexpectedTransactionFieldEntries => None,
            InvalidTransaction::CouldNotDeserializeField { error } => match error {
                FieldDeserializationError::IndexNotExists { .. }
                | FieldDeserializationError::LingeringBytesInField { .. } => None,
                FieldDeserializationError::FromBytesError { error, .. } => Some(error),
            },
            InvalidTransaction::ExpectedNamedArguments
            | InvalidTransaction::ExpectedBytesArguments
            | InvalidTransaction::InvalidTransactionRuntime { .. }
            | InvalidTransaction::MissingSeed
            | InvalidTransaction::PricingModeNotSupported
            | InvalidTransaction::InvalidPaymentAmount
            | InvalidTransaction::InsufficientBurnAmount { .. }
            | InvalidTransaction::UnexpectedEntryPoint { .. }
            | InvalidTransaction::CouldNotSerializeTransaction
            | InvalidTransaction::InsufficientAmount { .. }
            | InvalidTransaction::InvalidMinimumDelegationAmount { .. }
            | InvalidTransaction::InvalidMaximumDelegationAmount { .. }
            | InvalidTransaction::InvalidReservedSlots { .. }
            | InvalidTransaction::InvalidDelegationAmount { .. }
            | InvalidTransaction::TargetingPackageVersionNotSupported => None,
        }
    }
}

impl InvalidTransaction {
    pub fn unexpected_arg_type(arg_name: String, expected: Vec<CLType>, got: CLType) -> Self {
        let expected = expected.iter().map(|el| format!("{}", el)).collect();
        InvalidTransaction::UnexpectedArgType {
            arg_name,
            expected,
            got: format!("{}", got),
        }
    }
}
/// Error returned when a transaction is too large.
#[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Debug, Serialize)]
#[cfg_attr(feature = "datasize", derive(DataSize))]
//Default is needed only in testing to meet EnumIter needs
#[cfg_attr(any(feature = "testing", test), derive(Default))]
pub struct ExcessiveSizeErrorV1 {
    /// The maximum permitted serialized transaction size, in bytes.
    pub max_transaction_size: u32,
    /// The serialized size of the transaction provided, in bytes.
    pub actual_transaction_size: usize,
}

impl Display for ExcessiveSizeErrorV1 {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        write!(
            formatter,
            "transaction size of {} bytes exceeds limit of {}",
            self.actual_transaction_size, self.max_transaction_size
        )
    }
}

#[cfg(feature = "std")]
impl StdError for ExcessiveSizeErrorV1 {}

/// Errors other than validation failures relating to Transactions.
#[derive(Debug)]
#[non_exhaustive]
pub enum ErrorV1 {
    /// Error while encoding to JSON.
    EncodeToJson(serde_json::Error),

    /// Error while decoding from JSON.
    DecodeFromJson(DecodeFromJsonErrorV1),

    /// Unable to calculate payment.
    InvalidPayment,
}

impl From<serde_json::Error> for ErrorV1 {
    fn from(error: serde_json::Error) -> Self {
        ErrorV1::EncodeToJson(error)
    }
}

impl From<DecodeFromJsonErrorV1> for ErrorV1 {
    fn from(error: DecodeFromJsonErrorV1) -> Self {
        ErrorV1::DecodeFromJson(error)
    }
}

impl Display for ErrorV1 {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        match self {
            ErrorV1::EncodeToJson(error) => {
                write!(formatter, "encoding to json: {}", error)
            }
            ErrorV1::DecodeFromJson(error) => {
                write!(formatter, "decoding from json: {}", error)
            }
            ErrorV1::InvalidPayment => write!(formatter, "invalid payment"),
        }
    }
}

#[cfg(feature = "std")]
impl StdError for ErrorV1 {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            ErrorV1::EncodeToJson(error) => Some(error),
            ErrorV1::DecodeFromJson(error) => Some(error),
            ErrorV1::InvalidPayment => None,
        }
    }
}

/// Error while decoding a `TransactionV1` from JSON.
#[derive(Debug)]
#[non_exhaustive]
pub enum DecodeFromJsonErrorV1 {
    /// Failed to decode from base 16.
    FromHex(base16::DecodeError),

    /// Failed to convert slice to array.
    TryFromSlice(TryFromSliceError),
}

impl From<base16::DecodeError> for DecodeFromJsonErrorV1 {
    fn from(error: base16::DecodeError) -> Self {
        DecodeFromJsonErrorV1::FromHex(error)
    }
}

impl From<TryFromSliceError> for DecodeFromJsonErrorV1 {
    fn from(error: TryFromSliceError) -> Self {
        DecodeFromJsonErrorV1::TryFromSlice(error)
    }
}

impl Display for DecodeFromJsonErrorV1 {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        match self {
            DecodeFromJsonErrorV1::FromHex(error) => {
                write!(formatter, "{}", error)
            }
            DecodeFromJsonErrorV1::TryFromSlice(error) => {
                write!(formatter, "{}", error)
            }
        }
    }
}

#[cfg(feature = "std")]
impl StdError for DecodeFromJsonErrorV1 {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            DecodeFromJsonErrorV1::FromHex(error) => Some(error),
            DecodeFromJsonErrorV1::TryFromSlice(error) => Some(error),
        }
    }
}
