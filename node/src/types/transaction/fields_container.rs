#[cfg(test)]
use super::arg_handling;
use casper_types::{
    bytesrepr::{Bytes, ToBytes},
    TransactionArgs, TransactionEntryPoint, TransactionScheduling, TransactionTarget,
};
#[cfg(test)]
use casper_types::{
    testing::TestRng, PublicKey, RuntimeArgs, TransactionInvocationTarget, TransactionRuntime,
    TransferTarget,
};
#[cfg(test)]
use rand::{Rng, RngCore};
use std::collections::BTreeMap;

pub(crate) const ARGS_MAP_KEY: u16 = 0;
pub(crate) const TARGET_MAP_KEY: u16 = 1;
pub(crate) const ENTRY_POINT_MAP_KEY: u16 = 2;
pub(crate) const SCHEDULING_MAP_KEY: u16 = 3;

#[derive(Clone, Eq, PartialEq, Debug)]
pub(crate) enum FieldsContainerError {
    CouldNotSerializeField { field_index: u16 },
}

pub(crate) struct FieldsContainer {
    pub(super) args: TransactionArgs,
    pub(super) target: TransactionTarget,
    pub(super) entry_point: TransactionEntryPoint,
    pub(super) scheduling: TransactionScheduling,
}

impl FieldsContainer {
    pub(crate) fn new(
        args: TransactionArgs,
        target: TransactionTarget,
        entry_point: TransactionEntryPoint,
        scheduling: TransactionScheduling,
    ) -> Self {
        FieldsContainer {
            args,
            target,
            entry_point,
            scheduling,
        }
    }

    pub(crate) fn to_map(&self) -> Result<BTreeMap<u16, Bytes>, FieldsContainerError> {
        let mut map: BTreeMap<u16, Bytes> = BTreeMap::new();
        map.insert(
            ARGS_MAP_KEY,
            self.args.to_bytes().map(Into::into).map_err(|_| {
                FieldsContainerError::CouldNotSerializeField {
                    field_index: ARGS_MAP_KEY,
                }
            })?,
        );
        map.insert(
            TARGET_MAP_KEY,
            self.target.to_bytes().map(Into::into).map_err(|_| {
                FieldsContainerError::CouldNotSerializeField {
                    field_index: TARGET_MAP_KEY,
                }
            })?,
        );
        map.insert(
            ENTRY_POINT_MAP_KEY,
            self.entry_point.to_bytes().map(Into::into).map_err(|_| {
                FieldsContainerError::CouldNotSerializeField {
                    field_index: ENTRY_POINT_MAP_KEY,
                }
            })?,
        );
        map.insert(
            SCHEDULING_MAP_KEY,
            self.scheduling.to_bytes().map(Into::into).map_err(|_| {
                FieldsContainerError::CouldNotSerializeField {
                    field_index: SCHEDULING_MAP_KEY,
                }
            })?,
        );
        Ok(map)
    }

