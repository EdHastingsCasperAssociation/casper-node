use alloc::{string::String, vec::Vec};
use core::convert::TryFrom;

use serde::{de::Error as SerdeError, Deserialize, Deserializer, Serialize, Serializer};

use crate::Digest;

pub(crate) mod raw_32_byte_array {
    use super::*;

    pub(crate) fn serialize<S: Serializer>(
        array: &[u8; 32],
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            base16::encode_lower(array).serialize(serializer)
        } else {
            array.serialize(serializer)
        }
    }

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<[u8; 32], D::Error> {
        if deserializer.is_human_readable() {
            let hex_string = String::deserialize(deserializer)?;
            let bytes = base16::decode(hex_string.as_bytes()).map_err(SerdeError::custom)?;
            <[u8; 32]>::try_from(bytes.as_ref()).map_err(SerdeError::custom)
        } else {
            <[u8; 32]>::deserialize(deserializer)
        }
    }
}

pub(crate) mod contract_hash_as_digest {
    use super::*;
    use crate::contracts::ContractHash;

    pub(crate) fn serialize<S: Serializer>(
        contract_hash: &ContractHash,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        Digest::from(contract_hash.value()).serialize(serializer)
    }

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<ContractHash, D::Error> {
        let digest = Digest::deserialize(deserializer)?;
        Ok(ContractHash::new(digest.value()))
    }
}

pub(crate) mod contract_package_hash_as_digest {
    use super::*;
    use crate::contracts::ContractPackageHash;

    pub(crate) fn serialize<S: Serializer>(
        contract_package_hash: &ContractPackageHash,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        Digest::from(contract_package_hash.value()).serialize(serializer)
    }

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<ContractPackageHash, D::Error> {
        let digest = Digest::deserialize(deserializer)?;
        Ok(ContractPackageHash::new(digest.value()))
    }
}

/// This module allows `DeployHash`es to be serialized and deserialized using the underlying
/// `[u8; 32]` rather than delegating to the wrapped `Digest`, which in turn delegates to a
/// `Vec<u8>` for legacy reasons.
///
/// This is required as the `DeployHash` defined in `casper-types` up until v4.0.0 used the array
/// form, while the `DeployHash` defined in `casper-node` during this period delegated to `Digest`.
///
/// We use this module in places where the old `casper_types::DeployHash` was held as a member of a
/// type which implements `Serialize` and/or `Deserialize`.
pub(crate) mod deploy_hash_as_array {
    use super::*;
    use crate::DeployHash;

    pub(crate) fn serialize<S: Serializer>(
        deploy_hash: &DeployHash,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        if serializer.is_human_readable() {
            base16::encode_lower(&deploy_hash.inner().value()).serialize(serializer)
        } else {
            deploy_hash.inner().value().serialize(serializer)
        }
    }

    pub(crate) fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<DeployHash, D::Error> {
        let bytes = if deserializer.is_human_readable() {
            let hex_string = String::deserialize(deserializer)?;
            let vec_bytes = base16::decode(hex_string.as_bytes()).map_err(SerdeError::custom)?;
            <[u8; DeployHash::LENGTH]>::try_from(vec_bytes.as_ref()).map_err(SerdeError::custom)?
        } else {
            <[u8; DeployHash::LENGTH]>::deserialize(deserializer)?
        };
        Ok(DeployHash::new(Digest::from(bytes)))
    }
}

pub(crate) mod entry_point {
    use super::*;
    #[cfg(feature = "json-schema")]
    use schemars::JsonSchema;
    use serde::{Deserialize, Serialize};

    use crate::{contracts::EntryPoint, CLType, EntryPointAccess, EntryPointType, Parameters};

