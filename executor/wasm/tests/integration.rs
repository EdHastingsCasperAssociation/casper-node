use std::{fs::File, path::Path, sync::Arc};

use bytes::Bytes;
use casper_execution_engine::engine_state::ExecutionEngineV1;
use casper_executor_wasm::{
    install::{
        InstallContractError, InstallContractRequest, InstallContractRequestBuilder,
        InstallContractResult,
    },
    ExecutorConfigBuilder, ExecutorKind, ExecutorV2,
};
use casper_executor_wasm_interface::{
    executor::{ExecuteRequest, ExecuteRequestBuilder, ExecuteWithProviderResult, ExecutionKind},
    HostError,
};
use casper_storage::{
    data_access_layer::{
        prefixed_values::{PrefixedValuesRequest, PrefixedValuesResult},
        GenesisRequest, GenesisResult, MessageTopicsRequest, MessageTopicsResult, QueryRequest,
        QueryResult,
    },
    global_state::{
        self,
        state::{lmdb::LmdbGlobalState, CommitProvider, StateProvider},
        transaction_source::lmdb::LmdbEnvironment,
        trie_store::lmdb::LmdbTrieStore,
    },
    system::runtime_native::Id,
    AddressGenerator, KeyPrefix,
};
use casper_types::{
    account::AccountHash, BlockHash, ChainspecRegistry, Digest, GenesisAccount, GenesisConfig,
    HostFunction, HostFunctionCostsV2, Key, Motes, Phase, ProtocolVersion, PublicKey, SecretKey,
    StorageCosts, StoredValue, SystemConfig, Timestamp, TransactionHash, TransactionV1Hash,
    WasmConfig, WasmV2Config, U512,
};
use fs_extra::dir;
use itertools::Itertools;
use once_cell::sync::Lazy;
use parking_lot::RwLock;
use tempfile::TempDir;

static DEFAULT_ACCOUNT_SECRET_KEY: Lazy<SecretKey> =
    Lazy::new(|| SecretKey::ed25519_from_bytes([199; SecretKey::ED25519_LENGTH]).unwrap());
static DEFAULT_ACCOUNT_PUBLIC_KEY: Lazy<casper_types::PublicKey> =
    Lazy::new(|| PublicKey::from(&*DEFAULT_ACCOUNT_SECRET_KEY));
static DEFAULT_ACCOUNT_HASH: Lazy<AccountHash> =
    Lazy::new(|| DEFAULT_ACCOUNT_PUBLIC_KEY.to_account_hash());

const CSPR: u64 = 10u64.pow(9);

const VM2_HARNESS: Bytes = Bytes::from_static(include_bytes!("../vm2-harness.wasm"));
const VM2_CEP18: Bytes = Bytes::from_static(include_bytes!("../vm2_cep18.wasm"));
const VM2_LEGACY_COUNTER_PROXY: Bytes =
    Bytes::from_static(include_bytes!("../vm2_legacy_counter_proxy.wasm"));
const VM2_CEP18_CALLER: Bytes = Bytes::from_static(include_bytes!("../vm2-cep18-caller.wasm"));
const VM2_TRAIT: Bytes = Bytes::from_static(include_bytes!("../vm2_trait.wasm"));
const VM2_FLIPPER: Bytes = Bytes::from_static(include_bytes!("../vm2_flipper.wasm"));
const VM2_UPGRADABLE: Bytes = Bytes::from_static(include_bytes!("../vm2_upgradable.wasm"));
const VM2_UPGRADABLE_V2: Bytes = Bytes::from_static(include_bytes!("../vm2_upgradable_v2.wasm"));

const VM2_HOST: Bytes = Bytes::from_static(include_bytes!("../vm2_host.wasm"));

const TRANSACTION_HASH_BYTES: [u8; 32] = [55; 32];
const TRANSACTION_HASH: TransactionHash =
    TransactionHash::V1(TransactionV1Hash::from_raw(TRANSACTION_HASH_BYTES));

const DEFAULT_GAS_LIMIT: u64 = 100_000_000_000_000;
const DEFAULT_CHAIN_NAME: &str = "casper-test";

