//! Support for host function gas cost tables.
use core::ops::Add;

#[cfg(feature = "datasize")]
use datasize::DataSize;
use derive_more::Add;
use num_traits::Zero;
use rand::{distributions::Standard, prelude::Distribution, Rng};
use serde::{Deserialize, Serialize};

use crate::{
    bytesrepr::{self, FromBytes, ToBytes, U32_SERIALIZED_LENGTH},
    Gas,
};

/// Representation of argument's cost.
pub type Cost = u32;

const COST_SERIALIZED_LENGTH: usize = U32_SERIALIZED_LENGTH;

/// An identifier that represents an unused argument.
const NOT_USED: Cost = 0;

/// An arbitrary default fixed cost for host functions that were not researched yet.
const DEFAULT_FIXED_COST: Cost = 200;

const DEFAULT_ADD_COST: u32 = 5_800;
const DEFAULT_ADD_ASSOCIATED_KEY_COST: u32 = 1_200_000;

const DEFAULT_CALL_CONTRACT_COST: u32 = 300_000_000;

const DEFAULT_CREATE_PURSE_COST: u32 = 2_500_000_000;
const DEFAULT_GET_BALANCE_COST: u32 = 3_000_000;
const DEFAULT_GET_BLOCKTIME_COST: u32 = 330;
const DEFAULT_GET_CALLER_COST: u32 = 380;
const DEFAULT_GET_KEY_COST: u32 = 2_000;
const DEFAULT_GET_KEY_NAME_SIZE_WEIGHT: u32 = 440;
const DEFAULT_GET_MAIN_PURSE_COST: u32 = 1_300;
const DEFAULT_GET_PHASE_COST: u32 = 710;
const DEFAULT_GET_SYSTEM_CONTRACT_COST: u32 = 1_100;
const DEFAULT_HAS_KEY_COST: u32 = 1_500;
const DEFAULT_HAS_KEY_NAME_SIZE_WEIGHT: u32 = 840;
const DEFAULT_IS_VALID_UREF_COST: u32 = 760;
const DEFAULT_LOAD_NAMED_KEYS_COST: u32 = 42_000;
const DEFAULT_NEW_UREF_COST: u32 = 17_000;
const DEFAULT_NEW_UREF_VALUE_SIZE_WEIGHT: u32 = 590;

const DEFAULT_PRINT_COST: u32 = 20_000;
const DEFAULT_PRINT_TEXT_SIZE_WEIGHT: u32 = 4_600;

const DEFAULT_PUT_KEY_COST: u32 = 100_000_000;
const DEFAULT_PUT_KEY_NAME_SIZE_WEIGHT: u32 = 120_000;

const DEFAULT_READ_HOST_BUFFER_COST: u32 = 3_500;
const DEFAULT_READ_HOST_BUFFER_DEST_SIZE_WEIGHT: u32 = 310;

const DEFAULT_READ_VALUE_COST: u32 = 60_000;
const DEFAULT_DICTIONARY_GET_COST: u32 = 5_500;
const DEFAULT_DICTIONARY_GET_KEY_SIZE_WEIGHT: u32 = 590;

const DEFAULT_REMOVE_ASSOCIATED_KEY_COST: u32 = 4_200;

const DEFAULT_REMOVE_KEY_COST: u32 = 61_000;
const DEFAULT_REMOVE_KEY_NAME_SIZE_WEIGHT: u32 = 3_200;

const DEFAULT_RET_COST: u32 = 23_000;
const DEFAULT_RET_VALUE_SIZE_WEIGHT: u32 = 420_000;

const DEFAULT_REVERT_COST: u32 = 500;
const DEFAULT_SET_ACTION_THRESHOLD_COST: u32 = 74_000;
const DEFAULT_TRANSFER_FROM_PURSE_TO_ACCOUNT_COST: u32 = 2_500_000_000;
const DEFAULT_TRANSFER_FROM_PURSE_TO_PURSE_COST: u32 = 82_000_000;
const DEFAULT_TRANSFER_TO_ACCOUNT_COST: u32 = 2_500_000_000;
const DEFAULT_UPDATE_ASSOCIATED_KEY_COST: u32 = 4_200;

const DEFAULT_WRITE_COST: u32 = 14_000;
const DEFAULT_WRITE_VALUE_SIZE_WEIGHT: u32 = 980;

const DEFAULT_ARG_CHARGE: u32 = 120_000;

const DEFAULT_DICTIONARY_PUT_COST: u32 = 9_500;
const DEFAULT_DICTIONARY_PUT_KEY_BYTES_SIZE_WEIGHT: u32 = 1_800;
const DEFAULT_DICTIONARY_PUT_VALUE_SIZE_WEIGHT: u32 = 520;

/// Default cost for a new dictionary.
pub const DEFAULT_NEW_DICTIONARY_COST: u32 = DEFAULT_NEW_UREF_COST;

/// Host function cost unit for a new dictionary.
#[allow(unused)]
pub const DEFAULT_HOST_FUNCTION_NEW_DICTIONARY: HostFunction<[Cost; 1]> =
    HostFunction::new(DEFAULT_NEW_DICTIONARY_COST, [NOT_USED]);
const DEFAULT_BLAKE2B_COST: u32 = 1_200_000;

/// Default value that the cost of calling `casper_emit_message` increases by for every new message
/// emitted within an execution.
pub const DEFAULT_COST_INCREASE_PER_MESSAGE_EMITTED: u32 = 50;

const DEFAULT_MESSAGE_TOPIC_NAME_SIZE_WEIGHT: u32 = 30_000;
const DEFAULT_MESSAGE_PAYLOAD_SIZE_WEIGHT: u32 = 120_000;

const DEFAULT_CONTRACT_VERSION_ARG_SIZE_WEIGHT: u32 = 120_000;

const DEFAULT_GENERIC_HASH_COST: u32 = 1_200_000;

const DEFAULT_GENERIC_HASH_INPUT_COST: u32 = 120_000;

const DEFAULT_RECOVER_SECP256K1_COST: u32 = 1_300_000;
const DEFAULT_RECOVER_SECP256K1_SIZE_WEIGHT: u32 = 120_000;

const DEFAULT_VERIFY_SIGNATURE_COST: u32 = 1_300_000;
const DEFAULT_VERIFY_SIGNATURE_SIZE_WEIGHT: u32 = 120_000;

/// Representation of a host function cost.
///
/// The total gas cost is equal to `cost` + sum of each argument weight multiplied by the byte size
/// of the data.
#[derive(Copy, Clone, PartialEq, Eq, Deserialize, Serialize, Debug)]
#[cfg_attr(feature = "datasize", derive(DataSize))]
#[serde(deny_unknown_fields)]
pub struct HostFunction<T> {
    /// How much the user is charged for calling the host function.
    cost: Cost,
    /// Weights of the function arguments.
    arguments: T,
}

