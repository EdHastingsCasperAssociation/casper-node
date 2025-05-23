//! Units of execution.

use casper_storage::{
    global_state::{error::Error as GlobalStateError, state::StateReader},
    tracking_copy::{TrackingCopy, TrackingCopyExt},
};
use casper_types::{
    bytesrepr::Bytes, contracts::NamedKeys, AddressableEntityHash, Key, PackageHash, StoredValue,
    TransactionInvocationTarget,
};

use super::{wasm_v1::SessionKind, Error, ExecutableItem};
use crate::execution::ExecError;

/// The type of execution about to be performed.
#[derive(Clone, Debug)]
pub(crate) enum ExecutionKind<'a> {
    /// Standard (non-specialized) Wasm bytes related to a transaction of version 1 or later.
    Standard(&'a Bytes),
    /// Wasm bytes which install or upgrade a stored entity.
    InstallerUpgrader(&'a Bytes),
    /// Stored contract.
    Stored {
        /// AddressableEntity's hash.
        entity_hash: AddressableEntityHash,
        /// Entry point.
        entry_point: String,
    },
    /// Standard (non-specialized) Wasm bytes related to a `Deploy`.
    ///
    /// This is equivalent to the `Standard` variant with the exception that this kind will be
    /// allowed to install or upgrade stored entities to retain existing (pre-node 2.0) behavior.
    Deploy(&'a Bytes),
}

impl<'a> ExecutionKind<'a> {
    pub(crate) fn new<R>(
        tracking_copy: &mut TrackingCopy<R>,
        named_keys: &NamedKeys,
        executable_item: &'a ExecutableItem,
        entry_point: String,
    ) -> Result<Self, Error>
    where
        R: StateReader<Key, StoredValue, Error = GlobalStateError>,
    {
        match executable_item {
            ExecutableItem::Invocation(target) => {
                Self::new_direct_invocation(tracking_copy, named_keys, target, entry_point)
            }
            ExecutableItem::PaymentBytes(module_bytes)
            | ExecutableItem::SessionBytes {
                kind: SessionKind::GenericBytecode,
                module_bytes,
            } => Ok(ExecutionKind::Standard(module_bytes)),
            ExecutableItem::SessionBytes {
                kind: SessionKind::InstallUpgradeBytecode,
                module_bytes,
            } => Ok(ExecutionKind::InstallerUpgrader(module_bytes)),
            ExecutableItem::Deploy(module_bytes) => Ok(ExecutionKind::Deploy(module_bytes)),
        }
    }

    fn new_direct_invocation<R>(
        tracking_copy: &mut TrackingCopy<R>,
        named_keys: &NamedKeys,
        target: &TransactionInvocationTarget,
        entry_point: String,
    ) -> Result<Self, Error>
    where
        R: StateReader<Key, StoredValue, Error = GlobalStateError>,
    {
        let entity_hash = match target {
            TransactionInvocationTarget::ByHash(addr) => AddressableEntityHash::new(*addr),
            TransactionInvocationTarget::ByName(alias) => {
                let entity_key = named_keys
                    .get(alias)
                    .ok_or_else(|| Error::Exec(ExecError::NamedKeyNotFound(alias.clone())))?;

                match entity_key {
                    Key::Hash(hash) => AddressableEntityHash::new(*hash),
                    Key::AddressableEntity(entity_addr) => {
                        AddressableEntityHash::new(entity_addr.value())
                    }
                    _ => return Err(Error::InvalidKeyVariant(*entity_key)),
                }
            }
            TransactionInvocationTarget::ByPackageHash {
                addr,
                version_key,
                version: _, // version is defunct and should not be used
            } => {
                let package_hash = PackageHash::from(*addr);
                let package = tracking_copy.get_package(*addr)?;

                let maybe_version_key = version_key;

                let entity_version_key = maybe_version_key
                    .or_else(|| package.current_entity_version())
                    .ok_or(Error::Exec(ExecError::NoActiveEntityVersions(package_hash)))?;

                if package.is_version_missing(entity_version_key) {
                    return Err(Error::Exec(ExecError::MissingEntityVersion(
                        entity_version_key,
                    )));
                }

                if !package.is_version_enabled(entity_version_key) {
                    return Err(Error::Exec(ExecError::DisabledEntityVersion(
                        entity_version_key,
                    )));
                }

                let entity_addr =
                    *package
                        .lookup_entity_hash(entity_version_key)
                        .ok_or(Error::Exec(ExecError::InvalidEntityVersion(
                            entity_version_key,
                        )))?;

                AddressableEntityHash::new(entity_addr.value())
            }
            TransactionInvocationTarget::ByPackageName {
                name: alias,
                version_key,
                version: _, // version is defunct and should not be used
            } => {
                let package_key = named_keys
                    .get(alias)
                    .ok_or_else(|| Error::Exec(ExecError::NamedKeyNotFound(alias.to_string())))?;

                let package_hash = match package_key {
                    Key::Hash(hash) | Key::SmartContract(hash) => PackageHash::new(*hash),
                    _ => return Err(Error::InvalidKeyVariant(*package_key)),
                };

                let package = tracking_copy.get_package(package_hash.value())?;

                let entity_version_key =
                    version_key
                        .or_else(|| package.current_entity_version())
                        .ok_or(Error::Exec(ExecError::NoActiveEntityVersions(package_hash)))?;

                if package.is_version_missing(entity_version_key) {
                    return Err(Error::Exec(ExecError::MissingEntityVersion(
                        entity_version_key,
                    )));
                }

                if !package.is_version_enabled(entity_version_key) {
                    return Err(Error::Exec(ExecError::DisabledEntityVersion(
                        entity_version_key,
                    )));
                }

                let entity_addr =
                    *package
                        .lookup_entity_hash(entity_version_key)
                        .ok_or(Error::Exec(ExecError::InvalidEntityVersion(
                            entity_version_key,
                        )))?;

                AddressableEntityHash::new(entity_addr.value())
            }
        };
        Ok(ExecutionKind::Stored {
            entity_hash,
            entry_point,
        })
    }
}
