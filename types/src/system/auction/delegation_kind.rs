use crate::system::auction::UnbonderIdentity;
use crate::{
    bytesrepr,
    bytesrepr::{FromBytes, ToBytes, U8_SERIALIZED_LENGTH},
    system::auction::{
        bid::VestingSchedule, delegation_purse_bid::DelegatorPurseBid, BidAddr, BidAddrDelegator,
        Delegator,
    },
    PublicKey, URef, U512,
};
use core::{
    fmt,
    fmt::{Display, Formatter},
};
use datasize::DataSize;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// DelegationKindTag variants.
#[allow(clippy::large_enum_variant)]
#[repr(u8)]
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, Clone)]
pub enum DelegationKindTag {
    /// Delegation via public key. Expected to correspond with an existing Account.
    PublicKey = 0,
    /// Delegation via a purse.
    Purse = 1,
}

/// Delegation bid variants.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "datasize", derive(DataSize))]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
pub enum DelegationKind {
    /// A bid record containing delegator data associated with a PublicKey.
    PublicKey(Box<Delegator>), // Delegator is the original data structure.
    /// A bid record containing delegator data associated with a Purse.
    Purse(Box<DelegatorPurseBid>),
}

impl DelegationKind {
    /// Checks if a bid is still locked under a vesting schedule.
    ///
    /// Returns true if a timestamp falls below the initial lockup period + 91 days release
    /// schedule, otherwise false.
    pub fn is_locked(&self, timestamp_millis: u64) -> bool {
        match self {
            DelegationKind::PublicKey(delegator) => delegator.is_locked(timestamp_millis),
            DelegationKind::Purse(delegator) => delegator.is_locked(timestamp_millis),
        }
    }

    /// Checks if a bid is still locked under a vesting schedule.
    ///
    /// Returns true if a timestamp falls below the initial lockup period + 91 days release
    /// schedule, otherwise false.
    pub fn is_locked_with_vesting_schedule(
        &self,
        timestamp_millis: u64,
        vesting_schedule_period_millis: u64,
    ) -> bool {
        match self {
            DelegationKind::PublicKey(delegator) => delegator
                .is_locked_with_vesting_schedule(timestamp_millis, vesting_schedule_period_millis),
            DelegationKind::Purse(delegator) => delegator
                .is_locked_with_vesting_schedule(timestamp_millis, vesting_schedule_period_millis),
        }
    }

    /// The correct BidAddrDelegator for this instance.
    pub fn bid_addr_delegator(&self) -> BidAddrDelegator {
        match self {
            DelegationKind::PublicKey(delegator) => delegator.bid_addr_delegator(),
            DelegationKind::Purse(delegator) => delegator.bid_addr_delegator(),
        }
    }

    /// The correct BidAddr for this instance.
    pub fn bid_addr(&self) -> BidAddr {
        match self {
            DelegationKind::PublicKey(delegator) => delegator.bid_addr(),
            DelegationKind::Purse(delegator) => delegator.bid_addr(),
        }
    }

    /// The correct UnbonderIdentity for this instance.
    pub fn unbonder_identity(&self) -> UnbonderIdentity {
        match self {
            DelegationKind::PublicKey(delegator) => {
                UnbonderIdentity::PublicKey(delegator.delegator_public_key().clone())
            }
            DelegationKind::Purse(delegator) => {
                UnbonderIdentity::Purse(delegator.delegator_source_purse().addr())
            }
        }
    }

    /// Returns the staked amount
    pub fn staked_amount(&self) -> U512 {
        match self {
            DelegationKind::PublicKey(delegator) => delegator.staked_amount(),
            DelegationKind::Purse(delegator) => delegator.staked_amount(),
        }
    }

    /// Returns the bonding purse
    pub fn bonding_purse(&self) -> &URef {
        match self {
            DelegationKind::PublicKey(delegator) => delegator.bonding_purse(),
            DelegationKind::Purse(delegator) => delegator.bonding_purse(),
        }
    }

    /// Returns the public key of the validator this delegation is staked to.
    pub fn validator_public_key(&self) -> &PublicKey {
        match self {
            DelegationKind::PublicKey(delegator) => delegator.validator_public_key(),
            DelegationKind::Purse(delegator) => delegator.validator_public_key(),
        }
    }

    /// Returns the public key of the validator this delegation is staked to.
    pub fn delegator_public_key(&self) -> Option<&PublicKey> {
        match self {
            DelegationKind::PublicKey(delegator) => Some(delegator.delegator_public_key()),
            DelegationKind::Purse(_) => None,
        }
    }

    /// Returns a reference to the vesting schedule of the provided
    /// delegator bid.  `None` if a non-genesis validator.
    pub fn vesting_schedule(&self) -> Option<&VestingSchedule> {
        match self {
            DelegationKind::PublicKey(delegator) => delegator.vesting_schedule(),
            DelegationKind::Purse(delegator) => delegator.vesting_schedule(),
        }
    }

