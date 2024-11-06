use crate::{
    bytesrepr,
    bytesrepr::{FromBytes, ToBytes},
    system::auction::{
        bid::VestingSchedule, BidAddr, BidAddrDelegator, Error, VESTING_SCHEDULE_LENGTH_MILLIS,
    },
    CLType, CLTyped, PublicKey, URef, U512,
};
use core::{
    fmt,
    fmt::{Display, Formatter},
};
use datasize::DataSize;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// A delegation bid associated with a purse.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "datasize", derive(DataSize))]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct DelegatorPurseBid {
    source_purse: URef,
    staked_amount: U512,
    bonding_purse: URef,
    validator_public_key: PublicKey,
    vesting_schedule: Option<VestingSchedule>,
}

impl DelegatorPurseBid {
    /// Creates a new [`Delegator`]
    pub fn unlocked(
        source_purse: URef,
        staked_amount: U512,
        bonding_purse: URef,
        validator_public_key: PublicKey,
    ) -> Self {
        let vesting_schedule = None;
        DelegatorPurseBid {
            source_purse,
            staked_amount,
            bonding_purse,
            validator_public_key,
            vesting_schedule,
        }
    }

    /// Creates new instance of a [`Delegator`] with locked funds.
    pub fn locked(
        source_purse: URef,
        staked_amount: U512,
        bonding_purse: URef,
        validator_public_key: PublicKey,
        release_timestamp_millis: u64,
    ) -> Self {
        let vesting_schedule = Some(VestingSchedule::new(release_timestamp_millis));
        DelegatorPurseBid {
            source_purse,
            staked_amount,
            bonding_purse,
            validator_public_key,
            vesting_schedule,
        }
    }

    /// The correct BidAddrDelegator for this instance.
    pub fn bid_addr_delegator(&self) -> BidAddrDelegator {
        BidAddrDelegator::Purse(self.delegator_source_purse().addr())
    }

    /// The correct BidAddr for this instance.
    pub fn bid_addr(&self) -> BidAddr {
        let validator = self.validator_public_key().to_account_hash();
        let delegator = self.bid_addr_delegator();
        BidAddr::Delegator {
            validator,
            delegator,
        }
    }

    /// Returns source purse of the delegator.
    pub fn delegator_source_purse(&self) -> &URef {
        &self.source_purse
    }

    /// Checks if a bid is still locked under a vesting schedule.
    ///
    /// Returns true if a timestamp falls below the initial lockup period + 91 days release
    /// schedule, otherwise false.
    pub fn is_locked(&self, timestamp_millis: u64) -> bool {
        self.is_locked_with_vesting_schedule(timestamp_millis, VESTING_SCHEDULE_LENGTH_MILLIS)
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
        match &self.vesting_schedule {
            Some(vesting_schedule) => {
                vesting_schedule.is_vesting(timestamp_millis, vesting_schedule_period_millis)
            }
            None => false,
        }
    }

    /// Returns the staked amount
    pub fn staked_amount(&self) -> U512 {
        self.staked_amount
    }

    /// Returns the mutable staked amount
    pub fn staked_amount_mut(&mut self) -> &mut U512 {
        &mut self.staked_amount
    }

    /// Returns the bonding purse
    pub fn bonding_purse(&self) -> &URef {
        &self.bonding_purse
    }

    /// Returns the public key of the validator this delegation is staked to.
    pub fn validator_public_key(&self) -> &PublicKey {
        &self.validator_public_key
    }

    /// Decreases the stake of the provided bid
    pub fn decrease_stake(
        &mut self,
        amount: U512,
        era_end_timestamp_millis: u64,
    ) -> Result<U512, Error> {
        let updated_staked_amount = self
            .staked_amount
            .checked_sub(amount)
            .ok_or(Error::InvalidAmount)?;

        let vesting_schedule = match self.vesting_schedule.as_ref() {
            Some(vesting_schedule) => vesting_schedule,
            None => {
                self.staked_amount = updated_staked_amount;
                return Ok(updated_staked_amount);
            }
        };

        match vesting_schedule.locked_amount(era_end_timestamp_millis) {
            Some(locked_amount) if updated_staked_amount < locked_amount => {
                Err(Error::DelegatorFundsLocked)
            }
            None => {
                // If `None`, then the locked amounts table has yet to be initialized (likely
                // pre-90 day mark)
                Err(Error::DelegatorFundsLocked)
            }
            Some(_) => {
                self.staked_amount = updated_staked_amount;
                Ok(updated_staked_amount)
            }
        }
    }

    /// Increases the stake of the provided bid
    pub fn increase_stake(&mut self, amount: U512) -> Result<U512, Error> {
        let updated_staked_amount = self
            .staked_amount
            .checked_add(amount)
            .ok_or(Error::InvalidAmount)?;

        self.staked_amount = updated_staked_amount;

        Ok(updated_staked_amount)
    }

    /// Returns a reference to the vesting schedule of the provided
    /// delegator bid.  `None` if a non-genesis validator.
    pub fn vesting_schedule(&self) -> Option<&VestingSchedule> {
        self.vesting_schedule.as_ref()
    }