impl<T> Default for HostFunction<T>
where
    T: Default,
{
    fn default() -> Self {
        HostFunction::new(DEFAULT_FIXED_COST, Default::default())
    }
}

impl<T> HostFunction<T> {
    /// Creates a new instance of `HostFunction` with a fixed call cost and argument weights.
    pub const fn new(cost: Cost, arguments: T) -> Self {
        Self { cost, arguments }
    }

    pub fn with_new_static_cost(self, cost: Cost) -> Self {
        Self {
            cost,
            arguments: self.arguments,
        }
    }

    /// Returns the base gas fee for calling the host function.
    pub fn cost(&self) -> Cost {
        self.cost
    }
}

impl<T> HostFunction<T>
where
    T: Default,
{
    /// Creates a new fixed host function cost with argument weights of zero.
    pub fn fixed(cost: Cost) -> Self {
        Self {
            cost,
            ..Default::default()
        }
    }
}

impl<T> HostFunction<T>
where
    T: AsRef<[Cost]>,
{
    /// Returns a slice containing the argument weights.
    pub fn arguments(&self) -> &[Cost] {
        self.arguments.as_ref()
    }

    /// Calculate gas cost for a host function
    pub fn calculate_gas_cost(&self, weights: T) -> Option<Gas> {
        let mut gas = Gas::new(self.cost);
        for (argument, weight) in self.arguments.as_ref().iter().zip(weights.as_ref()) {
            let lhs = Gas::new(*argument);
            let rhs = Gas::new(*weight);
            let product = lhs.checked_mul(rhs)?;
            gas = gas.checked_add(product)?;
        }
        Some(gas)
    }
}

impl<const COUNT: usize> Add for HostFunction<[Cost; COUNT]> {
    type Output = HostFunction<[Cost; COUNT]>;

    fn add(self, rhs: Self) -> Self::Output {
        let mut result = HostFunction::new(self.cost + rhs.cost, [0; COUNT]);
        for i in 0..COUNT {
            result.arguments[i] = self.arguments[i] + rhs.arguments[i];
        }
        result
    }
}

impl<const COUNT: usize> Zero for HostFunction<[Cost; COUNT]> {
    fn zero() -> Self {
        HostFunction::new(0, [0; COUNT])
    }

    fn is_zero(&self) -> bool {
        !self.arguments.iter().any(|cost| *cost != 0) && self.cost.is_zero()
    }
}

impl<T> Distribution<HostFunction<T>> for Standard
where
    Standard: Distribution<T>,
    T: AsRef<[Cost]>,
{
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> HostFunction<T> {
        let cost = rng.gen::<Cost>();
        let arguments = rng.gen();
        HostFunction::new(cost, arguments)
    }
}

impl<T> ToBytes for HostFunction<T>
where
    T: AsRef<[Cost]>,
{
    fn to_bytes(&self) -> Result<Vec<u8>, bytesrepr::Error> {
        let mut ret = bytesrepr::unchecked_allocate_buffer(self);
        ret.append(&mut self.cost.to_bytes()?);
        for value in self.arguments.as_ref().iter() {
            ret.append(&mut value.to_bytes()?);
        }
        Ok(ret)
    }

    fn serialized_length(&self) -> usize {
        self.cost.serialized_length() + (COST_SERIALIZED_LENGTH * self.arguments.as_ref().len())
    }
}

impl<T> FromBytes for HostFunction<T>
where
    T: Default + AsMut<[Cost]>,
{
    fn from_bytes(bytes: &[u8]) -> Result<(Self, &[u8]), bytesrepr::Error> {
        let (cost, mut bytes) = FromBytes::from_bytes(bytes)?;
        let mut arguments = T::default();
        let arguments_mut = arguments.as_mut();
        for ith_argument in arguments_mut {
            let (cost, rem) = FromBytes::from_bytes(bytes)?;
            *ith_argument = cost;
            bytes = rem;
        }
        Ok((Self { cost, arguments }, bytes))
    }
}