fn make_address_generator() -> Arc<RwLock<AddressGenerator>> {
    let id = Id::Transaction(TRANSACTION_HASH);
    Arc::new(RwLock::new(AddressGenerator::new(
        &id.seed(),
        Phase::Session,
    )))
}

fn base_execute_builder() -> ExecuteRequestBuilder {
    ExecuteRequestBuilder::default()
        .with_initiator(*DEFAULT_ACCOUNT_HASH)
        .with_caller_key(Key::Account(*DEFAULT_ACCOUNT_HASH))
        .with_gas_limit(DEFAULT_GAS_LIMIT)
        .with_transferred_value(1000)
        .with_transaction_hash(TRANSACTION_HASH)
        .with_chain_name(DEFAULT_CHAIN_NAME)
        .with_block_time(Timestamp::now().into())
        .with_state_hash(Digest::hash(b"state"))
        .with_block_height(1)
        .with_parent_block_hash(BlockHash::new(Digest::hash(b"block1")))
}

fn base_install_request_builder() -> InstallContractRequestBuilder {
    InstallContractRequestBuilder::default()
        .with_initiator(*DEFAULT_ACCOUNT_HASH)
        .with_gas_limit(1_000_000)
        .with_transaction_hash(TRANSACTION_HASH)
        .with_chain_name(DEFAULT_CHAIN_NAME)
        .with_block_time(Timestamp::now().into())
        .with_state_hash(Digest::hash(b"state"))
        .with_block_height(1)
        .with_parent_block_hash(BlockHash::new(Digest::hash(b"block1")))
}

#[test]
fn harness() {
    let mut executor = make_executor();

    let (mut global_state, mut state_root_hash, _tempdir) = make_global_state_with_genesis();

    let address_generator = make_address_generator();

    let flipper_address;

    state_root_hash = {
        let input_data = borsh::to_vec(&("Foo Token".to_string(),))
            .map(Bytes::from)
            .unwrap();

        let install_request = base_install_request_builder()
            .with_wasm_bytes(VM2_CEP18.clone())
            .with_shared_address_generator(Arc::clone(&address_generator))
            .with_transferred_value(0)
            .with_entry_point("new".to_string())
            .with_input(input_data)
            .build()
            .expect("should build");

        let create_result = run_create_contract(
            &mut executor,
            &mut global_state,
            state_root_hash,
            install_request,
        );

        flipper_address = *create_result.smart_contract_addr();

        global_state
            .commit_effects(state_root_hash, create_result.effects().clone())
            .expect("Should commit")
    };

    let execute_request = ExecuteRequestBuilder::default()
        .with_initiator(*DEFAULT_ACCOUNT_HASH)
        .with_caller_key(Key::Account(*DEFAULT_ACCOUNT_HASH))
        .with_gas_limit(DEFAULT_GAS_LIMIT)
        .with_transferred_value(1000)
        .with_transaction_hash(TRANSACTION_HASH)
        .with_target(ExecutionKind::SessionBytes(VM2_HARNESS))
        .with_serialized_input((flipper_address,))
        .with_shared_address_generator(address_generator)
        .with_chain_name(DEFAULT_CHAIN_NAME)
        .with_block_time(Timestamp::now().into())
        .with_state_hash(state_root_hash)
        .with_block_height(1)
        .with_parent_block_hash(BlockHash::new(Digest::hash(b"bl0ck")))
        .build()
        .expect("should build");

    run_wasm_session(
        &mut executor,
        &mut global_state,
        state_root_hash,
        execute_request,
    );
}

pub(crate) fn make_executor() -> ExecutorV2 {
    let execution_engine_v1 = ExecutionEngineV1::default();
    let executor_config = ExecutorConfigBuilder::default()
        .with_memory_limit(17)
        .with_executor_kind(ExecutorKind::Compiled)
        .with_wasm_config(WasmV2Config::default())
        .with_storage_costs(StorageCosts::default())
        .build()
        .expect("Should build");
    ExecutorV2::new(executor_config, Arc::new(execution_engine_v1))
}