    /// Returns a mutable reference to the vesting schedule of the provided
    /// delegator bid.  `None` if a non-genesis validator.
    pub fn vesting_schedule_mut(&mut self) -> Option<&mut VestingSchedule> {
        match self {
            DelegationKind::PublicKey(delegator) => delegator.vesting_schedule_mut(),
            DelegationKind::Purse(delegator) => delegator.vesting_schedule_mut(),
        }
    }

    /// DelegationKindTag.
    pub fn tag(&self) -> DelegationKindTag {
        match self {
            DelegationKind::PublicKey(_) => DelegationKindTag::PublicKey,
            DelegationKind::Purse(_) => DelegationKindTag::Purse,
        }
    }

    /// Increases the stake of the provided bid
    pub fn increase_stake(&mut self, amount: U512) -> Result<U512, crate::system::auction::Error> {
        match self {
            DelegationKind::PublicKey(inner) => inner.increase_stake(amount),
            DelegationKind::Purse(inner) => inner.increase_stake(amount),
        }
    }

    /// Decreases the stake of the provided bid
    pub fn decrease_stake(
        &mut self,
        amount: U512,
        era_end_timestamp_millis: u64,
    ) -> Result<U512, crate::system::auction::Error> {
        match self {
            DelegationKind::PublicKey(inner) => {
                inner.decrease_stake(amount, era_end_timestamp_millis)
            }
            DelegationKind::Purse(inner) => inner.decrease_stake(amount, era_end_timestamp_millis),
        }
    }
}

impl ToBytes for DelegationKind {
    fn to_bytes(&self) -> Result<Vec<u8>, bytesrepr::Error> {
        let mut result = bytesrepr::allocate_buffer(self)?;
        let (tag, mut serialized_data) = match self {
            DelegationKind::PublicKey(delegator) => {
                (DelegationKindTag::PublicKey, delegator.to_bytes()?)
            }
            DelegationKind::Purse(delegator) => (DelegationKindTag::Purse, delegator.to_bytes()?),
        };
        result.push(tag as u8);
        result.append(&mut serialized_data);
        Ok(result)
    }

    fn serialized_length(&self) -> usize {
        U8_SERIALIZED_LENGTH
            + match self {
                DelegationKind::PublicKey(delegator) => delegator.serialized_length(),
                DelegationKind::Purse(delegator) => delegator.serialized_length(),
            }
    }

    fn write_bytes(&self, writer: &mut Vec<u8>) -> Result<(), bytesrepr::Error> {
        writer.push(self.tag() as u8);
        match self {
            DelegationKind::PublicKey(delegator) => delegator.write_bytes(writer)?,
            DelegationKind::Purse(delegator) => delegator.write_bytes(writer)?,
        };
        Ok(())
    }
}

impl FromBytes for DelegationKind {
    fn from_bytes(bytes: &[u8]) -> Result<(Self, &[u8]), bytesrepr::Error> {
        let (tag, remainder): (u8, &[u8]) = FromBytes::from_bytes(bytes)?;
        match tag {
            tag if tag == DelegationKindTag::PublicKey as u8 => Delegator::from_bytes(remainder)
                .map(|(delegator, remainder)| {
                    (DelegationKind::PublicKey(Box::new(delegator)), remainder)
                }),
            tag if tag == DelegationKindTag::Purse as u8 => {
                DelegatorPurseBid::from_bytes(remainder).map(|(delegator, remainder)| {
                    (DelegationKind::Purse(Box::new(delegator)), remainder)
                })
            }

            _ => Err(bytesrepr::Error::Formatting),
        }
    }
}

impl Display for DelegationKind {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        write!(
            formatter,
            "delegator {{ {} {} motes, bonding purse {}, validator {} }}",
            self.bid_addr_delegator(),
            self.staked_amount(),
            self.bonding_purse(),
            self.validator_public_key(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{bytesrepr, AccessRights, PublicKey, SecretKey, URef, U512};

    #[test]
    fn serialization_roundtrip() {
        let validator_public_key = PublicKey::from(
            &SecretKey::ed25519_from_bytes([0u8; SecretKey::ED25519_LENGTH]).unwrap(),
        );
        let bonding_purse = URef::new([42; 32], AccessRights::READ_ADD_WRITE);

        let delegator_public_key = PublicKey::from(
            &SecretKey::ed25519_from_bytes([1u8; SecretKey::ED25519_LENGTH]).unwrap(),
        );
        let delegator = Delegator::unlocked(
            delegator_public_key,
            U512::one(),
            bonding_purse,
            validator_public_key.clone(),
        );
        let delegator_bid = DelegationKind::PublicKey(Box::new(delegator));
        bytesrepr::test_serialization_roundtrip(&delegator_bid);

        let source_purse = URef::new([42; 32], AccessRights::READ_ADD_WRITE);
        let delegator = DelegatorPurseBid::unlocked(
            source_purse,
            U512::one(),
            bonding_purse,
            validator_public_key.clone(),
        );
        let delegator_bid = DelegationKind::Purse(Box::new(delegator));
        bytesrepr::test_serialization_roundtrip(&delegator_bid);
    }
}
