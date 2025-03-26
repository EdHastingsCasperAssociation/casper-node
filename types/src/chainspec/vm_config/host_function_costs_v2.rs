#[cfg(feature = "datasize")]
use datasize::DataSize;
use derive_more::Add;
use num_traits::Zero;
use rand::{distributions::Standard, prelude::Distribution, Rng};
use serde::{Deserialize, Serialize};

use crate::bytesrepr::{self, FromBytes, ToBytes};

use super::HostFunction;

/// Representation of argument's cost.
pub type Cost = u32;

/// An identifier that represents an unused argument.
const NOT_USED: Cost = 0;

/// An arbitrary default fixed cost for host functions that were not researched yet.
const DEFAULT_FIXED_COST: Cost = 200;

const DEFAULT_CALL_COST: u32 = 10_000;
const DEFAULT_ENV_TRANSFERRED_VALUE_COST: u32 = 10_000;
const DEFAULT_ENV_BALANCE_COST: u32 = 100;
const DEFAULT_ENV_BLOCK_TIME_COST: u32 = 100;
const DEFAULT_GET_CALLER_COST: u32 = 100;

const DEFAULT_PRINT_COST: u32 = 100;

const DEFAULT_READ_COST: u32 = 1_000;
const DEFAULT_READ_KEY_SIZE_WEIGHT: u32 = 100;

const DEFAULT_RET_COST: u32 = 300;
const DEFAULT_RET_VALUE_SIZE_WEIGHT: u32 = 100;

const DEFAULT_TRANSFER_COST: u32 = 2_500_000_000;

const DEFAULT_WRITE_COST: u32 = 25_000;
const DEFAULT_WRITE_SIZE_WEIGHT: u32 = 100_000;

const DEFAULT_COPY_INPUT_COST: u32 = 300;
const DEFAULT_COPY_INPUT_VALUE_SIZE_WEIGHT: u32 = 0;

const DEFAULT_CREATE_COST: u32 = 0;
const DEFAULT_CREATE_CODE_SIZE_WEIGHT: u32 = 0;
const DEFAULT_CREATE_ENTRYPOINT_SIZE_WEIGHT: u32 = 0;
const DEFAULT_CREATE_INPUT_SIZE_WEIGHT: u32 = 0;
const DEFAULT_CREATE_SEED_SIZE_WEIGHT: u32 = 0;

const DEFAULT_EMIT_COST: u32 = 200;
const DEFAULT_EMIT_TOPIC_SIZE_WEIGHT: u32 = 100;
const DEFAULT_EMIT_PAYLOAD_SIZE_HEIGHT: u32 = 100;

/// Definition of a host function cost table.
#[derive(Add, Copy, Clone, PartialEq, Eq, Serialize, Deserialize, Debug)]
#[cfg_attr(feature = "datasize", derive(DataSize))]
#[serde(deny_unknown_fields)]
pub struct HostFunctionCostsV2 {
    /// Cost of calling the `read` host function.
    pub read: HostFunction<[Cost; 6]>,
    /// Cost of calling the `write` host function.
    pub write: HostFunction<[Cost; 5]>,
    /// Cost of calling the `copy_input` host function.
    pub copy_input: HostFunction<[Cost; 2]>,
    /// Cost of calling the `ret` host function.
    pub ret: HostFunction<[Cost; 2]>,
    /// Cost of calling the `create` host function.
    pub create: HostFunction<[Cost; 10]>,
    /// Cost of calling the `env_caller` host function.
    pub env_caller: HostFunction<[Cost; 3]>,
    /// Cost of calling the `env_block_time` host function.
    pub env_block_time: HostFunction<[Cost; 0]>,
    /// Cost of calling the `env_transferred_value` host function.
    pub env_transferred_value: HostFunction<[Cost; 1]>,
    /// Cost of calling the `transfer` host function.
    pub transfer: HostFunction<[Cost; 3]>,
    /// Cost of calling the `env_balance` host function.
    pub env_balance: HostFunction<[Cost; 4]>,
    /// Cost of calling the `upgrade` host function.
    pub upgrade: HostFunction<[Cost; 6]>,
    /// Cost of calling the `call` host function.
    pub call: HostFunction<[Cost; 9]>,
    /// Cost of calling the `print` host function.
    pub print: HostFunction<[Cost; 2]>,
    /// Cost of calling the `emit` host function.
    pub emit: HostFunction<[Cost; 4]>,
}

impl Zero for HostFunctionCostsV2 {
    fn zero() -> Self {
        Self {
            read: HostFunction::zero(),
            write: HostFunction::zero(),
            copy_input: HostFunction::zero(),
            ret: HostFunction::zero(),
            create: HostFunction::zero(),
            env_caller: HostFunction::zero(),
            env_block_time: HostFunction::zero(),
            env_transferred_value: HostFunction::zero(),
            transfer: HostFunction::zero(),
            env_balance: HostFunction::zero(),
            upgrade: HostFunction::zero(),
            call: HostFunction::zero(),
            print: HostFunction::zero(),
            emit: HostFunction::zero(),
        }
    }