#[test]
fn cep18() {
    let mut executor = make_executor();

    let (mut global_state, mut state_root_hash, _tempdir) = make_global_state_with_genesis();

    let address_generator = make_address_generator();

    let input_data = borsh::to_vec(&("Foo Token".to_string(),))
        .map(Bytes::from)
        .unwrap();

    let block_time_1 = Timestamp::now().into();

    let create_request = InstallContractRequestBuilder::default()
        .with_initiator(*DEFAULT_ACCOUNT_HASH)
        .with_gas_limit(1_000_000)
        .with_transaction_hash(TRANSACTION_HASH)
        .with_wasm_bytes(VM2_CEP18.clone())
        .with_shared_address_generator(Arc::clone(&address_generator))
        .with_transferred_value(0)
        .with_entry_point("new".to_string())
        .with_input(input_data)
        .with_chain_name(DEFAULT_CHAIN_NAME)
        .with_block_time(block_time_1)
        .with_state_hash(Digest::from_raw([0; 32])) // TODO: Carry on state root hash
        .with_block_height(1) // TODO: Carry on block height
        .with_parent_block_hash(BlockHash::new(Digest::from_raw([0; 32]))) // TODO: Carry on parent block hash
        .build()
        .expect("should build");

    let create_result = run_create_contract(
        &mut executor,
        &mut global_state,
        state_root_hash,
        create_request,
    );

    let contract_hash = create_result.smart_contract_addr();

    state_root_hash = global_state
        .commit_effects(state_root_hash, create_result.effects().clone())
        .expect("Should commit");

    let msgs = global_state.prefixed_values(PrefixedValuesRequest::new(
        state_root_hash,
        KeyPrefix::MessageEntriesByEntity(*contract_hash),
    ));
    let PrefixedValuesResult::Success {
        key_prefix: _,
        values,
    } = msgs
    else {
        panic!("Expected success")
    };

    {
        let mut topics_1 = values
            .iter()
            .filter_map(|stored_value| stored_value.as_message_topic_summary())
            .collect_vec();
        topics_1
            .sort_by_key(|topic| (topic.topic_name(), topic.blocktime(), topic.message_count()));

        assert_eq!(topics_1[0].topic_name(), "Transfer");
        assert_eq!(topics_1[0].message_count(), 1);
        assert_eq!(topics_1[0].blocktime(), block_time_1);
    }

    let block_time_2 = (block_time_1.value() + 1).into();
    assert_ne!(block_time_1, block_time_2);

    let execute_request = ExecuteRequestBuilder::default()
        .with_initiator(*DEFAULT_ACCOUNT_HASH)
        .with_caller_key(Key::Account(*DEFAULT_ACCOUNT_HASH))
        .with_gas_limit(DEFAULT_GAS_LIMIT)
        .with_transferred_value(1000)
        .with_transaction_hash(TRANSACTION_HASH)
        .with_target(ExecutionKind::SessionBytes(VM2_CEP18_CALLER))
        .with_serialized_input((create_result.smart_contract_addr(),))
        .with_transferred_value(0)
        .with_shared_address_generator(Arc::clone(&address_generator))
        .with_chain_name(DEFAULT_CHAIN_NAME)
        .with_block_time(block_time_2)
        .with_state_hash(Digest::from_raw([0; 32])) // TODO: Carry on state root hash
        .with_block_height(2) // TODO: Carry on block height
        .with_parent_block_hash(BlockHash::new(Digest::from_raw([0; 32]))) // TODO: Carry on parent block hash
        .build()
        .expect("should build");

    let result_2 = run_wasm_session(
        &mut executor,
        &mut global_state,
        state_root_hash,
        execute_request,
    );

    state_root_hash = global_state
        .commit_effects(state_root_hash, result_2.effects().clone())
        .expect("Should commit");

    let MessageTopicsResult::Success { message_topics } =
        global_state.message_topics(MessageTopicsRequest::new(state_root_hash, *contract_hash))
    else {
        panic!("Expected success")
    };

    assert!(matches!(message_topics.get("Transfer"), Some(_)));
    assert_ne!(
        message_topics.get("Mint"),
        message_topics.get("Transfer"),
        "Mint and Transfer topics should have different hashes"
    );

    {
        let msgs = global_state.prefixed_values(PrefixedValuesRequest::new(
            state_root_hash,
            KeyPrefix::MessageEntriesByEntity(*contract_hash),
        ));
        let PrefixedValuesResult::Success {
            key_prefix: _,
            values,
        } = msgs
        else {
            panic!("Expected success")
        };

        let mut topics_2 = values
            .iter()
            .filter_map(|stored_value| stored_value.as_message_topic_summary())
            .collect_vec();
        topics_2
            .sort_by_key(|topic| (topic.topic_name(), topic.blocktime(), topic.message_count()));

        assert_eq!(topics_2.len(), 1);
        assert_eq!(topics_2[0].topic_name(), "Transfer");
        assert_eq!(topics_2[0].message_count(), 2);
        assert_eq!(topics_2[0].blocktime(), block_time_2); // NOTE: Session called mint; the topic
                                                           // summary blocktime is refreshed
    }

    let mut messages = result_2.messages.iter().collect_vec();
    messages.sort_by_key(|message| {
        (
            message.topic_name(),
            message.topic_index(),
            message.block_index(),
        )
    });
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].topic_name(), "Transfer");
    assert_eq!(messages[0].topic_index(), 0);
    assert_eq!(messages[0].block_index(), 0);

    assert_eq!(messages[1].topic_name(), "Transfer");
    assert_eq!(messages[1].topic_index(), 1);
    assert_eq!(messages[1].block_index(), 1);
}

