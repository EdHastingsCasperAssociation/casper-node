use alloc::vec::Vec;
use core::fmt::{self, Debug, Display, Formatter};

#[cfg(feature = "datasize")]
use datasize::DataSize;
#[cfg(feature = "json-schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::bytesrepr::U8_SERIALIZED_LENGTH;
use crate::{
    account::AccountHash,
    bytesrepr::{self, FromBytes, ToBytes},
    CLType, CLTyped, PublicKey, URefAddr,
};

use super::{BidAddr, BidAddrDelegator, DelegationRate};

const PUBLIC_KEY_TAG: u8 = 0;
const ACCOUNT_HASH_TAG: u8 = 1;
const PURSE_TAG: u8 = 2;

/// UnbonderIdentityTag variants.
#[repr(u8)]
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, Clone)]
pub enum ReservationIdentityTag {
    /// PublicKey unbond identity.
    PublicKey = PUBLIC_KEY_TAG,
    /// AccountHash unbond identity.
    AccountHash = ACCOUNT_HASH_TAG,
    /// Purse unbond identity.
    Purse = PURSE_TAG,
}

impl Display for ReservationIdentityTag {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        let tag = match self {
            ReservationIdentityTag::PublicKey => PUBLIC_KEY_TAG,
            ReservationIdentityTag::AccountHash => ACCOUNT_HASH_TAG,
            ReservationIdentityTag::Purse => PURSE_TAG,
        };
        write!(f, "{}", base16::encode_lower(&[tag]))
    }
}

/// UnbonderIdentity variants.
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "datasize", derive(DataSize))]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[serde(deny_unknown_fields)]
pub enum ReservationIdentity {
    /// PublicKey unbond identity.
    PublicKey(PublicKey),
    /// AccountHash unbond identity.
    AccountHash(AccountHash),
    /// Purse unbond identity.
    Purse(URefAddr),
}

impl ReservationIdentity {
    pub fn tag(&self) -> ReservationIdentityTag {
        match self {
            ReservationIdentity::PublicKey(_) => ReservationIdentityTag::PublicKey,
            ReservationIdentity::AccountHash(_) => ReservationIdentityTag::AccountHash,
            ReservationIdentity::Purse(_) => ReservationIdentityTag::Purse,
        }
    }
}

impl ToBytes for ReservationIdentity {
    fn to_bytes(&self) -> Result<Vec<u8>, bytesrepr::Error> {
        let mut result = bytesrepr::allocate_buffer(self)?;
        let (tag, mut serialized_data) = match self {
            ReservationIdentity::PublicKey(public_key) => {
                (ReservationIdentityTag::PublicKey, public_key.to_bytes()?)
            }
            ReservationIdentity::AccountHash(account_hash) => (
                ReservationIdentityTag::AccountHash,
                account_hash.to_bytes()?,
            ),
            ReservationIdentity::Purse(uref_addr) => {
                (ReservationIdentityTag::Purse, uref_addr.to_bytes()?)
            }
        };
        result.push(tag as u8);
        result.append(&mut serialized_data);
        Ok(result)
    }

    fn serialized_length(&self) -> usize {
        U8_SERIALIZED_LENGTH
            + match self {
                ReservationIdentity::PublicKey(public_key) => public_key.serialized_length(),
                ReservationIdentity::AccountHash(account_hash) => account_hash.serialized_length(),
                ReservationIdentity::Purse(uref_addr) => uref_addr.serialized_length(),
            }
    }

    fn write_bytes(&self, writer: &mut Vec<u8>) -> Result<(), bytesrepr::Error> {
        writer.push(self.tag() as u8);
        match self {
            ReservationIdentity::PublicKey(public_key) => public_key.write_bytes(writer)?,
            ReservationIdentity::AccountHash(account_hash) => account_hash.write_bytes(writer)?,
            ReservationIdentity::Purse(uref_addr) => uref_addr.write_bytes(writer)?,
        };
        Ok(())
    }
}

impl FromBytes for ReservationIdentity {
    fn from_bytes(bytes: &[u8]) -> Result<(Self, &[u8]), bytesrepr::Error> {
        let (tag, remainder): (u8, &[u8]) = FromBytes::from_bytes(bytes)?;
        match tag {
            tag if tag == ReservationIdentityTag::PublicKey as u8 => {
                PublicKey::from_bytes(remainder).map(|(public_key, remainder)| {
                    (ReservationIdentity::PublicKey(public_key), remainder)
                })
            }
            tag if tag == ReservationIdentityTag::AccountHash as u8 => {
                AccountHash::from_bytes(remainder).map(|(account_hash, remainder)| {
                    (ReservationIdentity::AccountHash(account_hash), remainder)
                })
            }
            tag if tag == ReservationIdentityTag::Purse as u8 => URefAddr::from_bytes(remainder)
                .map(|(uref_addr, remainder)| (ReservationIdentity::Purse(uref_addr), remainder)),

            _ => Err(bytesrepr::Error::Formatting),
        }
    }
}

impl Display for ReservationIdentity {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        let tag = self.tag();
        match self {
            ReservationIdentity::PublicKey(public_key) => {
                write!(f, "{}{}", tag, public_key)
            }
            ReservationIdentity::AccountHash(account_hash) => {
                write!(f, "{}{}", tag, account_hash)
            }

            ReservationIdentity::Purse(uref_addr) => {
                write!(f, "{}{}", tag, base16::encode_lower(&uref_addr),)
            }
        }
    }
}

