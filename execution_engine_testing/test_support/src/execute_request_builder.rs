use std::collections::BTreeSet;

use casper_execution_engine::engine_state::{
    BlockInfo, ExecutableItem, SessionDataV1, SessionInputData, WasmV1Request,
};
use casper_types::{
    account::AccountHash, addressable_entity::DEFAULT_ENTRY_POINT_NAME, runtime_args,
    AddressableEntityHash, BlockHash, BlockTime, Digest, EntityVersion, EntityVersionKey, Gas,
    InitiatorAddr, PackageHash, Phase, PricingMode, ProtocolVersion, RuntimeArgs,
    TransactionEntryPoint, TransactionHash, TransactionInvocationTarget, TransactionRuntimeParams,
    TransactionTarget, TransactionV1Hash,
};

use crate::{
    deploy_item::DeployItem, DeployItemBuilder, ARG_AMOUNT, DEFAULT_BLOCK_TIME, DEFAULT_PAYMENT,
    DEFAULT_PROTOCOL_VERSION,
};

/// A request comprising a [`WasmV1Request`] for use as session code, and an optional custom
/// payment `WasmV1Request`.
#[derive(Debug)]
pub struct ExecuteRequest {
    /// The session request.
    pub session: WasmV1Request,
    /// The optional custom payment request.
    pub custom_payment: Option<WasmV1Request>,
}

impl ExecuteRequest {
    /// Is install upgrade allowed?
    pub fn is_install_upgrade_allowed(&self) -> bool {
        self.session.executable_item.is_install_upgrade_allowed()
    }
}

/// Builds an [`ExecuteRequest`].
#[derive(Debug)]
pub struct ExecuteRequestBuilder {
    state_hash: Digest,
    block_time: BlockTime,
    block_height: u64,
    parent_block_hash: BlockHash,
    protocol_version: ProtocolVersion,
    transaction_hash: TransactionHash,
    initiator_addr: InitiatorAddr,
    payment: Option<ExecutableItem>,
    payment_gas_limit: Gas,
    payment_entry_point: String,
    payment_args: RuntimeArgs,
    session: ExecutableItem,
    session_gas_limit: Gas,
    session_entry_point: String,
    session_args: RuntimeArgs,
    authorization_keys: BTreeSet<AccountHash>,
}

const DEFAULT_GAS_LIMIT: u64 = 5_000_u64 * 10u64.pow(9);

impl ExecuteRequestBuilder {
    /// The default value used for `WasmV1Request::state_hash`.
    pub const DEFAULT_STATE_HASH: Digest = Digest::from_raw([1; 32]);
    /// The default value used for `WasmV1Request::transaction_hash`.
    pub const DEFAULT_TRANSACTION_HASH: TransactionHash =
        TransactionHash::V1(TransactionV1Hash::from_raw([2; 32]));
    /// The default value used for `WasmV1Request::entry_point`.
    pub const DEFAULT_ENTRY_POINT: &'static str = "call";
    /// The default protocol version stored in the BlockInfo
    pub const DEFAULT_PROTOCOL_VERSION: ProtocolVersion = ProtocolVersion::V2_0_0;

    /// Converts a `SessionInputData` into an `ExecuteRequestBuilder`.
    pub fn from_session_input_data(session_input_data: &SessionInputData) -> Self {
        let block_info = BlockInfo::new(
            Self::DEFAULT_STATE_HASH,
            BlockTime::new(DEFAULT_BLOCK_TIME),
            BlockHash::default(),
            0,
            DEFAULT_PROTOCOL_VERSION,
        );
        let authorization_keys = session_input_data.signers();
        let session =
            WasmV1Request::new_session(block_info, Gas::new(DEFAULT_GAS_LIMIT), session_input_data)
                .unwrap();

        let payment: Option<ExecutableItem>;
        let payment_gas_limit: Gas;
        let payment_entry_point: String;
        let payment_args: RuntimeArgs;
        if session_input_data.is_standard_payment() {
            payment = None;
            payment_gas_limit = Gas::zero();
            payment_entry_point = DEFAULT_ENTRY_POINT_NAME.to_string();
            payment_args = RuntimeArgs::new();
        } else {
            let block_info = BlockInfo::new(
                Self::DEFAULT_STATE_HASH,
                BlockTime::new(DEFAULT_BLOCK_TIME),
                BlockHash::default(),
                0,
                DEFAULT_PROTOCOL_VERSION,
            );
            let request = WasmV1Request::new_custom_payment(
                block_info,
                Gas::new(DEFAULT_GAS_LIMIT),
                session_input_data,
            )
            .unwrap();
            payment = Some(request.executable_item);
            payment_gas_limit = request.gas_limit;
            payment_entry_point = request.entry_point;
            payment_args = request.args;
        }

        ExecuteRequestBuilder {
            state_hash: session.block_info.state_hash,
            block_time: session.block_info.block_time,
            block_height: session.block_info.block_height,
            parent_block_hash: session.block_info.parent_block_hash,
            protocol_version: session.block_info.protocol_version,
            transaction_hash: session.transaction_hash,
            initiator_addr: session.initiator_addr,
            payment,
            payment_gas_limit,
            payment_entry_point,
            payment_args,
            session: session.executable_item,
            session_gas_limit: session.gas_limit,
            session_entry_point: session.entry_point,
            session_args: session.args,
            authorization_keys,
        }
    }