fn make_global_state_with_genesis() -> (LmdbGlobalState, Digest, TempDir) {
    let default_accounts = vec![GenesisAccount::Account {
        public_key: DEFAULT_ACCOUNT_PUBLIC_KEY.clone(),
        balance: Motes::new(U512::from(100 * CSPR)),
        validator: None,
    }];

    let (global_state, _state_root_hash, _tempdir) =
        global_state::state::lmdb::make_temporary_global_state([]);

    let genesis_config = GenesisConfig::new(
        default_accounts,
        WasmConfig::default(),
        SystemConfig::default(),
        10,
        10,
        0,
        Default::default(),
        14,
        Timestamp::now().millis(),
        casper_types::HoldBalanceHandling::Accrued,
        0,
        true,
        StorageCosts::default(),
    );
    let genesis_request: GenesisRequest = GenesisRequest::new(
        Digest::hash("foo"),
        ProtocolVersion::V2_0_0,
        genesis_config,
        ChainspecRegistry::new_with_genesis(b"", b""),
    );
    match global_state.genesis(genesis_request) {
        GenesisResult::Failure(failure) => panic!("Failed to run genesis: {:?}", failure),
        GenesisResult::Fatal(fatal) => panic!("Fatal error while running genesis: {}", fatal),
        GenesisResult::Success {
            post_state_hash,
            effects: _,
        } => (global_state, post_state_hash, _tempdir),
    }
}

#[test]
fn traits() {
    let mut executor = make_executor();
    let (mut global_state, state_root_hash, _tempdir) = make_global_state_with_genesis();

    let execute_request = base_execute_builder()
        .with_target(ExecutionKind::SessionBytes(VM2_TRAIT))
        .with_serialized_input(())
        .with_shared_address_generator(make_address_generator())
        .build()
        .expect("should build");

    run_wasm_session(
        &mut executor,
        &mut global_state,
        state_root_hash,
        execute_request,
    );
}

