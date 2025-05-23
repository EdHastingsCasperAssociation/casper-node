//! The chainspec is a set of configuration options for the network.  All validators must apply the
//! same set of options in order to join and act as a peer in a given network.

mod accounts_config;
mod activation_point;
mod chainspec_raw_bytes;
mod core_config;
mod fee_handling;
pub mod genesis_config;
mod global_state_update;
mod highway_config;
mod hold_balance_handling;
mod network_config;
mod next_upgrade;
mod pricing_handling;
mod protocol_config;
mod refund_handling;
mod transaction_config;
mod upgrade_config;
mod vacancy_config;
mod vm_config;

#[cfg(any(feature = "std", test))]
use std::{fmt::Debug, sync::Arc};

#[cfg(feature = "datasize")]
use datasize::DataSize;
#[cfg(any(feature = "testing", test))]
use rand::Rng;
use serde::Serialize;
use tracing::error;

#[cfg(any(feature = "testing", test))]
use crate::testing::TestRng;
use crate::{
    bytesrepr::{self, FromBytes, ToBytes},
    ChainNameDigest, Digest, EraId, ProtocolVersion, Timestamp,
};
pub use accounts_config::{
    AccountConfig, AccountsConfig, AdministratorAccount, DelegatorConfig, GenesisAccount,
    GenesisValidator, ValidatorConfig,
};
pub use activation_point::ActivationPoint;
pub use chainspec_raw_bytes::ChainspecRawBytes;
pub use core_config::{
    ConsensusProtocolName, CoreConfig, LegacyRequiredFinality, DEFAULT_GAS_HOLD_INTERVAL,
    DEFAULT_MINIMUM_BID_AMOUNT,
};
#[cfg(any(feature = "std", test))]
pub use core_config::{
    DEFAULT_BASELINE_MOTES_AMOUNT, DEFAULT_FEE_HANDLING, DEFAULT_REFUND_HANDLING,
};
pub use fee_handling::FeeHandling;
#[cfg(any(feature = "std", test))]
pub use genesis_config::GenesisConfig;
pub use global_state_update::{GlobalStateUpdate, GlobalStateUpdateConfig, GlobalStateUpdateError};
pub use highway_config::HighwayConfig;
pub use hold_balance_handling::HoldBalanceHandling;
pub use network_config::NetworkConfig;
pub use next_upgrade::NextUpgrade;
pub use pricing_handling::PricingHandling;
pub use protocol_config::ProtocolConfig;
pub use refund_handling::RefundHandling;
pub use transaction_config::{
    DeployConfig, TransactionConfig, TransactionLaneDefinition, TransactionV1Config,
};
#[cfg(any(feature = "testing", test))]
pub use transaction_config::{
    DEFAULT_LARGE_TRANSACTION_GAS_LIMIT, DEFAULT_MAX_PAYMENT_MOTES, DEFAULT_MIN_TRANSFER_MOTES,
};
pub use upgrade_config::ProtocolUpgradeConfig;
pub use vacancy_config::VacancyConfig;
pub use vm_config::{
    AuctionCosts, BrTableCost, ChainspecRegistry, ControlFlowCosts, HandlePaymentCosts,
    HostFunction, HostFunctionCost, HostFunctionCostsV1, HostFunctionCostsV2, HostFunctionV2,
    MessageLimits, MintCosts, OpcodeCosts, StandardPaymentCosts, StorageCosts, SystemConfig,
    WasmConfig, WasmV1Config, WasmV2Config, DEFAULT_HOST_FUNCTION_NEW_DICTIONARY,
};
#[cfg(any(feature = "testing", test))]
pub use vm_config::{
    DEFAULT_ADD_BID_COST, DEFAULT_ADD_COST, DEFAULT_BIT_COST, DEFAULT_CONST_COST,
    DEFAULT_CONTROL_FLOW_BLOCK_OPCODE, DEFAULT_CONTROL_FLOW_BR_IF_OPCODE,
    DEFAULT_CONTROL_FLOW_BR_OPCODE, DEFAULT_CONTROL_FLOW_BR_TABLE_MULTIPLIER,
    DEFAULT_CONTROL_FLOW_BR_TABLE_OPCODE, DEFAULT_CONTROL_FLOW_CALL_INDIRECT_OPCODE,
    DEFAULT_CONTROL_FLOW_CALL_OPCODE, DEFAULT_CONTROL_FLOW_DROP_OPCODE,
    DEFAULT_CONTROL_FLOW_ELSE_OPCODE, DEFAULT_CONTROL_FLOW_END_OPCODE,
    DEFAULT_CONTROL_FLOW_IF_OPCODE, DEFAULT_CONTROL_FLOW_LOOP_OPCODE,
    DEFAULT_CONTROL_FLOW_RETURN_OPCODE, DEFAULT_CONTROL_FLOW_SELECT_OPCODE,
    DEFAULT_CONVERSION_COST, DEFAULT_CURRENT_MEMORY_COST, DEFAULT_DELEGATE_COST, DEFAULT_DIV_COST,
    DEFAULT_GLOBAL_COST, DEFAULT_GROW_MEMORY_COST, DEFAULT_INTEGER_COMPARISON_COST,
    DEFAULT_LOAD_COST, DEFAULT_LOCAL_COST, DEFAULT_MAX_STACK_HEIGHT, DEFAULT_MUL_COST,
    DEFAULT_NEW_DICTIONARY_COST, DEFAULT_NOP_COST, DEFAULT_STORE_COST, DEFAULT_TRANSFER_COST,
    DEFAULT_UNREACHABLE_COST, DEFAULT_WASM_MAX_MEMORY,
};

