use crate::{
    account::{AccountHash, ACCOUNT_HASH_LENGTH},
    bytesrepr,
    bytesrepr::{FromBytes, ToBytes},
    system::auction::error::Error,
    EraId, Key, KeyTag, PublicKey, URefAddr, UREF_ADDR_LENGTH,
};
use alloc::vec::Vec;
use core::fmt::{Debug, Display, Formatter};
#[cfg(feature = "datasize")]
use datasize::DataSize;
#[cfg(any(feature = "testing", test))]
use rand::{
    distributions::{Distribution, Standard},
    Rng,
};
#[cfg(feature = "json-schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

const UNIFIED_TAG: u8 = 0;
const VALIDATOR_TAG: u8 = 1;
const DELEGATOR_TAG: u8 = 2;

const DELEGATOR_ACCOUNT_HASH_TAG: u8 = 0;
const DELEGATOR_PURSE_TAG: u8 = 1;

const CREDIT_TAG: u8 = 4;
const RESERVATION_TAG: u8 = 5;
const UNBOND_TAG: u8 = 6;

/// Serialization tag for BidAddr variants.
#[derive(
    Debug, Default, PartialOrd, Ord, PartialEq, Eq, Hash, Clone, Copy, Serialize, Deserialize,
)]
#[repr(u8)]
#[cfg_attr(feature = "datasize", derive(DataSize))]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
pub enum BidAddrTag {
    /// BidAddr for legacy unified bid.
    Unified = UNIFIED_TAG,

    /// BidAddr for validator bid.
    #[default]
    Validator = VALIDATOR_TAG,

    /// BidAddr for delegator bid.
    Delegator = DELEGATOR_TAG,

    /// BidAddr for auction credit.
    Credit = CREDIT_TAG,

    /// BidAddr for reservation bid.
    Reservation = RESERVATION_TAG,

    /// BidAddr for bid unbond records.
    Unbond = UNBOND_TAG,
}

impl BidAddrTag {
    /// The length in bytes of a [`BidAddrTag`].
    pub const BID_ADDR_TAG_LENGTH: usize = 1;

    /// Attempts to map `BidAddrTag` from a u8.
    pub fn try_from_u8(value: u8) -> Option<Self> {
        // TryFrom requires std, so doing this instead.
        if value == UNIFIED_TAG {
            return Some(BidAddrTag::Unified);
        }
        if value == VALIDATOR_TAG {
            return Some(BidAddrTag::Validator);
        }
        if value == DELEGATOR_TAG {
            return Some(BidAddrTag::Delegator);
        }
        if value == CREDIT_TAG {
            return Some(BidAddrTag::Credit);
        }
        if value == RESERVATION_TAG {
            return Some(BidAddrTag::Reservation);
        }
        if value == UNBOND_TAG {
            return Some(BidAddrTag::Unbond);
        }

        None
    }
}

impl Display for BidAddrTag {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        let tag = match self {
            BidAddrTag::Unified => UNIFIED_TAG,
            BidAddrTag::Validator => VALIDATOR_TAG,
            BidAddrTag::Delegator => DELEGATOR_TAG,
            BidAddrTag::Credit => CREDIT_TAG,
            BidAddrTag::Reservation => RESERVATION_TAG,
            BidAddrTag::Unbond => UNIFIED_TAG,
        };
        write!(f, "{}", base16::encode_lower(&[tag]))
    }
}

/// Serialization tag for BidAddr variants.
#[derive(
    Debug, Default, PartialOrd, Ord, PartialEq, Eq, Hash, Clone, Copy, Serialize, Deserialize,
)]
#[repr(u8)]
#[cfg_attr(feature = "datasize", derive(DataSize))]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
pub enum BidAddrDelegatorTag {
    /// BidAddr for account based delegation.
    #[default]
    AccountHash = DELEGATOR_ACCOUNT_HASH_TAG,

    /// BidAddr purse based delegation.    
    Purse = DELEGATOR_PURSE_TAG,
}