#[test]
fn upgradable() {
    let mut executor = make_executor();

    let (mut global_state, mut state_root_hash, _tempdir) = make_global_state_with_genesis();

    let address_generator = make_address_generator();

    let upgradable_address;

    state_root_hash = {
        let input_data = borsh::to_vec(&(0u8,)).map(Bytes::from).unwrap();

        let create_request = base_install_request_builder()
            .with_wasm_bytes(VM2_UPGRADABLE.clone())
            .with_shared_address_generator(Arc::clone(&address_generator))
            .with_gas_limit(DEFAULT_GAS_LIMIT)
            .with_transferred_value(0)
            .with_entry_point("new".to_string())
            .with_input(input_data)
            .build()
            .expect("should build");

        let create_result = run_create_contract(
            &mut executor,
            &mut global_state,
            state_root_hash,
            create_request,
        );

        upgradable_address = *create_result.smart_contract_addr();

        global_state
            .commit_effects(state_root_hash, create_result.effects().clone())
            .expect("Should commit")
    };

    let version_before_upgrade = {
        let execute_request = base_execute_builder()
            .with_target(ExecutionKind::Stored {
                address: upgradable_address,
                entry_point: "version".to_string(),
            })
            .with_input(Bytes::new())
            .with_gas_limit(DEFAULT_GAS_LIMIT)
            .with_transferred_value(0)
            .with_shared_address_generator(Arc::clone(&address_generator))
            .build()
            .expect("should build");
        let res = run_wasm_session(
            &mut executor,
            &mut global_state,
            state_root_hash,
            execute_request,
        );
        let output = res.output().expect("should have output");
        let version: String = borsh::from_slice(output).expect("should deserialize");
        version
    };
    assert_eq!(version_before_upgrade, "v1");

    {
        // Increment the value
        let execute_request = base_execute_builder()
            .with_target(ExecutionKind::Stored {
                address: upgradable_address,
                entry_point: "increment".to_string(),
            })
            .with_input(Bytes::new())
            .with_gas_limit(DEFAULT_GAS_LIMIT)
            .with_transferred_value(0)
            .with_shared_address_generator(Arc::clone(&address_generator))
            .build()
            .expect("should build");
        let res = run_wasm_session(
            &mut executor,
            &mut global_state,
            state_root_hash,
            execute_request,
        );
        state_root_hash = global_state
            .commit_effects(state_root_hash, res.effects().clone())
            .expect("Should commit");
    };

    let binding = VM2_UPGRADABLE_V2;
    let new_code = binding.as_ref();

    let execute_request = base_execute_builder()
        .with_transferred_value(0)
        .with_target(ExecutionKind::Stored {
            address: upgradable_address,
            entry_point: "perform_upgrade".to_string(),
        })
        .with_gas_limit(DEFAULT_GAS_LIMIT * 10)
        .with_serialized_input((new_code,))
        .with_shared_address_generator(Arc::clone(&address_generator))
        .build()
        .expect("should build");
    let res = run_wasm_session(
        &mut executor,
        &mut global_state,
        state_root_hash,
        execute_request,
    );
    state_root_hash = global_state
        .commit_effects(state_root_hash, res.effects().clone())
        .expect("Should commit");

    let version_after_upgrade = {
        let execute_request = base_execute_builder()
            .with_target(ExecutionKind::Stored {
                address: upgradable_address,
                entry_point: "version".to_string(),
            })
            .with_input(Bytes::new())
            .with_gas_limit(DEFAULT_GAS_LIMIT)
            .with_transferred_value(0)
            .with_shared_address_generator(Arc::clone(&address_generator))
            .build()
            .expect("should build");
        let res = run_wasm_session(
            &mut executor,
            &mut global_state,
            state_root_hash,
            execute_request,
        );
        let output = res.output().expect("should have output");
        let version: String = borsh::from_slice(output).expect("should deserialize");
        version
    };
    assert_eq!(version_after_upgrade, "v2");

    {
        // Increment the value
        let execute_request = base_execute_builder()
            .with_target(ExecutionKind::Stored {
                address: upgradable_address,
                entry_point: "increment_by".to_string(),
            })
            .with_serialized_input((10u64,))
            .with_gas_limit(DEFAULT_GAS_LIMIT)
            .with_transferred_value(0)
            .with_shared_address_generator(Arc::clone(&address_generator))
            .build()
            .expect("should build");
        let res = run_wasm_session(
            &mut executor,
            &mut global_state,
            state_root_hash,
            execute_request,
        );
        state_root_hash = global_state
            .commit_effects(state_root_hash, res.effects().clone())
            .expect("Should commit");
    };

    let _ = state_root_hash;
}

fn run_create_contract(
    executor: &mut ExecutorV2,
    global_state: &LmdbGlobalState,
    pre_state_hash: Digest,
    install_contract_request: InstallContractRequest,
) -> InstallContractResult {
    executor
        .install_contract(pre_state_hash, global_state, install_contract_request)
        .expect("Succeed")
}

