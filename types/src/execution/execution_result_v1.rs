//! Types for reporting results of execution pre `casper-node` v2.0.0.

use core::convert::TryFrom;

use alloc::{boxed::Box, string::String, vec::Vec};

#[cfg(feature = "datasize")]
use datasize::DataSize;
use num::{FromPrimitive, ToPrimitive};
use num_derive::{FromPrimitive, ToPrimitive};
#[cfg(any(feature = "testing", test))]
use rand::{
    distributions::{Distribution, Standard},
    seq::SliceRandom,
    Rng,
};
#[cfg(feature = "json-schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    account::AccountHash,
    bytesrepr::{self, FromBytes, ToBytes, U8_SERIALIZED_LENGTH},
    system::auction::{Bid, BidKind, EraInfo, UnbondingPurse, WithdrawPurse},
    CLValue, DeployInfo, Key, TransferAddr, TransferV1, U128, U256, U512,
};

#[derive(FromPrimitive, ToPrimitive, Debug)]
#[repr(u8)]
enum ExecutionResultTag {
    Failure = 0,
    Success = 1,
}

impl TryFrom<u8> for ExecutionResultTag {
    type Error = bytesrepr::Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        FromPrimitive::from_u8(value).ok_or(bytesrepr::Error::Formatting)
    }
}

#[derive(FromPrimitive, ToPrimitive, Debug)]
#[repr(u8)]
enum OpTag {
    Read = 0,
    Write = 1,
    Add = 2,
    NoOp = 3,
    Prune = 4,
}

impl TryFrom<u8> for OpTag {
    type Error = bytesrepr::Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        FromPrimitive::from_u8(value).ok_or(bytesrepr::Error::Formatting)
    }
}

#[derive(FromPrimitive, ToPrimitive, Debug)]
#[repr(u8)]
enum TransformTag {
    Identity = 0,
    WriteCLValue = 1,
    WriteAccount = 2,
    WriteByteCode = 3,
    WriteContract = 4,
    WritePackage = 5,
    WriteDeployInfo = 6,
    WriteTransfer = 7,
    WriteEraInfo = 8,
    WriteBid = 9,
    WriteWithdraw = 10,
    AddInt32 = 11,
    AddUInt64 = 12,
    AddUInt128 = 13,
    AddUInt256 = 14,
    AddUInt512 = 15,
    AddKeys = 16,
    Failure = 17,
    WriteUnbonding = 18,
    WriteAddressableEntity = 19,
    Prune = 20,
    WriteBidKind = 21,
}

impl TryFrom<u8> for TransformTag {
    type Error = bytesrepr::Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        FromPrimitive::from_u8(value).ok_or(bytesrepr::Error::Formatting)
    }
}

/// The result of executing a single deploy.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize, Debug)]
#[cfg_attr(feature = "datasize", derive(DataSize))]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[serde(deny_unknown_fields)]
pub enum ExecutionResultV1 {
    /// The result of a failed execution.
    Failure {
        /// The effect of executing the deploy.
        effect: ExecutionEffect,
        /// A record of version 1 Transfers performed while executing the deploy.
        transfers: Vec<TransferAddr>,
        /// The cost of executing the deploy.
        cost: U512,
        /// The error message associated with executing the deploy.
        error_message: String,
    },
    /// The result of a successful execution.
    Success {
        /// The effect of executing the deploy.
        effect: ExecutionEffect,
        /// A record of Transfers performed while executing the deploy.
        transfers: Vec<TransferAddr>,
        /// The cost of executing the deploy.
        cost: U512,
    },
}

impl ExecutionResultV1 {
    /// Returns cost amount.
    pub fn cost(&self) -> U512 {
        match self {
            ExecutionResultV1::Failure { cost, .. } | ExecutionResultV1::Success { cost, .. } => {
                *cost
            }
        }
    }
}

#[cfg(any(feature = "testing", test))]
impl Distribution<ExecutionResultV1> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> ExecutionResultV1 {
        let op_count = rng.gen_range(0..6);
        let mut operations = Vec::new();
        for _ in 0..op_count {
            let op = [OpKind::Read, OpKind::Add, OpKind::NoOp, OpKind::Write]
                .choose(rng)
                .unwrap();
            operations.push(Operation {
                key: rng.gen::<u64>().to_string(),
                kind: *op,
            });
        }

