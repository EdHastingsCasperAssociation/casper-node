use casper_engine_test_support::{
    ChainspecConfig, DeployItemBuilder, ExecuteRequest, ExecuteRequestBuilder, LmdbWasmTestBuilder,
    TransferRequestBuilder, DEFAULT_AUCTION_DELAY, DEFAULT_CHAINSPEC_REGISTRY,
    DEFAULT_GENESIS_CONFIG_HASH, DEFAULT_GENESIS_TIMESTAMP_MILLIS,
    DEFAULT_LOCKED_FUNDS_PERIOD_MILLIS, DEFAULT_PAYMENT, DEFAULT_PROTOCOL_VERSION,
    DEFAULT_ROUND_SEIGNIORAGE_RATE, DEFAULT_STORAGE_COSTS, DEFAULT_SYSTEM_CONFIG,
    DEFAULT_UNBONDING_DELAY, DEFAULT_VALIDATOR_SLOTS, DEFAULT_WASM_CONFIG,
};
use casper_execution_engine::{engine_state::Error, execution::ExecError};
use casper_storage::{data_access_layer::GenesisRequest, tracking_copy::TrackingCopyError};
use casper_types::{
    account::{AccountHash, Weight},
    bytesrepr::ToBytes,
    runtime_args,
    system::{
        auction::{self, DelegationRate},
        mint,
        standard_payment::{self, ARG_AMOUNT},
    },
    AddressableEntityHash, ApiError, CLType, CLValue, CoreConfig, EntityAddr, GenesisAccount, Key,
    Package, PackageHash, RuntimeArgs, U512,
};
use tempfile::TempDir;

use crate::{
    test::private_chain::{
        self, ACCOUNT_2_ADDR, ADMIN_1_ACCOUNT_ADDR, PRIVATE_CHAIN_ALLOW_AUCTION_BIDS,
        PRIVATE_CHAIN_COMPUTE_REWARDS, VALIDATOR_1_PUBLIC_KEY,
    },
    wasm_utils, GenesisConfigBuilder,
};

use super::{
    ACCOUNT_1_ADDR, ACCOUNT_1_PUBLIC_KEY, DEFAULT_ADMIN_ACCOUNT_ADDR,
    PRIVATE_CHAIN_DEFAULT_ACCOUNTS, PRIVATE_CHAIN_FEE_HANDLING,
    PRIVATE_CHAIN_GENESIS_ADMIN_ACCOUNTS, PRIVATE_CHAIN_GENESIS_ADMIN_SET,
    PRIVATE_CHAIN_REFUND_HANDLING,
};

const ADD_ASSOCIATED_KEY_CONTRACT: &str = "add_associated_key.wasm";
const REMOVE_ASSOCIATED_KEY_CONTRACT: &str = "remove_associated_key.wasm";
const SET_ACTION_THRESHOLDS_CONTRACT: &str = "set_action_thresholds.wasm";
const UPDATE_ASSOCIATED_KEY_CONTRACT: &str = "update_associated_key.wasm";
const DISABLE_CONTRACT: &str = "disable_contract.wasm";
const ENABLE_CONTRACT: &str = "enable_contract.wasm";
const TRANSFER_TO_ACCOUNT_CONTRACT: &&str = &"transfer_to_account.wasm";
const ARG_CONTRACT_PACKAGE_HASH: &str = "contract_package_hash";
const ARG_CONTRACT_HASH: &str = "contract_hash";

const ARG_ACCOUNT: &str = "account";
const ARG_WEIGHT: &str = "weight";

const ARG_KEY_MANAGEMENT_THRESHOLD: &str = "key_management_threshold";
const ARG_DEPLOY_THRESHOLD: &str = "deploy_threshold";
const DO_NOTHING_HASH_NAME: &str = "do_nothing_hash";

const DO_NOTHING_STORED_CONTRACT: &str = "do_nothing_stored.wasm";
const CALL_CONTRACT_PROXY: &str = "call_contract.wasm";
const DELEGATE_ENTRYPOINT: &str = "delegate";

const TEST_PAYMENT_STORED_CONTRACT: &str = "test_payment_stored.wasm";
const TEST_PAYMENT_STORED_HASH_NAME: &str = "test_payment_hash";
const PAY_ENTRYPOINT: &str = "pay";