    /// Converts a `DeployItem` into an `ExecuteRequestBuilder`.
    pub fn from_deploy_item(deploy_item: &DeployItem) -> Self {
        let authorization_keys = deploy_item.authorization_keys.clone();
        let block_info = BlockInfo::new(
            Self::DEFAULT_STATE_HASH,
            BlockTime::new(DEFAULT_BLOCK_TIME),
            BlockHash::default(),
            0,
            DEFAULT_PROTOCOL_VERSION,
        );
        let session = deploy_item
            .new_session_from_deploy_item(block_info, Gas::new(DEFAULT_GAS_LIMIT))
            .unwrap();

        let payment: Option<ExecutableItem>;
        let payment_gas_limit: Gas;
        let payment_entry_point: String;
        let payment_args: RuntimeArgs;
        if deploy_item.payment.is_standard_payment(Phase::Payment) {
            payment = None;
            payment_gas_limit = Gas::zero();
            payment_entry_point = DEFAULT_ENTRY_POINT_NAME.to_string();
            payment_args = RuntimeArgs::new();
        } else {
            let block_info = BlockInfo::new(
                Self::DEFAULT_STATE_HASH,
                BlockTime::new(DEFAULT_BLOCK_TIME),
                BlockHash::default(),
                0,
                DEFAULT_PROTOCOL_VERSION,
            );
            let request = deploy_item
                .new_custom_payment_from_deploy_item(block_info, Gas::new(DEFAULT_GAS_LIMIT))
                .unwrap();
            payment = Some(request.executable_item);
            payment_gas_limit = request.gas_limit;
            payment_entry_point = request.entry_point;
            payment_args = request.args;
        }

        ExecuteRequestBuilder {
            state_hash: session.block_info.state_hash,
            block_time: session.block_info.block_time,
            block_height: session.block_info.block_height,
            parent_block_hash: session.block_info.parent_block_hash,
            protocol_version: session.block_info.protocol_version,
            transaction_hash: session.transaction_hash,
            initiator_addr: session.initiator_addr,
            payment,
            payment_gas_limit,
            payment_entry_point,
            payment_args,
            session: session.executable_item,
            session_gas_limit: session.gas_limit,
            session_entry_point: session.entry_point,
            session_args: session.args,
            authorization_keys,
        }
    }

    /// Returns an [`ExecuteRequest`] derived from a deploy with standard dependencies.
    pub fn standard(
        account_hash: AccountHash,
        session_file: &str,
        session_args: RuntimeArgs,
    ) -> Self {
        let deploy_item = DeployItemBuilder::new()
            .with_address(account_hash)
            .with_session_code(session_file, session_args)
            .with_standard_payment(runtime_args! {
                ARG_AMOUNT => *DEFAULT_PAYMENT
            })
            .with_authorization_keys(&[account_hash])
            .build();
        Self::from_deploy_item(&deploy_item)
    }

    /// Returns an [`ExecuteRequest`] derived from a deploy with session module bytes.
    pub fn module_bytes(
        account_hash: AccountHash,
        module_bytes: Vec<u8>,
        session_args: RuntimeArgs,
    ) -> Self {
        let deploy_item = DeployItemBuilder::new()
            .with_address(account_hash)
            .with_session_bytes(module_bytes, session_args)
            .with_standard_payment(runtime_args! {
                ARG_AMOUNT => *DEFAULT_PAYMENT
            })
            .with_authorization_keys(&[account_hash])
            .build();
        Self::from_deploy_item(&deploy_item)
    }