    fn is_zero(&self) -> bool {
        let HostFunctionCostsV2 {
            read,
            write,
            copy_input,
            ret,
            create,
            env_caller,
            env_block_time,
            env_transferred_value,
            transfer,
            env_balance,
            upgrade,
            call,
            print,
            emit,
        } = self;
        read.is_zero()
            && write.is_zero()
            && copy_input.is_zero()
            && ret.is_zero()
            && create.is_zero()
            && env_caller.is_zero()
            && env_block_time.is_zero()
            && env_transferred_value.is_zero()
            && transfer.is_zero()
            && env_balance.is_zero()
            && upgrade.is_zero()
            && call.is_zero()
            && print.is_zero()
            && emit.is_zero()
    }
}

impl Default for HostFunctionCostsV2 {
    fn default() -> Self {
        Self {
            read: HostFunction::new(
                DEFAULT_READ_COST,
                [
                    NOT_USED,
                    NOT_USED,
                    DEFAULT_READ_KEY_SIZE_WEIGHT,
                    NOT_USED,
                    NOT_USED,
                    NOT_USED,
                ],
            ),
            write: HostFunction::new(
                DEFAULT_WRITE_COST,
                [
                    NOT_USED,
                    NOT_USED,
                    NOT_USED,
                    NOT_USED,
                    DEFAULT_WRITE_SIZE_WEIGHT,
                ],
            ),
            copy_input: HostFunction::new(
                DEFAULT_COPY_INPUT_COST,
                [NOT_USED, DEFAULT_COPY_INPUT_VALUE_SIZE_WEIGHT],
            ),
            ret: HostFunction::new(DEFAULT_RET_COST, [NOT_USED, DEFAULT_RET_VALUE_SIZE_WEIGHT]),
            create: HostFunction::new(
                DEFAULT_CREATE_COST,
                [
                    NOT_USED,
                    DEFAULT_CREATE_CODE_SIZE_WEIGHT,
                    NOT_USED,
                    NOT_USED,
                    DEFAULT_CREATE_ENTRYPOINT_SIZE_WEIGHT,
                    NOT_USED,
                    DEFAULT_CREATE_INPUT_SIZE_WEIGHT,
                    NOT_USED,
                    DEFAULT_CREATE_SEED_SIZE_WEIGHT,
                    NOT_USED,
                ],
            ),
            env_caller: HostFunction::new(DEFAULT_GET_CALLER_COST, [NOT_USED, NOT_USED, NOT_USED]),
            env_balance: HostFunction::fixed(DEFAULT_ENV_BALANCE_COST),
            env_block_time: HostFunction::fixed(DEFAULT_ENV_BLOCK_TIME_COST),
            env_transferred_value: HostFunction::fixed(DEFAULT_ENV_TRANSFERRED_VALUE_COST),
            transfer: HostFunction::new(DEFAULT_TRANSFER_COST, [NOT_USED, NOT_USED, NOT_USED]),
            upgrade: HostFunction::new(
                DEFAULT_FIXED_COST,
                [NOT_USED, NOT_USED, NOT_USED, NOT_USED, NOT_USED, NOT_USED],
            ),
            call: HostFunction::new(
                DEFAULT_CALL_COST,
                [
                    NOT_USED, NOT_USED, NOT_USED, NOT_USED, NOT_USED, NOT_USED, NOT_USED, NOT_USED,
                    NOT_USED,
                ],
            ),
            print: HostFunction::new(DEFAULT_PRINT_COST, [NOT_USED, NOT_USED]),
            emit: HostFunction::new(
                DEFAULT_EMIT_COST,
                [
                    NOT_USED,
                    DEFAULT_EMIT_TOPIC_SIZE_WEIGHT,
                    NOT_USED,
                    DEFAULT_EMIT_PAYLOAD_SIZE_HEIGHT,
                ],
            ),
        }
    }
}

impl ToBytes for HostFunctionCostsV2 {
    fn to_bytes(&self) -> Result<Vec<u8>, bytesrepr::Error> {
        let mut ret = bytesrepr::unchecked_allocate_buffer(self);
        ret.append(&mut self.read.to_bytes()?);
        ret.append(&mut self.write.to_bytes()?);
        ret.append(&mut self.copy_input.to_bytes()?);
        ret.append(&mut self.ret.to_bytes()?);
        ret.append(&mut self.create.to_bytes()?);
        ret.append(&mut self.env_caller.to_bytes()?);
        ret.append(&mut self.env_block_time.to_bytes()?);
        ret.append(&mut self.env_transferred_value.to_bytes()?);
        ret.append(&mut self.transfer.to_bytes()?);
        ret.append(&mut self.env_balance.to_bytes()?);
        ret.append(&mut self.upgrade.to_bytes()?);
        ret.append(&mut self.call.to_bytes()?);
        ret.append(&mut self.print.to_bytes()?);
        ret.append(&mut self.emit.to_bytes()?);
        Ok(ret)
    }

