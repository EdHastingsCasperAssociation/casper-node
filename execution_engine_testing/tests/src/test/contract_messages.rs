use num_traits::Zero;
use std::cell::RefCell;

use casper_execution_engine::runtime::cryptography;

use casper_engine_test_support::{
    ChainspecConfig, ExecuteRequestBuilder, LmdbWasmTestBuilder, DEFAULT_ACCOUNT_ADDR,
    DEFAULT_BLOCK_TIME, LOCAL_GENESIS_REQUEST,
};

use casper_types::{
    addressable_entity::MessageTopics,
    bytesrepr::ToBytes,
    contract_messages::{MessageChecksum, MessagePayload, MessageTopicSummary, TopicNameHash},
    runtime_args, AddressableEntityHash, BlockGlobalAddr, BlockTime, CLValue, CoreConfig, Digest,
    EntityAddr, HostFunction, HostFunctionCostsV1, HostFunctionCostsV2, Key, MessageLimits,
    OpcodeCosts, RuntimeArgs, StorageCosts, StoredValue, SystemConfig, WasmConfig, WasmV1Config,
    WasmV2Config, DEFAULT_MAX_STACK_HEIGHT, DEFAULT_WASM_MAX_MEMORY, U512,
};

const MESSAGE_EMITTER_INSTALLER_WASM: &str = "contract_messages_emitter.wasm";
const MESSAGE_EMITTER_UPGRADER_WASM: &str = "contract_messages_upgrader.wasm";
const MESSAGE_EMITTER_FROM_ACCOUNT: &str = "contract_messages_from_account.wasm";
const MESSAGE_EMITTER_PACKAGE_HASH_KEY_NAME: &str = "messages_emitter_package_hash";
const MESSAGE_EMITTER_GENERIC_TOPIC: &str = "generic_messages";
const MESSAGE_EMITTER_UPGRADED_TOPIC: &str = "new_topic_after_upgrade";
const ENTRY_POINT_EMIT_MESSAGE: &str = "emit_message";
const ENTRY_POINT_EMIT_MULTIPLE_MESSAGES: &str = "emit_multiple_messages";
const ENTRY_POINT_EMIT_MESSAGE_FROM_EACH_VERSION: &str = "emit_message_from_each_version";
const ARG_NUM_MESSAGES_TO_EMIT: &str = "num_messages_to_emit";
const ARG_TOPIC_NAME: &str = "topic_name";
const ENTRY_POINT_ADD_TOPIC: &str = "add_topic";
const ARG_MESSAGE_SUFFIX_NAME: &str = "message_suffix";
const ARG_REGISTER_DEFAULT_TOPIC_WITH_INIT: &str = "register_default_topic_with_init";

const EMITTER_MESSAGE_PREFIX: &str = "generic message: ";

// Number of messages that will be emitted when calling `ENTRY_POINT_EMIT_MESSAGE_FROM_EACH_VERSION`
const EMIT_MESSAGE_FROM_EACH_VERSION_NUM_MESSAGES: u32 = 3;

fn install_messages_emitter_contract(
    builder: &RefCell<LmdbWasmTestBuilder>,
    use_initializer: bool,
) -> AddressableEntityHash {
    // Request to install the contract that will be emitting messages.
    let install_request = ExecuteRequestBuilder::standard(
        *DEFAULT_ACCOUNT_ADDR,
        MESSAGE_EMITTER_INSTALLER_WASM,
        runtime_args! {
            ARG_REGISTER_DEFAULT_TOPIC_WITH_INIT => use_initializer,
        },
    )
    .build();

    // Execute the request to install the message emitting contract.
    // This will also register a topic for the contract to emit messages on.
    builder
        .borrow_mut()
        .exec(install_request)
        .expect_success()
        .commit();

    // Get the contract package for the messages_emitter.
    let query_result = builder
        .borrow_mut()
        .query(
            None,
            Key::from(*DEFAULT_ACCOUNT_ADDR),
            &[MESSAGE_EMITTER_PACKAGE_HASH_KEY_NAME.into()],
        )
        .expect("should query");

    let message_emitter_package = if let StoredValue::ContractPackage(package) = query_result {
        package
    } else {
        panic!("Stored value is not a contract package: {:?}", query_result);
    };

    // Get the contract hash of the messages_emitter contract.
    message_emitter_package
        .versions()
        .values()
        .last()
        .map(|contract_hash| AddressableEntityHash::new(contract_hash.value()))
        .expect("Should have contract hash")
}

fn upgrade_messages_emitter_contract(
    builder: &RefCell<LmdbWasmTestBuilder>,
    use_initializer: bool,
    expect_failure: bool,
) -> AddressableEntityHash {
    let upgrade_request = ExecuteRequestBuilder::standard(
        *DEFAULT_ACCOUNT_ADDR,
        MESSAGE_EMITTER_UPGRADER_WASM,
        runtime_args! {
            ARG_REGISTER_DEFAULT_TOPIC_WITH_INIT => use_initializer,
        },
    )
    .build();

    // let new_topics = BTreeMap::from([(
    //     MESSAGE_EMITTER_GENERIC_TOPIC.to_string(),
    //     MessageTopicOperation::Add,
    // )]);

    // println!("{}", new_topics.into_bytes().unwrap().len());

    // Execute the request to upgrade the message emitting contract.
    // This will also register a new topic for the contract to emit messages on.
    if expect_failure {
        builder
            .borrow_mut()
            .exec(upgrade_request)
            .expect_failure()
            .commit();
    } else {
        builder
            .borrow_mut()
            .exec(upgrade_request)
            .expect_success()
            .commit();
    }

    // Get the contract package for the upgraded messages emitter contract.
    let query_result = builder
        .borrow_mut()
        .query(
            None,
            Key::from(*DEFAULT_ACCOUNT_ADDR),
            &[MESSAGE_EMITTER_PACKAGE_HASH_KEY_NAME.into()],
        )
        .expect("should query");

    let message_emitter_package = if let StoredValue::ContractPackage(package) = query_result {
        package
    } else {
        panic!("Stored value is not a contract package: {:?}", query_result);
    };

    // Get the contract hash of the latest version of the messages emitter contract.
    message_emitter_package
        .versions()
        .values()
        .last()
        .map(|contract_hash| AddressableEntityHash::new(contract_hash.value()))
        .expect("Should have contract hash")
}