/// Definition of a host function cost table.
#[derive(Add, Copy, Clone, PartialEq, Eq, Serialize, Deserialize, Debug)]
#[cfg_attr(feature = "datasize", derive(DataSize))]
#[serde(deny_unknown_fields)]
pub struct HostFunctionCostsV1 {
    /// Cost increase for successive calls to `casper_emit_message` within an execution.
    pub cost_increase_per_message: u32,
    /// Cost of calling the `read_value` host function.
    pub read_value: HostFunction<[Cost; 3]>,
    /// Cost of calling the `dictionary_get` host function.
    pub dictionary_get: HostFunction<[Cost; 3]>,
    /// Cost of calling the `write` host function.
    pub write: HostFunction<[Cost; 4]>,
    /// Cost of calling the `dictionary_put` host function.
    pub dictionary_put: HostFunction<[Cost; 4]>,
    /// Cost of calling the `add` host function.
    pub add: HostFunction<[Cost; 4]>,
    /// Cost of calling the `new_uref` host function.
    pub new_uref: HostFunction<[Cost; 3]>,
    /// Cost of calling the `load_named_keys` host function.
    pub load_named_keys: HostFunction<[Cost; 2]>,
    /// Cost of calling the `ret` host function.
    pub ret: HostFunction<[Cost; 2]>,
    /// Cost of calling the `get_key` host function.
    pub get_key: HostFunction<[Cost; 5]>,
    /// Cost of calling the `has_key` host function.
    pub has_key: HostFunction<[Cost; 2]>,
    /// Cost of calling the `put_key` host function.
    pub put_key: HostFunction<[Cost; 4]>,
    /// Cost of calling the `remove_key` host function.
    pub remove_key: HostFunction<[Cost; 2]>,
    /// Cost of calling the `revert` host function.
    pub revert: HostFunction<[Cost; 1]>,
    /// Cost of calling the `is_valid_uref` host function.
    pub is_valid_uref: HostFunction<[Cost; 2]>,
    /// Cost of calling the `add_associated_key` host function.
    pub add_associated_key: HostFunction<[Cost; 3]>,
    /// Cost of calling the `remove_associated_key` host function.
    pub remove_associated_key: HostFunction<[Cost; 2]>,
    /// Cost of calling the `update_associated_key` host function.
    pub update_associated_key: HostFunction<[Cost; 3]>,
    /// Cost of calling the `set_action_threshold` host function.
    pub set_action_threshold: HostFunction<[Cost; 2]>,
    /// Cost of calling the `get_caller` host function.
    pub get_caller: HostFunction<[Cost; 1]>,
    /// Cost of calling the `get_blocktime` host function.
    pub get_blocktime: HostFunction<[Cost; 1]>,
    /// Cost of calling the `create_purse` host function.
    pub create_purse: HostFunction<[Cost; 2]>,
    /// Cost of calling the `transfer_to_account` host function.
    pub transfer_to_account: HostFunction<[Cost; 7]>,
    /// Cost of calling the `transfer_from_purse_to_account` host function.
    pub transfer_from_purse_to_account: HostFunction<[Cost; 9]>,
    /// Cost of calling the `transfer_from_purse_to_purse` host function.
    pub transfer_from_purse_to_purse: HostFunction<[Cost; 8]>,
    /// Cost of calling the `get_balance` host function.
    pub get_balance: HostFunction<[Cost; 3]>,
    /// Cost of calling the `get_phase` host function.
    pub get_phase: HostFunction<[Cost; 1]>,
    /// Cost of calling the `get_system_contract` host function.
    pub get_system_contract: HostFunction<[Cost; 3]>,
    /// Cost of calling the `get_main_purse` host function.
    pub get_main_purse: HostFunction<[Cost; 1]>,
    /// Cost of calling the `read_host_buffer` host function.
    pub read_host_buffer: HostFunction<[Cost; 3]>,
    /// Cost of calling the `create_contract_package_at_hash` host function.
    pub create_contract_package_at_hash: HostFunction<[Cost; 2]>,
    /// Cost of calling the `create_contract_user_group` host function.
    pub create_contract_user_group: HostFunction<[Cost; 8]>,
    /// Cost of calling the `add_contract_version` host function.
    pub add_contract_version: HostFunction<[Cost; 10]>,
    /// Cost of calling the `add_contract_version_with_message_topics` host function.
    pub add_contract_version_with_message_topics: HostFunction<[Cost; 11]>,
    /// Cost of calling the `add_package_version` host function.
    pub add_package_version_with_message_topics: HostFunction<[Cost; 11]>,
    /// Cost of calling the `disable_contract_version` host function.
    pub disable_contract_version: HostFunction<[Cost; 4]>,
    /// Cost of calling the `call_contract` host function.
    pub call_contract: HostFunction<[Cost; 7]>,
    /// Cost of calling the `call_versioned_contract` host function.
    pub call_versioned_contract: HostFunction<[Cost; 9]>,
    /// Cost of calling the `get_named_arg_size` host function.
    pub get_named_arg_size: HostFunction<[Cost; 3]>,
    /// Cost of calling the `get_named_arg` host function.
    pub get_named_arg: HostFunction<[Cost; 4]>,
    /// Cost of calling the `remove_contract_user_group` host function.
    pub remove_contract_user_group: HostFunction<[Cost; 4]>,
    /// Cost of calling the `provision_contract_user_group_uref` host function.
    pub provision_contract_user_group_uref: HostFunction<[Cost; 5]>,
    /// Cost of calling the `remove_contract_user_group_urefs` host function.
    pub remove_contract_user_group_urefs: HostFunction<[Cost; 6]>,
    /// Cost of calling the `print` host function.
    pub print: HostFunction<[Cost; 2]>,
    /// Cost of calling the `blake2b` host function.
    pub blake2b: HostFunction<[Cost; 4]>,
    /// Cost of calling the `next address` host function.
    pub random_bytes: HostFunction<[Cost; 2]>,
    /// Cost of calling the `enable_contract_version` host function.
    pub enable_contract_version: HostFunction<[Cost; 4]>,
    /// Cost of calling the `casper_manage_message_topic` host function.
    pub manage_message_topic: HostFunction<[Cost; 4]>,
    /// Cost of calling the `casper_emit_message` host function.
    pub emit_message: HostFunction<[Cost; 4]>,
    /// Cost of calling the `get_block_info` host function.
    pub get_block_info: HostFunction<[Cost; 2]>,
    /// Cost of calling the `generic_hash` host function.
    pub generic_hash: HostFunction<[Cost; 5]>,
    /// Cost of calling the 'recover_secp256k1' host function.
    pub recover_secp256k1: HostFunction<[Cost; 6]>,
    /// Cost of calling the 'recover_secp256k1' host function.
    pub verify_signature: HostFunction<[Cost; 6]>,
    /// Cost of calling the 'call_package_version' host function.
    pub call_package_version: HostFunction<[Cost; 11]>,
}

impl Zero for HostFunctionCostsV1 {
    fn zero() -> Self {
        Self {
            read_value: HostFunction::zero(),
            dictionary_get: HostFunction::zero(),
            write: HostFunction::zero(),
            dictionary_put: HostFunction::zero(),
            add: HostFunction::zero(),
            new_uref: HostFunction::zero(),
            load_named_keys: HostFunction::zero(),
            ret: HostFunction::zero(),
            get_key: HostFunction::zero(),
            has_key: HostFunction::zero(),
            put_key: HostFunction::zero(),
            remove_key: HostFunction::zero(),
            revert: HostFunction::zero(),
            is_valid_uref: HostFunction::zero(),
            add_associated_key: HostFunction::zero(),
            remove_associated_key: HostFunction::zero(),
            update_associated_key: HostFunction::zero(),
            set_action_threshold: HostFunction::zero(),
            get_caller: HostFunction::zero(),
            get_blocktime: HostFunction::zero(),
            create_purse: HostFunction::zero(),
            transfer_to_account: HostFunction::zero(),
            transfer_from_purse_to_account: HostFunction::zero(),
            transfer_from_purse_to_purse: HostFunction::zero(),
            get_balance: HostFunction::zero(),
            get_phase: HostFunction::zero(),
            get_system_contract: HostFunction::zero(),
            get_main_purse: HostFunction::zero(),
            read_host_buffer: HostFunction::zero(),
            create_contract_package_at_hash: HostFunction::zero(),
            create_contract_user_group: HostFunction::zero(),
            add_contract_version_with_message_topics: HostFunction::zero(),
            add_contract_version: HostFunction::zero(),
            add_package_version_with_message_topics: HostFunction::zero(),
            disable_contract_version: HostFunction::zero(),
            call_contract: HostFunction::zero(),
            call_versioned_contract: HostFunction::zero(),
            get_named_arg_size: HostFunction::zero(),
            get_named_arg: HostFunction::zero(),
            remove_contract_user_group: HostFunction::zero(),
            provision_contract_user_group_uref: HostFunction::zero(),
            remove_contract_user_group_urefs: HostFunction::zero(),
            print: HostFunction::zero(),
            blake2b: HostFunction::zero(),
            random_bytes: HostFunction::zero(),
            enable_contract_version: HostFunction::zero(),
            manage_message_topic: HostFunction::zero(),
            emit_message: HostFunction::zero(),
            cost_increase_per_message: Zero::zero(),
            get_block_info: HostFunction::zero(),
            generic_hash: HostFunction::zero(),
            recover_secp256k1: HostFunction::zero(),
            verify_signature: HostFunction::zero(),
            call_package_version: HostFunction::zero(),
        }
    }

