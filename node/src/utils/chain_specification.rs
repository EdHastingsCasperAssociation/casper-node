pub(crate) mod error;
pub(crate) mod parse_toml;

use std::collections::HashSet;

use num_rational::Ratio;
use once_cell::sync::Lazy;
use tracing::{error, info, warn};

use casper_types::{
    system::auction::VESTING_SCHEDULE_LENGTH_MILLIS, Chainspec, ConsensusProtocolName, CoreConfig,
    ProtocolConfig, TimeDiff, TransactionConfig, AUCTION_LANE_ID, INSTALL_UPGRADE_LANE_ID,
    MINT_LANE_ID,
};

use crate::components::network;

static RESERVED_LANE_IDS: Lazy<Vec<u8>> =
    Lazy::new(|| vec![MINT_LANE_ID, AUCTION_LANE_ID, INSTALL_UPGRADE_LANE_ID]);

/// Returns `false` and logs errors if the values set in the config don't make sense.
#[tracing::instrument(ret, level = "info", skip(chainspec), fields(hash = % chainspec.hash()))]
pub fn validate_chainspec(chainspec: &Chainspec) -> bool {
    info!("begin chainspec validation");

    if chainspec.core_config.unbonding_delay <= chainspec.core_config.auction_delay {
        warn!(
                "unbonding delay is set to {} but it should be greater than the auction delay (currently set to {})",
                chainspec.core_config.unbonding_delay, chainspec.core_config.auction_delay);
        return false;
    }

    // If the era duration is set to zero, we will treat it as explicitly stating that eras
    // should be defined by height only.
    if chainspec.core_config.era_duration.millis() > 0
        && chainspec.core_config.era_duration
            < chainspec.core_config.minimum_block_time * chainspec.core_config.minimum_era_height
    {
        warn!("era duration is less than minimum era height * block time!");
    }

    if chainspec.core_config.consensus_protocol == ConsensusProtocolName::Highway {
        if chainspec.core_config.minimum_block_time > chainspec.highway_config.maximum_round_length
        {
            error!(
                minimum_block_time = %chainspec.core_config.minimum_block_time,
                maximum_round_length = %chainspec.highway_config.maximum_round_length,
                "minimum_block_time must be less or equal than maximum_round_length",
            );
            return false;
        }
        match chainspec.highway_config.is_valid() {
            Ok(_) => return true,
            Err(msg) => {
                error!(msg);
                return false;
            }
        }
    }

    // We don't support lookback by more than one era in the rewards scheme.
    if chainspec.core_config.minimum_era_height < chainspec.core_config.signature_rewards_max_delay
    {
        error!(
            minimum_era_height = %chainspec.core_config.minimum_era_height,
            signature_rewards_max_delay = %chainspec.core_config.signature_rewards_max_delay,
            "signature_rewards_max_delay must be less than minimum_era_height"
        );
        return false;
    }

    network::within_message_size_limit_tolerance(chainspec)
        && validate_protocol_config(&chainspec.protocol_config)
        && validate_core_config(&chainspec.core_config)
        && validate_transaction_config(&chainspec.transaction_config)
}

/// Checks whether the values set in the config make sense and returns `false` if they don't.
pub(crate) fn validate_protocol_config(_protocol_config: &ProtocolConfig) -> bool {
    true
}