fn emit_message_with_suffix(
    builder: &RefCell<LmdbWasmTestBuilder>,
    suffix: &str,
    contract_hash: &AddressableEntityHash,
    block_time: u64,
) {
    let emit_message_request = ExecuteRequestBuilder::contract_call_by_hash(
        *DEFAULT_ACCOUNT_ADDR,
        *contract_hash,
        ENTRY_POINT_EMIT_MESSAGE,
        runtime_args! {
            ARG_MESSAGE_SUFFIX_NAME => suffix,
        },
    )
    .with_block_time(block_time)
    .build();

    builder
        .borrow_mut()
        .exec(emit_message_request)
        .expect_success()
        .commit();
}

struct ContractQueryView<'a> {
    builder: &'a RefCell<LmdbWasmTestBuilder>,
    contract_hash: AddressableEntityHash,
}

impl<'a> ContractQueryView<'a> {
    fn new(
        builder: &'a RefCell<LmdbWasmTestBuilder>,
        contract_hash: AddressableEntityHash,
    ) -> Self {
        Self {
            builder,
            contract_hash,
        }
    }

    fn message_topics(&self) -> MessageTopics {
        let message_topics_result = self
            .builder
            .borrow_mut()
            .message_topics(None, EntityAddr::SmartContract(self.contract_hash.value()))
            .expect("must get message topics");

        message_topics_result
    }

    fn message_topic(&self, topic_name_hash: TopicNameHash) -> MessageTopicSummary {
        let query_result = self
            .builder
            .borrow_mut()
            .query(
                None,
                Key::message_topic(
                    EntityAddr::SmartContract(self.contract_hash.value()),
                    topic_name_hash,
                ),
                &[],
            )
            .expect("should query");

        match query_result {
            StoredValue::MessageTopic(summary) => summary,
            _ => {
                panic!(
                    "Stored value is not a message topic summary: {:?}",
                    query_result
                );
            }
        }
    }

    fn message_summary(
        &self,
        topic_name_hash: TopicNameHash,
        message_index: u32,
        state_hash: Option<Digest>,
    ) -> Result<MessageChecksum, String> {
        let query_result = self.builder.borrow_mut().query(
            state_hash,
            Key::message(
                EntityAddr::SmartContract(self.contract_hash.value()),
                topic_name_hash,
                message_index,
            ),
            &[],
        )?;

        match query_result {
            StoredValue::Message(summary) => Ok(summary),
            _ => panic!("Stored value is not a message summary: {:?}", query_result),
        }
    }
}

#[ignore]
#[test]
fn should_emit_messages() {
    let builder = RefCell::new(LmdbWasmTestBuilder::default());
    builder
        .borrow_mut()
        .run_genesis(LOCAL_GENESIS_REQUEST.clone());

    let contract_hash = install_messages_emitter_contract(&builder, true);
    let query_view = ContractQueryView::new(&builder, contract_hash);

    let message_topics = query_view.message_topics();

    let (topic_name, message_topic_hash) = message_topics
        .iter()
        .next()
        .expect("should have at least one topic");

    assert_eq!(topic_name, &MESSAGE_EMITTER_GENERIC_TOPIC.to_string());
    // Check that the topic exists for the installed contract.
    assert_eq!(
        query_view
            .message_topic(*message_topic_hash)
            .message_count(),
        0
    );

    // Now call the entry point to emit some messages.
    emit_message_with_suffix(&builder, "test", &contract_hash, DEFAULT_BLOCK_TIME);
    let expected_message = MessagePayload::from(format!("{}{}", EMITTER_MESSAGE_PREFIX, "test"));
    let expected_message_hash = cryptography::blake2b(
        [
            0u64.to_bytes().unwrap(),
            expected_message.to_bytes().unwrap(),
        ]
        .concat(),
    );
    let queried_message_summary = query_view
        .message_summary(*message_topic_hash, 0, None)
        .expect("should have value")
        .value();
    assert_eq!(expected_message_hash, queried_message_summary);
    assert_eq!(
        query_view
            .message_topic(*message_topic_hash)
            .message_count(),
        1
    );

    // call again to emit a new message and check that the index in the topic incremented.
    emit_message_with_suffix(&builder, "test", &contract_hash, DEFAULT_BLOCK_TIME);
    let expected_message_hash = cryptography::blake2b(
        [
            1u64.to_bytes().unwrap(),
            expected_message.to_bytes().unwrap(),
        ]
        .concat(),
    );
    let queried_message_summary = query_view
        .message_summary(*message_topic_hash, 1, None)
        .expect("should have value")
        .value();
    assert_eq!(expected_message_hash, queried_message_summary);
    assert_eq!(
        query_view
            .message_topic(*message_topic_hash)
            .message_count(),
        2
    );

    let first_block_state_hash = builder.borrow().get_post_state_hash();

    // call to emit a new message but in another block.
    emit_message_with_suffix(
        &builder,
        "new block time",
        &contract_hash,
        DEFAULT_BLOCK_TIME + 1,
    );
    let expected_message =
        MessagePayload::from(format!("{}{}", EMITTER_MESSAGE_PREFIX, "new block time"));
    let expected_message_hash = cryptography::blake2b(
        [
            0u64.to_bytes().unwrap(),
            expected_message.to_bytes().unwrap(),
        ]
        .concat(),
    );
    let queried_message_summary = query_view
        .message_summary(*message_topic_hash, 0, None)
        .expect("should have value")
        .value();
    assert_eq!(expected_message_hash, queried_message_summary);
    assert_eq!(
        query_view
            .message_topic(*message_topic_hash)
            .message_count(),
        1
    );

    // old messages should be pruned from tip and inaccessible at the latest state hash.
    assert!(query_view
        .message_summary(*message_topic_hash, 1, None)
        .is_err());

    // old messages should still be discoverable at a state hash before pruning.
    assert!(query_view
        .message_summary(*message_topic_hash, 1, Some(first_block_state_hash))
        .is_ok());
}