/// A collection of configuration settings describing the state of the system at genesis and after
/// upgrades to basic system functionality occurring after genesis.
#[derive(Clone, PartialEq, Eq, Serialize, Debug, Default)]
#[cfg_attr(feature = "datasize", derive(DataSize))]
#[serde(deny_unknown_fields)]
pub struct Chainspec {
    /// Protocol config.
    #[serde(rename = "protocol")]
    pub protocol_config: ProtocolConfig,

    /// Network config.
    #[serde(rename = "network")]
    pub network_config: NetworkConfig,

    /// Core config.
    #[serde(rename = "core")]
    pub core_config: CoreConfig,

    /// Highway config.
    #[serde(rename = "highway")]
    pub highway_config: HighwayConfig,

    /// Transaction Config.
    #[serde(rename = "transactions")]
    pub transaction_config: TransactionConfig,

    /// Wasm config.
    #[serde(rename = "wasm")]
    pub wasm_config: WasmConfig,

    /// System costs config.
    #[serde(rename = "system_costs")]
    pub system_costs_config: SystemConfig,

    /// Vacancy behavior config
    #[serde(rename = "vacancy")]
    pub vacancy_config: VacancyConfig,

    /// Storage costs.
    pub storage_costs: StorageCosts,
}

impl Chainspec {
    /// Returns the hash of the chainspec's name.
    pub fn name_hash(&self) -> ChainNameDigest {
        ChainNameDigest::from_chain_name(&self.network_config.name)
    }

    /// Serializes `self` and hashes the resulting bytes.
    pub fn hash(&self) -> Digest {
        let serialized_chainspec = self.to_bytes().unwrap_or_else(|error| {
            error!(%error, "failed to serialize chainspec");
            vec![]
        });
        Digest::hash(serialized_chainspec)
    }

    /// Serializes `self` and hashes the resulting bytes, if able.
    pub fn try_hash(&self) -> Result<Digest, String> {
        let arr = self
            .to_bytes()
            .map_err(|_| "failed to serialize chainspec".to_string())?;
        Ok(Digest::hash(arr))
    }

    /// Returns the protocol version of the chainspec.
    pub fn protocol_version(&self) -> ProtocolVersion {
        self.protocol_config.version
    }