    /*
    This type exists to provide retro-compat for json representation of [`EntryPointType`] enum.
    The variants of this enum changed names in 2.x, but it also existed in 1.x. So for
    [`contract::EntryPoint`] we want to still json-serialize the variants as in 1.x.
     */
    #[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[cfg_attr(feature = "json-schema", derive(JsonSchema))]
    #[cfg_attr(
        feature = "json-schema",
        schemars(
            rename = "EntryPointType",
            description = "Type signature of a contract method."
        )
    )]
    pub(crate) enum HumanReadableEntryPointType {
        /// Runs as session code
        Session,
        /// Runs within contract's context
        Contract,
        /// Entry point type that installs
        /// wasm as new contracts. Runs using
        /// the called entity's context.
        Factory,
    }

    impl From<EntryPointType> for HumanReadableEntryPointType {
        fn from(value: EntryPointType) -> Self {
            match value {
                EntryPointType::Caller => HumanReadableEntryPointType::Session,
                EntryPointType::Called => HumanReadableEntryPointType::Contract,
                EntryPointType::Factory => HumanReadableEntryPointType::Factory,
            }
        }
    }

    impl From<HumanReadableEntryPointType> for EntryPointType {
        fn from(value: HumanReadableEntryPointType) -> Self {
            match value {
                HumanReadableEntryPointType::Session => EntryPointType::Caller,
                HumanReadableEntryPointType::Contract => EntryPointType::Called,
                HumanReadableEntryPointType::Factory => EntryPointType::Factory,
            }
        }
    }

    #[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
    #[cfg_attr(feature = "json-schema", derive(JsonSchema))]
    #[cfg_attr(
        feature = "json-schema",
        schemars(
            rename = "EntryPoint",
            description = "Type signature of a contract method."
        )
    )]
    pub(crate) struct HumanReadableEntryPoint {
        name: String,
        args: Parameters,
        ret: CLType,
        access: EntryPointAccess,
        entry_point_type: HumanReadableEntryPointType,
    }

    impl From<&EntryPoint> for HumanReadableEntryPoint {
        fn from(value: &EntryPoint) -> Self {
            Self {
                name: String::from(value.name()),
                args: value.args().to_vec(),
                ret: value.ret().clone(),
                access: value.access().clone(),
                entry_point_type: value.entry_point_type().into(),
            }
        }
    }

    impl From<HumanReadableEntryPoint> for EntryPoint {
        fn from(value: HumanReadableEntryPoint) -> Self {
            let HumanReadableEntryPoint {
                name,
                args,
                ret,
                access,
                entry_point_type,
            } = value;
            EntryPoint::new(name, args, ret, access, entry_point_type.into())
        }
    }
}

pub(crate) mod contract {
    use super::{entry_point::HumanReadableEntryPoint, *};
    use crate::{
        contracts::{ContractPackageHash, EntryPoints},
        Contract, ContractWasmHash, NamedKeys, ProtocolVersion,
    };
    use core::fmt::Display;
    #[cfg(feature = "json-schema")]
    use schemars::JsonSchema;
    use serde::{Deserialize, Serialize};