#[ignore]
#[test]
fn should_emit_message_on_empty_topic_in_new_block() {
    let builder = RefCell::new(LmdbWasmTestBuilder::default());
    builder
        .borrow_mut()
        .run_genesis(LOCAL_GENESIS_REQUEST.clone());

    let contract_hash = install_messages_emitter_contract(&builder, true);
    let query_view = ContractQueryView::new(&builder, contract_hash);

    let message_topics = query_view.message_topics();

    let (_, message_topic_hash) = message_topics
        .iter()
        .next()
        .expect("should have at least one topic");

    assert_eq!(
        query_view
            .message_topic(*message_topic_hash)
            .message_count(),
        0
    );

    emit_message_with_suffix(
        &builder,
        "new block time",
        &contract_hash,
        DEFAULT_BLOCK_TIME + 1,
    );
    assert_eq!(
        query_view
            .message_topic(*message_topic_hash)
            .message_count(),
        1
    );
}

#[ignore]
#[test]
fn should_add_topics() {
    let builder = RefCell::new(LmdbWasmTestBuilder::default());
    builder
        .borrow_mut()
        .run_genesis(LOCAL_GENESIS_REQUEST.clone());
    let contract_hash = install_messages_emitter_contract(&builder, true);
    let query_view = ContractQueryView::new(&builder, contract_hash);

    let add_topic_request = ExecuteRequestBuilder::contract_call_by_hash(
        *DEFAULT_ACCOUNT_ADDR,
        contract_hash,
        ENTRY_POINT_ADD_TOPIC,
        runtime_args! {
            ARG_TOPIC_NAME => "topic_1",
        },
    )
    .build();

    builder
        .borrow_mut()
        .exec(add_topic_request)
        .expect_success()
        .commit();

    let topic_1_hash = *query_view
        .message_topics()
        .get("topic_1")
        .expect("should have added topic `topic_1");
    assert_eq!(query_view.message_topic(topic_1_hash).message_count(), 0);

    let add_topic_request = ExecuteRequestBuilder::contract_call_by_hash(
        *DEFAULT_ACCOUNT_ADDR,
        contract_hash,
        ENTRY_POINT_ADD_TOPIC,
        runtime_args! {
            ARG_TOPIC_NAME => "topic_2",
        },
    )
    .build();

    builder
        .borrow_mut()
        .exec(add_topic_request)
        .expect_success()
        .commit();

    let topic_2_hash = *query_view
        .message_topics()
        .get("topic_2")
        .expect("should have added topic `topic_2");

    assert!(query_view.message_topics().get("topic_1").is_some());
    assert_eq!(query_view.message_topic(topic_1_hash).message_count(), 0);
    assert_eq!(query_view.message_topic(topic_2_hash).message_count(), 0);
}

#[ignore]
#[test]
fn should_not_add_duplicate_topics() {
    let builder = RefCell::new(LmdbWasmTestBuilder::default());
    builder
        .borrow_mut()
        .run_genesis(LOCAL_GENESIS_REQUEST.clone());

    let contract_hash = install_messages_emitter_contract(&builder, true);
    let query_view = ContractQueryView::new(&builder, contract_hash);
    let message_topics = query_view.message_topics();
    let (first_topic_name, _) = message_topics
        .iter()
        .next()
        .expect("should have at least one topic");

    let add_topic_request = ExecuteRequestBuilder::contract_call_by_hash(
        *DEFAULT_ACCOUNT_ADDR,
        contract_hash,
        ENTRY_POINT_ADD_TOPIC,
        runtime_args! {
            ARG_TOPIC_NAME => first_topic_name,
        },
    )
    .build();

    builder
        .borrow_mut()
        .exec(add_topic_request)
        .expect_failure()
        .commit();
}