    /// Returns a mutable reference to the vesting schedule of the provided
    /// delegator bid.  `None` if a non-genesis validator.
    pub fn vesting_schedule_mut(&mut self) -> Option<&mut VestingSchedule> {
        self.vesting_schedule.as_mut()
    }

    /// Creates a new inactive instance of a bid with 0 staked amount.
    pub fn empty(validator_public_key: PublicKey, source_purse: URef, bonding_purse: URef) -> Self {
        let vesting_schedule = None;
        let staked_amount = 0.into();
        Self {
            validator_public_key,
            source_purse,
            bonding_purse,
            staked_amount,
            vesting_schedule,
        }
    }

    /// Sets validator public key
    pub fn with_validator_public_key(&mut self, validator_public_key: PublicKey) -> &mut Self {
        self.validator_public_key = validator_public_key;
        self
    }
}

impl CLTyped for DelegatorPurseBid {
    fn cl_type() -> CLType {
        CLType::Any
    }
}

impl ToBytes for DelegatorPurseBid {
    fn to_bytes(&self) -> Result<Vec<u8>, bytesrepr::Error> {
        let mut buffer = bytesrepr::allocate_buffer(self)?;
        buffer.extend(self.source_purse.to_bytes()?);
        buffer.extend(self.staked_amount.to_bytes()?);
        buffer.extend(self.bonding_purse.to_bytes()?);
        buffer.extend(self.validator_public_key.to_bytes()?);
        buffer.extend(self.vesting_schedule.to_bytes()?);
        Ok(buffer)
    }

    fn serialized_length(&self) -> usize {
        self.source_purse.serialized_length()
            + self.staked_amount.serialized_length()
            + self.bonding_purse.serialized_length()
            + self.validator_public_key.serialized_length()
            + self.vesting_schedule.serialized_length()
    }

    fn write_bytes(&self, writer: &mut Vec<u8>) -> Result<(), bytesrepr::Error> {
        self.source_purse.write_bytes(writer)?;
        self.staked_amount.write_bytes(writer)?;
        self.bonding_purse.write_bytes(writer)?;
        self.validator_public_key.write_bytes(writer)?;
        self.vesting_schedule.write_bytes(writer)?;
        Ok(())
    }
}

impl FromBytes for DelegatorPurseBid {
    fn from_bytes(bytes: &[u8]) -> Result<(Self, &[u8]), bytesrepr::Error> {
        let (source_purse, bytes) = URef::from_bytes(bytes)?;
        let (staked_amount, bytes) = U512::from_bytes(bytes)?;
        let (bonding_purse, bytes) = URef::from_bytes(bytes)?;
        let (validator_public_key, bytes) = PublicKey::from_bytes(bytes)?;
        let (vesting_schedule, bytes) = FromBytes::from_bytes(bytes)?;
        Ok((
            DelegatorPurseBid {
                source_purse,
                staked_amount,
                bonding_purse,
                validator_public_key,
                vesting_schedule,
            },
            bytes,
        ))
    }
}

impl Display for DelegatorPurseBid {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        write!(
            formatter,
            "delegator {{ {} {} motes, bonding purse {}, validator {} }}",
            self.source_purse, self.staked_amount, self.bonding_purse, self.validator_public_key
        )
    }
}

#[cfg(test)]
mod tests {
    use super::DelegatorPurseBid;
    use crate::{bytesrepr, AccessRights, PublicKey, SecretKey, URef, U512};

    #[test]
    fn serialization_roundtrip() {
        let staked_amount = U512::one();
        let source_purse = URef::new([42; 32], AccessRights::READ_ADD_WRITE);
        let bonding_purse = URef::new([42; 32], AccessRights::READ_ADD_WRITE);
        let delegator_public_key: PublicKey = PublicKey::from(
            &SecretKey::ed25519_from_bytes([42; SecretKey::ED25519_LENGTH]).unwrap(),
        );

        let validator_public_key: PublicKey = PublicKey::from(
            &SecretKey::ed25519_from_bytes([43; SecretKey::ED25519_LENGTH]).unwrap(),
        );
        let unlocked_delegator = DelegatorPurseBid::unlocked(
            source_purse,
            staked_amount,
            bonding_purse,
            validator_public_key.clone(),
        );
        bytesrepr::test_serialization_roundtrip(&unlocked_delegator);

        let release_timestamp_millis = 42;
        let locked_delegator = DelegatorPurseBid::locked(
            source_purse,
            staked_amount,
            bonding_purse,
            validator_public_key,
            release_timestamp_millis,
        );
        bytesrepr::test_serialization_roundtrip(&locked_delegator);
    }
}

#[cfg(test)]
mod prop_tests {
    use proptest::prelude::*;

    use crate::{bytesrepr, gens};

    proptest! {
        #[test]
        fn test_value_bid(bid in gens::delegator_arb()) {
            bytesrepr::test_serialization_roundtrip(&bid);
        }
    }
}