impl Display for BidAddrDelegatorTag {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        let tag = match self {
            BidAddrDelegatorTag::AccountHash => DELEGATOR_ACCOUNT_HASH_TAG,
            BidAddrDelegatorTag::Purse => DELEGATOR_PURSE_TAG,
        };
        write!(f, "{}", base16::encode_lower(&[tag]))
    }
}

/// Delegated bid address.
#[derive(PartialOrd, Ord, PartialEq, Eq, Hash, Clone, Copy, Serialize, Deserialize)]
#[cfg_attr(feature = "datasize", derive(DataSize))]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
pub enum BidAddrDelegator {
    Account(AccountHash),
    Purse(URefAddr),
}

impl BidAddrDelegator {
    /// The tag.
    fn tag(&self) -> BidAddrDelegatorTag {
        match self {
            BidAddrDelegator::Account(_) => BidAddrDelegatorTag::AccountHash,
            BidAddrDelegator::Purse(_) => BidAddrDelegatorTag::Purse,
        }
    }

    /// The delegator's account hash, if any.
    fn delegator_account_hash(&self) -> Option<AccountHash> {
        match self {
            BidAddrDelegator::Account(account_hash) => Some(*account_hash),
            BidAddrDelegator::Purse(_) => None,
        }
    }
    /// The delegator's account hash, if any.
    fn delegator_purse(&self) -> Option<URefAddr> {
        match self {
            BidAddrDelegator::Account(_) => None,
            BidAddrDelegator::Purse(uref_addr) => Some(*uref_addr),
        }
    }

    /// How long will be the serialized value for this instance.
    pub fn serialized_length(&self) -> usize {
        let len = match self {
            BidAddrDelegator::Account(account_hash) => ToBytes::serialized_length(account_hash),
            BidAddrDelegator::Purse(uref_addr) => ToBytes::serialized_length(uref_addr),
        };
        len + 1 // plus one for tag len
    }
}

impl ToBytes for BidAddrDelegator {
    fn to_bytes(&self) -> Result<Vec<u8>, bytesrepr::Error> {
        let mut buffer = bytesrepr::allocate_buffer(self)?;
        buffer.push(self.tag() as u8);
        match self {
            BidAddrDelegator::Account(account_hash) => {
                buffer.append(&mut account_hash.to_bytes()?);
            }
            BidAddrDelegator::Purse(uref_addr) => {
                buffer.append(&mut uref_addr.to_bytes()?);
            }
        }
        Ok(buffer)
    }

    fn serialized_length(&self) -> usize {
        self.serialized_length()
    }
}

impl FromBytes for BidAddrDelegator {
    fn from_bytes(bytes: &[u8]) -> Result<(Self, &[u8]), bytesrepr::Error> {
        let (tag, remainder): (u8, &[u8]) = FromBytes::from_bytes(bytes)?;
        match tag {
            delegator_bid_addr_tag
                if delegator_bid_addr_tag == BidAddrDelegatorTag::AccountHash as u8 =>
            {
                let (delegator_account_hash, remainder) = AccountHash::from_bytes(remainder)?;

                Ok((BidAddrDelegator::Account(delegator_account_hash), remainder))
            }
            delegator_bid_addr_tag
                if delegator_bid_addr_tag == BidAddrDelegatorTag::Purse as u8 =>
            {
                let (delegator_source_purse, remainder) = URefAddr::from_bytes(remainder)?;
                Ok((BidAddrDelegator::Purse(delegator_source_purse), remainder))
            }
            _ => Err(bytesrepr::Error::Formatting),
        }
    }
}

impl Display for BidAddrDelegator {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        let tag = self.tag();
        match self {
            BidAddrDelegator::Account(account_hash) => {
                write!(f, "{}{}", tag, account_hash)
            }
            BidAddrDelegator::Purse(source_purse) => {
                write!(f, "{}{}", tag, base16::encode_lower(source_purse),)
            }
        }
    }
}