#[should_panic(expected = "DuplicatedAdministratorEntry")]
#[ignore]
#[test]
fn should_not_run_genesis_with_duplicated_administrator_accounts() {
    let core_config = CoreConfig {
        administrators: PRIVATE_CHAIN_GENESIS_ADMIN_SET.clone(),
        ..Default::default()
    };
    let chainspec = ChainspecConfig {
        core_config,
        wasm_config: Default::default(),
        system_costs_config: Default::default(),
        storage_costs: Default::default(),
    };

    let data_dir = TempDir::new().expect("should create temp dir");
    let mut builder = LmdbWasmTestBuilder::new_with_config(data_dir.as_ref(), chainspec);

    let duplicated_administrator_accounts = {
        let mut accounts = PRIVATE_CHAIN_DEFAULT_ACCOUNTS.clone();

        let genesis_admins = PRIVATE_CHAIN_GENESIS_ADMIN_ACCOUNTS
            .clone()
            .into_iter()
            .map(GenesisAccount::from);
        accounts.extend(genesis_admins);
        accounts
    };

    let genesis_config = GenesisConfigBuilder::default()
        .with_accounts(duplicated_administrator_accounts)
        .with_wasm_config(*DEFAULT_WASM_CONFIG)
        .with_system_config(*DEFAULT_SYSTEM_CONFIG)
        .with_validator_slots(DEFAULT_VALIDATOR_SLOTS)
        .with_auction_delay(DEFAULT_AUCTION_DELAY)
        .with_locked_funds_period_millis(DEFAULT_LOCKED_FUNDS_PERIOD_MILLIS)
        .with_round_seigniorage_rate(DEFAULT_ROUND_SEIGNIORAGE_RATE)
        .with_unbonding_delay(DEFAULT_UNBONDING_DELAY)
        .with_genesis_timestamp_millis(DEFAULT_GENESIS_TIMESTAMP_MILLIS)
        .with_storage_costs(*DEFAULT_STORAGE_COSTS)
        .build();

    let modified_genesis_request = GenesisRequest::new(
        DEFAULT_GENESIS_CONFIG_HASH,
        DEFAULT_PROTOCOL_VERSION,
        genesis_config,
        DEFAULT_CHAINSPEC_REGISTRY.clone(),
    );

    builder.run_genesis(modified_genesis_request);
}

#[ignore]
#[test]
fn genesis_accounts_should_not_update_key_weight() {
    let mut builder = super::private_chain_setup();

    let exec_request_1 = {
        let session_args = runtime_args! {
            ARG_ACCOUNT => *ACCOUNT_1_ADDR,
            ARG_WEIGHT => Weight::MAX,
        };
        ExecuteRequestBuilder::standard(
            *ACCOUNT_1_ADDR,
            UPDATE_ASSOCIATED_KEY_CONTRACT,
            session_args,
        )
        .build()
    };

    builder.exec(exec_request_1).expect_failure().commit();

    let error = builder.get_error().expect("should have error");
    assert!(
        matches!(
            error,
            Error::Exec(ExecError::Revert(ApiError::PermissionDenied))
        ),
        "{:?}",
        error
    );

    let exec_request_2 = {
        let session_args = runtime_args! {
            ARG_ACCOUNT => *DEFAULT_ADMIN_ACCOUNT_ADDR,
            ARG_WEIGHT => Weight::new(1),
        };
        ExecuteRequestBuilder::standard(
            *ACCOUNT_1_ADDR,
            UPDATE_ASSOCIATED_KEY_CONTRACT,
            session_args,
        )
        .build()
    };

    builder.exec(exec_request_2).expect_failure().commit();
}

#[ignore]
#[test]
fn genesis_accounts_should_not_modify_action_thresholds() {
    let mut builder = super::private_chain_setup();

    let exec_request = {
        let session_args = runtime_args! {
            ARG_DEPLOY_THRESHOLD => Weight::new(1),
            ARG_KEY_MANAGEMENT_THRESHOLD => Weight::new(1),
        };
        ExecuteRequestBuilder::standard(
            *ACCOUNT_1_ADDR,
            SET_ACTION_THRESHOLDS_CONTRACT,
            session_args,
        )
        .build()
    };

    builder.exec(exec_request).expect_failure().commit();
    let error = builder.get_error().expect("should have error");
    assert!(
        matches!(
            error,
            Error::Exec(ExecError::Revert(ApiError::PermissionDenied))
        ),
        "{:?}",
        error
    );
}

