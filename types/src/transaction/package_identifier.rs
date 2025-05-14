use alloc::{string::String, vec::Vec};
use core::fmt::{self, Display, Formatter};

#[cfg(feature = "datasize")]
use datasize::DataSize;
use hex_fmt::HexFmt;
#[cfg(any(feature = "testing", test))]
use rand::Rng;
#[cfg(feature = "json-schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[cfg(any(feature = "testing", test))]
use crate::testing::TestRng;
use crate::{
    bytesrepr::{self, FromBytes, ToBytes, U8_SERIALIZED_LENGTH},
    EntityVersion, EntityVersionKey, PackageHash,
};
#[cfg(doc)]
use crate::{ExecutableDeployItem, TransactionTarget};

const HASH_TAG: u8 = 0;
const NAME_TAG: u8 = 1;
const HASH_WITH_VERSION_TAG: u8 = 2;
const NAME_WITH_VERSION_TAG: u8 = 3;

/// Identifier for the package object within a [`TransactionTarget::Stored`] or an
/// [`ExecutableDeployItem`].
#[derive(Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize, Debug)]
#[cfg_attr(feature = "datasize", derive(DataSize))]
#[cfg_attr(
    feature = "json-schema",
    derive(JsonSchema),
    schemars(
        description = "Identifier for the package object within a `Stored` transaction target or \
        an `ExecutableDeployItem`."
    )
)]
pub enum PackageIdentifier {
    /// The hash and optional version identifying the contract package.
    Hash {
        /// The hash of the contract package.
        package_hash: PackageHash,
        /// The version of the contract package.
        ///
        /// `None` implies latest version.
        version: Option<EntityVersion>,
    },
    /// The name and optional version identifying the contract package.
    Name {
        /// The name of the contract package.
        name: String,
        /// The version of the contract package.
        ///
        /// `None` implies latest version.
        version: Option<EntityVersion>,
    },
    /// The hash and optional version key identifying the contract package.
    HashWithVersion {
        /// The hash of the contract package.
        package_hash: PackageHash,
        /// The version key of the contract package.
        ///
        /// `None` implies latest version.
        version_key: Option<EntityVersionKey>,
    },
    /// The name and optional version key identifying the contract package.
    NameWithVersion {
        /// The name of the contract package.
        name: String,
        /// The version key of the contract package.
        ///
        /// `None` implies latest version.
        version_key: Option<EntityVersionKey>,
    },
}

impl PackageIdentifier {
    /// Returns the optional version of the contract package.
    ///
    /// `None` implies latest version.
    #[deprecated(since = "5.0.1", note = "please use `version_key` instead")]
    pub fn version(&self) -> Option<EntityVersion> {
        match self {
            PackageIdentifier::HashWithVersion { .. }
            | PackageIdentifier::NameWithVersion { .. } => None,
            PackageIdentifier::Hash { version, .. } | PackageIdentifier::Name { version, .. } => {
                *version
            }
        }
    }

    /// Returns the optional version key of the contract package.
    ///
    /// `None` implies latest version.
    pub fn version_key(&self) -> Option<EntityVersionKey> {
        match self {
            PackageIdentifier::HashWithVersion { version_key, .. }
            | PackageIdentifier::NameWithVersion { version_key, .. } => *version_key,
            PackageIdentifier::Hash { .. } | PackageIdentifier::Name { .. } => None,
        }
    }

    /// Returns a random `PackageIdentifier`.
    #[cfg(any(feature = "testing", test))]
    pub fn random(rng: &mut TestRng) -> Self {
        let version_key = rng.gen::<bool>().then(|| rng.gen::<EntityVersionKey>());
        if rng.gen() {
            PackageIdentifier::HashWithVersion {
                package_hash: PackageHash::new(rng.gen()),
                version_key,
            }
        } else {
            PackageIdentifier::NameWithVersion {
                name: rng.random_string(1..21),
                version_key,
            }
        }
    }
}