impl Debug for BidAddrDelegator {
    fn fmt(&self, f: &mut Formatter) -> core::fmt::Result {
        match self {
            BidAddrDelegator::Account(account_hash) => {
                write!(f, "BidAddrDelegator::Account({:?})", account_hash)
            }
            BidAddrDelegator::Purse(source_purse) => {
                write!(f, "BidAddrDelegator::Purse({:?})", source_purse)
            }
        }
    }
}

/// Bid address
#[derive(PartialOrd, Ord, PartialEq, Eq, Hash, Clone, Copy, Serialize, Deserialize)]
#[cfg_attr(feature = "datasize", derive(DataSize))]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
pub enum BidAddr {
    /// Unified BidAddr.
    Unified(AccountHash),
    /// Validator BidAddr.
    Validator(AccountHash),
    /// Delegator BidAddr.
    Delegator {
        /// The validator addr.
        validator: AccountHash,
        /// The delegator addr.
        delegator: BidAddrDelegator,
    },
    /// Validator credit BidAddr.
    Credit {
        /// The validator addr.
        validator: AccountHash,
        /// The era id.
        era_id: EraId,
    },
    /// Reservation BidAddr
    Reservation {
        /// The validator addr.
        validator: AccountHash,
        /// The delegator addr.
        delegator: BidAddrDelegator,
    },
    /// Unbond
    Unbond {
        /// The validator addr.
        validator: AccountHash,
        /// The delegator addr, if this is a delegator unbond.
        maybe_delegator: Option<BidAddrDelegator>,
    },
}

impl BidAddr {
    /// The length in bytes of a [`BidAddr`] for a validator bid.
    pub const VALIDATOR_BID_ADDR_LENGTH: usize =
        ACCOUNT_HASH_LENGTH + BidAddrTag::BID_ADDR_TAG_LENGTH;

    /// The length in bytes of a [`BidAddr`] for a delegator bid.
    pub const DELEGATOR_BID_ADDR_LENGTH: usize =
        (ACCOUNT_HASH_LENGTH * 2) + BidAddrTag::BID_ADDR_TAG_LENGTH;

    /// Constructs a new [`BidAddr`] instance from a validator's [`AccountHash`].
    pub const fn new_validator_addr(validator: [u8; ACCOUNT_HASH_LENGTH]) -> Self {
        BidAddr::Validator(AccountHash::new(validator))
    }

    /// Constructs a new [`BidAddr::Delegator`] instance from the [`AccountHash`] pair of a
    /// validator and a delegator.
    pub const fn new_delegator_account_addr(
        pair: ([u8; ACCOUNT_HASH_LENGTH], [u8; ACCOUNT_HASH_LENGTH]),
    ) -> Self {
        BidAddr::Delegator {
            validator: AccountHash::new(pair.0),
            delegator: BidAddrDelegator::Account(AccountHash::new(pair.1)),
        }
    }

    /// Constructs a new [`BidAddr::Delegator`] instance from the [`AccountHash`] pair of a
    /// validator and a delegator.
    pub const fn new_delegator_purse_addr(
        pair: ([u8; ACCOUNT_HASH_LENGTH], [u8; UREF_ADDR_LENGTH]),
    ) -> Self {
        BidAddr::Delegator {
            validator: AccountHash::new(pair.0),
            delegator: BidAddrDelegator::Purse(pair.1),
        }
    }

    /// Constructs a new [`BidAddr::Delegator`] instance from a validator [`PublicKey`] and a
    /// [`AccountHash`] for the delegating account.
    pub fn new_delegator_account(
        validator: &PublicKey,
        delegator_account_hash: AccountHash,
    ) -> Self {
        BidAddr::Delegator {
            validator: AccountHash::from(validator),
            delegator: BidAddrDelegator::Account(delegator_account_hash),
        }
    }