    fn is_zero(&self) -> bool {
        let HostFunctionCostsV1 {
            cost_increase_per_message,
            read_value,
            dictionary_get,
            write,
            dictionary_put,
            add,
            new_uref,
            load_named_keys,
            ret,
            get_key,
            has_key,
            put_key,
            remove_key,
            revert,
            is_valid_uref,
            add_associated_key,
            remove_associated_key,
            update_associated_key,
            set_action_threshold,
            get_caller,
            get_blocktime,
            create_purse,
            transfer_to_account,
            transfer_from_purse_to_account,
            transfer_from_purse_to_purse,
            get_balance,
            get_phase,
            get_system_contract,
            get_main_purse,
            read_host_buffer,
            create_contract_package_at_hash,
            create_contract_user_group,
            add_contract_version_with_message_topics,
            add_contract_version,
            add_package_version_with_message_topics: add_package_version,
            disable_contract_version,
            call_contract,
            call_versioned_contract,
            get_named_arg_size,
            get_named_arg,
            remove_contract_user_group,
            provision_contract_user_group_uref,
            remove_contract_user_group_urefs,
            print,
            blake2b,
            random_bytes,
            enable_contract_version,
            manage_message_topic,
            emit_message,
            get_block_info,
            generic_hash,
            recover_secp256k1,
            verify_signature,
            call_package_version,
        } = self;
        read_value.is_zero()
            && dictionary_get.is_zero()
            && write.is_zero()
            && dictionary_put.is_zero()
            && add.is_zero()
            && new_uref.is_zero()
            && load_named_keys.is_zero()
            && ret.is_zero()
            && get_key.is_zero()
            && has_key.is_zero()
            && put_key.is_zero()
            && remove_key.is_zero()
            && revert.is_zero()
            && is_valid_uref.is_zero()
            && add_associated_key.is_zero()
            && remove_associated_key.is_zero()
            && update_associated_key.is_zero()
            && set_action_threshold.is_zero()
            && get_caller.is_zero()
            && get_blocktime.is_zero()
            && create_purse.is_zero()
            && transfer_to_account.is_zero()
            && transfer_from_purse_to_account.is_zero()
            && transfer_from_purse_to_purse.is_zero()
            && get_balance.is_zero()
            && get_phase.is_zero()
            && get_system_contract.is_zero()
            && get_main_purse.is_zero()
            && read_host_buffer.is_zero()
            && create_contract_package_at_hash.is_zero()
            && create_contract_user_group.is_zero()
            && add_contract_version.is_zero()
            && disable_contract_version.is_zero()
            && call_contract.is_zero()
            && call_versioned_contract.is_zero()
            && get_named_arg_size.is_zero()
            && get_named_arg.is_zero()
            && remove_contract_user_group.is_zero()
            && provision_contract_user_group_uref.is_zero()
            && remove_contract_user_group_urefs.is_zero()
            && print.is_zero()
            && blake2b.is_zero()
            && random_bytes.is_zero()
            && enable_contract_version.is_zero()
            && manage_message_topic.is_zero()
            && emit_message.is_zero()
            && cost_increase_per_message.is_zero()
            && add_package_version.is_zero()
            && get_block_info.is_zero()
            && add_contract_version_with_message_topics.is_zero()
            && generic_hash.is_zero()
            && recover_secp256k1.is_zero()
            && verify_signature.is_zero()
            && call_package_version.is_zero()
    }
}