impl Display for PackageIdentifier {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        match self {
            PackageIdentifier::Hash {
                package_hash: contract_package_hash,
                version: Some(ver),
            } => write!(
                formatter,
                "package-id({}, version {})",
                HexFmt(contract_package_hash),
                ver
            ),
            PackageIdentifier::Hash {
                package_hash: contract_package_hash,
                ..
            } => write!(
                formatter,
                "package-id({}, latest)",
                HexFmt(contract_package_hash),
            ),
            PackageIdentifier::Name {
                name,
                version: Some(ver),
            } => write!(formatter, "package-id({}, version {})", name, ver),
            PackageIdentifier::Name { name, .. } => {
                write!(formatter, "package-id({}, latest)", name)
            }
            PackageIdentifier::HashWithVersion {
                package_hash,
                version_key,
            } => {
                write!(
                    formatter,
                    "package-id({}, {:?})",
                    HexFmt(package_hash),
                    version_key
                )
            }
            PackageIdentifier::NameWithVersion { name, version_key } => {
                write!(formatter, "package-id({}, {:?})", name, version_key)
            }
        }
    }
}

impl ToBytes for PackageIdentifier {
    fn write_bytes(&self, writer: &mut Vec<u8>) -> Result<(), bytesrepr::Error> {
        match self {
            PackageIdentifier::Hash {
                package_hash,
                version,
            } => {
                HASH_TAG.write_bytes(writer)?;
                package_hash.write_bytes(writer)?;
                version.write_bytes(writer)
            }
            PackageIdentifier::HashWithVersion {
                package_hash,
                version_key,
            } => {
                HASH_WITH_VERSION_TAG.write_bytes(writer)?;
                package_hash.write_bytes(writer)?;
                version_key.write_bytes(writer)
            }
            PackageIdentifier::Name { name, version } => {
                NAME_TAG.write_bytes(writer)?;
                name.write_bytes(writer)?;
                version.write_bytes(writer)
            }
            PackageIdentifier::NameWithVersion { name, version_key } => {
                NAME_WITH_VERSION_TAG.write_bytes(writer)?;
                name.write_bytes(writer)?;
                version_key.write_bytes(writer)
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
                PackageIdentifier::Hash {
                    package_hash,
                    version,
                } => package_hash.serialized_length() + version.serialized_length(),
                PackageIdentifier::Name { name, version } => {
                    name.serialized_length() + version.serialized_length()
                }
                PackageIdentifier::HashWithVersion {
                    package_hash,
                    version_key,
                } => package_hash.serialized_length() + version_key.serialized_length(),
                PackageIdentifier::NameWithVersion { name, version_key } => {
                    name.serialized_length() + version_key.serialized_length()
                }
            }
    }
}

impl FromBytes for PackageIdentifier {
    fn from_bytes(bytes: &[u8]) -> Result<(Self, &[u8]), bytesrepr::Error> {
        let (tag, remainder) = u8::from_bytes(bytes)?;
        match tag {
            HASH_TAG => {
                let (package_hash, remainder) = PackageHash::from_bytes(remainder)?;
                let (version, remainder) = Option::<EntityVersion>::from_bytes(remainder)?;
                let id = PackageIdentifier::Hash {
                    package_hash,
                    version,
                };
                Ok((id, remainder))
            }
            NAME_TAG => {
                let (name, remainder) = String::from_bytes(remainder)?;
                let (version, remainder) = Option::<EntityVersion>::from_bytes(remainder)?;
                let id = PackageIdentifier::Name { name, version };
                Ok((id, remainder))
            }
            HASH_WITH_VERSION_TAG => {
                let (package_hash, remainder) = PackageHash::from_bytes(remainder)?;
                let (version_key, remainder) = Option::<EntityVersionKey>::from_bytes(remainder)?;
                let id = PackageIdentifier::HashWithVersion {
                    package_hash,
                    version_key,
                };
                Ok((id, remainder))
            }
            NAME_WITH_VERSION_TAG => {
                let (name, remainder) = String::from_bytes(remainder)?;
                let (version_key, remainder) = Option::<EntityVersionKey>::from_bytes(remainder)?;
                let id = PackageIdentifier::NameWithVersion { name, version_key };
                Ok((id, remainder))
            }
            _ => Err(bytesrepr::Error::Formatting),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytesrepr_roundtrip() {
        let rng = &mut TestRng::new();
        bytesrepr::test_serialization_roundtrip(&PackageIdentifier::random(rng));
    }
}