    /// Constructs a new [`BidAddr::Delegator`] instance from a validator [`PublicKey`] and a
    /// [`URefAddr`] for a purse.
    pub fn new_delegator_purse(validator: &PublicKey, purse_addr: URefAddr) -> Self {
        BidAddr::Delegator {
            validator: AccountHash::from(validator),
            delegator: BidAddrDelegator::Purse(purse_addr),
        }
    }

    /// Constructs a new [`BidAddr::Reservation`] instance from the [`AccountHash`] pair of a
    /// validator and a delegator.
    pub const fn new_reservation_account_addr(
        pair: ([u8; ACCOUNT_HASH_LENGTH], [u8; ACCOUNT_HASH_LENGTH]),
    ) -> Self {
        let delegator = BidAddrDelegator::Account(AccountHash::new(pair.1));
        BidAddr::Reservation {
            validator: AccountHash::new(pair.0),
            delegator,
        }
    }

    /// Constructs a new [`BidAddr::Unbond`] instance.
    pub fn unbond(validator: &PublicKey, maybe_delegator: Option<BidAddrDelegator>) -> Self {
        BidAddr::Unbond {
            validator: AccountHash::from(validator),
            maybe_delegator,
        }
    }

    /// Constructs a new [`BidAddr::Unbond`] instance.
    pub fn unbond_delegator_public_key(validator: &PublicKey, delegator: &PublicKey) -> Self {
        BidAddr::Unbond {
            validator: AccountHash::from(validator),
            maybe_delegator: Some(BidAddrDelegator::Account(AccountHash::from(delegator))),
        }
    }

    #[allow(missing_docs)]
    pub const fn legacy(validator: [u8; ACCOUNT_HASH_LENGTH]) -> Self {
        BidAddr::Unified(AccountHash::new(validator))
    }

    /// Create a new instance of a [`BidAddr`].
    pub fn new_from_public_keys(
        validator: &PublicKey,
        maybe_delegator: Option<&PublicKey>,
    ) -> Self {
        if let Some(delegator) = maybe_delegator {
            BidAddr::Delegator {
                validator: AccountHash::from(validator),
                delegator: BidAddrDelegator::Account(AccountHash::from(delegator)),
            }
        } else {
            BidAddr::Validator(AccountHash::from(validator))
        }
    }

    /// Create a new instance of a [`BidAddr`].
    pub fn new_credit(validator: &PublicKey, era_id: EraId) -> Self {
        BidAddr::Credit {
            validator: AccountHash::from(validator),
            era_id,
        }
    }

    /// Create a new instance of a [`BidAddr`].
    pub fn new_reservation_public_key(validator: &PublicKey, delegator: &PublicKey) -> Self {
        BidAddr::Reservation {
            validator: validator.into(),
            delegator: BidAddrDelegator::Account(delegator.into()),
        }
    }

    /// Returns the common prefix of all delegators to the cited validator.
    pub fn delegators_prefix(&self) -> Result<Vec<u8>, Error> {
        let validator = self.validator_account_hash();
        let mut ret = Vec::with_capacity(validator.serialized_length() + 2);
        ret.push(KeyTag::BidAddr as u8);
        ret.push(BidAddrTag::Delegator as u8);
        validator.write_bytes(&mut ret)?;
        Ok(ret)
    }

    /// Returns the common prefix of all reservations to the cited validator.
    pub fn reservation_prefix(&self) -> Result<Vec<u8>, Error> {
        let validator = self.validator_account_hash();
        let mut ret = Vec::with_capacity(validator.serialized_length() + 2);
        ret.push(KeyTag::BidAddr as u8);
        ret.push(BidAddrTag::Reservation as u8);
        validator.write_bytes(&mut ret)?;
        Ok(ret)
    }

    /// Returns the common prefix of all unbonds from the cited validator.
    pub fn unbond_prefix(&self) -> Result<Vec<u8>, Error> {
        let validator = self.validator_account_hash();
        let mut ret = Vec::with_capacity(validator.serialized_length() + 2);
        ret.push(KeyTag::BidAddr as u8);
        ret.push(BidAddrTag::Unbond as u8);
        validator.write_bytes(&mut ret)?;
        Ok(ret)
    }