    /// Returns a random `FieldsContainer`.
    #[cfg(test)]
    pub(crate) fn random(rng: &mut TestRng) -> Self {
        match rng.gen_range(0..8) {
            0 => {
                let amount = rng.gen_range(2_500_000_000..=u64::MAX);
                let maybe_source = if rng.gen() { Some(rng.gen()) } else { None };
                let target = TransferTarget::random(rng);
                let maybe_id = rng.gen::<bool>().then(|| rng.gen());
                let args = arg_handling::new_transfer_args(amount, maybe_source, target, maybe_id)
                    .unwrap();
                FieldsContainer::new(
                    TransactionArgs::Named(args),
                    TransactionTarget::Native,
                    TransactionEntryPoint::Transfer,
                    TransactionScheduling::random(rng),
                )
            }
            1 => {
                let public_key = PublicKey::random(rng);
                let delegation_rate = rng.gen();
                let amount = rng.gen::<u64>();
                let minimum_delegation_amount = rng.gen::<bool>().then(|| rng.gen());
                let maximum_delegation_amount =
                    minimum_delegation_amount.map(|minimum_delegation_amount| {
                        minimum_delegation_amount + rng.gen::<u32>() as u64
                    });
                let reserved_slots = rng.gen::<bool>().then(|| rng.gen::<u32>());
                let args = arg_handling::new_add_bid_args(
                    public_key,
                    delegation_rate,
                    amount,
                    minimum_delegation_amount,
                    maximum_delegation_amount,
                    reserved_slots,
                )
                .unwrap();
                FieldsContainer::new(
                    TransactionArgs::Named(args),
                    TransactionTarget::Native,
                    TransactionEntryPoint::AddBid,
                    TransactionScheduling::random(rng),
                )
            }
            2 => {
                let public_key = PublicKey::random(rng);
                let amount = rng.gen::<u64>();
                let args = arg_handling::new_withdraw_bid_args(public_key, amount).unwrap();
                FieldsContainer::new(
                    TransactionArgs::Named(args),
                    TransactionTarget::Native,
                    TransactionEntryPoint::WithdrawBid,
                    TransactionScheduling::random(rng),
                )
            }
            3 => {
                let delegator = PublicKey::random(rng);
                let validator = PublicKey::random(rng);
                let amount = rng.gen::<u64>();
                let args = arg_handling::new_delegate_args(delegator, validator, amount).unwrap();
                FieldsContainer::new(
                    TransactionArgs::Named(args),
                    TransactionTarget::Native,
                    TransactionEntryPoint::Delegate,
                    TransactionScheduling::random(rng),
                )
            }
            4 => {
                let delegator = PublicKey::random(rng);
                let validator = PublicKey::random(rng);
                let amount = rng.gen::<u64>();
                let args = arg_handling::new_undelegate_args(delegator, validator, amount).unwrap();
                FieldsContainer::new(
                    TransactionArgs::Named(args),
                    TransactionTarget::Native,
                    TransactionEntryPoint::Undelegate,
                    TransactionScheduling::random(rng),
                )
            }
            5 => {
                let delegator = PublicKey::random(rng);
                let validator = PublicKey::random(rng);
                let amount = rng.gen::<u64>();
                let new_validator = PublicKey::random(rng);
                let args =
                    arg_handling::new_redelegate_args(delegator, validator, amount, new_validator)
                        .unwrap();
                FieldsContainer::new(
                    TransactionArgs::Named(args),
                    TransactionTarget::Native,
                    TransactionEntryPoint::Redelegate,
                    TransactionScheduling::random(rng),
                )
            }
            6 => Self::random_standard(rng),
            7 => {
                let mut buffer = vec![0u8; rng.gen_range(1..100)];
                rng.fill_bytes(buffer.as_mut());
                let is_install_upgrade = rng.gen();
                let target = TransactionTarget::Session {
                    is_install_upgrade,
                    module_bytes: Bytes::from(buffer),
                    runtime: TransactionRuntime::VmCasperV1,
                    transferred_value: rng.gen(),
                    seed: rng.gen(),
                };
                FieldsContainer::new(
                    TransactionArgs::Named(RuntimeArgs::random(rng)),
                    target,
                    TransactionEntryPoint::Call,
                    TransactionScheduling::random(rng),
                )
            }
            _ => unreachable!(),
        }
    }

    #[cfg(test)]
    fn random_standard(rng: &mut TestRng) -> Self {
        let target = TransactionTarget::Stored {
            id: TransactionInvocationTarget::random(rng),
            runtime: TransactionRuntime::VmCasperV1,
            transferred_value: rng.gen(),
        };
        FieldsContainer::new(
            TransactionArgs::Named(RuntimeArgs::random(rng)),
            target,
            TransactionEntryPoint::Custom(rng.random_string(1..11)),
            TransactionScheduling::random(rng),
        )
    }
}