impl Default for HostFunctionCostsV1 {
    fn default() -> Self {
        Self {
            read_value: HostFunction::new(
                DEFAULT_READ_VALUE_COST,
                [NOT_USED, DEFAULT_ARG_CHARGE, NOT_USED],
            ),
            dictionary_get: HostFunction::new(
                DEFAULT_DICTIONARY_GET_COST,
                [NOT_USED, DEFAULT_DICTIONARY_GET_KEY_SIZE_WEIGHT, NOT_USED],
            ),
            write: HostFunction::new(
                DEFAULT_WRITE_COST,
                [
                    NOT_USED,
                    NOT_USED,
                    NOT_USED,
                    DEFAULT_WRITE_VALUE_SIZE_WEIGHT,
                ],
            ),
            dictionary_put: HostFunction::new(
                DEFAULT_DICTIONARY_PUT_COST,
                [
                    NOT_USED,
                    DEFAULT_DICTIONARY_PUT_KEY_BYTES_SIZE_WEIGHT,
                    NOT_USED,
                    DEFAULT_DICTIONARY_PUT_VALUE_SIZE_WEIGHT,
                ],
            ),
            add: HostFunction::fixed(DEFAULT_ADD_COST),
            new_uref: HostFunction::new(
                DEFAULT_NEW_UREF_COST,
                [NOT_USED, NOT_USED, DEFAULT_NEW_UREF_VALUE_SIZE_WEIGHT],
            ),
            load_named_keys: HostFunction::fixed(DEFAULT_LOAD_NAMED_KEYS_COST),
            ret: HostFunction::new(DEFAULT_RET_COST, [NOT_USED, DEFAULT_RET_VALUE_SIZE_WEIGHT]),
            get_key: HostFunction::new(
                DEFAULT_GET_KEY_COST,
                [
                    NOT_USED,
                    DEFAULT_GET_KEY_NAME_SIZE_WEIGHT,
                    NOT_USED,
                    NOT_USED,
                    NOT_USED,
                ],
            ),
            has_key: HostFunction::new(
                DEFAULT_HAS_KEY_COST,
                [NOT_USED, DEFAULT_HAS_KEY_NAME_SIZE_WEIGHT],
            ),
            put_key: HostFunction::new(
                DEFAULT_PUT_KEY_COST,
                [
                    NOT_USED,
                    DEFAULT_PUT_KEY_NAME_SIZE_WEIGHT,
                    NOT_USED,
                    DEFAULT_ARG_CHARGE,
                ],
            ),
            remove_key: HostFunction::new(
                DEFAULT_REMOVE_KEY_COST,
                [NOT_USED, DEFAULT_REMOVE_KEY_NAME_SIZE_WEIGHT],
            ),
            revert: HostFunction::fixed(DEFAULT_REVERT_COST),
            is_valid_uref: HostFunction::fixed(DEFAULT_IS_VALID_UREF_COST),
            add_associated_key: HostFunction::fixed(DEFAULT_ADD_ASSOCIATED_KEY_COST),
            remove_associated_key: HostFunction::fixed(DEFAULT_REMOVE_ASSOCIATED_KEY_COST),
            update_associated_key: HostFunction::fixed(DEFAULT_UPDATE_ASSOCIATED_KEY_COST),
            set_action_threshold: HostFunction::fixed(DEFAULT_SET_ACTION_THRESHOLD_COST),
            get_caller: HostFunction::fixed(DEFAULT_GET_CALLER_COST),
            get_blocktime: HostFunction::fixed(DEFAULT_GET_BLOCKTIME_COST),
            create_purse: HostFunction::fixed(DEFAULT_CREATE_PURSE_COST),
            transfer_to_account: HostFunction::fixed(DEFAULT_TRANSFER_TO_ACCOUNT_COST),
            transfer_from_purse_to_account: HostFunction::fixed(
                DEFAULT_TRANSFER_FROM_PURSE_TO_ACCOUNT_COST,
            ),
            transfer_from_purse_to_purse: HostFunction::fixed(
                DEFAULT_TRANSFER_FROM_PURSE_TO_PURSE_COST,
            ),
            get_balance: HostFunction::fixed(DEFAULT_GET_BALANCE_COST),
            get_phase: HostFunction::fixed(DEFAULT_GET_PHASE_COST),
            get_system_contract: HostFunction::fixed(DEFAULT_GET_SYSTEM_CONTRACT_COST),
            get_main_purse: HostFunction::fixed(DEFAULT_GET_MAIN_PURSE_COST),
            read_host_buffer: HostFunction::new(
                DEFAULT_READ_HOST_BUFFER_COST,
                [
                    NOT_USED,
                    DEFAULT_READ_HOST_BUFFER_DEST_SIZE_WEIGHT,
                    NOT_USED,
                ],
            ),
            create_contract_package_at_hash: HostFunction::default(),
            create_contract_user_group: HostFunction::default(),
            add_package_version_with_message_topics: HostFunction::new(
                DEFAULT_FIXED_COST,
                [
                    NOT_USED,
                    NOT_USED,
                    NOT_USED,
                    NOT_USED,
                    DEFAULT_CONTRACT_VERSION_ARG_SIZE_WEIGHT,
                    NOT_USED,
                    NOT_USED,
                    NOT_USED,
                    DEFAULT_MESSAGE_TOPIC_NAME_SIZE_WEIGHT,
                    NOT_USED,
                    NOT_USED,
                ],
            ),
            add_contract_version: HostFunction::new(
                DEFAULT_FIXED_COST,
                [
                    NOT_USED,
                    NOT_USED,
                    NOT_USED,
                    NOT_USED,
                    DEFAULT_CONTRACT_VERSION_ARG_SIZE_WEIGHT,
                    NOT_USED,
                    NOT_USED,
                    NOT_USED,
                    NOT_USED,
                    NOT_USED,
                ],
            ),
            add_contract_version_with_message_topics: HostFunction::new(
                DEFAULT_FIXED_COST,
                [
                    NOT_USED,
                    NOT_USED,
                    NOT_USED,
                    NOT_USED,
                    DEFAULT_CONTRACT_VERSION_ARG_SIZE_WEIGHT,
                    NOT_USED,
                    NOT_USED,
                    NOT_USED,
                    DEFAULT_MESSAGE_TOPIC_NAME_SIZE_WEIGHT,
                    NOT_USED,
                    NOT_USED,
                ],
            ),
            disable_contract_version: HostFunction::default(),
            call_contract: HostFunction::new(
                DEFAULT_CALL_CONTRACT_COST,
                [
                    NOT_USED,
                    NOT_USED,
                    NOT_USED,
                    DEFAULT_ARG_CHARGE,
                    NOT_USED,
                    DEFAULT_ARG_CHARGE,
                    NOT_USED,
                ],
            ),
            call_versioned_contract: HostFunction::new(
                DEFAULT_CALL_CONTRACT_COST,
                [
                    NOT_USED,
                    NOT_USED,
                    NOT_USED,
                    NOT_USED,
                    NOT_USED,
                    DEFAULT_ARG_CHARGE,
                    NOT_USED,
                    DEFAULT_ARG_CHARGE,
                    NOT_USED,
                ],
            ),
            get_named_arg_size: HostFunction::default(),
            get_named_arg: HostFunction::new(
                200,
                [NOT_USED, DEFAULT_ARG_CHARGE, NOT_USED, DEFAULT_ARG_CHARGE],
            ),
            remove_contract_user_group: HostFunction::default(),
            provision_contract_user_group_uref: HostFunction::default(),
            remove_contract_user_group_urefs: HostFunction::new(
                200,
                [
                    NOT_USED,
                    NOT_USED,
                    NOT_USED,
                    NOT_USED,
                    NOT_USED,
                    DEFAULT_ARG_CHARGE,
                ],
            ),
            print: HostFunction::new(
                DEFAULT_PRINT_COST,
                [NOT_USED, DEFAULT_PRINT_TEXT_SIZE_WEIGHT],
            ),
            blake2b: HostFunction::new(
                DEFAULT_BLAKE2B_COST,
                [NOT_USED, DEFAULT_ARG_CHARGE, NOT_USED, NOT_USED],
            ),
            random_bytes: HostFunction::default(),
            enable_contract_version: HostFunction::default(),
            manage_message_topic: HostFunction::new(
                DEFAULT_FIXED_COST,
                [
                    NOT_USED,
                    DEFAULT_MESSAGE_TOPIC_NAME_SIZE_WEIGHT,
                    NOT_USED,
                    NOT_USED,
                ],
            ),
            emit_message: HostFunction::new(
                DEFAULT_FIXED_COST,
                [
                    NOT_USED,
                    DEFAULT_MESSAGE_TOPIC_NAME_SIZE_WEIGHT,
                    NOT_USED,
                    DEFAULT_MESSAGE_PAYLOAD_SIZE_WEIGHT,
                ],
            ),
            generic_hash: HostFunction::new(
                DEFAULT_GENERIC_HASH_COST,
                [
                    NOT_USED,
                    DEFAULT_GENERIC_HASH_INPUT_COST,
                    NOT_USED,
                    NOT_USED,
                    NOT_USED,
                ],
            ),
            cost_increase_per_message: DEFAULT_COST_INCREASE_PER_MESSAGE_EMITTED,
            get_block_info: HostFunction::new(DEFAULT_GET_BLOCKTIME_COST, [NOT_USED, NOT_USED]),
            recover_secp256k1: HostFunction::new(
                DEFAULT_RECOVER_SECP256K1_COST,
                [
                    NOT_USED,
                    DEFAULT_RECOVER_SECP256K1_SIZE_WEIGHT,
                    NOT_USED,
                    NOT_USED,
                    NOT_USED,
                    NOT_USED,
                ],
            ),
            verify_signature: HostFunction::new(
                DEFAULT_VERIFY_SIGNATURE_COST,
                [
                    NOT_USED,
                    DEFAULT_VERIFY_SIGNATURE_SIZE_WEIGHT,
                    NOT_USED,
                    NOT_USED,
                    NOT_USED,
                    NOT_USED,
                ],
            ),
            call_package_version: HostFunction::new(
                DEFAULT_CALL_CONTRACT_COST,
                [
                    NOT_USED,
                    NOT_USED,
                    NOT_USED,
                    NOT_USED,
                    NOT_USED,
                    NOT_USED,
                    NOT_USED,
                    DEFAULT_ARG_CHARGE,
                    NOT_USED,
                    DEFAULT_ARG_CHARGE,
                    NOT_USED,
                ],
            ),
        }
    }
}