    /// Validator account hash.
    pub fn validator_account_hash(&self) -> AccountHash {
        match self {
            BidAddr::Unified(account_hash) | BidAddr::Validator(account_hash) => *account_hash,
            BidAddr::Delegator { validator, .. }
            | BidAddr::Credit { validator, .. }
            | BidAddr::Reservation { validator, .. }
            | BidAddr::Unbond { validator, .. } => *validator,
        }
    }

    /// Delegator account hash or none.
    pub fn maybe_delegator_account_hash(&self) -> Option<AccountHash> {
        match self {
            BidAddr::Unified(_) | BidAddr::Validator(_) | BidAddr::Credit { .. } => None,
            BidAddr::Delegator { delegator, .. } | BidAddr::Reservation { delegator, .. } => {
                delegator.delegator_account_hash()
            }
            BidAddr::Unbond {
                maybe_delegator, ..
            } => match maybe_delegator {
                Some(delegator) => delegator.delegator_account_hash(),
                None => None,
            },
        }
    }

    /// Delegator purse or none.
    pub fn maybe_delegator_purse(&self) -> Option<URefAddr> {
        match self {
            BidAddr::Unified(_) | BidAddr::Validator(_) | BidAddr::Credit { .. } => None,
            BidAddr::Delegator { delegator, .. } | BidAddr::Reservation { delegator, .. } => {
                delegator.delegator_purse()
            }
            BidAddr::Unbond {
                maybe_delegator, ..
            } => match maybe_delegator {
                None => None,
                Some(delegator) => delegator.delegator_purse(),
            },
        }
    }

    /// Era id or none.
    pub fn maybe_era_id(&self) -> Option<EraId> {
        match self {
            BidAddr::Unified(_)
            | BidAddr::Validator(_)
            | BidAddr::Delegator { .. }
            | BidAddr::Reservation { .. }
            | BidAddr::Unbond { .. } => None,
            BidAddr::Credit { era_id, .. } => Some(*era_id),
        }
    }

    /// If true, this instance is the key for a delegator bid record.
    pub fn is_delegator_bid_addr(&self) -> bool {
        match self {
            BidAddr::Unified(_)
            | BidAddr::Validator(_)
            | BidAddr::Credit { .. }
            | BidAddr::Reservation { .. }
            | BidAddr::Unbond { .. } => false,
            BidAddr::Delegator { .. } => true,
        }
    }

    /// If true, this instance is the key for a unbid record.
    pub fn is_unbond_bid_addr(&self) -> bool {
        match self {
            BidAddr::Unified(_)
            | BidAddr::Validator(_)
            | BidAddr::Credit { .. }
            | BidAddr::Reservation { .. }
            | BidAddr::Delegator { .. } => false,
            BidAddr::Unbond { .. } => true,
        }
    }

    /// How long will be the serialized value for this instance.
    pub fn serialized_length(&self) -> usize {
        match self {
            BidAddr::Unified(account_hash) | BidAddr::Validator(account_hash) => {
                ToBytes::serialized_length(account_hash) + 1
            }
            BidAddr::Delegator {
                validator,
                delegator,
            } => ToBytes::serialized_length(validator) + ToBytes::serialized_length(delegator) + 1,
            BidAddr::Credit { validator, era_id } => {
                ToBytes::serialized_length(validator) + ToBytes::serialized_length(era_id) + 1
            }
            BidAddr::Reservation {
                validator,
                delegator,
            } => ToBytes::serialized_length(validator) + ToBytes::serialized_length(delegator) + 1,
            BidAddr::Unbond {
                validator,
                maybe_delegator: delegator,
            } => ToBytes::serialized_length(validator) + ToBytes::serialized_length(delegator) + 1,
        }
    }