    /// Returns the era ID of where we should reset back to.  This means stored blocks in that and
    /// subsequent eras are deleted from storage.
    pub fn hard_reset_to_start_of_era(&self) -> Option<EraId> {
        self.protocol_config
            .hard_reset
            .then(|| self.protocol_config.activation_point.era_id())
    }

    /// Creates an upgrade config instance from parts.
    pub fn upgrade_config_from_parts(
        &self,
        pre_state_hash: Digest,
        current_protocol_version: ProtocolVersion,
        era_id: EraId,
        chainspec_raw_bytes: Arc<ChainspecRawBytes>,
    ) -> Result<ProtocolUpgradeConfig, String> {
        let chainspec_registry = ChainspecRegistry::new_with_optional_global_state(
            chainspec_raw_bytes.chainspec_bytes(),
            chainspec_raw_bytes.maybe_global_state_bytes(),
        );
        let global_state_update = match self.protocol_config.get_update_mapping() {
            Ok(global_state_update) => global_state_update,
            Err(err) => {
                return Err(format!("failed to generate global state update: {}", err));
            }
        };
        let fee_handling = self.core_config.fee_handling;
        let validator_minimum_bid_amount = self.core_config.minimum_bid_amount;
        let maximum_delegation_amount = self.core_config.maximum_delegation_amount;
        let minimum_delegation_amount = self.core_config.minimum_delegation_amount;
        let enable_addressable_entity = self.core_config.enable_addressable_entity;

        Ok(ProtocolUpgradeConfig::new(
            pre_state_hash,
            current_protocol_version,
            self.protocol_config.version,
            Some(era_id),
            Some(self.core_config.gas_hold_balance_handling),
            Some(self.core_config.gas_hold_interval.millis()),
            Some(self.core_config.validator_slots),
            Some(self.core_config.auction_delay),
            Some(self.core_config.locked_funds_period.millis()),
            Some(self.core_config.round_seigniorage_rate),
            Some(self.core_config.unbonding_delay),
            global_state_update,
            chainspec_registry,
            fee_handling,
            validator_minimum_bid_amount,
            maximum_delegation_amount,
            minimum_delegation_amount,
            enable_addressable_entity,
        ))
    }

    /// Returns balance hold epoch based upon configured hold interval, calculated from the imputed
    /// timestamp.
    pub fn balance_holds_epoch(&self, timestamp: Timestamp) -> u64 {
        timestamp
            .millis()
            .saturating_sub(self.core_config.gas_hold_interval.millis())
    }

    /// Is the given transaction lane supported.
    pub fn is_supported(&self, lane: u8) -> bool {
        self.transaction_config
            .transaction_v1_config
            .is_supported(lane)
    }

    /// Returns the max serialized for the given category.
    pub fn get_max_serialized_length_by_category(&self, lane: u8) -> u64 {
        self.transaction_config
            .transaction_v1_config
            .get_max_serialized_length(lane)
    }

    /// Returns the max args length for the given category.
    pub fn get_max_args_length_by_category(&self, lane: u8) -> u64 {
        self.transaction_config
            .transaction_v1_config
            .get_max_args_length(lane)
    }

    /// Returns the max gas limit for the given category.
    pub fn get_max_gas_limit_by_category(&self, lane: u8) -> u64 {
        self.transaction_config
            .transaction_v1_config
            .get_max_transaction_gas_limit(lane)
    }

    /// Returns the max transaction count for the given category.
    pub fn get_max_transaction_count_by_category(&self, lane: u8) -> u64 {
        self.transaction_config
            .transaction_v1_config
            .get_max_transaction_count(lane)
    }
}

#[cfg(any(feature = "testing", test))]
impl Chainspec {
    /// Generates a random instance using a `TestRng`.
    pub fn random(rng: &mut TestRng) -> Self {
        let protocol_config = ProtocolConfig::random(rng);
        let network_config = NetworkConfig::random(rng);
        let core_config = CoreConfig::random(rng);
        let highway_config = HighwayConfig::random(rng);
        let transaction_config = TransactionConfig::random(rng);
        let wasm_config = rng.gen();
        let system_costs_config = SystemConfig::random(rng);
        let vacancy_config = VacancyConfig::random(rng);

        Chainspec {
            protocol_config,
            network_config,
            core_config,
            highway_config,
            transaction_config,
            wasm_config,
            system_costs_config,
            vacancy_config,
            storage_costs: rng.gen(),
        }
    }