fn run_wasm_session(
    executor: &mut ExecutorV2,
    global_state: &LmdbGlobalState,
    pre_state_hash: Digest,
    execute_request: ExecuteRequest,
) -> ExecuteWithProviderResult {
    let result = executor
        .execute_with_provider(pre_state_hash, global_state, execute_request)
        .expect("Succeed");

    if let Some(host_error) = result.host_error {
        panic!("Host error: {host_error:?}")
    }

    result
}

#[test]
fn backwards_compatibility() {
    let (mut global_state, post_state_hash, _temp) = {
        let fixture_name = "counter_contract";
        // /Users/michal/Dev/casper-node/execution_engine_testing/tests/fixtures/counter_contract/
        // global_state/data.lmdb
        let lmdb_fixtures_base_dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../")
            .join("../")
            .join("execution_engine_testing")
            .join("tests")
            .join("fixtures");
        assert!(lmdb_fixtures_base_dir.exists());

        let source = lmdb_fixtures_base_dir.join("counter_contract");
        let to = tempfile::tempdir().expect("should create temp dir");
        fs_extra::copy_items(&[source], &to, &dir::CopyOptions::default())
            .expect("should copy global state fixture");

        let path_to_state = to.path().join(fixture_name).join("state.json");

        let lmdb_fixture_state: serde_json::Value =
            serde_json::from_reader(File::open(path_to_state).unwrap()).unwrap();
        let post_state_hash =
            Digest::from_hex(lmdb_fixture_state["post_state_hash"].as_str().unwrap()).unwrap();

        let path_to_gs = to.path().join(fixture_name).join("global_state");

        const DEFAULT_LMDB_PAGES: usize = 256_000_000;
        const DEFAULT_MAX_READERS: u32 = 512;

        let environment = LmdbEnvironment::new(
            &path_to_gs,
            16384 * DEFAULT_LMDB_PAGES,
            DEFAULT_MAX_READERS,
            true,
        )
        .expect("should create LmdbEnvironment");

        let trie_store =
            LmdbTrieStore::open(&environment, None).expect("should open LmdbTrieStore");
        (
            LmdbGlobalState::new(
                Arc::new(environment),
                Arc::new(trie_store),
                post_state_hash,
                100,
                false,
            ),
            post_state_hash,
            to,
        )
    };

    let result = global_state.query(QueryRequest::new(
        post_state_hash,
        Key::Account(*DEFAULT_ACCOUNT_HASH),
        Vec::new(),
    ));
    let value = match result {
        QueryResult::RootNotFound => todo!(),
        QueryResult::ValueNotFound(value) => panic!("Value not found: {:?}", value),
        QueryResult::Success { value, .. } => value,
        QueryResult::Failure(failure) => panic!("Failed to query: {:?}", failure),
    };

    //
    // Calling legacy contract directly by it's address
    //

    let mut state_root_hash = post_state_hash;

    let value = match *value {
        StoredValue::Account(account) => account,
        _ => panic!("Expected CLValue"),
    };

    let counter_hash = match value.named_keys().get("counter") {
        Some(Key::Hash(hash_address)) => hash_address,
        _ => panic!("Expected counter URef"),
    };

    let mut executor = make_executor();
    let address_generator = make_address_generator();

    // Calling v1 vm directly by hash is not currently supported (i.e. disabling vm1 runtime, and
    // allowing vm1 direct calls may circumvent chainspec setting) let execute_request =
    // base_execute_builder()     .with_target(ExecutionKind::Stored {
    //         address: *counter_hash,
    //         entry_point: "counter_get".to_string(),
    //     })
    //     .with_input(runtime_args.into())
    //     .with_gas_limit(DEFAULT_GAS_LIMIT)
    //     .with_transferred_value(0)
    //     .with_shared_address_generator(Arc::clone(&address_generator))
    //     .with_state_hash(state_root_hash)
    //     .with_block_height(1)
    //     .with_parent_block_hash(BlockHash::new(Digest::hash(b"block1")))
    //     .build()
    //     .expect("should build");
    // let res = run_wasm_session(
    //     &mut executor,
    //     &mut global_state,
    //     state_root_hash,
    //     execute_request,
    // );
    // state_root_hash = global_state
    //     .commit_effects(state_root_hash, res.effects().clone())
    //     .expect("Should commit");

    //
    // Instantiate v2 runtime proxy contract
    //
    let input_data = counter_hash.to_vec();
    let install_request: InstallContractRequest = base_install_request_builder()
        .with_wasm_bytes(VM2_LEGACY_COUNTER_PROXY.clone())
        .with_shared_address_generator(Arc::clone(&address_generator))
        .with_transferred_value(0)
        .with_entry_point("new".to_string())
        .with_input(input_data.into())
        .with_state_hash(state_root_hash)
        .with_block_height(2)
        .with_parent_block_hash(BlockHash::new(Digest::hash(b"block2")))
        .build()
        .expect("should build");

    let create_result = run_create_contract(
        &mut executor,
        &mut global_state,
        state_root_hash,
        install_request,
    );

    state_root_hash = create_result.post_state_hash();

    let proxy_address = *create_result.smart_contract_addr();

    // Call v2 contract

    let call_request = base_execute_builder()
        .with_target(ExecutionKind::Stored {
            address: proxy_address,
            entry_point: "perform_test".to_string(),
        })
        .with_input(Bytes::new())
        .with_gas_limit(DEFAULT_GAS_LIMIT)
        .with_transferred_value(0)
        .with_shared_address_generator(Arc::clone(&address_generator))
        .with_state_hash(state_root_hash)
        .with_block_height(3)
        .with_parent_block_hash(BlockHash::new(Digest::hash(b"block3")))
        .build()
        .expect("should build");

    run_wasm_session(
        &mut executor,
        &mut global_state,
        state_root_hash,
        call_request,
    );
}