        let transform_count = rng.gen_range(0..6);
        let mut transforms = Vec::new();
        for _ in 0..transform_count {
            transforms.push(TransformV1 {
                key: rng.gen::<u64>().to_string(),
                transform: rng.gen(),
            });
        }

        let execution_effect = ExecutionEffect {
            operations,
            transforms,
        };

        let transfer_count = rng.gen_range(0..6);
        let mut transfers = Vec::new();
        for _ in 0..transfer_count {
            transfers.push(TransferAddr::new(rng.gen()))
        }

        if rng.gen() {
            ExecutionResultV1::Failure {
                effect: execution_effect,
                transfers,
                cost: rng.gen::<u64>().into(),
                error_message: format!("Error message {}", rng.gen::<u64>()),
            }
        } else {
            ExecutionResultV1::Success {
                effect: execution_effect,
                transfers,
                cost: rng.gen::<u64>().into(),
            }
        }
    }
}

impl ToBytes for ExecutionResultV1 {
    fn write_bytes(&self, writer: &mut Vec<u8>) -> Result<(), bytesrepr::Error> {
        match self {
            ExecutionResultV1::Failure {
                effect,
                transfers,
                cost,
                error_message,
            } => {
                (ExecutionResultTag::Failure as u8).write_bytes(writer)?;
                effect.write_bytes(writer)?;
                transfers.write_bytes(writer)?;
                cost.write_bytes(writer)?;
                error_message.write_bytes(writer)
            }
            ExecutionResultV1::Success {
                effect,
                transfers,
                cost,
            } => {
                (ExecutionResultTag::Success as u8).write_bytes(writer)?;
                effect.write_bytes(writer)?;
                transfers.write_bytes(writer)?;
                cost.write_bytes(writer)
            }
        }
    }

    fn to_bytes(&self) -> Result<Vec<u8>, bytesrepr::Error> {
        let mut buffer = bytesrepr::allocate_buffer(self)?;
        self.write_bytes(&mut buffer)?;
        Ok(buffer)
    }

    fn serialized_length(&self) -> usize {
        U8_SERIALIZED_LENGTH
            + match self {
                ExecutionResultV1::Failure {
                    effect,
                    transfers,
                    cost,
                    error_message,
                } => {
                    effect.serialized_length()
                        + transfers.serialized_length()
                        + cost.serialized_length()
                        + error_message.serialized_length()
                }
                ExecutionResultV1::Success {
                    effect,
                    transfers,
                    cost,
                } => {
                    effect.serialized_length()
                        + transfers.serialized_length()
                        + cost.serialized_length()
                }
            }
    }
}

impl FromBytes for ExecutionResultV1 {
    fn from_bytes(bytes: &[u8]) -> Result<(Self, &[u8]), bytesrepr::Error> {
        let (tag, remainder) = u8::from_bytes(bytes)?;
        match TryFrom::try_from(tag)? {
            ExecutionResultTag::Failure => {
                let (effect, remainder) = ExecutionEffect::from_bytes(remainder)?;
                let (transfers, remainder) = Vec::<TransferAddr>::from_bytes(remainder)?;
                let (cost, remainder) = U512::from_bytes(remainder)?;
                let (error_message, remainder) = String::from_bytes(remainder)?;
                let execution_result = ExecutionResultV1::Failure {
                    effect,
                    transfers,
                    cost,
                    error_message,
                };
                Ok((execution_result, remainder))
            }
            ExecutionResultTag::Success => {
                let (execution_effect, remainder) = ExecutionEffect::from_bytes(remainder)?;
                let (transfers, remainder) = Vec::<TransferAddr>::from_bytes(remainder)?;
                let (cost, remainder) = U512::from_bytes(remainder)?;
                let execution_result = ExecutionResultV1::Success {
                    effect: execution_effect,
                    transfers,
                    cost,
                };
                Ok((execution_result, remainder))
            }
        }
    }
}

/// The sequence of execution transforms from a single deploy.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize, Default, Debug)]
#[cfg_attr(feature = "datasize", derive(DataSize))]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ExecutionEffect {
    /// The resulting operations.
    pub operations: Vec<Operation>,
    /// The sequence of execution transforms.
    pub transforms: Vec<TransformV1>,
}