#[ignore]
#[test]
fn should_not_exceed_configured_limits() {
    let chainspec = {
        let default_wasm_v1_config = WasmV1Config::default();
        let default_wasm_v2_config = WasmV2Config::default();
        let wasm_v1_config = WasmV1Config::new(
            default_wasm_v1_config.max_memory(),
            default_wasm_v1_config.max_stack_height(),
            default_wasm_v1_config.opcode_costs(),
            default_wasm_v1_config.take_host_function_costs(),
        );
        let wasm_v2_config = WasmV2Config::new(
            default_wasm_v2_config.max_memory(),
            default_wasm_v2_config.opcode_costs(),
            default_wasm_v2_config.take_host_function_costs(),
        );
        let wasm_config = WasmConfig::new(
            MessageLimits {
                max_topic_name_size: 32,
                max_message_size: 100,
                max_topics_per_contract: 2,
            },
            wasm_v1_config,
            wasm_v2_config,
        );
        ChainspecConfig {
            system_costs_config: SystemConfig::default(),
            core_config: CoreConfig::default(),
            wasm_config,
            storage_costs: StorageCosts::default(),
        }
    };

    let builder = RefCell::new(LmdbWasmTestBuilder::new_temporary_with_config(chainspec));
    builder
        .borrow_mut()
        .run_genesis(LOCAL_GENESIS_REQUEST.clone());

    let contract_hash = install_messages_emitter_contract(&builder, true);

    // if the topic larger than the limit, registering should fail.
    // string is 33 bytes > limit established above
    let too_large_topic_name = std::str::from_utf8(&[0x4du8; 33]).unwrap();
    let add_topic_request = ExecuteRequestBuilder::contract_call_by_hash(
        *DEFAULT_ACCOUNT_ADDR,
        contract_hash,
        ENTRY_POINT_ADD_TOPIC,
        runtime_args! {
            ARG_TOPIC_NAME => too_large_topic_name,
        },
    )
    .build();

    builder
        .borrow_mut()
        .exec(add_topic_request)
        .expect_failure()
        .commit();

    // if the topic name is equal to the limit, registering should work.
    // string is 32 bytes == limit established above
    let topic_name_at_limit = std::str::from_utf8(&[0x4du8; 32]).unwrap();
    let add_topic_request = ExecuteRequestBuilder::contract_call_by_hash(
        *DEFAULT_ACCOUNT_ADDR,
        contract_hash,
        ENTRY_POINT_ADD_TOPIC,
        runtime_args! {
            ARG_TOPIC_NAME => topic_name_at_limit,
        },
    )
    .build();

    builder
        .borrow_mut()
        .exec(add_topic_request)
        .expect_success()
        .commit();

    // Check that the max number of topics limit is enforced.
    // 2 topics are already registered, so registering another topic should
    // fail since the limit is already reached.
    let add_topic_request = ExecuteRequestBuilder::contract_call_by_hash(
        *DEFAULT_ACCOUNT_ADDR,
        contract_hash,
        ENTRY_POINT_ADD_TOPIC,
        runtime_args! {
            ARG_TOPIC_NAME => "topic_1",
        },
    )
    .build();

    builder
        .borrow_mut()
        .exec(add_topic_request)
        .expect_failure()
        .commit();

    // Check message size limit
    let large_message = std::str::from_utf8(&[0x4du8; 128]).unwrap();
    let emit_message_request = ExecuteRequestBuilder::contract_call_by_hash(
        *DEFAULT_ACCOUNT_ADDR,
        contract_hash,
        ENTRY_POINT_EMIT_MESSAGE,
        runtime_args! {
            ARG_MESSAGE_SUFFIX_NAME => large_message,
        },
    )
    .build();

    builder
        .borrow_mut()
        .exec(emit_message_request)
        .expect_failure()
        .commit();
}

fn should_carry_message_topics_on_upgraded_contract(use_initializer: bool) {
    let builder = RefCell::new(LmdbWasmTestBuilder::default());
    builder
        .borrow_mut()
        .run_genesis(LOCAL_GENESIS_REQUEST.clone());

    let _ = install_messages_emitter_contract(&builder, true);
    let contract_hash = upgrade_messages_emitter_contract(&builder, use_initializer, false);
    let query_view = ContractQueryView::new(&builder, contract_hash);

    let message_topics = query_view.message_topics();
    assert_eq!(message_topics.len(), 2);
    let mut expected_topic_names = 0;
    for (topic_name, topic_hash) in message_topics.iter() {
        if topic_name == MESSAGE_EMITTER_GENERIC_TOPIC
            || topic_name == MESSAGE_EMITTER_UPGRADED_TOPIC
        {
            expected_topic_names += 1;
        }

        assert_eq!(query_view.message_topic(*topic_hash).message_count(), 0);
    }
    assert_eq!(expected_topic_names, 2);
}

#[ignore]
#[test]
fn should_carry_message_topics_on_upgraded_contract_with_initializer() {
    should_carry_message_topics_on_upgraded_contract(true);
}

#[ignore]
#[test]
fn should_carry_message_topics_on_upgraded_contract_without_initializer() {
    should_carry_message_topics_on_upgraded_contract(false);
}

#[ignore]
#[test]
fn should_not_emit_messages_from_account() {
    let builder = RefCell::new(LmdbWasmTestBuilder::default());
    builder
        .borrow_mut()
        .run_genesis(LOCAL_GENESIS_REQUEST.clone());

    // Request to run a deploy that tries to register a message topic without a stored contract.
    let install_request = ExecuteRequestBuilder::standard(
        *DEFAULT_ACCOUNT_ADDR,
        MESSAGE_EMITTER_FROM_ACCOUNT,
        RuntimeArgs::default(),
    )
    .build();

    // Expect to fail since topics can only be registered by stored contracts.
    builder
        .borrow_mut()
        .exec(install_request)
        .expect_failure()
        .commit();
}