/// Returns `false` if unbonding delay is not greater than auction delay to ensure
/// that `recent_era_count()` yields a value of at least 1.
pub(crate) fn validate_core_config(core_config: &CoreConfig) -> bool {
    if core_config.unbonding_delay <= core_config.auction_delay {
        warn!(
            unbonding_delay = core_config.unbonding_delay,
            auction_delay = core_config.auction_delay,
            "unbonding delay should be greater than auction delay",
        );
        return false;
    }

    // If the era duration is set to zero, we will treat it as explicitly stating that eras
    // should be defined by height only.  Warn only.
    if core_config.era_duration.millis() > 0
        && core_config.era_duration.millis()
            < core_config.minimum_era_height * core_config.minimum_block_time.millis()
    {
        warn!("era duration is less than minimum era height * round length!");
    }

    if core_config.finality_threshold_fraction <= Ratio::new(0, 1)
        || core_config.finality_threshold_fraction >= Ratio::new(1, 1)
    {
        error!(
            ftf = %core_config.finality_threshold_fraction,
            "finality threshold fraction is not in the range (0, 1)",
        );
        return false;
    }

    if core_config.finality_signature_proportion <= Ratio::new(0, 1)
        || core_config.finality_signature_proportion >= Ratio::new(1, 1)
    {
        error!(
            fsp = %core_config.finality_signature_proportion,
            "finality signature proportion is not in the range (0, 1)",
        );
        return false;
    }
    if core_config.finders_fee <= Ratio::new(0, 1) || core_config.finders_fee >= Ratio::new(1, 1) {
        error!(
            fsp = %core_config.finders_fee,
            "finder's fee proportion is not in the range (0, 1)",
        );
        return false;
    }

    if core_config.vesting_schedule_period > TimeDiff::from_millis(VESTING_SCHEDULE_LENGTH_MILLIS) {
        error!(
            vesting_schedule_millis = core_config.vesting_schedule_period.millis(),
            max_millis = VESTING_SCHEDULE_LENGTH_MILLIS,
            "vesting schedule period too long",
        );
        return false;
    }

    true
}

/// Validates `TransactionConfig` parameters
pub(crate) fn validate_transaction_config(transaction_config: &TransactionConfig) -> bool {
    // The total number of transactions should not exceed the number of approvals because each
    // transaction needs at least one approval to be valid.
    let total_txn_slots = transaction_config
        .transaction_v1_config
        .get_max_block_count();
    if transaction_config.block_max_approval_count < total_txn_slots as u32 {
        return false;
    }
    let mut seen_max_transaction_size = HashSet::new();
    if transaction_config
        .transaction_v1_config
        .wasm_lanes()
        .is_empty()
    {
        error!("Wasm lanes chainspec config is empty.");
        return false;
    }
    for wasm_lane_config in transaction_config.transaction_v1_config.wasm_lanes().iter() {
        if RESERVED_LANE_IDS.contains(&wasm_lane_config.id) {
            error!("One of the defined wasm lanes has declared an id that is reserved for system lanes. Offending lane id: {}", wasm_lane_config.id);
            return false;
        }
        let max_transaction_length = wasm_lane_config.max_transaction_length;
        if seen_max_transaction_size.contains(&max_transaction_length) {
            error!("Found wasm lane configuration that has non-unique max_transaction_length. Duplicate value: {}", max_transaction_length);
            return false;
        }
        seen_max_transaction_size.insert(max_transaction_length);
    }

    let mut seen_max_gas_prices = HashSet::new();
    for wasm_lane_config in transaction_config.transaction_v1_config.wasm_lanes().iter() {
        //No need to check reserved lanes, we just did that
        let max_transaction_gas_limit = wasm_lane_config.max_transaction_gas_limit;
        if seen_max_gas_prices.contains(&max_transaction_gas_limit) {
            error!("Found wasm lane configuration that has non-unique max_transaction_gas_limit. Duplicate value: {}", max_transaction_gas_limit);
            return false;
        }
        seen_max_gas_prices.insert(max_transaction_gas_limit);
    }
    true
}

#[cfg(test)]
mod tests {
    use std::fs;

    use num_rational::Ratio;
    use once_cell::sync::Lazy;

    use casper_types::{
        bytesrepr::FromBytes, ActivationPoint, BrTableCost, ChainspecRawBytes, ControlFlowCosts,
        CoreConfig, EraId, GlobalStateUpdate, HighwayConfig, HostFunction, HostFunctionCostsV1,
        HostFunctionCostsV2, MessageLimits, Motes, OpcodeCosts, ProtocolConfig, ProtocolVersion,
        StoredValue, TestBlockBuilder, TimeDiff, Timestamp, TransactionConfig,
        TransactionLaneDefinition, TransactionV1Config, WasmConfig, WasmV1Config, WasmV2Config,
        MINT_LANE_ID,
    };

    use super::*;
    use crate::{
        testing::init_logging,
        utils::{Loadable, RESOURCES_PATH},
    };