impl ToBytes for ExecutionEffect {
    fn write_bytes(&self, writer: &mut Vec<u8>) -> Result<(), bytesrepr::Error> {
        self.operations.write_bytes(writer)?;
        self.transforms.write_bytes(writer)
    }

    fn to_bytes(&self) -> Result<Vec<u8>, bytesrepr::Error> {
        let mut buffer = bytesrepr::allocate_buffer(self)?;
        self.write_bytes(&mut buffer)?;
        Ok(buffer)
    }

    fn serialized_length(&self) -> usize {
        self.operations.serialized_length() + self.transforms.serialized_length()
    }
}

impl FromBytes for ExecutionEffect {
    fn from_bytes(bytes: &[u8]) -> Result<(Self, &[u8]), bytesrepr::Error> {
        let (operations, remainder) = Vec::<Operation>::from_bytes(bytes)?;
        let (transforms, remainder) = Vec::<TransformV1>::from_bytes(remainder)?;
        let json_effects = ExecutionEffect {
            operations,
            transforms,
        };
        Ok((json_effects, remainder))
    }
}

/// An operation performed while executing a deploy.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize, Debug)]
#[cfg_attr(feature = "datasize", derive(DataSize))]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Operation {
    /// The formatted string of the `Key`.
    pub key: String,
    /// The type of operation.
    pub kind: OpKind,
}

impl ToBytes for Operation {
    fn write_bytes(&self, writer: &mut Vec<u8>) -> Result<(), bytesrepr::Error> {
        self.key.write_bytes(writer)?;
        self.kind.write_bytes(writer)
    }

    fn to_bytes(&self) -> Result<Vec<u8>, bytesrepr::Error> {
        let mut buffer = bytesrepr::allocate_buffer(self)?;
        self.write_bytes(&mut buffer)?;
        Ok(buffer)
    }

    fn serialized_length(&self) -> usize {
        self.key.serialized_length() + self.kind.serialized_length()
    }
}

impl FromBytes for Operation {
    fn from_bytes(bytes: &[u8]) -> Result<(Self, &[u8]), bytesrepr::Error> {
        let (key, remainder) = String::from_bytes(bytes)?;
        let (kind, remainder) = OpKind::from_bytes(remainder)?;
        let operation = Operation { key, kind };
        Ok((operation, remainder))
    }
}

/// The type of operation performed while executing a deploy.
#[derive(Clone, Copy, Eq, PartialEq, Serialize, Deserialize, Debug)]
#[cfg_attr(feature = "datasize", derive(DataSize))]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[serde(deny_unknown_fields)]
pub enum OpKind {
    /// A read operation.
    Read,
    /// A write operation.
    Write,
    /// An addition.
    Add,
    /// An operation which has no effect.
    NoOp,
    /// A prune operation.
    Prune,
}

impl OpKind {
    fn tag(&self) -> OpTag {
        match self {
            OpKind::Read => OpTag::Read,
            OpKind::Write => OpTag::Write,
            OpKind::Add => OpTag::Add,
            OpKind::NoOp => OpTag::NoOp,
            OpKind::Prune => OpTag::Prune,
        }
    }
}

impl ToBytes for OpKind {
    fn write_bytes(&self, writer: &mut Vec<u8>) -> Result<(), bytesrepr::Error> {
        let tag_byte = self.tag().to_u8().ok_or(bytesrepr::Error::Formatting)?;
        tag_byte.write_bytes(writer)
    }

    fn to_bytes(&self) -> Result<Vec<u8>, bytesrepr::Error> {
        let mut buffer = bytesrepr::allocate_buffer(self)?;
        self.write_bytes(&mut buffer)?;
        Ok(buffer)
    }

    fn serialized_length(&self) -> usize {
        U8_SERIALIZED_LENGTH
    }
}

impl FromBytes for OpKind {
    fn from_bytes(bytes: &[u8]) -> Result<(Self, &[u8]), bytesrepr::Error> {
        let (tag, remainder) = u8::from_bytes(bytes)?;
        match TryFrom::try_from(tag)? {
            OpTag::Read => Ok((OpKind::Read, remainder)),
            OpTag::Write => Ok((OpKind::Write, remainder)),
            OpTag::Add => Ok((OpKind::Add, remainder)),
            OpTag::NoOp => Ok((OpKind::NoOp, remainder)),
            OpTag::Prune => Ok((OpKind::Prune, remainder)),
        }
    }
}