#[ignore]
#[test]
fn should_charge_expected_gas_for_storage() {
    const GAS_PER_BYTE_COST: u32 = 100;

    let chainspec = {
        let wasm_v1_config = WasmV1Config::new(
            DEFAULT_WASM_MAX_MEMORY,
            DEFAULT_MAX_STACK_HEIGHT,
            OpcodeCosts::zero(),
            HostFunctionCostsV1::zero(),
        );
        let wasm_v2_config = WasmV2Config::new(
            DEFAULT_WASM_MAX_MEMORY,
            OpcodeCosts::zero(),
            HostFunctionCostsV2::zero(),
        );
        let wasm_config = WasmConfig::new(MessageLimits::default(), wasm_v1_config, wasm_v2_config);
        ChainspecConfig {
            wasm_config,
            core_config: CoreConfig::default(),
            system_costs_config: SystemConfig::default(),
            storage_costs: StorageCosts::new(GAS_PER_BYTE_COST),
        }
    };

    let builder = RefCell::new(LmdbWasmTestBuilder::new_temporary_with_config(chainspec));
    builder
        .borrow_mut()
        .run_genesis(LOCAL_GENESIS_REQUEST.clone());

    let contract_hash = install_messages_emitter_contract(&builder, true);

    let topic_name = "consume_topic";

    // check the consume of adding a new topic
    let add_topic_request = ExecuteRequestBuilder::contract_call_by_hash(
        *DEFAULT_ACCOUNT_ADDR,
        contract_hash,
        ENTRY_POINT_ADD_TOPIC,
        runtime_args! {
            ARG_TOPIC_NAME => topic_name,
        },
    )
    .build();

    builder
        .borrow_mut()
        .exec(add_topic_request)
        .expect_success()
        .commit();

    let add_topic_consumed = builder.borrow().last_exec_gas_consumed().value();

    let default_topic_summary =
        MessageTopicSummary::new(0, BlockTime::new(0), topic_name.to_string());
    let written_size_expected =
        StoredValue::MessageTopic(default_topic_summary.clone()).serialized_length();
    assert_eq!(
        U512::from(written_size_expected * GAS_PER_BYTE_COST as usize),
        add_topic_consumed
    );

    let message_topic =
        MessageTopicSummary::new(0, BlockTime::new(0), "generic_messages".to_string());
    emit_message_with_suffix(&builder, "test", &contract_hash, DEFAULT_BLOCK_TIME);
    // check that the storage consume charged is variable since the message topic hash a variable
    // string field with message size that is emitted.
    let written_size_expected = StoredValue::Message(MessageChecksum([0; 32])).serialized_length()
        + StoredValue::MessageTopic(message_topic).serialized_length()
        + StoredValue::CLValue(CLValue::from_t((BlockTime::new(0), 0u64)).unwrap())
            .serialized_length();
    let emit_message_gas_consumed = builder.borrow().last_exec_gas_consumed().value();
    assert_eq!(
        U512::from(written_size_expected * GAS_PER_BYTE_COST as usize),
        emit_message_gas_consumed
    );

    emit_message_with_suffix(&builder, "test 12345", &contract_hash, DEFAULT_BLOCK_TIME);
    let written_size_expected = StoredValue::Message(MessageChecksum([0; 32])).serialized_length()
        + StoredValue::MessageTopic(MessageTopicSummary::new(
            0,
            BlockTime::new(0),
            "generic_messages".to_string(),
        ))
        .serialized_length()
        + StoredValue::CLValue(CLValue::from_t((BlockTime::new(0), 0u64)).unwrap())
            .serialized_length();
    let emit_message_gas_consumed = builder.borrow().last_exec_gas_consumed().value();
    assert_eq!(
        U512::from(written_size_expected * GAS_PER_BYTE_COST as usize),
        emit_message_gas_consumed
    );

    // emitting messages in a different block will also prune the old entries so check the consumed.
    emit_message_with_suffix(
        &builder,
        "message in different block",
        &contract_hash,
        DEFAULT_BLOCK_TIME + 1,
    );
    let emit_message_gas_consumed = builder.borrow().last_exec_gas_consumed().value();
    assert_eq!(
        U512::from(written_size_expected * GAS_PER_BYTE_COST as usize),
        emit_message_gas_consumed
    );
}