impl ToBytes for HostFunctionCostsV1 {
    fn to_bytes(&self) -> Result<Vec<u8>, bytesrepr::Error> {
        let mut ret = bytesrepr::unchecked_allocate_buffer(self);
        ret.append(&mut self.read_value.to_bytes()?);
        ret.append(&mut self.dictionary_get.to_bytes()?);
        ret.append(&mut self.write.to_bytes()?);
        ret.append(&mut self.dictionary_put.to_bytes()?);
        ret.append(&mut self.add.to_bytes()?);
        ret.append(&mut self.new_uref.to_bytes()?);
        ret.append(&mut self.load_named_keys.to_bytes()?);
        ret.append(&mut self.ret.to_bytes()?);
        ret.append(&mut self.get_key.to_bytes()?);
        ret.append(&mut self.has_key.to_bytes()?);
        ret.append(&mut self.put_key.to_bytes()?);
        ret.append(&mut self.remove_key.to_bytes()?);
        ret.append(&mut self.revert.to_bytes()?);
        ret.append(&mut self.is_valid_uref.to_bytes()?);
        ret.append(&mut self.add_associated_key.to_bytes()?);
        ret.append(&mut self.remove_associated_key.to_bytes()?);
        ret.append(&mut self.update_associated_key.to_bytes()?);
        ret.append(&mut self.set_action_threshold.to_bytes()?);
        ret.append(&mut self.get_caller.to_bytes()?);
        ret.append(&mut self.get_blocktime.to_bytes()?);
        ret.append(&mut self.create_purse.to_bytes()?);
        ret.append(&mut self.transfer_to_account.to_bytes()?);
        ret.append(&mut self.transfer_from_purse_to_account.to_bytes()?);
        ret.append(&mut self.transfer_from_purse_to_purse.to_bytes()?);
        ret.append(&mut self.get_balance.to_bytes()?);
        ret.append(&mut self.get_phase.to_bytes()?);
        ret.append(&mut self.get_system_contract.to_bytes()?);
        ret.append(&mut self.get_main_purse.to_bytes()?);
        ret.append(&mut self.read_host_buffer.to_bytes()?);
        ret.append(&mut self.create_contract_package_at_hash.to_bytes()?);
        ret.append(&mut self.create_contract_user_group.to_bytes()?);
        ret.append(&mut self.add_contract_version_with_message_topics.to_bytes()?);
        ret.append(&mut self.add_contract_version.to_bytes()?);
        ret.append(&mut self.add_package_version_with_message_topics.to_bytes()?);
        ret.append(&mut self.disable_contract_version.to_bytes()?);
        ret.append(&mut self.call_contract.to_bytes()?);
        ret.append(&mut self.call_versioned_contract.to_bytes()?);
        ret.append(&mut self.get_named_arg_size.to_bytes()?);
        ret.append(&mut self.get_named_arg.to_bytes()?);
        ret.append(&mut self.remove_contract_user_group.to_bytes()?);
        ret.append(&mut self.provision_contract_user_group_uref.to_bytes()?);
        ret.append(&mut self.remove_contract_user_group_urefs.to_bytes()?);
        ret.append(&mut self.print.to_bytes()?);
        ret.append(&mut self.blake2b.to_bytes()?);
        ret.append(&mut self.random_bytes.to_bytes()?);
        ret.append(&mut self.enable_contract_version.to_bytes()?);
        ret.append(&mut self.manage_message_topic.to_bytes()?);
        ret.append(&mut self.emit_message.to_bytes()?);
        ret.append(&mut self.cost_increase_per_message.to_bytes()?);
        ret.append(&mut self.get_block_info.to_bytes()?);
        ret.append(&mut self.generic_hash.to_bytes()?);
        ret.append(&mut self.recover_secp256k1.to_bytes()?);
        ret.append(&mut self.verify_signature.to_bytes()?);
        ret.append(&mut self.call_package_version.to_bytes()?);
        Ok(ret)
    }

    fn serialized_length(&self) -> usize {
        self.read_value.serialized_length()
            + self.dictionary_get.serialized_length()
            + self.write.serialized_length()
            + self.dictionary_put.serialized_length()
            + self.add.serialized_length()
            + self.new_uref.serialized_length()
            + self.load_named_keys.serialized_length()
            + self.ret.serialized_length()
            + self.get_key.serialized_length()
            + self.has_key.serialized_length()
            + self.put_key.serialized_length()
            + self.remove_key.serialized_length()
            + self.revert.serialized_length()
            + self.is_valid_uref.serialized_length()
            + self.add_associated_key.serialized_length()
            + self.remove_associated_key.serialized_length()
            + self.update_associated_key.serialized_length()
            + self.set_action_threshold.serialized_length()
            + self.get_caller.serialized_length()
            + self.get_blocktime.serialized_length()
            + self.create_purse.serialized_length()
            + self.transfer_to_account.serialized_length()
            + self.transfer_from_purse_to_account.serialized_length()
            + self.transfer_from_purse_to_purse.serialized_length()
            + self.get_balance.serialized_length()
            + self.get_phase.serialized_length()
            + self.get_system_contract.serialized_length()
            + self.get_main_purse.serialized_length()
            + self.read_host_buffer.serialized_length()
            + self.create_contract_package_at_hash.serialized_length()
            + self.create_contract_user_group.serialized_length()
            + self
                .add_contract_version_with_message_topics
                .serialized_length()
            + self.add_contract_version.serialized_length()
            + self
                .add_package_version_with_message_topics
                .serialized_length()
            + self.disable_contract_version.serialized_length()
            + self.call_contract.serialized_length()
            + self.call_versioned_contract.serialized_length()
            + self.get_named_arg_size.serialized_length()
            + self.get_named_arg.serialized_length()
            + self.remove_contract_user_group.serialized_length()
            + self.provision_contract_user_group_uref.serialized_length()
            + self.remove_contract_user_group_urefs.serialized_length()
            + self.print.serialized_length()
            + self.blake2b.serialized_length()
            + self.random_bytes.serialized_length()
            + self.enable_contract_version.serialized_length()
            + self.manage_message_topic.serialized_length()
            + self.emit_message.serialized_length()
            + self.cost_increase_per_message.serialized_length()
            + self.get_block_info.serialized_length()
            + self.generic_hash.serialized_length()
            + self.recover_secp256k1.serialized_length()
            + self.verify_signature.serialized_length()
            + self.call_package_version.serialized_length()
    }
}