/// A transformation performed while executing a deploy.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize, Debug)]
#[cfg_attr(feature = "datasize", derive(DataSize))]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct TransformV1 {
    /// The formatted string of the `Key`.
    pub key: String,
    /// The transformation.
    pub transform: TransformKindV1,
}

impl ToBytes for TransformV1 {
    fn write_bytes(&self, writer: &mut Vec<u8>) -> Result<(), bytesrepr::Error> {
        self.key.write_bytes(writer)?;
        self.transform.write_bytes(writer)
    }

    fn to_bytes(&self) -> Result<Vec<u8>, bytesrepr::Error> {
        let mut buffer = bytesrepr::allocate_buffer(self)?;
        self.write_bytes(&mut buffer)?;
        Ok(buffer)
    }

    fn serialized_length(&self) -> usize {
        self.key.serialized_length() + self.transform.serialized_length()
    }
}

impl FromBytes for TransformV1 {
    fn from_bytes(bytes: &[u8]) -> Result<(Self, &[u8]), bytesrepr::Error> {
        let (key, remainder) = String::from_bytes(bytes)?;
        let (transform, remainder) = TransformKindV1::from_bytes(remainder)?;
        let transform_entry = TransformV1 { key, transform };
        Ok((transform_entry, remainder))
    }
}

/// The actual transformation performed while executing a deploy.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize, Debug)]
#[cfg_attr(feature = "datasize", derive(DataSize))]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[serde(deny_unknown_fields)]
pub enum TransformKindV1 {
    /// A transform having no effect.
    Identity,
    /// Writes the given CLValue to global state.
    WriteCLValue(CLValue),
    /// Writes the given Account to global state.
    WriteAccount(AccountHash),
    /// Writes a smart contract as Wasm to global state.
    WriteContractWasm,
    /// Writes a smart contract to global state.
    WriteContract,
    /// Writes a smart contract package to global state.
    WriteContractPackage,
    /// Writes the given DeployInfo to global state.
    WriteDeployInfo(DeployInfo),
    /// Writes the given EraInfo to global state.
    WriteEraInfo(EraInfo),
    /// Writes the given version 1 Transfer to global state.
    WriteTransfer(TransferV1),
    /// Writes the given Bid to global state.
    WriteBid(Box<Bid>),
    /// Writes the given Withdraw to global state.
    WriteWithdraw(Vec<WithdrawPurse>),
    /// Adds the given `i32`.
    AddInt32(i32),
    /// Adds the given `u64`.
    AddUInt64(u64),
    /// Adds the given `U128`.
    AddUInt128(U128),
    /// Adds the given `U256`.
    AddUInt256(U256),
    /// Adds the given `U512`.
    AddUInt512(U512),
    /// Adds the given collection of named keys.
    AddKeys(Vec<NamedKey>),
    /// A failed transformation, containing an error message.
    Failure(String),
    /// Writes the given Unbonding to global state.
    WriteUnbonding(Vec<UnbondingPurse>),
    /// Writes the addressable entity to global state.
    WriteAddressableEntity,
    /// Removes pathing to keyed value within global state. This is a form of soft delete; the
    /// underlying value remains in global state and is reachable from older global state root
    /// hashes where it was included in the hash up.
    Prune(Key),
    /// Writes the given BidKind to global state.
    WriteBidKind(BidKind),
}