#[ignore]
#[test]
fn should_charge_increasing_gas_consumed_for_multiple_messages_emitted() {
    const FIRST_MESSAGE_EMIT_COST: u32 = 100;
    const COST_INCREASE_PER_MESSAGE: u32 = 50;
    const fn emit_consumed_per_execution(num_messages: u32) -> u32 {
        FIRST_MESSAGE_EMIT_COST * num_messages
            + (num_messages - 1) * num_messages / 2 * COST_INCREASE_PER_MESSAGE
    }

    const MESSAGES_TO_EMIT: u32 = 4;
    const EMIT_MULTIPLE_EXPECTED_COST: u32 = emit_consumed_per_execution(MESSAGES_TO_EMIT);
    const EMIT_MESSAGES_FROM_MULTIPLE_CONTRACTS: u32 =
        emit_consumed_per_execution(EMIT_MESSAGE_FROM_EACH_VERSION_NUM_MESSAGES);
    let chainspec = {
        let wasm_v1_config = WasmV1Config::new(
            DEFAULT_WASM_MAX_MEMORY,
            DEFAULT_MAX_STACK_HEIGHT,
            OpcodeCosts::zero(),
            HostFunctionCostsV1 {
                emit_message: HostFunction::fixed(FIRST_MESSAGE_EMIT_COST),
                cost_increase_per_message: COST_INCREASE_PER_MESSAGE,
                ..Zero::zero()
            },
        );
        let wasm_v2_config = WasmV2Config::new(
            DEFAULT_WASM_MAX_MEMORY,
            OpcodeCosts::zero(),
            HostFunctionCostsV2::default(),
        );
        let wasm_config = WasmConfig::new(MessageLimits::default(), wasm_v1_config, wasm_v2_config);
        ChainspecConfig {
            wasm_config,
            core_config: CoreConfig::default(),
            system_costs_config: SystemConfig::default(),
            storage_costs: StorageCosts::zero(),
        }
    };

    let builder = RefCell::new(LmdbWasmTestBuilder::new_temporary_with_config(chainspec));
    builder
        .borrow_mut()
        .run_genesis(LOCAL_GENESIS_REQUEST.clone());

    let contract_hash = install_messages_emitter_contract(&builder, true);

    // Emit one message in this execution. Cost should be `FIRST_MESSAGE_EMIT_COST`.
    emit_message_with_suffix(&builder, "test", &contract_hash, DEFAULT_BLOCK_TIME);
    let emit_message_gas_consume = builder.borrow().last_exec_gas_consumed().value();
    assert_eq!(emit_message_gas_consume, FIRST_MESSAGE_EMIT_COST.into());

    // Emit multiple messages in this execution. Cost should increase for each message emitted.
    let emit_messages_request = ExecuteRequestBuilder::contract_call_by_hash(
        *DEFAULT_ACCOUNT_ADDR,
        contract_hash,
        ENTRY_POINT_EMIT_MULTIPLE_MESSAGES,
        runtime_args! {
            ARG_NUM_MESSAGES_TO_EMIT => MESSAGES_TO_EMIT,
        },
    )
    .build();
    builder
        .borrow_mut()
        .exec(emit_messages_request)
        .expect_success()
        .commit();

    let emit_multiple_messages_consume = builder.borrow().last_exec_gas_consumed().value();
    assert_eq!(
        emit_multiple_messages_consume,
        EMIT_MULTIPLE_EXPECTED_COST.into()
    );

    // Try another execution where we emit a single message.
    // Cost should be `FIRST_MESSAGE_EMIT_COST`
    emit_message_with_suffix(&builder, "test", &contract_hash, DEFAULT_BLOCK_TIME);
    let emit_message_gas_consume = builder.borrow().last_exec_gas_consumed().value();
    assert_eq!(emit_message_gas_consume, FIRST_MESSAGE_EMIT_COST.into());

    // Check gas consume when multiple messages are emitted from different contracts.
    let contract_hash = upgrade_messages_emitter_contract(&builder, true, false);
    let emit_message_request = ExecuteRequestBuilder::contract_call_by_hash(
        *DEFAULT_ACCOUNT_ADDR,
        contract_hash,
        ENTRY_POINT_EMIT_MESSAGE_FROM_EACH_VERSION,
        runtime_args! {
            ARG_MESSAGE_SUFFIX_NAME => "test message",
        },
    )
    .build();

    builder
        .borrow_mut()
        .exec(emit_message_request)
        .expect_success()
        .commit();

    // 3 messages are emitted by this execution so the consume would be:
    // `EMIT_MESSAGES_FROM_MULTIPLE_CONTRACTS`
    let emit_message_gas_consume = builder.borrow().last_exec_gas_consumed().value();
    assert_eq!(
        emit_message_gas_consume,
        U512::from(EMIT_MESSAGES_FROM_MULTIPLE_CONTRACTS)
    );
}

#[ignore]
#[test]
fn should_register_topic_on_contract_creation() {
    let builder = RefCell::new(LmdbWasmTestBuilder::default());
    builder
        .borrow_mut()
        .run_genesis(LOCAL_GENESIS_REQUEST.clone());

    let contract_hash = install_messages_emitter_contract(&builder, false);
    let query_view = ContractQueryView::new(&builder, contract_hash);

    let message_topics = query_view.message_topics();
    let (topic_name, message_topic_hash) = message_topics
        .iter()
        .next()
        .expect("should have at least one topic");

    assert_eq!(topic_name, &MESSAGE_EMITTER_GENERIC_TOPIC.to_string());
    // Check that the topic exists for the installed contract.
    assert_eq!(
        query_view
            .message_topic(*message_topic_hash)
            .message_count(),
        0
    );
}