    /// Returns the BiddAddrTag of this instance.
    pub fn tag(&self) -> BidAddrTag {
        match self {
            BidAddr::Unified(_) => BidAddrTag::Unified,
            BidAddr::Validator(_) => BidAddrTag::Validator,
            BidAddr::Delegator { .. } => BidAddrTag::Delegator,
            BidAddr::Credit { .. } => BidAddrTag::Credit,
            BidAddr::Reservation { .. } => BidAddrTag::Reservation,
            BidAddr::Unbond { .. } => BidAddrTag::Unbond,
        }
    }
}

impl ToBytes for BidAddr {
    fn to_bytes(&self) -> Result<Vec<u8>, bytesrepr::Error> {
        let mut buffer = bytesrepr::allocate_buffer(self)?;
        buffer.push(self.tag() as u8);
        buffer.append(&mut self.validator_account_hash().to_bytes()?);
        if let Some(delegator_account_hash) = self.maybe_delegator_account_hash() {
            buffer.append(&mut delegator_account_hash.to_bytes()?);
        }
        if let Some(delegator_source_purse) = self.maybe_delegator_purse() {
            buffer.append(&mut delegator_source_purse.to_bytes()?);
        }
        if let Some(era_id) = self.maybe_era_id() {
            buffer.append(&mut era_id.to_bytes()?);
        }
        Ok(buffer)
    }

    fn serialized_length(&self) -> usize {
        self.serialized_length()
    }
}

impl FromBytes for BidAddr {
    fn from_bytes(bytes: &[u8]) -> Result<(Self, &[u8]), bytesrepr::Error> {
        let (tag, remainder): (u8, &[u8]) = FromBytes::from_bytes(bytes)?;
        match tag {
            tag if tag == BidAddrTag::Unified as u8 => AccountHash::from_bytes(remainder)
                .map(|(account_hash, remainder)| (BidAddr::Unified(account_hash), remainder)),
            tag if tag == BidAddrTag::Validator as u8 => AccountHash::from_bytes(remainder)
                .map(|(account_hash, remainder)| (BidAddr::Validator(account_hash), remainder)),
            tag if tag == BidAddrTag::Delegator as u8 => {
                let (validator, remainder) = AccountHash::from_bytes(remainder)?;
                let (delegator, remainder) = BidAddrDelegator::from_bytes(remainder)?;
                Ok((
                    BidAddr::Delegator {
                        validator,
                        delegator,
                    },
                    remainder,
                ))
            }
            tag if tag == BidAddrTag::Credit as u8 => {
                let (validator, remainder) = AccountHash::from_bytes(remainder)?;
                let (era_id, remainder) = EraId::from_bytes(remainder)?;
                Ok((BidAddr::Credit { validator, era_id }, remainder))
            }
            tag if tag == BidAddrTag::Reservation as u8 => {
                let (validator, remainder) = AccountHash::from_bytes(remainder)?;
                let (delegator, remainder) = BidAddrDelegator::from_bytes(remainder)?;
                Ok((
                    BidAddr::Reservation {
                        validator,
                        delegator,
                    },
                    remainder,
                ))
            }
            _ => Err(bytesrepr::Error::Formatting),
        }
    }
}

impl Default for BidAddr {
    fn default() -> Self {
        BidAddr::Validator(AccountHash::default())
    }
}

impl From<BidAddr> for Key {
    fn from(bid_addr: BidAddr) -> Self {
        Key::BidAddr(bid_addr)
    }
}

impl From<AccountHash> for BidAddr {
    fn from(account_hash: AccountHash) -> Self {
        BidAddr::Validator(account_hash)
    }
}

impl From<PublicKey> for BidAddr {
    fn from(public_key: PublicKey) -> Self {
        BidAddr::Validator(public_key.to_account_hash())
    }
}