impl ToBytes for TransformKindV1 {
    fn write_bytes(&self, writer: &mut Vec<u8>) -> Result<(), bytesrepr::Error> {
        match self {
            TransformKindV1::Identity => (TransformTag::Identity as u8).write_bytes(writer),
            TransformKindV1::WriteCLValue(value) => {
                (TransformTag::WriteCLValue as u8).write_bytes(writer)?;
                value.write_bytes(writer)
            }
            TransformKindV1::WriteAccount(account_hash) => {
                (TransformTag::WriteAccount as u8).write_bytes(writer)?;
                account_hash.write_bytes(writer)
            }
            TransformKindV1::WriteContractWasm => {
                (TransformTag::WriteByteCode as u8).write_bytes(writer)
            }
            TransformKindV1::WriteContract => {
                (TransformTag::WriteContract as u8).write_bytes(writer)
            }
            TransformKindV1::WriteContractPackage => {
                (TransformTag::WritePackage as u8).write_bytes(writer)
            }
            TransformKindV1::WriteDeployInfo(deploy_info) => {
                (TransformTag::WriteDeployInfo as u8).write_bytes(writer)?;
                deploy_info.write_bytes(writer)
            }
            TransformKindV1::WriteEraInfo(era_info) => {
                (TransformTag::WriteEraInfo as u8).write_bytes(writer)?;
                era_info.write_bytes(writer)
            }
            TransformKindV1::WriteTransfer(transfer) => {
                (TransformTag::WriteTransfer as u8).write_bytes(writer)?;
                transfer.write_bytes(writer)
            }
            TransformKindV1::WriteBid(bid) => {
                (TransformTag::WriteBid as u8).write_bytes(writer)?;
                bid.write_bytes(writer)
            }
            TransformKindV1::WriteWithdraw(unbonding_purses) => {
                (TransformTag::WriteWithdraw as u8).write_bytes(writer)?;
                unbonding_purses.write_bytes(writer)
            }
            TransformKindV1::AddInt32(value) => {
                (TransformTag::AddInt32 as u8).write_bytes(writer)?;
                value.write_bytes(writer)
            }
            TransformKindV1::AddUInt64(value) => {
                (TransformTag::AddUInt64 as u8).write_bytes(writer)?;
                value.write_bytes(writer)
            }
            TransformKindV1::AddUInt128(value) => {
                (TransformTag::AddUInt128 as u8).write_bytes(writer)?;
                value.write_bytes(writer)
            }
            TransformKindV1::AddUInt256(value) => {
                (TransformTag::AddUInt256 as u8).write_bytes(writer)?;
                value.write_bytes(writer)
            }
            TransformKindV1::AddUInt512(value) => {
                (TransformTag::AddUInt512 as u8).write_bytes(writer)?;
                value.write_bytes(writer)
            }
            TransformKindV1::AddKeys(value) => {
                (TransformTag::AddKeys as u8).write_bytes(writer)?;
                value.write_bytes(writer)
            }
            TransformKindV1::Failure(value) => {
                (TransformTag::Failure as u8).write_bytes(writer)?;
                value.write_bytes(writer)
            }
            TransformKindV1::WriteUnbonding(value) => {
                (TransformTag::WriteUnbonding as u8).write_bytes(writer)?;
                value.write_bytes(writer)
            }
            TransformKindV1::WriteAddressableEntity => {
                (TransformTag::WriteAddressableEntity as u8).write_bytes(writer)
            }
            TransformKindV1::Prune(value) => {
                (TransformTag::Prune as u8).write_bytes(writer)?;
                value.write_bytes(writer)
            }
            TransformKindV1::WriteBidKind(value) => {
                (TransformTag::WriteBidKind as u8).write_bytes(writer)?;
                value.write_bytes(writer)
            }
        }
    }

    fn to_bytes(&self) -> Result<Vec<u8>, bytesrepr::Error> {
        let mut buffer = bytesrepr::allocate_buffer(self)?;
        self.write_bytes(&mut buffer)?;
        Ok(buffer)
    }