// host function tests

fn call_dummy_host_fn_by_name(
    host_function_name: &str,
    gas_limit: u64,
) -> Result<InstallContractResult, InstallContractError> {
    let executor = {
        let execution_engine_v1 = ExecutionEngineV1::default();
        let default_wasm_config = WasmV2Config::default();
        let wasm_config = WasmV2Config::new(
            default_wasm_config.max_memory(),
            default_wasm_config.opcode_costs(),
            HostFunctionCostsV2 {
                read: HostFunction::fixed(1),
                write: HostFunction::fixed(1),
                copy_input: HostFunction::fixed(1),
                ret: HostFunction::fixed(1),
                create: HostFunction::fixed(1),
                env_caller: HostFunction::fixed(1),
                env_block_time: HostFunction::fixed(1),
                env_transferred_value: HostFunction::fixed(1),
                transfer: HostFunction::fixed(1),
                env_balance: HostFunction::fixed(1),
                upgrade: HostFunction::fixed(1),
                call: HostFunction::fixed(1),
                print: HostFunction::fixed(1),
            },
        );
        let executor_config = ExecutorConfigBuilder::default()
            .with_memory_limit(17)
            .with_executor_kind(ExecutorKind::Compiled)
            .with_wasm_config(wasm_config)
            .with_storage_costs(StorageCosts::default())
            .build()
            .expect("Should build");
        ExecutorV2::new(executor_config, Arc::new(execution_engine_v1))
    };

    let (mut global_state, state_root_hash, _tempdir) = make_global_state_with_genesis();

    let address_generator = make_address_generator();

    let input_data = borsh::to_vec(&(host_function_name.to_owned(),))
        .map(Bytes::from)
        .unwrap();

    let create_request = InstallContractRequestBuilder::default()
        .with_initiator(*DEFAULT_ACCOUNT_HASH)
        .with_gas_limit(gas_limit)
        .with_transaction_hash(TRANSACTION_HASH)
        .with_wasm_bytes(VM2_HOST.clone())
        .with_shared_address_generator(Arc::clone(&address_generator))
        .with_transferred_value(0)
        .with_entry_point("new".to_string())
        .with_input(input_data)
        .with_chain_name(DEFAULT_CHAIN_NAME)
        .with_block_time(Timestamp::now().into())
        .with_state_hash(Digest::from_raw([0; 32]))
        .with_block_height(1)
        .with_parent_block_hash(BlockHash::new(Digest::from_raw([0; 32])))
        .build()
        .expect("should build");

    executor.install_contract(state_root_hash, &mut global_state, create_request)
}

