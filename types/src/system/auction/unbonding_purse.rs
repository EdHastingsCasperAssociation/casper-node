use alloc::vec::Vec;

#[cfg(feature = "datasize")]
use datasize::DataSize;
#[cfg(feature = "json-schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{
    account::AccountHash,
    bytesrepr::{self, FromBytes, ToBytes, U8_SERIALIZED_LENGTH},
    CLType, CLTyped, EraId, PublicKey, URef, URefAddr, U512,
};

use super::WithdrawPurse;

pub use v2::{Unbond, UnbonderIdentity, UnbonderIdentityTag};

/// UnbondKindTag variants.
#[allow(clippy::large_enum_variant)]
#[repr(u8)]
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize, Clone)]
pub enum UnbondKindTag {
    /// V1 unbond records.
    V1 = 0,
    /// V2 unbond records.
    V2 = 1,
}

pub enum UnbondKind {
    V1(UnbondingPurse),
    V2(Unbond),
}

#[allow(unused)]
impl UnbondKind {
    pub fn tag(&self) -> UnbondKindTag {
        match self {
            UnbondKind::V1(_) => UnbondKindTag::V1,
            UnbondKind::V2(_) => UnbondKindTag::V2,
        }
    }
    /// Checks if given request is made by a validator by checking if public key of unbonder is same
    /// as a key owned by validator.
    pub fn is_validator(&self) -> bool {
        match self {
            UnbondKind::V1(unbonding_purse) => unbonding_purse.is_validator(),
            UnbondKind::V2(unbond) => unbond.is_validator(),
        }
    }

    /// Returns bonding purse used to make this unbonding request.
    pub fn bonding_purse(&self) -> &URef {
        match self {
            UnbondKind::V1(unbonding_purse) => unbonding_purse.bonding_purse(),
            UnbondKind::V2(unbond) => unbond.bonding_purse(),
        }
    }

    /// Returns public key of validator.
    pub fn validator_public_key(&self) -> &PublicKey {
        match self {
            UnbondKind::V1(unbonding_purse) => unbonding_purse.validator_public_key(),
            UnbondKind::V2(unbond) => unbond.validator_public_key(),
        }
    }

    /// Returns public key of unbonder.
    ///
    /// For withdrawal requests that originated from validator's public key through `withdraw_bid`
    /// entrypoint this is equal to [`UnbondingPurse::validator_public_key`] and
    /// [`UnbondingPurse::is_validator`] is `true`.
    pub fn unbonder_public_key(&self) -> Option<&PublicKey> {
        match self {
            UnbondKind::V1(unbonding_purse) => Some(unbonding_purse.unbonder_public_key()),
            UnbondKind::V2(unbond) => unbond.unbonder_public_key(),
        }
    }

    /// Returns era which was used to create this unbonding request.
    pub fn era_of_creation(&self) -> EraId {
        match self {
            UnbondKind::V1(unbonding_purse) => unbonding_purse.era_of_creation(),
            UnbondKind::V2(unbond) => unbond.era_of_creation(),
        }
    }

    /// Returns unbonding amount.
    pub fn amount(&self) -> &U512 {
        match self {
            UnbondKind::V1(unbonding_purse) => unbonding_purse.amount(),
            UnbondKind::V2(unbond) => unbond.amount(),
        }
    }

    /// Returns the public key for the new validator.
    pub fn new_validator(&self) -> &Option<PublicKey> {
        match self {
            UnbondKind::V1(unbonding_purse) => unbonding_purse.new_validator(),
            UnbondKind::V2(unbond) => unbond.new_validator(),
        }
    }

    /// Sets amount to provided value.
    pub fn with_amount(&mut self, amount: U512) {
        match self {
            UnbondKind::V1(unbonding_purse) => unbonding_purse.with_amount(amount),
            UnbondKind::V2(unbond) => unbond.with_amount(amount),
        }
    }
}

impl ToBytes for UnbondKind {
    fn to_bytes(&self) -> Result<Vec<u8>, bytesrepr::Error> {
        let mut result = bytesrepr::allocate_buffer(self)?;
        let (tag, mut serialized_data) = match self {
            UnbondKind::V1(unbonding_purse) => (UnbondKindTag::V1, unbonding_purse.to_bytes()?),
            UnbondKind::V2(unbond) => (UnbondKindTag::V2, unbond.to_bytes()?),
        };
        result.push(tag as u8);
        result.append(&mut serialized_data);
        Ok(result)
    }