#[ignore]
#[test]
fn should_not_exceed_configured_topic_name_limits_on_contract_upgrade_no_init() {
    let chainspec = {
        let default_wasm_v1_config = WasmV1Config::default();
        let default_wasm_v2_config = WasmV2Config::default();
        let wasm_v1_config = WasmV1Config::new(
            default_wasm_v1_config.max_memory(),
            default_wasm_v1_config.max_stack_height(),
            default_wasm_v1_config.opcode_costs(),
            default_wasm_v1_config.take_host_function_costs(),
        );
        let wasm_v2_config = WasmV2Config::new(
            default_wasm_v2_config.max_memory(),
            default_wasm_v2_config.opcode_costs(),
            default_wasm_v2_config.take_host_function_costs(),
        );
        let wasm_config = WasmConfig::new(
            MessageLimits {
                max_topic_name_size: 16, //length of MESSAGE_EMITTER_GENERIC_TOPIC
                max_message_size: 100,
                max_topics_per_contract: 3,
            },
            wasm_v1_config,
            wasm_v2_config,
        );
        ChainspecConfig {
            wasm_config,
            core_config: CoreConfig::default(),
            system_costs_config: SystemConfig::default(),
            storage_costs: StorageCosts::default(),
        }
    };

    let builder = RefCell::new(LmdbWasmTestBuilder::new_temporary_with_config(chainspec));
    builder
        .borrow_mut()
        .run_genesis(LOCAL_GENESIS_REQUEST.clone());

    let _ = install_messages_emitter_contract(&builder, false);
    let _ = upgrade_messages_emitter_contract(&builder, false, true);
}

#[ignore]
#[test]
fn should_not_exceed_configured_max_topics_per_contract_upgrade_no_init() {
    let chainspec = {
        let default_wasm_v1_config = WasmV1Config::default();
        let wasm_v1_config = WasmV1Config::new(
            default_wasm_v1_config.max_memory(),
            default_wasm_v1_config.max_stack_height(),
            default_wasm_v1_config.opcode_costs(),
            default_wasm_v1_config.take_host_function_costs(),
        );
        let default_wasm_v2_config = WasmV2Config::default();
        let wasm_v2_config = WasmV2Config::new(
            default_wasm_v2_config.max_memory(),
            default_wasm_v2_config.opcode_costs(),
            default_wasm_v2_config.take_host_function_costs(),
        );
        let wasm_config = WasmConfig::new(
            MessageLimits {
                max_topic_name_size: 32,
                max_message_size: 100,
                max_topics_per_contract: 1, /* only allow 1 topic. Since on upgrade previous
                                             * topics carry over, the upgrade should fail. */
            },
            wasm_v1_config,
            wasm_v2_config,
        );
        ChainspecConfig {
            wasm_config,
            system_costs_config: SystemConfig::default(),
            core_config: CoreConfig::default(),
            storage_costs: StorageCosts::default(),
        }
    };

    let builder = RefCell::new(LmdbWasmTestBuilder::new_temporary_with_config(chainspec));
    builder
        .borrow_mut()
        .run_genesis(LOCAL_GENESIS_REQUEST.clone());

    let _ = install_messages_emitter_contract(&builder, false);
    let _ = upgrade_messages_emitter_contract(&builder, false, true);
}

#[ignore]
#[test]
fn should_produce_per_block_message_ordering() {
    let builder = RefCell::new(LmdbWasmTestBuilder::default());
    builder
        .borrow_mut()
        .run_genesis(LOCAL_GENESIS_REQUEST.clone());

    let emitter_contract_hash = install_messages_emitter_contract(&builder, true);
    let query_view = ContractQueryView::new(&builder, emitter_contract_hash);

    let message_topics = query_view.message_topics();
    let (_, message_topic_hash) = message_topics
        .iter()
        .next()
        .expect("should have at least one topic");

    let assert_last_message_block_index = |expected_index: u64| {
        assert_eq!(
            builder
                .borrow()
                .get_last_exec_result()
                .unwrap()
                .messages()
                .first()
                .unwrap()
                .block_index(),
            expected_index
        )
    };

    let query_message_count = || -> Option<(BlockTime, u64)> {
        let query_result =
            builder
                .borrow_mut()
                .query(None, Key::BlockGlobal(BlockGlobalAddr::MessageCount), &[]);

        match query_result {
            Ok(StoredValue::CLValue(cl_value)) => Some(cl_value.into_t().unwrap()),
            Err(_) => None,
            _ => panic!("Stored value is not a CLvalue: {:?}", query_result),
        }
    };

    // Emit the first message in the block. It should have block index 0.
    emit_message_with_suffix(
        &builder,
        "test 0",
        &emitter_contract_hash,
        DEFAULT_BLOCK_TIME,
    );
    assert_last_message_block_index(0);
    assert_eq!(
        query_message_count(),
        Some((BlockTime::new(DEFAULT_BLOCK_TIME), 1))
    );

    let expected_message = MessagePayload::from(format!("{}{}", EMITTER_MESSAGE_PREFIX, "test 0"));
    let expected_message_hash = cryptography::blake2b(
        [
            0u64.to_bytes().unwrap(),
            expected_message.to_bytes().unwrap(),
        ]
        .concat(),
    );
    let queried_message_summary = query_view
        .message_summary(*message_topic_hash, 0, None)
        .expect("should have value")
        .value();
    assert_eq!(expected_message_hash, queried_message_summary);

    // Emit the second message in the same block. It should have block index 1.
    emit_message_with_suffix(
        &builder,
        "test 1",
        &emitter_contract_hash,
        DEFAULT_BLOCK_TIME,
    );
    assert_last_message_block_index(1);
    assert_eq!(
        query_message_count(),
        Some((BlockTime::new(DEFAULT_BLOCK_TIME), 2))
    );

    let expected_message = MessagePayload::from(format!("{}{}", EMITTER_MESSAGE_PREFIX, "test 1"));
    let expected_message_hash = cryptography::blake2b(
        [
            1u64.to_bytes().unwrap(),
            expected_message.to_bytes().unwrap(),
        ]
        .concat(),
    );
    let queried_message_summary = query_view
        .message_summary(*message_topic_hash, 1, None)
        .expect("should have value")
        .value();
    assert_eq!(expected_message_hash, queried_message_summary);

    // Upgrade the message emitter contract end emit a message from this contract in the same block
    // as before. The block index of the message should be 2 since the block hasn't changed.
    let upgraded_contract_hash = upgrade_messages_emitter_contract(&builder, true, false);
    let upgraded_contract_query_view = ContractQueryView::new(&builder, upgraded_contract_hash);

    let upgraded_topics = upgraded_contract_query_view.message_topics();
    let upgraded_message_topic_hash = upgraded_topics
        .get(MESSAGE_EMITTER_UPGRADED_TOPIC)
        .expect("should have upgraded topic");

    let emit_message_request = ExecuteRequestBuilder::contract_call_by_hash(
        *DEFAULT_ACCOUNT_ADDR,
        upgraded_contract_hash,
        "upgraded_emit_message",
        runtime_args! {
            ARG_MESSAGE_SUFFIX_NAME => "test 2",
        },
    )
    .with_block_time(DEFAULT_BLOCK_TIME)
    .build();

    builder
        .borrow_mut()
        .exec(emit_message_request)
        .expect_success()
        .commit();
    assert_last_message_block_index(2);
    assert_eq!(
        query_message_count(),
        Some((BlockTime::new(DEFAULT_BLOCK_TIME), 3))
    );

    let expected_message = MessagePayload::from(format!("{}{}", EMITTER_MESSAGE_PREFIX, "test 2"));
    let expected_message_hash = cryptography::blake2b(
        [
            2u64.to_bytes().unwrap(),
            expected_message.to_bytes().unwrap(),
        ]
        .concat(),
    );
    let queried_message_summary = upgraded_contract_query_view
        .message_summary(*upgraded_message_topic_hash, 0, None)
        .expect("should have value")
        .value();
    assert_eq!(expected_message_hash, queried_message_summary);

    // Now emit a message in a different block. The block index should be 0 since it's the first
    // message in the new block.
    emit_message_with_suffix(
        &builder,
        "test 3",
        &emitter_contract_hash,
        DEFAULT_BLOCK_TIME + 1,
    );
    assert_last_message_block_index(0);
    assert_eq!(
        query_message_count(),
        Some((BlockTime::new(DEFAULT_BLOCK_TIME + 1), 1))
    );
    let expected_message = MessagePayload::from(format!("{}{}", EMITTER_MESSAGE_PREFIX, "test 3"));
    let expected_message_hash = cryptography::blake2b(
        [
            0u64.to_bytes().unwrap(),
            expected_message.to_bytes().unwrap(),
        ]
        .concat(),
    );
    let queried_message_summary = query_view
        .message_summary(*message_topic_hash, 0, None)
        .expect("should have value")
        .value();
    assert_eq!(expected_message_hash, queried_message_summary);
}