#[ignore]
#[test]
fn genesis_accounts_should_not_add_associated_keys() {
    let secondary_account_hash = AccountHash::new([55; 32]);

    let mut builder = super::private_chain_setup();

    let exec_request = {
        let session_args = runtime_args! {
            ARG_ACCOUNT => secondary_account_hash,
            ARG_WEIGHT => Weight::MAX,
        };
        ExecuteRequestBuilder::standard(*ACCOUNT_1_ADDR, ADD_ASSOCIATED_KEY_CONTRACT, session_args)
            .build()
    };

    builder.exec(exec_request).expect_failure().commit();

    let error = builder.get_error().expect("should have error");
    assert!(
        matches!(
            error,
            Error::Exec(ExecError::Revert(ApiError::PermissionDenied))
        ),
        "{:?}",
        error
    );
}

#[ignore]
#[test]
fn genesis_accounts_should_not_remove_associated_keys() {
    let secondary_account_hash = AccountHash::new([55; 32]);

    let mut builder = super::private_chain_setup();

    let session_args = runtime_args! {
        ARG_ACCOUNT => secondary_account_hash,
        ARG_WEIGHT => Weight::MAX,
    };

    let account_hash = *ACCOUNT_1_ADDR;
    let deploy_hash: [u8; 32] = [55; 32];

    let deploy_item = DeployItemBuilder::new()
        .with_address(account_hash)
        .with_session_code(ADD_ASSOCIATED_KEY_CONTRACT, session_args)
        .with_standard_payment(runtime_args! {
            ARG_AMOUNT => *DEFAULT_PAYMENT
        })
        .with_authorization_keys(&[*ADMIN_1_ACCOUNT_ADDR])
        .with_deploy_hash(deploy_hash)
        .build();

    let add_associated_key_request = ExecuteRequestBuilder::from_deploy_item(&deploy_item).build();

    builder
        .exec(add_associated_key_request)
        .expect_success()
        .commit();

    let remove_associated_key_request = {
        let session_args = runtime_args! {
            ARG_ACCOUNT => secondary_account_hash,
        };
        ExecuteRequestBuilder::standard(
            *ACCOUNT_1_ADDR,
            REMOVE_ASSOCIATED_KEY_CONTRACT,
            session_args,
        )
        .build()
    };

    builder
        .exec(remove_associated_key_request)
        .expect_failure()
        .commit();

    let error = builder.get_error().expect("should have error");
    assert!(
        matches!(
            error,
            Error::Exec(ExecError::Revert(ApiError::PermissionDenied))
        ),
        "{:?}",
        error
    );
}