    fn serialized_length(&self) -> usize {
        U8_SERIALIZED_LENGTH
            + match self {
                UnbondKind::V1(unbonding_purse) => unbonding_purse.serialized_length(),
                UnbondKind::V2(unbond) => unbond.serialized_length(),
            }
    }

    fn write_bytes(&self, writer: &mut Vec<u8>) -> Result<(), bytesrepr::Error> {
        writer.push(self.tag() as u8);
        match self {
            UnbondKind::V1(unbonding_purse) => unbonding_purse.write_bytes(writer)?,
            UnbondKind::V2(unbond) => unbond.write_bytes(writer)?,
        };
        Ok(())
    }
}

impl FromBytes for UnbondKind {
    fn from_bytes(bytes: &[u8]) -> Result<(Self, &[u8]), bytesrepr::Error> {
        let (tag, remainder): (u8, &[u8]) = FromBytes::from_bytes(bytes)?;
        match tag {
            tag if tag == UnbondKindTag::V1 as u8 => UnbondingPurse::from_bytes(remainder)
                .map(|(unbonding_purse, remainder)| (UnbondKind::V1(unbonding_purse), remainder)),
            tag if tag == UnbondKindTag::V2 as u8 => Unbond::from_bytes(remainder)
                .map(|(unbond, remainder)| (UnbondKind::V2(unbond), remainder)),

            _ => Err(bytesrepr::Error::Formatting),
        }
    }
}

mod v2 {
    use super::*;
    use crate::system::auction::{BidAddr, BidAddrDelegator};
    use crate::AccessRights;

    /// UnbonderIdentityTag variants.
    #[repr(u8)]
    #[derive(Debug, PartialEq, Eq, Serialize, Deserialize, Clone)]
    pub enum UnbonderIdentityTag {
        /// PublicKey unbond identity.
        PublicKey = 0,
        /// AccountHash unbond identity.
        AccountHash = 1,
        /// Purse unbond identity.
        Purse = 2,
    }

    /// UnbonderIdentity variants.
    #[derive(Debug, PartialEq, Eq, Serialize, Deserialize, Clone)]
    #[cfg_attr(feature = "datasize", derive(DataSize))]
    #[cfg_attr(feature = "json-schema", derive(JsonSchema))]
    #[serde(deny_unknown_fields)]
    pub enum UnbonderIdentity {
        /// PublicKey unbond identity.
        PublicKey(PublicKey),
        /// AccountHash unbond identity.
        AccountHash(AccountHash),
        /// Purse unbond identity.
        Purse(URefAddr),
    }

    impl UnbonderIdentity {
        /// The UnbonderIdentityTag for this instance.
        pub fn tag(&self) -> UnbonderIdentityTag {
            match self {
                UnbonderIdentity::PublicKey(_) => UnbonderIdentityTag::PublicKey,
                UnbonderIdentity::AccountHash(_) => UnbonderIdentityTag::AccountHash,
                UnbonderIdentity::Purse(_) => UnbonderIdentityTag::Purse,
            }
        }

        /// Public key, if available.
        pub fn maybe_public_key(&self) -> Option<&PublicKey> {
            match self {
                UnbonderIdentity::PublicKey(public_key) => Some(public_key),
                UnbonderIdentity::AccountHash(_) | UnbonderIdentity::Purse(_) => None,
            }
        }

        /// Account hash, if available.
        pub fn maybe_account_hash(&self) -> Option<AccountHash> {
            match self {
                UnbonderIdentity::PublicKey(public_key) => {
                    Some(AccountHash::from(&public_key.clone()))
                }
                UnbonderIdentity::AccountHash(account_hash) => Some(*account_hash),
                UnbonderIdentity::Purse(_) => None,
            }
        }

        /// Source purse uref addr, if available.
        pub fn maybe_source_purse_uref(&self) -> Option<URefAddr> {
            match self {
                UnbonderIdentity::PublicKey(_) | UnbonderIdentity::AccountHash(_) => None,
                UnbonderIdentity::Purse(uref_addr) => Some(*uref_addr),
            }
        }
    }