    /// Returns an [`ExecuteRequest`] derived from a deploy with a session item that will call a
    /// stored contract by hash.
    pub fn contract_call_by_hash(
        sender: AccountHash,
        contract_hash: AddressableEntityHash,
        entry_point: &str,
        args: RuntimeArgs,
    ) -> Self {
        let deploy_item = DeployItemBuilder::new()
            .with_address(sender)
            .with_stored_session_hash(contract_hash, entry_point, args)
            .with_standard_payment(runtime_args! { ARG_AMOUNT => *DEFAULT_PAYMENT, })
            .with_authorization_keys(&[sender])
            .build();
        Self::from_deploy_item(&deploy_item)
    }

    /// Returns an [`ExecuteRequest`] derived from a deploy with a session item that will call a
    /// stored contract by name.
    pub fn contract_call_by_name(
        sender: AccountHash,
        contract_name: &str,
        entry_point: &str,
        args: RuntimeArgs,
    ) -> Self {
        let deploy_item = DeployItemBuilder::new()
            .with_address(sender)
            .with_stored_session_named_key(contract_name, entry_point, args)
            .with_standard_payment(runtime_args! { ARG_AMOUNT => *DEFAULT_PAYMENT, })
            .with_authorization_keys(&[sender])
            .build();
        Self::from_deploy_item(&deploy_item)
    }

    /// Returns an [`ExecuteRequest`] derived from a deploy with a session item that will call a
    /// versioned stored contract by hash.
    pub fn key_versioned_contract_call_by_hash(
        sender: AccountHash,
        contract_package_hash: PackageHash,
        version_key: Option<EntityVersionKey>,
        entry_point_name: &str,
        args: RuntimeArgs,
    ) -> Self {
        let initiator_addr = InitiatorAddr::AccountHash(sender);
        let target = TransactionTarget::Stored {
            id: TransactionInvocationTarget::ByPackageHash {
                addr: contract_package_hash.value(),
                version: None,
                version_key,
            },
            runtime: TransactionRuntimeParams::VmCasperV1,
        };
        let entry_point = TransactionEntryPoint::Custom(entry_point_name.to_owned());
        let hash = TransactionV1Hash::from_raw([1; 32]);
        let pricing_mode = PricingMode::PaymentLimited {
            payment_amount: DEFAULT_PAYMENT.as_u64(),
            gas_price_tolerance: 1,
            standard_payment: true,
        };
        let mut signers = BTreeSet::new();
        signers.insert(sender);
        let session_input_data = SessionInputData::SessionDataV1 {
            data: SessionDataV1::new(
                &args,
                &target,
                &entry_point,
                false,
                &hash,
                &pricing_mode,
                &initiator_addr,
                signers,
                pricing_mode.is_standard_payment(),
            ),
        };
        Self::from_session_input_data(&session_input_data)
    }

    /// Returns an [`ExecuteRequest`] derived from a deploy with a session item that will call a
    /// versioned stored contract by hash.
    pub fn versioned_contract_call_by_hash(
        sender: AccountHash,
        contract_package_hash: PackageHash,
        version: Option<EntityVersion>,
        entry_point_name: &str,
        args: RuntimeArgs,
    ) -> Self {
        let deploy_item = DeployItemBuilder::new()
            .with_address(sender)
            .with_stored_versioned_contract_by_hash(
                contract_package_hash.value(),
                version,
                entry_point_name,
                args,
            )
            .with_standard_payment(runtime_args! { ARG_AMOUNT => *DEFAULT_PAYMENT, })
            .with_authorization_keys(&[sender])
            .build();
        Self::from_deploy_item(&deploy_item)
    }