#[ignore]
#[test]
fn administrator_account_should_disable_any_account() {
    let mut builder = super::private_chain_setup();

    let account_1_genesis = builder
        .get_entity_by_account_hash(*ACCOUNT_1_ADDR)
        .expect("should have account 1 after genesis");

    // Account 1 can deploy after genesis
    let exec_request_1 = ExecuteRequestBuilder::module_bytes(
        *ACCOUNT_1_ADDR,
        wasm_utils::do_minimum_bytes(),
        RuntimeArgs::default(),
    )
    .build();
    builder.exec(exec_request_1).expect_success().commit();

    // Disable account 1
    let session_args = runtime_args! {
        ARG_DEPLOY_THRESHOLD => Weight::MAX,
        ARG_KEY_MANAGEMENT_THRESHOLD => Weight::MAX,
    };

    let sender = *ACCOUNT_1_ADDR;
    let deploy_hash = [54; 32];

    // Here, deploy is sent as an account, but signed by an administrator.
    let deploy_item = DeployItemBuilder::new()
        .with_address(sender)
        .with_session_code(SET_ACTION_THRESHOLDS_CONTRACT, session_args)
        .with_standard_payment(runtime_args! { ARG_AMOUNT => *DEFAULT_PAYMENT, })
        .with_authorization_keys(&[*DEFAULT_ADMIN_ACCOUNT_ADDR])
        .with_deploy_hash(deploy_hash)
        .build();

    let disable_request_1 = ExecuteRequestBuilder::from_deploy_item(&deploy_item).build();

    builder.exec(disable_request_1).expect_success().commit();
    // Account 1 can not deploy after freezing
    let exec_request_2 = ExecuteRequestBuilder::module_bytes(
        *ACCOUNT_1_ADDR,
        wasm_utils::do_minimum_bytes(),
        RuntimeArgs::default(),
    )
    .build();
    builder.exec(exec_request_2).expect_failure().commit();

    let error = builder.get_error().expect("should have error");
    assert!(matches!(
        error,
        Error::TrackingCopy(TrackingCopyError::DeploymentAuthorizationFailure)
    ));

    let account_1_disabled = builder
        .get_entity_by_account_hash(*ACCOUNT_1_ADDR)
        .expect("should have account 1 after genesis");
    assert_ne!(
        account_1_genesis, account_1_disabled,
        "account 1 should be modified"
    );

    // Unfreeze account 1
    let session_args = runtime_args! {
        ARG_DEPLOY_THRESHOLD => Weight::new(1),
        ARG_KEY_MANAGEMENT_THRESHOLD => Weight::new(0),
    };

    let sender = *ACCOUNT_1_ADDR;
    let deploy_hash = [53; 32];

    // Here, deploy is sent as an account, but signed by an administrator.
    let deploy_item = DeployItemBuilder::new()
        .with_address(sender)
        .with_session_code(SET_ACTION_THRESHOLDS_CONTRACT, session_args)
        .with_standard_payment(runtime_args! { ARG_AMOUNT => *DEFAULT_PAYMENT, })
        .with_authorization_keys(&[*DEFAULT_ADMIN_ACCOUNT_ADDR])
        .with_deploy_hash(deploy_hash)
        .build();

    let enable_request_1 = ExecuteRequestBuilder::from_deploy_item(&deploy_item).build();

    let session_args = runtime_args! {
        ARG_DEPLOY_THRESHOLD => Weight::new(0),
        ARG_KEY_MANAGEMENT_THRESHOLD => Weight::new(1),
    };

    let sender = *ACCOUNT_1_ADDR;
    let deploy_hash = [52; 32];

    // Here, deploy is sent as an account, but signed by an administrator.
    let deploy_item = DeployItemBuilder::new()
        .with_address(sender)
        .with_session_code(SET_ACTION_THRESHOLDS_CONTRACT, session_args)
        .with_standard_payment(runtime_args! { ARG_AMOUNT => *DEFAULT_PAYMENT, })
        .with_authorization_keys(&[*DEFAULT_ADMIN_ACCOUNT_ADDR])
        .with_deploy_hash(deploy_hash)
        .build();

    let enable_request_2 = ExecuteRequestBuilder::from_deploy_item(&deploy_item).build();

    builder.exec(enable_request_1).expect_success().commit();
    builder.exec(enable_request_2).expect_success().commit();

    // Account 1 can deploy after unfreezing
    let exec_request_3 = ExecuteRequestBuilder::module_bytes(
        *ACCOUNT_1_ADDR,
        wasm_utils::do_minimum_bytes(),
        RuntimeArgs::default(),
    )
    .build();
    builder.exec(exec_request_3).expect_success().commit();

    let account_1_unfrozen = builder
        .get_entity_by_account_hash(*ACCOUNT_1_ADDR)
        .expect("should have account 1 after genesis");
    assert_eq!(
        account_1_genesis, account_1_unfrozen,
        "account 1 should be modified back to genesis state"
    );
}

#[ignore]
#[test]
fn native_transfer_should_create_new_private_account() {
    let mut builder = super::private_chain_setup();

    // Account 1 can deploy after genesis
    let transfer_request = TransferRequestBuilder::new(1, *ACCOUNT_2_ADDR)
        .with_initiator(*DEFAULT_ADMIN_ACCOUNT_ADDR)
        .build();

    builder
        .transfer_and_commit(transfer_request)
        .expect_success();

    let _account_2 = builder
        .get_entity_by_account_hash(*ACCOUNT_2_ADDR)
        .expect("should have account 1 after transfer");
}