fn assert_consumes_gas(host_function_name: &str) {
    let result = call_dummy_host_fn_by_name(host_function_name, 1);
    assert!(result.is_err_and(|e| match e {
        InstallContractError::Constructor {
            host_error: HostError::CalleeGasDepleted,
        } => true,
        _ => false,
    }));
}

#[test]
fn host_functions_consume_gas() {
    assert_consumes_gas("get_caller");
    assert_consumes_gas("get_block_time");
    assert_consumes_gas("get_transferred_value");
    assert_consumes_gas("get_balance_of");
    assert_consumes_gas("call");
    assert_consumes_gas("input");
    assert_consumes_gas("create");
    assert_consumes_gas("print");
    assert_consumes_gas("read");
    assert_consumes_gas("ret");
    assert_consumes_gas("transfer");
    assert_consumes_gas("upgrade");
    assert_consumes_gas("write");
}

fn write_n_bytes_at_limit(
    bytes_len: u64,
    gas_limit: u64,
) -> Result<InstallContractResult, InstallContractError> {
    let executor = {
        let execution_engine_v1 = ExecutionEngineV1::default();
        let default_wasm_config = WasmV2Config::default();
        let wasm_config = WasmV2Config::new(
            default_wasm_config.max_memory(),
            default_wasm_config.opcode_costs(),
            HostFunctionCostsV2 {
                read: HostFunction::fixed(0),
                write: HostFunction::fixed(0),
                copy_input: HostFunction::fixed(0),
                ret: HostFunction::fixed(0),
                create: HostFunction::fixed(0),
                env_caller: HostFunction::fixed(0),
                env_block_time: HostFunction::fixed(0),
                env_transferred_value: HostFunction::fixed(0),
                transfer: HostFunction::fixed(0),
                env_balance: HostFunction::fixed(0),
                upgrade: HostFunction::fixed(0),
                call: HostFunction::fixed(0),
                print: HostFunction::fixed(0),
            },
        );
        let executor_config = ExecutorConfigBuilder::default()
            .with_memory_limit(17)
            .with_executor_kind(ExecutorKind::Compiled)
            .with_wasm_config(wasm_config)
            .with_storage_costs(StorageCosts::new(1))
            .build()
            .expect("Should build");
        ExecutorV2::new(executor_config, Arc::new(execution_engine_v1))
    };

    let (mut global_state, state_root_hash, _tempdir) = make_global_state_with_genesis();

    let address_generator = make_address_generator();

    let input_data = borsh::to_vec(&(bytes_len,)).map(Bytes::from).unwrap();

    let create_request = InstallContractRequestBuilder::default()
        .with_initiator(*DEFAULT_ACCOUNT_HASH)
        .with_gas_limit(gas_limit)
        .with_transaction_hash(TRANSACTION_HASH)
        .with_wasm_bytes(VM2_HOST.clone())
        .with_shared_address_generator(Arc::clone(&address_generator))
        .with_transferred_value(0)
        .with_entry_point("new_with_write".to_string())
        .with_input(input_data)
        .with_chain_name(DEFAULT_CHAIN_NAME)
        .with_block_time(Timestamp::now().into())
        .with_state_hash(Digest::from_raw([0; 32]))
        .with_block_height(1)
        .with_parent_block_hash(BlockHash::new(Digest::from_raw([0; 32])))
        .build()
        .expect("should build");

    executor.install_contract(state_root_hash, &mut global_state, create_request)
}

#[test]
fn consume_gas_on_write() {
    let successful_write = write_n_bytes_at_limit(50, 10_000);
    assert!(successful_write.is_ok());

    let out_of_gas_write_exceeded_gas_limit = write_n_bytes_at_limit(50, 10);
    assert!(out_of_gas_write_exceeded_gas_limit.is_err_and(|e| match e {
        InstallContractError::Constructor {
            host_error: HostError::CalleeGasDepleted,
        } => true,
        _ => false,
    }));
}