    impl ToBytes for UnbonderIdentity {
        fn to_bytes(&self) -> Result<Vec<u8>, bytesrepr::Error> {
            let mut result = bytesrepr::allocate_buffer(self)?;
            let (tag, mut serialized_data) = match self {
                UnbonderIdentity::PublicKey(public_key) => {
                    (UnbonderIdentityTag::PublicKey, public_key.to_bytes()?)
                }
                UnbonderIdentity::AccountHash(account_hash) => {
                    (UnbonderIdentityTag::AccountHash, account_hash.to_bytes()?)
                }
                UnbonderIdentity::Purse(uref_addr) => {
                    (UnbonderIdentityTag::Purse, uref_addr.to_bytes()?)
                }
            };
            result.push(tag as u8);
            result.append(&mut serialized_data);
            Ok(result)
        }

        fn serialized_length(&self) -> usize {
            U8_SERIALIZED_LENGTH
                + match self {
                    UnbonderIdentity::PublicKey(public_key) => public_key.serialized_length(),
                    UnbonderIdentity::AccountHash(account_hash) => account_hash.serialized_length(),
                    UnbonderIdentity::Purse(uref_addr) => uref_addr.serialized_length(),
                }
        }

        fn write_bytes(&self, writer: &mut Vec<u8>) -> Result<(), bytesrepr::Error> {
            writer.push(self.tag() as u8);
            match self {
                UnbonderIdentity::PublicKey(public_key) => public_key.write_bytes(writer)?,
                UnbonderIdentity::AccountHash(account_hash) => account_hash.write_bytes(writer)?,
                UnbonderIdentity::Purse(uref_addr) => uref_addr.write_bytes(writer)?,
            };
            Ok(())
        }
    }

    impl FromBytes for UnbonderIdentity {
        fn from_bytes(bytes: &[u8]) -> Result<(Self, &[u8]), bytesrepr::Error> {
            let (tag, remainder): (u8, &[u8]) = FromBytes::from_bytes(bytes)?;
            match tag {
                tag if tag == UnbonderIdentityTag::PublicKey as u8 => {
                    PublicKey::from_bytes(remainder).map(|(public_key, remainder)| {
                        (UnbonderIdentity::PublicKey(public_key), remainder)
                    })
                }
                tag if tag == UnbonderIdentityTag::AccountHash as u8 => {
                    AccountHash::from_bytes(remainder).map(|(account_hash, remainder)| {
                        (UnbonderIdentity::AccountHash(account_hash), remainder)
                    })
                }
                tag if tag == UnbonderIdentityTag::Purse as u8 => URefAddr::from_bytes(remainder)
                    .map(|(uref_addr, remainder)| (UnbonderIdentity::Purse(uref_addr), remainder)),

                _ => Err(bytesrepr::Error::Formatting),
            }
        }
    }

    /// Unbonding purse.
    #[derive(PartialEq, Eq, Debug, Serialize, Deserialize, Clone)]
    #[cfg_attr(feature = "datasize", derive(DataSize))]
    #[cfg_attr(feature = "json-schema", derive(JsonSchema))]
    #[serde(deny_unknown_fields)]
    pub struct Unbond {
        /// Bonding Purse
        bonding_purse: URef,
        /// Validators public key.
        validator_public_key: PublicKey,
        /// Unbonder's identity.
        unbonder_identity: UnbonderIdentity,
        /// Era in which this unbonding request was created.
        era_of_creation: EraId,
        /// Unbonding Amount.
        amount: U512,
        /// The validator public key to re-delegate to.
        new_validator: Option<PublicKey>,
    }

    #[allow(unused)]
    impl Unbond {
        /// Creates [`Unbond`] instance for an unbonding request.
        pub const fn new(
            bonding_purse: URef,
            validator_public_key: PublicKey,
            unbonder_identity: UnbonderIdentity,
            era_of_creation: EraId,
            amount: U512,
            new_validator: Option<PublicKey>,
        ) -> Self {
            Self {
                bonding_purse,
                validator_public_key,
                unbonder_identity,
                era_of_creation,
                amount,
                new_validator,
            }
        }

        /// The correct BidAddr for this instance.
        pub fn bid_addr(&self) -> BidAddr {
            let validator = self.validator_public_key().to_account_hash();
            match &self.unbonder_identity {
                UnbonderIdentity::PublicKey(public_key) => BidAddr::Delegator {
                    validator,
                    delegator: BidAddrDelegator::Account(AccountHash::from(public_key)),
                },
                UnbonderIdentity::AccountHash(account_hash) => BidAddr::Delegator {
                    validator,
                    delegator: BidAddrDelegator::Account(*account_hash),
                },
                UnbonderIdentity::Purse(uref_addr) => BidAddr::Delegator {
                    validator,
                    delegator: BidAddrDelegator::Purse(*uref_addr),
                },
            }
        }