#[ignore]
#[test]
fn wasm_transfer_should_create_new_private_account() {
    let mut builder = super::private_chain_setup();

    // Account 1 can deploy after genesis
    let transfer_args = runtime_args! {
        mint::ARG_TARGET => *ACCOUNT_2_ADDR,
        mint::ARG_AMOUNT => 1u64,
    };
    let transfer_request = ExecuteRequestBuilder::standard(
        *DEFAULT_ADMIN_ACCOUNT_ADDR,
        TRANSFER_TO_ACCOUNT_CONTRACT,
        transfer_args,
    )
    .build();

    builder.exec(transfer_request).expect_success().commit();

    let _account_2 = builder
        .get_entity_by_account_hash(*ACCOUNT_2_ADDR)
        .expect("should have account 1 after genesis");
}

#[ignore]
#[test]
fn administrator_account_should_disable_any_contract_used_as_session() {
    let mut builder = super::private_chain_setup();

    let store_contract_request = ExecuteRequestBuilder::standard(
        *ACCOUNT_1_ADDR,
        DO_NOTHING_STORED_CONTRACT,
        RuntimeArgs::default(),
    )
    .build();

    builder
        .exec(store_contract_request)
        .expect_success()
        .commit();

    let account_1_genesis = builder
        .get_entity_with_named_keys_by_account_hash(*ACCOUNT_1_ADDR)
        .expect("should have account 1 after genesis");

    let stored_entity_key = account_1_genesis
        .named_keys()
        .get(DO_NOTHING_HASH_NAME)
        .unwrap();

    let stored_entity_hash = stored_entity_key
        .into_entity_hash_addr()
        .map(AddressableEntityHash::new)
        .expect("should have stored contract hash");

    let do_nothing_contract_package_key = {
        let addressable_entity = builder
            .get_addressable_entity(stored_entity_hash)
            .expect("should be entity");
        Key::Hash(addressable_entity.package_hash().value())
    };

    let contract_package_before = Package::try_from(
        builder
            .query(None, do_nothing_contract_package_key, &[])
            .expect("should query"),
    )
    .expect("should be contract package");

    let stored_entity_addr = stored_entity_key
        .into_hash_addr()
        .map(EntityAddr::SmartContract)
        .expect("must get entity addr");

    assert!(
        contract_package_before.is_entity_enabled(&stored_entity_addr),
        "newly stored contract should be enabled"
    );

    // Account 1 can deploy after genesis
    let exec_request_1 = ExecuteRequestBuilder::contract_call_by_name(
        *ACCOUNT_1_ADDR,
        DO_NOTHING_HASH_NAME,
        DELEGATE_ENTRYPOINT,
        RuntimeArgs::default(),
    )
    .build();
    builder.exec(exec_request_1).expect_success().commit();

    let do_nothing_contract_package_hash =
        PackageHash::new(do_nothing_contract_package_key.into_hash_addr().unwrap());

    // Disable stored contract
    let disable_request = {
        let session_args = runtime_args! {
            ARG_CONTRACT_PACKAGE_HASH => do_nothing_contract_package_hash,
            ARG_CONTRACT_HASH => stored_entity_hash,
        };

        ExecuteRequestBuilder::standard(*DEFAULT_ADMIN_ACCOUNT_ADDR, DISABLE_CONTRACT, session_args)
            .build()
    };

    builder.exec(disable_request).expect_success().commit();

    let contract_package_after_disable = Package::try_from(
        builder
            .query(None, do_nothing_contract_package_key, &[])
            .expect("should query"),
    )
    .expect("should be contract package");

    assert_ne!(
        contract_package_before, contract_package_after_disable,
        "contract package should be disabled"
    );
    assert!(!contract_package_after_disable.is_entity_enabled(&stored_entity_addr),);

    let call_delegate_requests_1 = {
        // Unable to call disabled stored contract directly
        let call_delegate_by_name = ExecuteRequestBuilder::contract_call_by_name(
            *ACCOUNT_1_ADDR,
            DO_NOTHING_HASH_NAME,
            DELEGATE_ENTRYPOINT,
            RuntimeArgs::default(),
        )
        .build();

        let call_delegate_by_hash = ExecuteRequestBuilder::contract_call_by_hash(
            *ACCOUNT_1_ADDR,
            stored_entity_hash,
            DELEGATE_ENTRYPOINT,
            RuntimeArgs::default(),
        )
        .build();

        let call_delegate_from_wasm = make_call_contract_session_request(
            *ACCOUNT_1_ADDR,
            stored_entity_hash,
            DELEGATE_ENTRYPOINT,
            RuntimeArgs::default(),
        );

        vec![
            call_delegate_by_name,
            call_delegate_by_hash,
            call_delegate_from_wasm,
        ]
    };

    for call_delegate_request in call_delegate_requests_1 {
        builder
            .exec(call_delegate_request)
            .expect_failure()
            .commit();
        let error = builder.get_error().expect("should have error");
        assert!(
            matches!(
                error,
                Error::Exec(ExecError::DisabledEntity(disabled_contract_hash))
                if disabled_contract_hash == stored_entity_hash
            ),
            "expected disabled contract error, found {:?}",
            error
        );
    }

    // Enable stored contract
    let enable_request = {
        let session_args = runtime_args! {
            ARG_CONTRACT_PACKAGE_HASH => do_nothing_contract_package_hash,
            ARG_CONTRACT_HASH => stored_entity_hash,
        };

        ExecuteRequestBuilder::standard(*DEFAULT_ADMIN_ACCOUNT_ADDR, ENABLE_CONTRACT, session_args)
            .build()
    };

    builder.exec(enable_request).expect_success().commit();

    let call_delegate_requests_2 = {
        // Unable to call disabled stored contract directly
        let call_delegate_by_name = ExecuteRequestBuilder::contract_call_by_name(
            *ACCOUNT_1_ADDR,
            DO_NOTHING_HASH_NAME,
            DELEGATE_ENTRYPOINT,
            RuntimeArgs::default(),
        )
        .build();

        let call_delegate_by_hash = ExecuteRequestBuilder::contract_call_by_hash(
            *ACCOUNT_1_ADDR,
            stored_entity_hash,
            DELEGATE_ENTRYPOINT,
            RuntimeArgs::default(),
        )
        .build();

        let call_delegate_from_wasm = make_call_contract_session_request(
            *ACCOUNT_1_ADDR,
            stored_entity_hash,
            DELEGATE_ENTRYPOINT,
            RuntimeArgs::default(),
        );

        vec![
            call_delegate_by_name,
            call_delegate_by_hash,
            call_delegate_from_wasm,
        ]
    };

    for exec_request in call_delegate_requests_2 {
        builder.exec(exec_request).expect_success().commit();
    }
}