impl FromBytes for HostFunctionCostsV1 {
    fn from_bytes(bytes: &[u8]) -> Result<(Self, &[u8]), bytesrepr::Error> {
        let (read_value, rem) = FromBytes::from_bytes(bytes)?;
        let (dictionary_get, rem) = FromBytes::from_bytes(rem)?;
        let (write, rem) = FromBytes::from_bytes(rem)?;
        let (dictionary_put, rem) = FromBytes::from_bytes(rem)?;
        let (add, rem) = FromBytes::from_bytes(rem)?;
        let (new_uref, rem) = FromBytes::from_bytes(rem)?;
        let (load_named_keys, rem) = FromBytes::from_bytes(rem)?;
        let (ret, rem) = FromBytes::from_bytes(rem)?;
        let (get_key, rem) = FromBytes::from_bytes(rem)?;
        let (has_key, rem) = FromBytes::from_bytes(rem)?;
        let (put_key, rem) = FromBytes::from_bytes(rem)?;
        let (remove_key, rem) = FromBytes::from_bytes(rem)?;
        let (revert, rem) = FromBytes::from_bytes(rem)?;
        let (is_valid_uref, rem) = FromBytes::from_bytes(rem)?;
        let (add_associated_key, rem) = FromBytes::from_bytes(rem)?;
        let (remove_associated_key, rem) = FromBytes::from_bytes(rem)?;
        let (update_associated_key, rem) = FromBytes::from_bytes(rem)?;
        let (set_action_threshold, rem) = FromBytes::from_bytes(rem)?;
        let (get_caller, rem) = FromBytes::from_bytes(rem)?;
        let (get_blocktime, rem) = FromBytes::from_bytes(rem)?;
        let (create_purse, rem) = FromBytes::from_bytes(rem)?;
        let (transfer_to_account, rem) = FromBytes::from_bytes(rem)?;
        let (transfer_from_purse_to_account, rem) = FromBytes::from_bytes(rem)?;
        let (transfer_from_purse_to_purse, rem) = FromBytes::from_bytes(rem)?;
        let (get_balance, rem) = FromBytes::from_bytes(rem)?;
        let (get_phase, rem) = FromBytes::from_bytes(rem)?;
        let (get_system_contract, rem) = FromBytes::from_bytes(rem)?;
        let (get_main_purse, rem) = FromBytes::from_bytes(rem)?;
        let (read_host_buffer, rem) = FromBytes::from_bytes(rem)?;
        let (create_contract_package_at_hash, rem) = FromBytes::from_bytes(rem)?;
        let (create_contract_user_group, rem) = FromBytes::from_bytes(rem)?;
        let (add_contract_version_with_message_topics, rem) = FromBytes::from_bytes(rem)?;
        let (add_contract_version, rem) = FromBytes::from_bytes(rem)?;
        let (add_package_version_with_message_topics, rem) = FromBytes::from_bytes(rem)?;
        let (disable_contract_version, rem) = FromBytes::from_bytes(rem)?;
        let (call_contract, rem) = FromBytes::from_bytes(rem)?;
        let (call_versioned_contract, rem) = FromBytes::from_bytes(rem)?;
        let (get_named_arg_size, rem) = FromBytes::from_bytes(rem)?;
        let (get_named_arg, rem) = FromBytes::from_bytes(rem)?;
        let (remove_contract_user_group, rem) = FromBytes::from_bytes(rem)?;
        let (provision_contract_user_group_uref, rem) = FromBytes::from_bytes(rem)?;
        let (remove_contract_user_group_urefs, rem) = FromBytes::from_bytes(rem)?;
        let (print, rem) = FromBytes::from_bytes(rem)?;
        let (blake2b, rem) = FromBytes::from_bytes(rem)?;
        let (random_bytes, rem) = FromBytes::from_bytes(rem)?;
        let (enable_contract_version, rem) = FromBytes::from_bytes(rem)?;
        let (manage_message_topic, rem) = FromBytes::from_bytes(rem)?;
        let (emit_message, rem) = FromBytes::from_bytes(rem)?;
        let (cost_increase_per_message, rem) = FromBytes::from_bytes(rem)?;
        let (get_block_info, rem) = FromBytes::from_bytes(rem)?;
        let (generic_hash, rem) = FromBytes::from_bytes(rem)?;
        let (recover_secp256k1, rem) = FromBytes::from_bytes(rem)?;
        let (verify_signature, rem) = FromBytes::from_bytes(rem)?;
        let (call_package_version, rem) = FromBytes::from_bytes(rem)?;
        Ok((
            HostFunctionCostsV1 {
                read_value,
                dictionary_get,
                write,
                dictionary_put,
                add,
                new_uref,
                load_named_keys,
                ret,
                get_key,
                has_key,
                put_key,
                remove_key,
                revert,
                is_valid_uref,
                add_associated_key,
                remove_associated_key,
                update_associated_key,
                set_action_threshold,
                get_caller,
                get_blocktime,
                create_purse,
                transfer_to_account,
                transfer_from_purse_to_account,
                transfer_from_purse_to_purse,
                get_balance,
                get_phase,
                get_system_contract,
                get_main_purse,
                read_host_buffer,
                create_contract_package_at_hash,
                create_contract_user_group,
                add_contract_version_with_message_topics,
                add_contract_version,
                add_package_version_with_message_topics,
                disable_contract_version,
                call_contract,
                call_versioned_contract,
                get_named_arg_size,
                get_named_arg,
                remove_contract_user_group,
                provision_contract_user_group_uref,
                remove_contract_user_group_urefs,
                print,
                blake2b,
                random_bytes,
                enable_contract_version,
                manage_message_topic,
                emit_message,
                cost_increase_per_message,
                get_block_info,
                generic_hash,
                recover_secp256k1,
                verify_signature,
                call_package_version,
            },
            rem,
        ))
    }
}

#[cfg(any(feature = "testing", test))]
impl Distribution<HostFunctionCostsV1> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> HostFunctionCostsV1 {
        HostFunctionCostsV1 {
            read_value: rng.gen(),
            dictionary_get: rng.gen(),
            write: rng.gen(),
            dictionary_put: rng.gen(),
            add: rng.gen(),
            new_uref: rng.gen(),
            load_named_keys: rng.gen(),
            ret: rng.gen(),
            get_key: rng.gen(),
            has_key: rng.gen(),
            put_key: rng.gen(),
            remove_key: rng.gen(),
            revert: rng.gen(),
            is_valid_uref: rng.gen(),
            add_associated_key: rng.gen(),
            remove_associated_key: rng.gen(),
            update_associated_key: rng.gen(),
            set_action_threshold: rng.gen(),
            get_caller: rng.gen(),
            get_blocktime: rng.gen(),
            create_purse: rng.gen(),
            transfer_to_account: rng.gen(),
            transfer_from_purse_to_account: rng.gen(),
            transfer_from_purse_to_purse: rng.gen(),
            get_balance: rng.gen(),
            get_phase: rng.gen(),
            get_system_contract: rng.gen(),
            get_main_purse: rng.gen(),
            read_host_buffer: rng.gen(),
            create_contract_package_at_hash: rng.gen(),
            create_contract_user_group: rng.gen(),
            add_contract_version_with_message_topics: rng.gen(),
            add_contract_version: rng.gen(),
            add_package_version_with_message_topics: rng.gen(),
            disable_contract_version: rng.gen(),
            call_contract: rng.gen(),
            call_versioned_contract: rng.gen(),
            get_named_arg_size: rng.gen(),
            get_named_arg: rng.gen(),
            remove_contract_user_group: rng.gen(),
            provision_contract_user_group_uref: rng.gen(),
            remove_contract_user_group_urefs: rng.gen(),
            print: rng.gen(),
            blake2b: rng.gen(),
            random_bytes: rng.gen(),
            enable_contract_version: rng.gen(),
            manage_message_topic: rng.gen(),
            emit_message: rng.gen(),
            cost_increase_per_message: rng.gen(),
            get_block_info: rng.gen(),
            generic_hash: rng.gen(),
            recover_secp256k1: rng.gen(),
            verify_signature: rng.gen(),
            call_package_version: rng.gen(),
        }
    }
}