        /// Checks if given request is made by a validator by checking if public key of unbonder is
        /// same as a key owned by validator.
        pub fn is_validator(&self) -> bool {
            if let UnbonderIdentity::PublicKey(unbonder_public_key) = &self.unbonder_identity {
                self.validator_public_key == *unbonder_public_key
            } else {
                false
            }
        }

        /// Returns bonding purse used to make this unbonding request.
        pub fn bonding_purse(&self) -> &URef {
            &self.bonding_purse
        }

        /// Returns public key of validator.
        pub fn validator_public_key(&self) -> &PublicKey {
            &self.validator_public_key
        }

        /// Returns public key of unbonder.
        ///
        /// For withdrawal requests that originated from validator's public key through
        /// `withdraw_bid` entrypoint this is equal to [`Unbond::validator_public_key`] and
        /// [`Unbond::is_validator`] is `true`.
        pub fn unbonder_public_key(&self) -> Option<&PublicKey> {
            match &self.unbonder_identity {
                UnbonderIdentity::PublicKey(public_key) => Some(public_key),
                UnbonderIdentity::AccountHash(_) | UnbonderIdentity::Purse(_) => None,
            }
        }

        /// Returns account hash of unbonder, if available.
        pub fn account_hash(&self) -> Option<AccountHash> {
            match &self.unbonder_identity {
                UnbonderIdentity::PublicKey(public_key) => Some(AccountHash::from(public_key)),
                UnbonderIdentity::AccountHash(account_hash) => Some(*account_hash),
                UnbonderIdentity::Purse(_) => None,
            }
        }

        /// Returns source_purse of unbonder, if available.
        pub fn source_purse(&self) -> Option<URef> {
            match &self.unbonder_identity {
                UnbonderIdentity::PublicKey(_) | UnbonderIdentity::AccountHash(_) => None,
                UnbonderIdentity::Purse(uref_addr) => {
                    Some(URef::new(*uref_addr, AccessRights::READ_ADD_WRITE))
                }
            }
        }

        /// Returns the unbonder identity.
        pub fn unbonder_identity(&self) -> &UnbonderIdentity {
            &self.unbonder_identity
        }

        /// Returns era which was used to create this unbonding request.
        pub fn era_of_creation(&self) -> EraId {
            self.era_of_creation
        }

        /// Returns unbonding amount.
        pub fn amount(&self) -> &U512 {
            &self.amount
        }

        /// Returns the public key for the new validator.
        pub fn new_validator(&self) -> &Option<PublicKey> {
            &self.new_validator
        }

        /// Sets amount to provided value.
        pub fn with_amount(&mut self, amount: U512) {
            self.amount = amount;
        }
    }

    impl ToBytes for Unbond {
        fn to_bytes(&self) -> Result<Vec<u8>, bytesrepr::Error> {
            let mut result = bytesrepr::allocate_buffer(self)?;
            result.extend(&self.bonding_purse.to_bytes()?);
            result.extend(&self.validator_public_key.to_bytes()?);
            result.extend(&self.unbonder_identity.to_bytes()?);
            result.extend(&self.era_of_creation.to_bytes()?);
            result.extend(&self.amount.to_bytes()?);
            result.extend(&self.new_validator.to_bytes()?);
            Ok(result)
        }
        fn serialized_length(&self) -> usize {
            self.bonding_purse.serialized_length()
                + self.validator_public_key.serialized_length()
                + self.unbonder_identity.serialized_length()
                + self.era_of_creation.serialized_length()
                + self.amount.serialized_length()
                + self.new_validator.serialized_length()
        }

        fn write_bytes(&self, writer: &mut Vec<u8>) -> Result<(), bytesrepr::Error> {
            self.bonding_purse.write_bytes(writer)?;
            self.validator_public_key.write_bytes(writer)?;
            self.unbonder_identity.write_bytes(writer)?;
            self.era_of_creation.write_bytes(writer)?;
            self.amount.write_bytes(writer)?;
            self.new_validator.write_bytes(writer)?;
            Ok(())
        }
    }