    /// Set the chain name;
    pub fn with_chain_name(&mut self, chain_name: String) -> &mut Self {
        self.network_config.name = chain_name;
        self
    }

    /// Set max associated keys.
    pub fn with_max_associated_keys(&mut self, max_associated_keys: u32) -> &mut Self {
        self.core_config.max_associated_keys = max_associated_keys;
        self
    }

    /// Set pricing handling.
    pub fn with_pricing_handling(&mut self, pricing_handling: PricingHandling) -> &mut Self {
        self.core_config.pricing_handling = pricing_handling;
        self
    }

    /// Set allow prepaid.
    pub fn with_allow_prepaid(&mut self, allow_prepaid: bool) -> &mut Self {
        self.core_config.allow_prepaid = allow_prepaid;
        self
    }

    /// Set block gas limit.
    pub fn with_block_gas_limit(&mut self, block_gas_limit: u64) -> &mut Self {
        self.transaction_config.block_gas_limit = block_gas_limit;
        self
    }
}

impl ToBytes for Chainspec {
    fn write_bytes(&self, writer: &mut Vec<u8>) -> Result<(), bytesrepr::Error> {
        self.protocol_config.write_bytes(writer)?;
        self.network_config.write_bytes(writer)?;
        self.core_config.write_bytes(writer)?;
        self.highway_config.write_bytes(writer)?;
        self.transaction_config.write_bytes(writer)?;
        self.wasm_config.write_bytes(writer)?;
        self.system_costs_config.write_bytes(writer)?;
        self.vacancy_config.write_bytes(writer)?;
        self.storage_costs.write_bytes(writer)
    }

    fn to_bytes(&self) -> Result<Vec<u8>, bytesrepr::Error> {
        let mut buffer = bytesrepr::allocate_buffer(self)?;
        self.write_bytes(&mut buffer)?;
        Ok(buffer)
    }

    fn serialized_length(&self) -> usize {
        self.protocol_config.serialized_length()
            + self.network_config.serialized_length()
            + self.core_config.serialized_length()
            + self.highway_config.serialized_length()
            + self.transaction_config.serialized_length()
            + self.wasm_config.serialized_length()
            + self.system_costs_config.serialized_length()
            + self.vacancy_config.serialized_length()
            + self.storage_costs.serialized_length()
    }
}

impl FromBytes for Chainspec {
    fn from_bytes(bytes: &[u8]) -> Result<(Self, &[u8]), bytesrepr::Error> {
        let (protocol_config, remainder) = ProtocolConfig::from_bytes(bytes)?;
        let (network_config, remainder) = NetworkConfig::from_bytes(remainder)?;
        let (core_config, remainder) = CoreConfig::from_bytes(remainder)?;
        let (highway_config, remainder) = HighwayConfig::from_bytes(remainder)?;
        let (transaction_config, remainder) = TransactionConfig::from_bytes(remainder)?;
        let (wasm_config, remainder) = WasmConfig::from_bytes(remainder)?;
        let (system_costs_config, remainder) = SystemConfig::from_bytes(remainder)?;
        let (vacancy_config, remainder) = VacancyConfig::from_bytes(remainder)?;
        let (storage_costs, remainder) = FromBytes::from_bytes(remainder)?;
        let chainspec = Chainspec {
            protocol_config,
            network_config,
            core_config,
            highway_config,
            transaction_config,
            wasm_config,
            system_costs_config,
            vacancy_config,
            storage_costs,
        };
        Ok((chainspec, remainder))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use rand::SeedableRng;

    #[test]
    fn bytesrepr_roundtrip() {
        let mut rng = TestRng::from_entropy();
        let chainspec = Chainspec::random(&mut rng);
        bytesrepr::test_serialization_roundtrip(&chainspec);
    }
}