    fn serialized_length(&self) -> usize {
        let body_len = match self {
            TransformKindV1::Prune(key) => key.serialized_length(),
            TransformKindV1::WriteCLValue(value) => value.serialized_length(),
            TransformKindV1::WriteAccount(value) => value.serialized_length(),
            TransformKindV1::WriteDeployInfo(value) => value.serialized_length(),
            TransformKindV1::WriteEraInfo(value) => value.serialized_length(),
            TransformKindV1::WriteTransfer(value) => value.serialized_length(),
            TransformKindV1::AddInt32(value) => value.serialized_length(),
            TransformKindV1::AddUInt64(value) => value.serialized_length(),
            TransformKindV1::AddUInt128(value) => value.serialized_length(),
            TransformKindV1::AddUInt256(value) => value.serialized_length(),
            TransformKindV1::AddUInt512(value) => value.serialized_length(),
            TransformKindV1::AddKeys(value) => value.serialized_length(),
            TransformKindV1::Failure(value) => value.serialized_length(),
            TransformKindV1::Identity
            | TransformKindV1::WriteContractWasm
            | TransformKindV1::WriteContract
            | TransformKindV1::WriteContractPackage
            | TransformKindV1::WriteAddressableEntity => 0,
            TransformKindV1::WriteBid(value) => value.serialized_length(),
            TransformKindV1::WriteBidKind(value) => value.serialized_length(),
            TransformKindV1::WriteWithdraw(value) => value.serialized_length(),
            TransformKindV1::WriteUnbonding(value) => value.serialized_length(),
        };
        U8_SERIALIZED_LENGTH + body_len
    }
}

impl FromBytes for TransformKindV1 {
    fn from_bytes(bytes: &[u8]) -> Result<(Self, &[u8]), bytesrepr::Error> {
        let (tag, remainder) = u8::from_bytes(bytes)?;
        match TryFrom::try_from(tag)? {
            TransformTag::Identity => Ok((TransformKindV1::Identity, remainder)),
            TransformTag::WriteCLValue => {
                let (cl_value, remainder) = CLValue::from_bytes(remainder)?;
                Ok((TransformKindV1::WriteCLValue(cl_value), remainder))
            }
            TransformTag::WriteAccount => {
                let (account_hash, remainder) = AccountHash::from_bytes(remainder)?;
                Ok((TransformKindV1::WriteAccount(account_hash), remainder))
            }
            TransformTag::WriteByteCode => Ok((TransformKindV1::WriteContractWasm, remainder)),
            TransformTag::WriteContract => Ok((TransformKindV1::WriteContract, remainder)),
            TransformTag::WritePackage => Ok((TransformKindV1::WriteContractPackage, remainder)),
            TransformTag::WriteDeployInfo => {
                let (deploy_info, remainder) = DeployInfo::from_bytes(remainder)?;
                Ok((TransformKindV1::WriteDeployInfo(deploy_info), remainder))
            }
            TransformTag::WriteEraInfo => {
                let (era_info, remainder) = EraInfo::from_bytes(remainder)?;
                Ok((TransformKindV1::WriteEraInfo(era_info), remainder))
            }
            TransformTag::WriteTransfer => {
                let (transfer, remainder) = TransferV1::from_bytes(remainder)?;
                Ok((TransformKindV1::WriteTransfer(transfer), remainder))
            }
            TransformTag::AddInt32 => {
                let (value_i32, remainder) = i32::from_bytes(remainder)?;
                Ok((TransformKindV1::AddInt32(value_i32), remainder))
            }
            TransformTag::AddUInt64 => {
                let (value_u64, remainder) = u64::from_bytes(remainder)?;
                Ok((TransformKindV1::AddUInt64(value_u64), remainder))
            }
            TransformTag::AddUInt128 => {
                let (value_u128, remainder) = U128::from_bytes(remainder)?;
                Ok((TransformKindV1::AddUInt128(value_u128), remainder))
            }
            TransformTag::AddUInt256 => {
                let (value_u256, remainder) = U256::from_bytes(remainder)?;
                Ok((TransformKindV1::AddUInt256(value_u256), remainder))
            }
            TransformTag::AddUInt512 => {
                let (value_u512, remainder) = U512::from_bytes(remainder)?;
                Ok((TransformKindV1::AddUInt512(value_u512), remainder))
            }
            TransformTag::AddKeys => {
                let (value, remainder) = Vec::<NamedKey>::from_bytes(remainder)?;
                Ok((TransformKindV1::AddKeys(value), remainder))
            }
            TransformTag::Failure => {
                let (value, remainder) = String::from_bytes(remainder)?;
                Ok((TransformKindV1::Failure(value), remainder))
            }
            TransformTag::WriteBid => {
                let (bid, remainder) = Bid::from_bytes(remainder)?;
                Ok((TransformKindV1::WriteBid(Box::new(bid)), remainder))
            }
            TransformTag::WriteWithdraw => {
                let (withdraw_purses, remainder) =
                    <Vec<WithdrawPurse> as FromBytes>::from_bytes(remainder)?;
                Ok((TransformKindV1::WriteWithdraw(withdraw_purses), remainder))
            }
            TransformTag::WriteUnbonding => {
                let (unbonding_purses, remainder) =
                    <Vec<UnbondingPurse> as FromBytes>::from_bytes(remainder)?;
                Ok((TransformKindV1::WriteUnbonding(unbonding_purses), remainder))
            }
            TransformTag::WriteAddressableEntity => {
                Ok((TransformKindV1::WriteAddressableEntity, remainder))
            }
            TransformTag::Prune => {
                let (key, remainder) = Key::from_bytes(remainder)?;
                Ok((TransformKindV1::Prune(key), remainder))
            }
            TransformTag::WriteBidKind => {
                let (value, remainder) = BidKind::from_bytes(remainder)?;
                Ok((TransformKindV1::WriteBidKind(value), remainder))
            }
        }
    }
}