impl Display for BidAddr {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        let tag = self.tag();
        match self {
            BidAddr::Unified(account_hash) | BidAddr::Validator(account_hash) => {
                write!(f, "{}{}", tag, account_hash)
            }
            BidAddr::Delegator {
                validator,
                delegator,
            } => write!(f, "{}{}{}", tag, validator, delegator),

            BidAddr::Credit { validator, era_id } => write!(
                f,
                "{}{}{}",
                tag,
                validator,
                base16::encode_lower(&era_id.to_le_bytes())
            ),
            BidAddr::Reservation {
                validator,
                delegator,
            } => write!(f, "{}{}{}", tag, validator, delegator),
            BidAddr::Unbond {
                validator,
                maybe_delegator,
            } => match maybe_delegator {
                Some(delegator) => write!(f, "{}{}{}", tag, validator, delegator),
                None => write!(f, "{}{}", tag, validator),
            },
        }
    }
}

impl Debug for BidAddr {
    fn fmt(&self, f: &mut Formatter) -> core::fmt::Result {
        match self {
            BidAddr::Unified(validator) => write!(f, "BidAddr::Unified({:?})", validator),
            BidAddr::Validator(validator) => write!(f, "BidAddr::Validator({:?})", validator),
            BidAddr::Delegator {
                validator,
                delegator,
            } => {
                write!(f, "BidAddr::Delegator({:?}{:?})", validator, delegator)
            }
            BidAddr::Credit { validator, era_id } => {
                write!(f, "BidAddr::Credit({:?}{:?})", validator, era_id)
            }
            BidAddr::Reservation {
                validator,
                delegator,
            } => {
                write!(f, "BidAddr::Reservation({:?}{:?})", validator, delegator)
            }
            BidAddr::Unbond {
                validator,
                maybe_delegator,
            } => match maybe_delegator {
                Some(delegator) => {
                    write!(f, "BidAddr::Unbond({:?}{:?})", validator, delegator)
                }
                None => write!(f, "BidAddr::Unbond({:?})", validator),
            },
        }
    }
}

#[cfg(any(feature = "testing", test))]
impl Distribution<BidAddr> for Standard {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> BidAddr {
        BidAddr::Validator(AccountHash::new(rng.gen()))
    }
}

#[cfg(test)]
mod tests {
    use crate::{bytesrepr, system::auction::BidAddr, EraId, PublicKey, SecretKey};

    #[test]
    fn serialization_roundtrip() {
        let bid_addr = BidAddr::legacy([1; 32]);
        bytesrepr::test_serialization_roundtrip(&bid_addr);
        let bid_addr = BidAddr::new_validator_addr([1; 32]);
        bytesrepr::test_serialization_roundtrip(&bid_addr);
        let bid_addr = BidAddr::new_delegator_account_addr(([1; 32], [2; 32]));
        bytesrepr::test_serialization_roundtrip(&bid_addr);
        let bid_addr = BidAddr::new_delegator_purse_addr(([1; 32], [3; 32]));
        bytesrepr::test_serialization_roundtrip(&bid_addr);
        let bid_addr = BidAddr::new_credit(
            &PublicKey::from(
                &SecretKey::ed25519_from_bytes([0u8; SecretKey::ED25519_LENGTH]).unwrap(),
            ),
            EraId::new(0),
        );
        bytesrepr::test_serialization_roundtrip(&bid_addr);
        let bid_addr = BidAddr::new_reservation_account_addr(([1; 32], [2; 32]));
        bytesrepr::test_serialization_roundtrip(&bid_addr);
    }
}

#[cfg(test)]
mod prop_test_validator_addr {
    use proptest::prelude::*;

    use crate::{bytesrepr, gens};

    proptest! {
        #[test]
        fn test_value_bid_addr_validator(validator_bid_addr in gens::bid_addr_validator_arb()) {
            bytesrepr::test_serialization_roundtrip(&validator_bid_addr);
        }
    }
}

#[cfg(test)]
mod prop_test_delegator_addr {
    use proptest::prelude::*;

    use crate::{bytesrepr, gens};

    proptest! {
        #[test]
        fn test_value_bid_addr_delegator(delegator_bid_addr in gens::bid_addr_delegator_arb()) {
            bytesrepr::test_serialization_roundtrip(&delegator_bid_addr);
        }
    }
}