#[doc(hidden)]
#[cfg(any(feature = "gens", test))]
pub mod gens {
    use proptest::{num, prelude::*};

    use crate::{HostFunction, HostFunctionCost, HostFunctionCostsV1};

    #[allow(unused)]
    pub fn host_function_cost_arb<T: Copy + Arbitrary>() -> impl Strategy<Value = HostFunction<T>> {
        (any::<HostFunctionCost>(), any::<T>())
            .prop_map(|(cost, arguments)| HostFunction::new(cost, arguments))
    }

    prop_compose! {
        pub fn host_function_costs_arb() (
            read_value in host_function_cost_arb(),
            dictionary_get in host_function_cost_arb(),
            write in host_function_cost_arb(),
            dictionary_put in host_function_cost_arb(),
            add in host_function_cost_arb(),
            new_uref in host_function_cost_arb(),
            load_named_keys in host_function_cost_arb(),
            ret in host_function_cost_arb(),
            get_key in host_function_cost_arb(),
            has_key in host_function_cost_arb(),
            put_key in host_function_cost_arb(),
            remove_key in host_function_cost_arb(),
            revert in host_function_cost_arb(),
            is_valid_uref in host_function_cost_arb(),
            add_associated_key in host_function_cost_arb(),
            remove_associated_key in host_function_cost_arb(),
            update_associated_key in host_function_cost_arb(),
            set_action_threshold in host_function_cost_arb(),
            get_caller in host_function_cost_arb(),
            get_blocktime in host_function_cost_arb(),
            create_purse in host_function_cost_arb(),
            transfer_to_account in host_function_cost_arb(),
            transfer_from_purse_to_account in host_function_cost_arb(),
            transfer_from_purse_to_purse in host_function_cost_arb(),
            get_balance in host_function_cost_arb(),
            get_phase in host_function_cost_arb(),
            get_system_contract in host_function_cost_arb(),
            get_main_purse in host_function_cost_arb(),
            read_host_buffer in host_function_cost_arb(),
            create_contract_package_at_hash in host_function_cost_arb(),
            create_contract_user_group in host_function_cost_arb(),
            add_contract_version_with_message_topics in host_function_cost_arb(),
            add_contract_version in host_function_cost_arb(),
            add_package_version_with_message_topics in host_function_cost_arb(),
            disable_contract_version in host_function_cost_arb(),
            call_contract in host_function_cost_arb(),
            call_versioned_contract in host_function_cost_arb(),
            get_named_arg_size in host_function_cost_arb(),
            get_named_arg in host_function_cost_arb(),
            remove_contract_user_group in host_function_cost_arb(),
            provision_contract_user_group_uref in host_function_cost_arb(),
            remove_contract_user_group_urefs in host_function_cost_arb(),
            print in host_function_cost_arb(),
            blake2b in host_function_cost_arb(),
            random_bytes in host_function_cost_arb(),
            enable_contract_version in host_function_cost_arb(),
            manage_message_topic in host_function_cost_arb(),
            emit_message in host_function_cost_arb(),
            cost_increase_per_message in num::u32::ANY,
            get_block_info in host_function_cost_arb(),
            generic_hash in host_function_cost_arb(),
            recover_secp256k1 in host_function_cost_arb(),
            verify_signature in host_function_cost_arb(),
            call_package_version in host_function_cost_arb(),
        ) -> HostFunctionCostsV1 {
            HostFunctionCostsV1 {
                read_value,
                dictionary_get,
                write,
                dictionary_put,
                add,
                new_uref,
                load_named_keys,
                ret,
                get_key,
                has_key,
                put_key,
                remove_key,
                revert,
                is_valid_uref,
                add_associated_key,
                remove_associated_key,
                update_associated_key,
                set_action_threshold,
                get_caller,
                get_blocktime,
                create_purse,
                transfer_to_account,
                transfer_from_purse_to_account,
                transfer_from_purse_to_purse,
                get_balance,
                get_phase,
                get_system_contract,
                get_main_purse,
                read_host_buffer,
                create_contract_package_at_hash,
                create_contract_user_group,
                add_contract_version_with_message_topics,
                add_contract_version,
                add_package_version_with_message_topics,
                disable_contract_version,
                call_contract,
                call_versioned_contract,
                get_named_arg_size,
                get_named_arg,
                remove_contract_user_group,
                provision_contract_user_group_uref,
                remove_contract_user_group_urefs,
                print,
                blake2b,
                random_bytes,
                enable_contract_version,
                manage_message_topic,
                emit_message,
                cost_increase_per_message,
                get_block_info,
                generic_hash,
                recover_secp256k1,
                verify_signature,
                call_package_version,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::U512;

    use super::*;

    const COST: Cost = 42;
    const ARGUMENT_COSTS: [Cost; 3] = [123, 456, 789];
    const WEIGHTS: [Cost; 3] = [1000, 1100, 1200];

    #[test]
    fn calculate_gas_cost_for_host_function() {
        let host_function = HostFunction::new(COST, ARGUMENT_COSTS);
        let expected_cost = COST
            + (ARGUMENT_COSTS[0] * WEIGHTS[0])
            + (ARGUMENT_COSTS[1] * WEIGHTS[1])
            + (ARGUMENT_COSTS[2] * WEIGHTS[2]);
        assert_eq!(
            host_function.calculate_gas_cost(WEIGHTS),
            Some(Gas::new(expected_cost))
        );
    }

    #[test]
    fn calculate_gas_cost_would_overflow() {
        let large_value = Cost::MAX;

        let host_function = HostFunction::new(
            large_value,
            [large_value, large_value, large_value, large_value],
        );

        let lhs =
            host_function.calculate_gas_cost([large_value, large_value, large_value, large_value]);

        let large_value = U512::from(large_value);
        let rhs = large_value + (U512::from(4) * large_value * large_value);

        assert_eq!(lhs, Some(Gas::new(rhs)));
    }
}

#[cfg(test)]
mod proptests {
    use proptest::prelude::*;

    use crate::bytesrepr;

    use super::*;

    type Signature = [Cost; 10];

    proptest! {
        #[test]
        fn test_host_function(host_function in gens::host_function_cost_arb::<Signature>()) {
            bytesrepr::test_serialization_roundtrip(&host_function);
        }

        #[test]
        fn test_host_function_costs(host_function_costs in gens::host_function_costs_arb()) {
            bytesrepr::test_serialization_roundtrip(&host_function_costs);
        }
    }
}