#[cfg(any(feature = "testing", test))]
impl Distribution<TransformKindV1> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> TransformKindV1 {
        // TODO - cover all options
        match rng.gen_range(0..13) {
            0 => TransformKindV1::Identity,
            1 => TransformKindV1::WriteCLValue(CLValue::from_t(true).unwrap()),
            2 => TransformKindV1::WriteAccount(AccountHash::new(rng.gen())),
            3 => TransformKindV1::WriteContractWasm,
            4 => TransformKindV1::WriteContract,
            5 => TransformKindV1::WriteContractPackage,
            6 => TransformKindV1::AddInt32(rng.gen()),
            7 => TransformKindV1::AddUInt64(rng.gen()),
            8 => TransformKindV1::AddUInt128(rng.gen::<u64>().into()),
            9 => TransformKindV1::AddUInt256(rng.gen::<u64>().into()),
            10 => TransformKindV1::AddUInt512(rng.gen::<u64>().into()),
            11 => {
                let mut named_keys = Vec::new();
                for _ in 0..rng.gen_range(1..6) {
                    named_keys.push(NamedKey {
                        name: rng.gen::<u64>().to_string(),
                        key: rng.gen::<u64>().to_string(),
                    });
                }
                TransformKindV1::AddKeys(named_keys)
            }
            12 => TransformKindV1::Failure(rng.gen::<u64>().to_string()),
            13 => TransformKindV1::WriteAddressableEntity,
            _ => unreachable!(),
        }
    }
}

/// A key with a name.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize, Default, Debug)]
#[cfg_attr(feature = "datasize", derive(DataSize))]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct NamedKey {
    /// The name of the entry.
    pub name: String,
    /// The value of the entry: a casper `Key` type.
    #[cfg_attr(feature = "json-schema", schemars(with = "Key"))]
    pub key: String,
}

impl ToBytes for NamedKey {
    fn write_bytes(&self, writer: &mut Vec<u8>) -> Result<(), bytesrepr::Error> {
        self.name.write_bytes(writer)?;
        self.key.write_bytes(writer)
    }

    fn to_bytes(&self) -> Result<Vec<u8>, bytesrepr::Error> {
        let mut buffer = bytesrepr::allocate_buffer(self)?;
        self.write_bytes(&mut buffer)?;
        Ok(buffer)
    }

    fn serialized_length(&self) -> usize {
        self.name.serialized_length() + self.key.serialized_length()
    }
}

impl FromBytes for NamedKey {
    fn from_bytes(bytes: &[u8]) -> Result<(Self, &[u8]), bytesrepr::Error> {
        let (name, remainder) = String::from_bytes(bytes)?;
        let (key, remainder) = String::from_bytes(remainder)?;
        let named_key = NamedKey { name, key };
        Ok((named_key, remainder))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::TestRng;

    #[test]
    fn bytesrepr_test_transform() {
        let mut rng = TestRng::new();
        let transform: TransformKindV1 = rng.gen();
        bytesrepr::test_serialization_roundtrip(&transform);
    }

    #[test]
    fn bytesrepr_test_execution_result() {
        let mut rng = TestRng::new();
        let execution_result: ExecutionResultV1 = rng.gen();
        bytesrepr::test_serialization_roundtrip(&execution_result);
    }
}
