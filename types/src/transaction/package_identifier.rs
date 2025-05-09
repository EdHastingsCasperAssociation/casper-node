use alloc::string::String;
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
use crate::{EntityVersionKey, PackageHash};
#[cfg(doc)]
use crate::{ExecutableDeployItem, TransactionTarget};

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
    pub fn version_key(&self) -> Option<EntityVersionKey> {
        match self {
            PackageIdentifier::HashWithVersion { version_key, .. }
            | PackageIdentifier::NameWithVersion { version_key, .. } => *version_key,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_serialization_roundtrip() {
        let rng = &mut TestRng::new();
        let p_id = PackageIdentifier::random(rng);
        let json_str = serde_json::to_string(&p_id).unwrap();
        let deserialized = serde_json::from_str::<PackageIdentifier>(&json_str).unwrap();
        assert_eq!(p_id, deserialized);
    }
}