#[ignore]
#[test]
fn administrator_account_should_disable_any_contract_used_as_payment() {
    // We'll simulate enabled unrestricted transfers here to test if stored payment contract is
    // disabled.
    let mut builder = private_chain::custom_setup_genesis_only(
        PRIVATE_CHAIN_ALLOW_AUCTION_BIDS,
        true,
        PRIVATE_CHAIN_REFUND_HANDLING,
        PRIVATE_CHAIN_FEE_HANDLING,
        PRIVATE_CHAIN_COMPUTE_REWARDS,
    );

    let store_contract_request = ExecuteRequestBuilder::standard(
        *ACCOUNT_1_ADDR,
        TEST_PAYMENT_STORED_CONTRACT,
        RuntimeArgs::default(),
    )
    .build();

    builder
        .exec(store_contract_request)
        .expect_success()
        .commit();

    let account_1_genesis = builder
        .get_entity_with_named_keys_by_account_hash(*ACCOUNT_1_ADDR)
        .expect("should have account 1 after genesis");

    let stored_entity_key = account_1_genesis
        .named_keys()
        .get(TEST_PAYMENT_STORED_HASH_NAME)
        .unwrap();

    let stored_entity_hash = stored_entity_key
        .into_entity_hash_addr()
        .map(AddressableEntityHash::new)
        .expect("should have stored entity hash");

    let addressable_entity = builder
        .get_addressable_entity(stored_entity_hash)
        .expect("should be addressable entity");
    let test_payment_stored_package_key = { Key::Hash(addressable_entity.package_hash().value()) };

    let test_payment_stored_package_hash =
        PackageHash::new(addressable_entity.package_hash().value());

    let contract_package_before = Package::try_from(
        builder
            .query(None, test_payment_stored_package_key, &[])
            .expect("should query"),
    )
    .expect("should be contract package");
    let stored_entity_addr = stored_entity_key
        .into_entity_addr()
        .expect("must get entity addr");
    assert!(
        contract_package_before.is_entity_enabled(&stored_entity_addr),
        "newly stored contract should be enabled"
    );

    // Account 1 can deploy after genesis
    let sender = *ACCOUNT_1_ADDR;
    let deploy_hash = [100; 32];

    let payment_args = runtime_args! {
        standard_payment::ARG_AMOUNT => *DEFAULT_PAYMENT,
    };
    let session_args = RuntimeArgs::default();

    let deploy_item = DeployItemBuilder::new()
        .with_address(sender)
        .with_session_bytes(wasm_utils::do_minimum_bytes(), session_args)
        .with_stored_payment_named_key(TEST_PAYMENT_STORED_HASH_NAME, PAY_ENTRYPOINT, payment_args)
        .with_authorization_keys(&[sender])
        .with_deploy_hash(deploy_hash)
        .build();
    let exec_request_1 = ExecuteRequestBuilder::from_deploy_item(&deploy_item).build();

    builder.exec(exec_request_1).expect_failure();

    // Disable payment contract
    let disable_request = {
        let session_args = runtime_args! {
            ARG_CONTRACT_PACKAGE_HASH => test_payment_stored_package_hash,
            ARG_CONTRACT_HASH => stored_entity_hash,
        };

        ExecuteRequestBuilder::standard(*DEFAULT_ADMIN_ACCOUNT_ADDR, DISABLE_CONTRACT, session_args)
            .build()
    };

    builder.exec(disable_request).expect_success().commit();

    let contract_package_after_disable = Package::try_from(
        builder
            .query(None, test_payment_stored_package_key, &[])
            .expect("should query"),
    )
    .expect("should be contract package");

    assert_ne!(
        contract_package_before, contract_package_after_disable,
        "contract package should be disabled"
    );
    assert!(!contract_package_after_disable.is_entity_enabled(&stored_entity_addr),);

    let payment_args = runtime_args! {
        standard_payment::ARG_AMOUNT => *DEFAULT_PAYMENT,
    };
    let session_args = RuntimeArgs::default();

    let sender = *ACCOUNT_1_ADDR;
    let deploy_hash = [100; 32];

    let deploy_item = DeployItemBuilder::new()
        .with_address(sender)
        .with_session_bytes(wasm_utils::do_minimum_bytes(), session_args.clone())
        .with_stored_payment_named_key(
            TEST_PAYMENT_STORED_HASH_NAME,
            PAY_ENTRYPOINT,
            payment_args.clone(),
        )
        .with_authorization_keys(&[sender])
        .with_deploy_hash(deploy_hash)
        .build();
    let call_by_name = ExecuteRequestBuilder::from_deploy_item(&deploy_item).build();

    let deploy_item = DeployItemBuilder::new()
        .with_address(sender)
        .with_session_bytes(wasm_utils::do_minimum_bytes(), session_args)
        .with_stored_payment_hash(stored_entity_hash, PAY_ENTRYPOINT, payment_args)
        .with_authorization_keys(&[sender])
        .with_deploy_hash(deploy_hash)
        .build();
    let call_by_hash = ExecuteRequestBuilder::from_deploy_item(&deploy_item).build();

    for execute_request in [call_by_name, call_by_hash] {
        builder.exec(execute_request).expect_failure().commit();
        let error = builder.get_error().expect("should have error");
        assert!(
            matches!(
                error,
                Error::Exec(ExecError::DisabledEntity(disabled_contract_hash))
                if disabled_contract_hash == stored_entity_hash
            ),
            "expected disabled contract error, found {:?}",
            error
        );
    }

    // Enable stored contract
    let enable_request = {
        let session_args = runtime_args! {
            ARG_CONTRACT_PACKAGE_HASH => test_payment_stored_package_hash,
            ARG_CONTRACT_HASH => stored_entity_hash,
        };

        ExecuteRequestBuilder::standard(*DEFAULT_ADMIN_ACCOUNT_ADDR, ENABLE_CONTRACT, session_args)
            .build()
    };

    builder.exec(enable_request).expect_success().commit();

    let payment_args = runtime_args! { ARG_AMOUNT => *DEFAULT_PAYMENT };
    let session_args = RuntimeArgs::default();
    let sender = *ACCOUNT_1_ADDR;
    let deploy_hash = [100; 32];
    let deploy_item = DeployItemBuilder::new()
        .with_address(sender)
        .with_session_bytes(wasm_utils::do_minimum_bytes(), session_args.clone())
        .with_stored_payment_named_key(
            TEST_PAYMENT_STORED_HASH_NAME,
            PAY_ENTRYPOINT,
            payment_args.clone(),
        )
        .with_authorization_keys(&[sender])
        .with_deploy_hash(deploy_hash)
        .build();
    let call_by_name = ExecuteRequestBuilder::from_deploy_item(&deploy_item).build();

    let deploy_item = DeployItemBuilder::new()
        .with_address(sender)
        .with_session_bytes(wasm_utils::do_minimum_bytes(), session_args)
        .with_stored_payment_hash(stored_entity_hash, PAY_ENTRYPOINT, payment_args)
        .with_authorization_keys(&[sender])
        .with_deploy_hash(deploy_hash)
        .build();
    let call_by_hash = ExecuteRequestBuilder::from_deploy_item(&deploy_item).build();

    for exec_request in [call_by_name, call_by_hash] {
        builder.exec(exec_request).expect_failure();
    }
}