    /// Returns an [`ExecuteRequest`] derived from a deploy with a session item that will call a
    /// versioned stored contract by name.
    pub fn key_versioned_contract_call_by_name(
        sender: AccountHash,
        contract_name: &str,
        version_key: Option<EntityVersionKey>,
        entry_point_name: &str,
        args: RuntimeArgs,
    ) -> Self {
        let initiator_addr = InitiatorAddr::AccountHash(sender);
        let target = TransactionTarget::Stored {
            id: TransactionInvocationTarget::ByPackageName {
                name: contract_name.to_owned(),
                version: None,
                version_key,
            },
            runtime: TransactionRuntimeParams::VmCasperV1,
        };
        let entry_point = TransactionEntryPoint::Custom(entry_point_name.to_owned());
        let hash = TransactionV1Hash::from_raw([1; 32]);
        let pricing_mode = PricingMode::PaymentLimited {
            payment_amount: DEFAULT_PAYMENT.as_u64(),
            gas_price_tolerance: 1,
            standard_payment: true,
        };
        let mut signers = BTreeSet::new();
        signers.insert(sender);
        let session_input_data = SessionInputData::SessionDataV1 {
            data: SessionDataV1::new(
                &args,
                &target,
                &entry_point,
                false,
                &hash,
                &pricing_mode,
                &initiator_addr,
                signers,
                pricing_mode.is_standard_payment(),
            ),
        };
        Self::from_session_input_data(&session_input_data)
    }

    /// Returns an [`ExecuteRequest`] derived from a deploy with a session item that will call a
    /// versioned stored contract by name.
    pub fn versioned_contract_call_by_name(
        sender: AccountHash,
        contract_name: &str,
        version: Option<EntityVersion>,
        entry_point_name: &str,
        args: RuntimeArgs,
    ) -> Self {
        let deploy_item = DeployItemBuilder::new()
            .with_address(sender)
            .with_stored_versioned_contract_by_name(contract_name, version, entry_point_name, args)
            .with_standard_payment(runtime_args! { ARG_AMOUNT => *DEFAULT_PAYMENT, })
            .with_authorization_keys(&[sender])
            .build();
        Self::from_deploy_item(&deploy_item)
    }

    /// Sets the block time of the [`WasmV1Request`]s.
    pub fn with_block_time<T: Into<BlockTime>>(mut self, block_time: T) -> Self {
        self.block_time = block_time.into();
        self
    }

    /// Sets the block height of the [`WasmV1Request`]s.
    pub fn with_block_height(mut self, block_height: u64) -> Self {
        self.block_height = block_height;
        self
    }

    /// Sets the parent block hash of the [`WasmV1Request`]s.
    pub fn with_parent_block_hash(mut self, parent_block_hash: BlockHash) -> Self {
        self.parent_block_hash = parent_block_hash;
        self
    }

    /// Sets the parent block hash of the [`WasmV1Request`]s.
    pub fn with_state_hash(mut self, state_hash: Digest) -> Self {
        self.state_hash = state_hash;
        self
    }

    /// Sets the authorization keys used by the [`WasmV1Request`]s.
    pub fn with_authorization_keys(mut self, authorization_keys: BTreeSet<AccountHash>) -> Self {
        self.authorization_keys = authorization_keys;
        self
    }

    /// Consumes self and returns an `ExecuteRequest`.
    pub fn build(self) -> ExecuteRequest {
        let ExecuteRequestBuilder {
            state_hash,
            block_time,
            block_height,
            parent_block_hash,
            protocol_version,
            transaction_hash,
            initiator_addr,
            payment,
            payment_gas_limit,
            payment_entry_point,
            payment_args,
            session,
            session_gas_limit,
            session_entry_point,
            session_args,
            authorization_keys,
        } = self;

        let block_info = BlockInfo::new(
            state_hash,
            block_time,
            parent_block_hash,
            block_height,
            protocol_version,
        );
        let maybe_custom_payment = payment.map(|executable_item| WasmV1Request {
            block_info,
            transaction_hash,
            gas_limit: payment_gas_limit,
            initiator_addr: initiator_addr.clone(),
            executable_item,
            entry_point: payment_entry_point,
            args: payment_args,
            authorization_keys: authorization_keys.clone(),
            phase: Phase::Payment,
        });

        let session = WasmV1Request {
            block_info,
            transaction_hash,
            gas_limit: session_gas_limit,
            initiator_addr,
            executable_item: session,
            entry_point: session_entry_point,
            args: session_args,
            authorization_keys,
            phase: Phase::Session,
        };

        ExecuteRequest {
            session,
            custom_payment: maybe_custom_payment,
        }
    }
}