    #[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
    #[cfg_attr(feature = "json-schema", derive(JsonSchema))]
    #[cfg_attr(
        feature = "json-schema",
        schemars(
            rename = "Contract",
            description = "Methods and type signatures supported by a contract.",
        )
    )]
    pub(crate) struct HumanReadableContract {
        contract_package_hash: ContractPackageHash,
        contract_wasm_hash: ContractWasmHash,
        named_keys: NamedKeys,
        entry_points: Vec<HumanReadableEntryPoint>,
        protocol_version: ProtocolVersion,
    }

    impl From<&Contract> for HumanReadableContract {
        fn from(value: &Contract) -> Self {
            Self {
                contract_package_hash: value.contract_package_hash(),
                contract_wasm_hash: value.contract_wasm_hash(),
                named_keys: value.named_keys().clone(),
                protocol_version: value.protocol_version(),
                entry_points: value
                    .entry_points()
                    .clone()
                    .take_entry_points()
                    .iter()
                    .map(Into::into)
                    .collect(),
            }
        }
    }

    /// Parsing error when deserializing StoredValue.
    #[derive(Debug, Clone)]
    pub(crate) enum ContractDeserializationError {
        /// Contract not deserializable.
        NonUniqueEntryPointName,
    }

    impl Display for ContractDeserializationError {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            match self {
                ContractDeserializationError::NonUniqueEntryPointName => {
                    write!(f, "Non unique `entry_points.name`")
                }
            }
        }
    }

    impl TryFrom<HumanReadableContract> for Contract {
        type Error = ContractDeserializationError;
        fn try_from(value: HumanReadableContract) -> Result<Self, Self::Error> {
            let HumanReadableContract {
                contract_package_hash,
                contract_wasm_hash,
                named_keys,
                entry_points,
                protocol_version,
            } = value;
            let mut entry_points_map = EntryPoints::new();
            for entry_point in entry_points {
                if entry_points_map
                    .add_entry_point(entry_point.into())
                    .is_some()
                {
                    //There were duplicate entries in regards to 'name'
                    return Err(ContractDeserializationError::NonUniqueEntryPointName);
                }
            }

            Ok(Contract::new(
                contract_package_hash,
                contract_wasm_hash,
                named_keys,
                entry_points_map,
                protocol_version,
            ))
        }
    }
}

pub(crate) mod contract_package {
    use core::convert::TryFrom;

    use super::*;
    #[cfg(feature = "json-schema")]
    use schemars::JsonSchema;
    use serde::{Deserialize, Serialize};

    use crate::{
        contracts::{
            ContractHash, ContractPackage, ContractPackageStatus, ContractVersion,
            ContractVersionKey, ContractVersions, DisabledVersions, ProtocolVersionMajor,
        },
        Groups, URef,
    };

    #[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
    #[cfg_attr(feature = "json-schema", derive(JsonSchema))]
    #[cfg_attr(feature = "json-schema", schemars(rename = "ContractVersion"))]
    pub(crate) struct HumanReadableContractVersion {
        protocol_version_major: ProtocolVersionMajor,
        contract_version: ContractVersion,
        contract_hash: ContractHash,
    }

    /// Helper struct for deserializing/serializing `ContractPackage` from and to JSON.
    #[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
    #[cfg_attr(feature = "json-schema", derive(JsonSchema))]
    #[cfg_attr(feature = "json-schema", schemars(rename = "ContractPackage"))]
    pub(crate) struct HumanReadableContractPackage {
        access_key: URef,
        versions: Vec<HumanReadableContractVersion>,
        disabled_versions: DisabledVersions,
        groups: Groups,
        lock_status: ContractPackageStatus,
    }

    impl From<&ContractPackage> for HumanReadableContractPackage {
        fn from(package: &ContractPackage) -> Self {
            let mut versions = vec![];
            for (key, hash) in package.versions() {
                versions.push(HumanReadableContractVersion {
                    protocol_version_major: key.protocol_version_major(),
                    contract_version: key.contract_version(),
                    contract_hash: *hash,
                });
            }
            HumanReadableContractPackage {
                access_key: package.access_key(),
                versions,
                disabled_versions: package.disabled_versions().clone(),
                groups: package.groups().clone(),
                lock_status: package.lock_status(),
            }
        }
    }

    impl TryFrom<HumanReadableContractPackage> for ContractPackage {
        type Error = String;

        fn try_from(value: HumanReadableContractPackage) -> Result<Self, Self::Error> {
            let mut versions = ContractVersions::default();
            for version in value.versions.iter() {
                let key = ContractVersionKey::new(
                    version.protocol_version_major,
                    version.contract_version,
                );
                if versions.contains_key(&key) {
                    return Err(format!("duplicate contract version: {:?}", key));
                }
                versions.insert(key, version.contract_hash);
            }
            Ok(ContractPackage::new(
                value.access_key,
                versions,
                value.disabled_versions,
                value.groups,
                value.lock_status,
            ))
        }
    }
}