#[ignore]
#[test]
fn emit_message_should_consume_variable_gas_based_on_topic_and_message_size() {
    const MESSAGE_EMIT_COST: u32 = 1_000_000;

    const COST_PER_MESSAGE_TOPIC_NAME_SIZE: u32 = 2;
    const COST_PER_MESSAGE_LENGTH: u32 = 1_000;
    const MESSAGE_SUFFIX: &str = "test";

    let chainspec = {
        let wasm_v1_config = WasmV1Config::new(
            DEFAULT_WASM_MAX_MEMORY,
            DEFAULT_MAX_STACK_HEIGHT,
            OpcodeCosts::zero(),
            HostFunctionCostsV1 {
                emit_message: HostFunction::new(
                    MESSAGE_EMIT_COST,
                    [
                        0,
                        COST_PER_MESSAGE_TOPIC_NAME_SIZE,
                        0,
                        COST_PER_MESSAGE_LENGTH,
                    ],
                ),
                ..Zero::zero()
            },
        );
        let wasm_v2_config = WasmV2Config::new(
            DEFAULT_WASM_MAX_MEMORY,
            OpcodeCosts::zero(),
            HostFunctionCostsV2::default(),
        );
        let wasm_config = WasmConfig::new(MessageLimits::default(), wasm_v1_config, wasm_v2_config);
        ChainspecConfig {
            wasm_config,
            core_config: CoreConfig::default(),
            system_costs_config: SystemConfig::default(),
            storage_costs: StorageCosts::zero(),
        }
    };

    let builder = RefCell::new(LmdbWasmTestBuilder::new_temporary_with_config(chainspec));
    builder
        .borrow_mut()
        .run_genesis(LOCAL_GENESIS_REQUEST.clone());

    let contract_hash = install_messages_emitter_contract(&builder, true);

    // Emit one message in this execution. Cost should be consume of the call to emit message +
    // consume charged for message topic name length + consume for message payload size.
    emit_message_with_suffix(&builder, MESSAGE_SUFFIX, &contract_hash, DEFAULT_BLOCK_TIME);
    let emit_message_gas_consume = builder.borrow().last_exec_gas_consumed().value();
    let payload: MessagePayload = format!("{}{}", EMITTER_MESSAGE_PREFIX, MESSAGE_SUFFIX).into();
    let expected_consume = MESSAGE_EMIT_COST
        + COST_PER_MESSAGE_TOPIC_NAME_SIZE * MESSAGE_EMITTER_GENERIC_TOPIC.len() as u32
        + COST_PER_MESSAGE_LENGTH * payload.serialized_length() as u32;
    assert_eq!(emit_message_gas_consume, expected_consume.into());
}