    impl FromBytes for Unbond {
        fn from_bytes(bytes: &[u8]) -> Result<(Self, &[u8]), bytesrepr::Error> {
            let (bonding_purse, remainder) = FromBytes::from_bytes(bytes)?;
            let (validator_public_key, remainder) = FromBytes::from_bytes(remainder)?;
            let (unbonder_identity, remainder) = FromBytes::from_bytes(remainder)?;
            let (era_of_creation, remainder) = FromBytes::from_bytes(remainder)?;
            let (amount, remainder) = FromBytes::from_bytes(remainder)?;
            let (new_validator, remainder) = Option::<PublicKey>::from_bytes(remainder)?;

            Ok((
                Unbond {
                    bonding_purse,
                    validator_public_key,
                    unbonder_identity,
                    era_of_creation,
                    amount,
                    new_validator,
                },
                remainder,
            ))
        }
    }

    impl CLTyped for Unbond {
        fn cl_type() -> CLType {
            CLType::Any
        }
    }

    impl From<UnbondingPurse> for Unbond {
        fn from(unbonding_purse: UnbondingPurse) -> Self {
            let identity = UnbonderIdentity::PublicKey(unbonding_purse.unbonder_public_key.clone());
            Unbond::new(
                unbonding_purse.bonding_purse,
                unbonding_purse.validator_public_key.clone(),
                identity,
                unbonding_purse.era_of_creation,
                unbonding_purse.amount,
                unbonding_purse.new_validator().clone(),
            )
        }
    }
}

/// Unbonding purse.
#[derive(PartialEq, Eq, Debug, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "datasize", derive(DataSize))]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct UnbondingPurse {
    /// Bonding Purse
    bonding_purse: URef,
    /// Validators public key.
    validator_public_key: PublicKey,
    /// Unbonders public key.
    unbonder_public_key: PublicKey,
    /// Era in which this unbonding request was created.
    era_of_creation: EraId,
    /// Unbonding Amount.
    amount: U512,
    /// The validator public key to re-delegate to.
    new_validator: Option<PublicKey>,
}

impl UnbondingPurse {
    /// Creates [`UnbondingPurse`] instance for an unbonding request.
    pub const fn new(
        bonding_purse: URef,
        validator_public_key: PublicKey,
        unbonder_public_key: PublicKey,
        era_of_creation: EraId,
        amount: U512,
        new_validator: Option<PublicKey>,
    ) -> Self {
        Self {
            bonding_purse,
            validator_public_key,
            unbonder_public_key,
            era_of_creation,
            amount,
            new_validator,
        }
    }

    /// Checks if given request is made by a validator by checking if public key of unbonder is same
    /// as a key owned by validator.
    pub fn is_validator(&self) -> bool {
        self.validator_public_key == self.unbonder_public_key
    }

    /// Returns bonding purse used to make this unbonding request.
    pub fn bonding_purse(&self) -> &URef {
        &self.bonding_purse
    }

    /// Returns public key of validator.
    pub fn validator_public_key(&self) -> &PublicKey {
        &self.validator_public_key
    }

    /// Returns public key of unbonder.
    ///
    /// For withdrawal requests that originated from validator's public key through `withdraw_bid`
    /// entrypoint this is equal to [`UnbondingPurse::validator_public_key`] and
    /// [`UnbondingPurse::is_validator`] is `true`.
    pub fn unbonder_public_key(&self) -> &PublicKey {
        &self.unbonder_public_key
    }

    /// Returns era which was used to create this unbonding request.
    pub fn era_of_creation(&self) -> EraId {
        self.era_of_creation
    }

    /// Returns unbonding amount.
    pub fn amount(&self) -> &U512 {
        &self.amount
    }

    /// Returns the public key for the new validator.
    pub fn new_validator(&self) -> &Option<PublicKey> {
        &self.new_validator
    }

    /// Sets amount to provided value.
    pub fn with_amount(&mut self, amount: U512) {
        self.amount = amount;
    }
}

impl ToBytes for UnbondingPurse {
    fn to_bytes(&self) -> Result<Vec<u8>, bytesrepr::Error> {
        let mut result = bytesrepr::allocate_buffer(self)?;
        result.extend(&self.bonding_purse.to_bytes()?);
        result.extend(&self.validator_public_key.to_bytes()?);
        result.extend(&self.unbonder_public_key.to_bytes()?);
        result.extend(&self.era_of_creation.to_bytes()?);
        result.extend(&self.amount.to_bytes()?);
        result.extend(&self.new_validator.to_bytes()?);
        Ok(result)
    }
    fn serialized_length(&self) -> usize {
        self.bonding_purse.serialized_length()
            + self.validator_public_key.serialized_length()
            + self.unbonder_public_key.serialized_length()
            + self.era_of_creation.serialized_length()
            + self.amount.serialized_length()
            + self.new_validator.serialized_length()
    }