#[ignore]
#[test]
fn should_not_allow_add_bid_on_private_chain() {
    let mut builder = super::private_chain_setup();

    let delegation_rate: DelegationRate = 4;
    let session_args = runtime_args! {
        auction::ARG_PUBLIC_KEY => ACCOUNT_1_PUBLIC_KEY.clone(),
        auction::ARG_AMOUNT => U512::one(),
        auction::ARG_DELEGATION_RATE => delegation_rate,
    };

    let exec_request =
        ExecuteRequestBuilder::standard(*ACCOUNT_1_ADDR, "add_bid.wasm", session_args).build();

    builder.exec(exec_request).expect_failure().commit();

    let error = builder.get_error().expect("should have error");

    assert!(
        matches!(
            error,
            Error::Exec(ExecError::Revert(api_error))
            if api_error == auction::Error::AuctionBidsDisabled.into(),
        ),
        "{:?}",
        error,
    );
}

#[ignore]
#[test]
fn should_not_allow_delegate_on_private_chain() {
    let mut builder = super::private_chain_setup();

    let session_args = runtime_args! {
        auction::ARG_DELEGATOR => ACCOUNT_1_PUBLIC_KEY.clone(),
        auction::ARG_VALIDATOR => VALIDATOR_1_PUBLIC_KEY.clone(),
        auction::ARG_AMOUNT => U512::one(),
    };

    let exec_request =
        ExecuteRequestBuilder::standard(*ACCOUNT_1_ADDR, "delegate.wasm", session_args).build();

    builder.exec(exec_request).expect_failure().commit();

    let error = builder.get_error().expect("should have error");

    assert!(
        matches!(
            error,
            Error::Exec(ExecError::Revert(api_error))
            if api_error == auction::Error::AuctionBidsDisabled.into()
        ),
        "{:?}",
        error
    );
    // Redelegation would not work since delegate, and add_bid are disabled on private chains
    // therefore there is nothing to test.
}

fn make_call_contract_session_request(
    account_hash: AccountHash,
    contract_hash: AddressableEntityHash,
    entrypoint: &str,
    arguments: RuntimeArgs,
) -> ExecuteRequest {
    let arguments_any = {
        let arg_bytes = arguments.to_bytes().unwrap();
        CLValue::from_components(CLType::Any, arg_bytes)
    };

    let mut session_args = runtime_args! {
        "entrypoint" => entrypoint,
        "contract_hash" => contract_hash,
    };
    session_args.insert_cl_value("arguments", arguments_any);

    ExecuteRequestBuilder::standard(account_hash, CALL_CONTRACT_PROXY, session_args).build()
}