/// Represents a validator reserving a slot for specific delegator
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "datasize", derive(DataSize))]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct Reservation {
    /// Reservation identity
    reservation_identity: ReservationIdentity,
    /// Validator public key
    validator_public_key: PublicKey,
    /// Individual delegation rate
    delegation_rate: DelegationRate,
}

impl Reservation {
    /// Creates a new [`Reservation`]
    pub fn new_delegator_public_key(
        validator_public_key: PublicKey,
        delegator_public_key: PublicKey,
        delegation_rate: DelegationRate,
    ) -> Self {
        let reservation_identity = ReservationIdentity::PublicKey(delegator_public_key);
        Self {
            reservation_identity,
            validator_public_key,
            delegation_rate,
        }
    }

    /// Creates a new [`Reservation`]
    pub fn new_delegator_purse_addr(
        validator_public_key: PublicKey,
        delegator_purse_uref_addr: URefAddr,
        delegation_rate: DelegationRate,
    ) -> Self {
        let reservation_identity = ReservationIdentity::Purse(delegator_purse_uref_addr);
        Self {
            reservation_identity,
            validator_public_key,
            delegation_rate,
        }
    }

    /// The correct BidAddr for this instance.
    pub fn bid_addr(&self) -> BidAddr {
        let validator = self.validator_public_key().to_account_hash();
        let delegator = match &self.reservation_identity {
            ReservationIdentity::PublicKey(delegator_public_key) => {
                BidAddrDelegator::Account(delegator_public_key.to_account_hash())
            }
            ReservationIdentity::AccountHash(account_hash) => {
                BidAddrDelegator::Account(*account_hash)
            }
            ReservationIdentity::Purse(uref_addr) => BidAddrDelegator::Purse(*uref_addr),
        };
        BidAddr::Reservation {
            validator,
            delegator,
        }
    }

    /// Returns public key of the delegator.
    pub fn delegator_public_key(&self) -> Option<PublicKey> {
        match &self.reservation_identity {
            ReservationIdentity::PublicKey(public_key) => Some(public_key.clone()),
            ReservationIdentity::AccountHash(_) | ReservationIdentity::Purse(_) => None,
        }
    }

    /// Returns public key of the delegator.
    pub fn reservation_identity(&self) -> &ReservationIdentity {
        &self.reservation_identity
    }

    /// Returns delegatee
    pub fn validator_public_key(&self) -> &PublicKey {
        &self.validator_public_key
    }

    /// Gets the delegation rate of the provided bid
    pub fn delegation_rate(&self) -> &DelegationRate {
        &self.delegation_rate
    }
}

impl CLTyped for Reservation {
    fn cl_type() -> CLType {
        CLType::Any
    }
}

impl ToBytes for Reservation {
    fn to_bytes(&self) -> Result<Vec<u8>, bytesrepr::Error> {
        let mut buffer = bytesrepr::allocate_buffer(self)?;
        buffer.extend(self.reservation_identity.to_bytes()?);
        buffer.extend(self.validator_public_key.to_bytes()?);
        buffer.extend(self.delegation_rate.to_bytes()?);
        Ok(buffer)
    }

    fn serialized_length(&self) -> usize {
        self.reservation_identity.serialized_length()
            + self.validator_public_key.serialized_length()
            + self.delegation_rate.serialized_length()
    }

    fn write_bytes(&self, writer: &mut Vec<u8>) -> Result<(), bytesrepr::Error> {
        self.reservation_identity.write_bytes(writer)?;
        self.validator_public_key.write_bytes(writer)?;
        self.delegation_rate.write_bytes(writer)?;
        Ok(())
    }
}

impl FromBytes for Reservation {
    fn from_bytes(bytes: &[u8]) -> Result<(Self, &[u8]), bytesrepr::Error> {
        let (reservation_identity, bytes) = ReservationIdentity::from_bytes(bytes)?;
        let (validator_public_key, bytes) = PublicKey::from_bytes(bytes)?;
        let (delegation_rate, bytes) = FromBytes::from_bytes(bytes)?;
        Ok((
            Self {
                reservation_identity,
                validator_public_key,
                delegation_rate,
            },
            bytes,
        ))
    }
}

impl Display for Reservation {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        write!(
            formatter,
            "Reservation {{ delegator {}, validator {} }}",
            self.reservation_identity, self.validator_public_key
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::{bytesrepr, system::auction::Reservation, PublicKey, SecretKey};

    #[test]
    fn serialization_roundtrip() {
        let delegator_public_key: PublicKey = PublicKey::from(
            &SecretKey::ed25519_from_bytes([42; SecretKey::ED25519_LENGTH]).unwrap(),
        );

        let validator_public_key: PublicKey = PublicKey::from(
            &SecretKey::ed25519_from_bytes([43; SecretKey::ED25519_LENGTH]).unwrap(),
        );
        let entry =
            Reservation::new_delegator_public_key(delegator_public_key, validator_public_key, 0);
        bytesrepr::test_serialization_roundtrip(&entry);
    }
}

#[cfg(test)]
mod prop_tests {
    use proptest::prelude::*;

    use crate::{bytesrepr, gens};

    proptest! {
        #[test]
        fn test_value_bid(bid in gens::reservation_arb()) {
            bytesrepr::test_serialization_roundtrip(&bid);
        }
    }
}