    fn serialized_length(&self) -> usize {
        self.read.serialized_length()
            + self.write.serialized_length()
            + self.copy_input.serialized_length()
            + self.ret.serialized_length()
            + self.create.serialized_length()
            + self.env_caller.serialized_length()
            + self.env_block_time.serialized_length()
            + self.env_transferred_value.serialized_length()
            + self.transfer.serialized_length()
            + self.env_balance.serialized_length()
            + self.upgrade.serialized_length()
            + self.call.serialized_length()
            + self.print.serialized_length()
            + self.emit.serialized_length()
    }
}

impl FromBytes for HostFunctionCostsV2 {
    fn from_bytes(bytes: &[u8]) -> Result<(Self, &[u8]), bytesrepr::Error> {
        let (read, rem) = FromBytes::from_bytes(bytes)?;
        let (write, rem) = FromBytes::from_bytes(rem)?;
        let (copy_input, rem) = FromBytes::from_bytes(rem)?;
        let (ret, rem) = FromBytes::from_bytes(rem)?;
        let (create, rem) = FromBytes::from_bytes(rem)?;
        let (env_caller, rem) = FromBytes::from_bytes(rem)?;
        let (env_block_time, rem) = FromBytes::from_bytes(rem)?;
        let (env_transferred_value, rem) = FromBytes::from_bytes(rem)?;
        let (transfer, rem) = FromBytes::from_bytes(rem)?;
        let (env_balance, rem) = FromBytes::from_bytes(rem)?;
        let (upgrade, rem) = FromBytes::from_bytes(rem)?;
        let (call, rem) = FromBytes::from_bytes(rem)?;
        let (print, rem) = FromBytes::from_bytes(rem)?;
        let (emit, rem) = FromBytes::from_bytes(rem)?;
        Ok((
            HostFunctionCostsV2 {
                read,
                write,
                copy_input,
                ret,
                create,
                env_caller,
                env_block_time,
                env_transferred_value,
                transfer,
                env_balance,
                upgrade,
                call,
                print,
                emit,
            },
            rem,
        ))
    }
}

#[cfg(any(feature = "testing", test))]
impl Distribution<HostFunctionCostsV2> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> HostFunctionCostsV2 {
        HostFunctionCostsV2 {
            read: rng.gen(),
            write: rng.gen(),
            copy_input: rng.gen(),
            ret: rng.gen(),
            create: rng.gen(),
            env_caller: rng.gen(),
            env_block_time: rng.gen(),
            env_transferred_value: rng.gen(),
            transfer: rng.gen(),
            env_balance: rng.gen(),
            upgrade: rng.gen(),
            call: rng.gen(),
            print: rng.gen(),
            emit: rng.gen(),
        }
    }
}

#[doc(hidden)]
#[cfg(any(feature = "gens", test))]
pub mod gens {
    use proptest::prelude::*;

    use crate::{HostFunction, HostFunctionCost, HostFunctionCostsV2};

    #[allow(unused)]
    pub fn host_function_cost_v2_arb<T: Copy + Arbitrary>() -> impl Strategy<Value = HostFunction<T>>
    {
        (any::<HostFunctionCost>(), any::<T>())
            .prop_map(|(cost, arguments)| HostFunction::new(cost, arguments))
    }

    prop_compose! {
        pub fn host_function_costs_v2_arb() (
            read in host_function_cost_v2_arb(),
            write in host_function_cost_v2_arb(),
            copy_input in host_function_cost_v2_arb(),
            ret in host_function_cost_v2_arb(),
            create in host_function_cost_v2_arb(),
            env_caller in host_function_cost_v2_arb(),
            env_block_time in host_function_cost_v2_arb(),
            env_transferred_value in host_function_cost_v2_arb(),
            transfer in host_function_cost_v2_arb(),
            env_balance in host_function_cost_v2_arb(),
            upgrade in host_function_cost_v2_arb(),
            call in host_function_cost_v2_arb(),
            print in host_function_cost_v2_arb(),
            emit in host_function_cost_v2_arb(),
        ) -> HostFunctionCostsV2 {
            HostFunctionCostsV2 {
                read,
                write,
                copy_input,
                ret,
                create,
                env_caller,
                env_block_time,
                env_transferred_value,
                transfer,
                env_balance,
                upgrade,
                call,
                print,
                emit,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{Gas, U512};

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
        fn test_host_function(host_function in gens::host_function_cost_v2_arb::<Signature>()) {
            bytesrepr::test_serialization_roundtrip(&host_function);
        }

        #[test]
        fn test_host_function_costs(host_function_costs in gens::host_function_costs_v2_arb()) {
            bytesrepr::test_serialization_roundtrip(&host_function_costs);
        }
    }
}