    const EXPECTED_GENESIS_COSTS: OpcodeCosts = OpcodeCosts {
        bit: 13,
        add: 14,
        mul: 15,
        div: 16,
        load: 17,
        store: 18,
        op_const: 19,
        local: 20,
        global: 21,
        control_flow: ControlFlowCosts {
            block: 1,
            op_loop: 2,
            op_if: 3,
            op_else: 4,
            end: 5,
            br: 6,
            br_if: 7,
            br_table: BrTableCost {
                cost: 0,
                size_multiplier: 1,
            },
            op_return: 8,
            call: 9,
            call_indirect: 10,
            drop: 11,
            select: 12,
        },
        integer_comparison: 22,
        conversion: 23,
        unreachable: 24,
        nop: 25,
        current_memory: 26,
        grow_memory: 27,
        sign: 28,
    };
    static EXPECTED_GENESIS_HOST_FUNCTION_COSTS: Lazy<HostFunctionCostsV1> =
        Lazy::new(|| HostFunctionCostsV1 {
            read_value: HostFunction::new(127, [0, 1, 0]),
            dictionary_get: HostFunction::new(128, [0, 1, 0]),
            write: HostFunction::new(140, [0, 1, 0, 2]),
            dictionary_put: HostFunction::new(141, [0, 1, 2, 3]),
            add: HostFunction::new(100, [0, 1, 2, 3]),
            new_uref: HostFunction::new(122, [0, 1, 2]),
            load_named_keys: HostFunction::new(121, [0, 1]),
            ret: HostFunction::new(133, [0, 1]),
            get_key: HostFunction::new(113, [0, 1, 2, 3, 4]),
            has_key: HostFunction::new(119, [0, 1]),
            put_key: HostFunction::new(125, [0, 1, 2, 3]),
            remove_key: HostFunction::new(132, [0, 1]),
            revert: HostFunction::new(134, [0]),
            is_valid_uref: HostFunction::new(120, [0, 1]),
            add_associated_key: HostFunction::new(101, [0, 1, 2]),
            remove_associated_key: HostFunction::new(129, [0, 1]),
            update_associated_key: HostFunction::new(139, [0, 1, 2]),
            set_action_threshold: HostFunction::new(135, [0, 1]),
            get_caller: HostFunction::new(112, [0]),
            get_blocktime: HostFunction::new(111, [0]),
            create_purse: HostFunction::new(108, [0, 1]),
            transfer_to_account: HostFunction::new(138, [0, 1, 2, 3, 4, 5, 6]),
            transfer_from_purse_to_account: HostFunction::new(136, [0, 1, 2, 3, 4, 5, 6, 7, 8]),
            transfer_from_purse_to_purse: HostFunction::new(137, [0, 1, 2, 3, 4, 5, 6, 7]),
            get_balance: HostFunction::new(110, [0, 1, 2]),
            get_phase: HostFunction::new(117, [0]),
            get_system_contract: HostFunction::new(118, [0, 1, 2]),
            get_main_purse: HostFunction::new(114, [0]),
            read_host_buffer: HostFunction::new(126, [0, 1, 2]),
            create_contract_package_at_hash: HostFunction::new(106, [0, 1]),
            create_contract_user_group: HostFunction::new(107, [0, 1, 2, 3, 4, 5, 6, 7]),
            add_contract_version: HostFunction::new(102, [0, 1, 2, 3, 4, 5, 6, 7, 8, 9]),
            add_contract_version_with_message_topics: HostFunction::new(
                102,
                [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
            ),
            add_package_version_with_message_topics: HostFunction::new(
                102,
                [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10],
            ),
            disable_contract_version: HostFunction::new(109, [0, 1, 2, 3]),
            call_contract: HostFunction::new(104, [0, 1, 2, 3, 4, 5, 6]),
            call_versioned_contract: HostFunction::new(105, [0, 1, 2, 3, 4, 5, 6, 7, 8]),
            get_named_arg_size: HostFunction::new(116, [0, 1, 2]),
            get_named_arg: HostFunction::new(115, [0, 1, 2, 3]),
            remove_contract_user_group: HostFunction::new(130, [0, 1, 2, 3]),
            provision_contract_user_group_uref: HostFunction::new(124, [0, 1, 2, 3, 4]),
            remove_contract_user_group_urefs: HostFunction::new(131, [0, 1, 2, 3, 4, 5]),
            print: HostFunction::new(123, [0, 1]),
            blake2b: HostFunction::new(133, [0, 1, 2, 3]),
            random_bytes: HostFunction::new(123, [0, 1]),
            enable_contract_version: HostFunction::new(142, [0, 1, 2, 3]),
            generic_hash: HostFunction::new(152, [0, 1, 2, 3, 4]),
            manage_message_topic: HostFunction::new(100, [0, 1, 2, 4]),
            emit_message: HostFunction::new(100, [0, 1, 2, 3]),
            cost_increase_per_message: 50,
            get_block_info: HostFunction::new(330, [0, 0]),
            recover_secp256k1: HostFunction::new(331, [0, 1, 2, 3, 4, 5]),
            verify_signature: HostFunction::new(332, [0, 1, 2, 3, 4, 5]),
        });
    static EXPECTED_GENESIS_HOST_FUNCTION_COSTS_V2: Lazy<HostFunctionCostsV2> =
        Lazy::new(|| HostFunctionCostsV2 {
            read: HostFunction::new(100, [0, 1, 2, 3, 4, 5]),
            write: HostFunction::new(101, [0, 1, 2, 3, 4]),
            remove: HostFunction::new(114, [0, 1, 2]),
            copy_input: HostFunction::new(102, [0, 1]),
            ret: HostFunction::new(103, [0, 1]),
            create: HostFunction::new(104, [0, 1, 2, 3, 4, 5, 6, 7, 8, 9]),
            env_caller: HostFunction::new(105, [0, 1, 2]),
            env_block_time: HostFunction::new(106, []),
            env_transferred_value: HostFunction::new(107, [0]),
            transfer: HostFunction::new(108, [0, 1, 2]),
            env_balance: HostFunction::new(109, [0, 1, 2, 3]),
            upgrade: HostFunction::new(110, [0, 1, 2, 3, 4, 5]),
            call: HostFunction::new(111, [0, 1, 2, 3, 4, 5, 6, 7, 8]),
            print: HostFunction::new(112, [0, 1]),
            emit: HostFunction::new(113, [0, 1, 2, 3]),
            env_info: HostFunction::new(114, [0, 1, 2]),
        });
    static EXPECTED_GENESIS_WASM_COSTS: Lazy<WasmConfig> = Lazy::new(|| {
        let wasm_v1_config = WasmV1Config::new(
            17, // initial_memory
            19, // max_stack_height
            EXPECTED_GENESIS_COSTS,
            *EXPECTED_GENESIS_HOST_FUNCTION_COSTS,
        );
        let wasm_v2_config = WasmV2Config::new(
            17, // initial_memory
            EXPECTED_GENESIS_COSTS,
            *EXPECTED_GENESIS_HOST_FUNCTION_COSTS_V2,
        );
        WasmConfig::new(MessageLimits::default(), wasm_v1_config, wasm_v2_config)
    });

    #[test]
    fn core_config_toml_roundtrip() {
        let mut rng = crate::new_rng();
        let config = CoreConfig::random(&mut rng);
        let encoded = toml::to_string_pretty(&config).unwrap();
        let decoded = toml::from_str(&encoded).unwrap();
        assert_eq!(config, decoded);
    }

    #[test]
    fn transaction_config_toml_roundtrip() {
        let mut rng = crate::new_rng();
        let config = TransactionConfig::random(&mut rng);
        let encoded = toml::to_string_pretty(&config).unwrap();
        let decoded = toml::from_str(&encoded).unwrap();
        assert_eq!(config, decoded);
    }

    #[test]
    fn protocol_config_toml_roundtrip() {
        let mut rng = crate::new_rng();
        let config = ProtocolConfig::random(&mut rng);
        let encoded = toml::to_string_pretty(&config).unwrap();
        let decoded = toml::from_str(&encoded).unwrap();
        assert_eq!(config, decoded);
    }

    #[test]
    fn highway_config_toml_roundtrip() {
        let mut rng = crate::new_rng();
        let config = HighwayConfig::random(&mut rng);
        let encoded = toml::to_string_pretty(&config).unwrap();
        let decoded = toml::from_str(&encoded).unwrap();
        assert_eq!(config, decoded);
    }

    #[test]
    fn should_validate_round_length() {
        let (mut chainspec, _) = <(Chainspec, ChainspecRawBytes)>::from_resources("local");

        // Minimum block time greater than maximum round length.
        chainspec.core_config.consensus_protocol = ConsensusProtocolName::Highway;
        chainspec.core_config.minimum_block_time = TimeDiff::from_millis(8);
        chainspec.highway_config.maximum_round_length = TimeDiff::from_millis(7);
        assert!(
            !validate_chainspec(&chainspec),
            "chainspec should not be valid"
        );

        chainspec.core_config.minimum_block_time = TimeDiff::from_millis(7);
        chainspec.highway_config.maximum_round_length = TimeDiff::from_millis(7);
        assert!(validate_chainspec(&chainspec), "chainspec should be valid");
    }

    #[ignore = "We probably need to reconsider our approach here"]
    #[test]
    fn should_have_deterministic_chainspec_hash() {
        const PATH: &str = "test/valid/0_9_0";
        const PATH_UNORDERED: &str = "test/valid/0_9_0_unordered";

        let accounts: Vec<u8> = {
            let path = RESOURCES_PATH.join(PATH).join("accounts.toml");
            fs::read(path).expect("should read file")
        };

        let accounts_unordered: Vec<u8> = {
            let path = RESOURCES_PATH.join(PATH_UNORDERED).join("accounts.toml");
            fs::read(path).expect("should read file")
        };

        // Different accounts.toml file content
        assert_ne!(accounts, accounts_unordered);

        let (chainspec, _) = <(Chainspec, ChainspecRawBytes)>::from_resources(PATH);
        let (chainspec_unordered, _) =
            <(Chainspec, ChainspecRawBytes)>::from_resources(PATH_UNORDERED);

        // Deserializes into equal objects
        assert_eq!(chainspec, chainspec_unordered);

        // With equal hashes
        assert_eq!(chainspec.hash(), chainspec_unordered.hash());
    }

    #[test]
    fn should_have_valid_finality_threshold() {
        let mut rng = crate::new_rng();
        let mut core_config = CoreConfig::random(&mut rng);
        // Should be valid for FTT > 0 and < 1.
        core_config.finality_threshold_fraction = Ratio::new(1, u64::MAX);
        assert!(
            validate_core_config(&core_config),
            "1 over max should be valid ftt"
        );
        core_config.finality_threshold_fraction = Ratio::new(u64::MAX - 1, u64::MAX);
        assert!(
            validate_core_config(&core_config),
            "less than max over max should be valid ftt"
        );
        core_config.finality_threshold_fraction = Ratio::new(0, 1);
        assert!(
            !validate_core_config(&core_config),
            "FTT == 0 or >= 1 should be invalid ftt"
        );
        core_config.finality_threshold_fraction = Ratio::new(1, 1);
        assert!(
            !validate_core_config(&core_config),
            "1 over 1 should be invalid ftt"
        );
        core_config.finality_threshold_fraction = Ratio::new(u64::MAX, u64::MAX);
        assert!(
            !validate_core_config(&core_config),
            "max over max should be invalid ftt"
        );
        core_config.finality_threshold_fraction = Ratio::new(u64::MAX, u64::MAX - 1);
        assert!(
            !validate_core_config(&core_config),
            "max over less than max should be invalid ftt"
        );
    }

    #[test]
    fn should_have_valid_transaction_counts() {
        let transaction_v1_config = TransactionV1Config::default();

        let transaction_v1_config =
            transaction_v1_config.with_count_limits(Some(100), Some(1), None, None);

        let transaction_config = TransactionConfig {
            block_max_approval_count: 100,
            transaction_v1_config,
            ..Default::default()
        };
        assert!(
            !validate_transaction_config(&transaction_config),
            "max approval count that is not at least equal to sum of `block_max_[txn type]_count`s \
            should be invalid"
        );

        let transaction_v1_config = TransactionV1Config::default();

        let transaction_v1_config =
            transaction_v1_config.with_count_limits(Some(100), Some(50), Some(25), Some(25));

        let transaction_config = TransactionConfig {
            block_max_approval_count: 200,
            transaction_v1_config,
            ..Default::default()
        };
        assert!(
            validate_transaction_config(&transaction_config),
            "max approval count equal to sum of `block_max_[txn type]_count`s should be valid"
        );

        let transaction_v1_config = TransactionV1Config::default();
        let transaction_v1_config =
            transaction_v1_config.with_count_limits(Some(100), Some(50), Some(25), Some(24));

        let transaction_config = TransactionConfig {
            block_max_approval_count: 200,
            transaction_v1_config,
            ..Default::default()
        };
        assert!(
            validate_transaction_config(&transaction_config),
            "max approval count greater than sum of `block_max_[txn type]_count`s should be valid"
        );
    }

    #[test]
    fn should_perform_checks_with_global_state_update() {
        let mut rng = crate::new_rng();
        let mut protocol_config = ProtocolConfig::random(&mut rng);

        // We force `global_state_update` to be `Some`.
        protocol_config.global_state_update = Some(GlobalStateUpdate::random(&mut rng));

        // TODO: seems like either protocol config validity should be implemented, or this sham of
        // a test should be removed.
        assert!(validate_protocol_config(&protocol_config), "currently there are no validation rules for this config, so minimal type correctness should be valid");
    }

    #[test]
    fn should_perform_checks_without_global_state_update() {
        let mut rng = crate::new_rng();
        let mut protocol_config = ProtocolConfig::random(&mut rng);

        // We force `global_state_update` to be `None`.
        protocol_config.global_state_update = None;

        // TODO: seems like either protocol config validity should be implemented, or this sham of
        // a test should be removed.
        assert!(validate_protocol_config(&protocol_config), "currently there are no validation rules for this config, so minimal type correctness should be valid");
    }

    #[test]
    fn should_recognize_blocks_before_activation_point() {
        let past_version = ProtocolVersion::from_parts(1, 0, 0);
        let current_version = ProtocolVersion::from_parts(2, 0, 0);
        let future_version = ProtocolVersion::from_parts(3, 0, 0);

        let upgrade_era = EraId::from(5);
        let previous_era = upgrade_era.saturating_sub(1);

        let rng = &mut crate::new_rng();
        let protocol_config = ProtocolConfig {
            version: current_version,
            hard_reset: false,
            activation_point: ActivationPoint::EraId(upgrade_era),
            global_state_update: None,
        };

        let block = TestBlockBuilder::new()
            .era(previous_era)
            .height(100)
            .protocol_version(past_version)
            .switch_block(true)
            .build(rng);
        assert!(
            block
                .header()
                .is_last_block_before_activation(&protocol_config),
            "The block before this protocol version: a switch block with previous era and version."
        );

        //
        let block = TestBlockBuilder::new()
            .era(upgrade_era)
            .height(100)
            .protocol_version(past_version)
            .switch_block(true)
            .build(rng);
        assert!(
            !block
                .header()
                .is_last_block_before_activation(&protocol_config),
            "Not the activation point: wrong era."
        );
        let block = TestBlockBuilder::new()
            .era(previous_era)
            .height(100)
            .protocol_version(current_version)
            .switch_block(true)
            .build(rng);
        assert!(
            !block
                .header()
                .is_last_block_before_activation(&protocol_config),
            "Not the activation point: wrong version."
        );

        let block = TestBlockBuilder::new()
            .era(previous_era)
            .height(100)
            .protocol_version(future_version)
            .switch_block(true)
            .build(rng);
        assert!(
            !block
                .header()
                .is_last_block_before_activation(&protocol_config),
            "Alleged upgrade is in the past"
        );

        let block = TestBlockBuilder::new()
            .era(previous_era)
            .height(100)
            .protocol_version(past_version)
            .switch_block(false)
            .build(rng);
        assert!(
            !block
                .header()
                .is_last_block_before_activation(&protocol_config),
            "Not the activation point: not a switch block."
        );
    }

    #[test]
    fn should_have_valid_production_chainspec() {
        init_logging();

        let (chainspec, _raw_bytes): (Chainspec, ChainspecRawBytes) =
            Loadable::from_resources("production");

        assert!(validate_chainspec(&chainspec));
    }

    fn check_spec(spec: Chainspec, is_first_version: bool) {
        if is_first_version {
            assert_eq!(
                spec.protocol_config.version,
                ProtocolVersion::from_parts(0, 9, 0)
            );
            assert_eq!(
                spec.protocol_config.activation_point.genesis_timestamp(),
                Some(Timestamp::from(1600454700000))
            );
            assert_eq!(spec.network_config.accounts_config.accounts().len(), 4);

            let accounts: Vec<_> = {
                let mut accounts = spec.network_config.accounts_config.accounts().to_vec();
                accounts.sort_by_key(|account_config| {
                    (account_config.balance(), account_config.bonded_amount())
                });
                accounts
            };

            for (index, account_config) in accounts.into_iter().enumerate() {
                assert_eq!(account_config.balance(), Motes::new(index + 1),);
                assert_eq!(
                    account_config.bonded_amount(),
                    Motes::new((index as u64 + 1) * 10)
                );
            }
        } else {
            assert_eq!(
                spec.protocol_config.version,
                ProtocolVersion::from_parts(1, 0, 0)
            );
            assert_eq!(
                spec.protocol_config.activation_point.era_id(),
                EraId::from(1)
            );
            assert!(spec.network_config.accounts_config.accounts().is_empty());
            assert!(spec.protocol_config.global_state_update.is_some());
            assert!(spec
                .protocol_config
                .global_state_update
                .as_ref()
                .unwrap()
                .validators
                .is_some());
            for value in spec
                .protocol_config
                .global_state_update
                .unwrap()
                .entries
                .values()
            {
                assert!(StoredValue::from_bytes(value).is_ok());
            }
        }

        assert_eq!(spec.network_config.name, "test-chain");

        assert_eq!(spec.core_config.era_duration, TimeDiff::from_seconds(180));
        assert_eq!(spec.core_config.minimum_era_height, 9);
        assert_eq!(
            spec.core_config.finality_threshold_fraction,
            Ratio::new(2, 25)
        );
        assert_eq!(
            spec.highway_config.maximum_round_length,
            TimeDiff::from_seconds(525)
        );

        assert_eq!(
            spec.transaction_config.deploy_config.max_payment_cost,
            Motes::new(9)
        );
        assert_eq!(
            spec.transaction_config.max_ttl,
            TimeDiff::from_seconds(26_300_160)
        );
        assert_eq!(spec.transaction_config.max_block_size, 12);
        assert_eq!(
            spec.transaction_config
                .transaction_v1_config
                .get_max_transaction_count(MINT_LANE_ID),
            125
        );
        assert_eq!(spec.transaction_config.block_gas_limit, 13);

        assert_eq!(spec.wasm_config, *EXPECTED_GENESIS_WASM_COSTS);
    }

    #[ignore = "We probably need to reconsider our approach here"]
    #[test]
    fn check_bundled_spec() {
        let (chainspec, _) = <(Chainspec, ChainspecRawBytes)>::from_resources("test/valid/0_9_0");
        check_spec(chainspec, true);
        let (chainspec, _) = <(Chainspec, ChainspecRawBytes)>::from_resources("test/valid/1_0_0");
        check_spec(chainspec, false);
    }

    #[test]
    fn should_fail_when_wasm_lanes_have_duplicate_max_transaction_length() {
        let mut v1_config = TransactionV1Config::default();
        let definition_1 = TransactionLaneDefinition {
            id: 3,
            max_transaction_length: 100,
            max_transaction_args_length: 100,
            max_transaction_gas_limit: 100,
            max_transaction_count: 10,
        };
        let definition_2 = TransactionLaneDefinition {
            id: 4,
            max_transaction_length: 10000,
            max_transaction_args_length: 100,
            max_transaction_gas_limit: 101,
            max_transaction_count: 10,
        };
        let definition_3 = TransactionLaneDefinition {
            id: 5,
            max_transaction_length: 1000,
            max_transaction_args_length: 100,
            max_transaction_gas_limit: 102,
            max_transaction_count: 10,
        };
        v1_config.set_wasm_lanes(vec![
            definition_1.clone(),
            definition_2.clone(),
            definition_3.clone(),
        ]);
        let transaction_config = TransactionConfig {
            transaction_v1_config: v1_config.clone(),
            ..Default::default()
        };
        assert!(validate_transaction_config(&transaction_config));
        let mut definition_2 = definition_2.clone();
        definition_2.max_transaction_length = definition_1.max_transaction_length;
        v1_config.set_wasm_lanes(vec![
            definition_1.clone(),
            definition_2.clone(),
            definition_3.clone(),
        ]);
        let transaction_config = TransactionConfig {
            transaction_v1_config: v1_config,
            ..Default::default()
        };
        assert!(!validate_transaction_config(&transaction_config));
    }

    #[test]
    fn should_fail_when_wasm_lanes_have_duplicate_max_gas_price() {
        let mut v1_config = TransactionV1Config::default();
        let definition_1 = TransactionLaneDefinition {
            id: 3,
            max_transaction_length: 100,
            max_transaction_args_length: 100,
            max_transaction_gas_limit: 100,
            max_transaction_count: 10,
        };
        let definition_2 = TransactionLaneDefinition {
            id: 4,
            max_transaction_length: 10000,
            max_transaction_args_length: 100,
            max_transaction_gas_limit: 101,
            max_transaction_count: 10,
        };
        let definition_3 = TransactionLaneDefinition {
            id: 5,
            max_transaction_length: 1000,
            max_transaction_args_length: 100,
            max_transaction_gas_limit: 102,
            max_transaction_count: 10,
        };
        v1_config.set_wasm_lanes(vec![
            definition_1.clone(),
            definition_2.clone(),
            definition_3.clone(),
        ]);
        let transaction_config = TransactionConfig {
            transaction_v1_config: v1_config.clone(),
            ..Default::default()
        };
        assert!(validate_transaction_config(&transaction_config));
        let mut definition_2 = definition_2.clone();
        definition_2.max_transaction_gas_limit = definition_1.max_transaction_gas_limit;
        v1_config.set_wasm_lanes(vec![
            definition_1.clone(),
            definition_2.clone(),
            definition_3.clone(),
        ]);
        let transaction_config = TransactionConfig {
            transaction_v1_config: v1_config,
            ..Default::default()
        };
        assert!(!validate_transaction_config(&transaction_config));
    }

    #[test]
    fn should_fail_when_wasm_lanes_have_reseved_ids() {
        fail_validation_with_lane_id(MINT_LANE_ID);
        fail_validation_with_lane_id(AUCTION_LANE_ID);
        fail_validation_with_lane_id(INSTALL_UPGRADE_LANE_ID);
    }

    fn fail_validation_with_lane_id(lane_id: u8) {
        let mut v1_config = TransactionV1Config::default();
        let definition_1 = TransactionLaneDefinition {
            id: lane_id,
            max_transaction_length: 100,
            max_transaction_args_length: 100,
            max_transaction_gas_limit: 100,
            max_transaction_count: 10,
        };
        v1_config.set_wasm_lanes(vec![definition_1.clone()]);
        let transaction_config = TransactionConfig {
            transaction_v1_config: v1_config.clone(),
            ..Default::default()
        };
        assert!(!validate_transaction_config(&transaction_config));
    }

    #[test]
    fn should_valid_no_wasm_lanes() {
        let mut v1_config = TransactionV1Config::default();
        v1_config.set_wasm_lanes(vec![]);
        let transaction_config = TransactionConfig {
            transaction_v1_config: v1_config.clone(),
            ..Default::default()
        };
        assert!(!validate_transaction_config(&transaction_config));
    }
}