    fn write_bytes(&self, writer: &mut Vec<u8>) -> Result<(), bytesrepr::Error> {
        self.bonding_purse.write_bytes(writer)?;
        self.validator_public_key.write_bytes(writer)?;
        self.unbonder_public_key.write_bytes(writer)?;
        self.era_of_creation.write_bytes(writer)?;
        self.amount.write_bytes(writer)?;
        self.new_validator.write_bytes(writer)?;
        Ok(())
    }
}

impl FromBytes for UnbondingPurse {
    fn from_bytes(bytes: &[u8]) -> Result<(Self, &[u8]), bytesrepr::Error> {
        let (bonding_purse, remainder) = FromBytes::from_bytes(bytes)?;
        let (validator_public_key, remainder) = FromBytes::from_bytes(remainder)?;
        let (unbonder_public_key, remainder) = FromBytes::from_bytes(remainder)?;
        let (era_of_creation, remainder) = FromBytes::from_bytes(remainder)?;
        let (amount, remainder) = FromBytes::from_bytes(remainder)?;
        let (new_validator, remainder) = Option::<PublicKey>::from_bytes(remainder)?;

        Ok((
            UnbondingPurse {
                bonding_purse,
                validator_public_key,
                unbonder_public_key,
                era_of_creation,
                amount,
                new_validator,
            },
            remainder,
        ))
    }
}

impl CLTyped for UnbondingPurse {
    fn cl_type() -> CLType {
        CLType::Any
    }
}

impl From<WithdrawPurse> for UnbondingPurse {
    fn from(withdraw_purse: WithdrawPurse) -> Self {
        UnbondingPurse::new(
            withdraw_purse.bonding_purse,
            withdraw_purse.validator_public_key,
            withdraw_purse.unbonder_public_key,
            withdraw_purse.era_of_creation,
            withdraw_purse.amount,
            None,
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        bytesrepr, system::auction::UnbondingPurse, AccessRights, EraId, PublicKey, SecretKey,
        URef, U512,
    };

    const BONDING_PURSE: URef = URef::new([14; 32], AccessRights::READ_ADD_WRITE);
    const ERA_OF_WITHDRAWAL: EraId = EraId::MAX;

    fn validator_public_key() -> PublicKey {
        let secret_key = SecretKey::ed25519_from_bytes([42; SecretKey::ED25519_LENGTH]).unwrap();
        PublicKey::from(&secret_key)
    }

    fn unbonder_public_key() -> PublicKey {
        let secret_key = SecretKey::ed25519_from_bytes([43; SecretKey::ED25519_LENGTH]).unwrap();
        PublicKey::from(&secret_key)
    }

    fn amount() -> U512 {
        U512::max_value() - 1
    }

    #[test]
    fn serialization_roundtrip_for_unbonding_purse() {
        let unbonding_purse = UnbondingPurse {
            bonding_purse: BONDING_PURSE,
            validator_public_key: validator_public_key(),
            unbonder_public_key: unbonder_public_key(),
            era_of_creation: ERA_OF_WITHDRAWAL,
            amount: amount(),
            new_validator: None,
        };

        bytesrepr::test_serialization_roundtrip(&unbonding_purse);
    }

    #[test]
    fn should_be_validator_condition_for_unbonding_purse() {
        let validator_unbonding_purse = UnbondingPurse::new(
            BONDING_PURSE,
            validator_public_key(),
            validator_public_key(),
            ERA_OF_WITHDRAWAL,
            amount(),
            None,
        );
        assert!(validator_unbonding_purse.is_validator());
    }

    #[test]
    fn should_be_delegator_condition_for_unbonding_purse() {
        let delegator_unbonding_purse = UnbondingPurse::new(
            BONDING_PURSE,
            validator_public_key(),
            unbonder_public_key(),
            ERA_OF_WITHDRAWAL,
            amount(),
            None,
        );
        assert!(!delegator_unbonding_purse.is_validator());
    }
}
