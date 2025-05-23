mod deploy_config;
mod runtime_config;
mod transaction_v1_config;

#[cfg(feature = "datasize")]
use datasize::DataSize;
#[cfg(any(feature = "testing", test))]
use rand::Rng;
use runtime_config::RuntimeConfig;
use serde::{Deserialize, Serialize};

#[cfg(any(feature = "testing", test))]
use crate::testing::TestRng;
use crate::{
    bytesrepr::{self, FromBytes, ToBytes},
    TimeDiff,
};

pub use deploy_config::DeployConfig;
#[cfg(any(feature = "testing", test))]
pub use deploy_config::DEFAULT_MAX_PAYMENT_MOTES;
#[cfg(any(feature = "testing", test))]
pub use transaction_v1_config::DEFAULT_LARGE_TRANSACTION_GAS_LIMIT;
pub use transaction_v1_config::{TransactionLaneDefinition, TransactionV1Config};

/// The default minimum number of motes that can be transferred.
pub const DEFAULT_MIN_TRANSFER_MOTES: u64 = 2_500_000_000;

/// Configuration values associated with Transactions.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize, Debug)]
#[cfg_attr(feature = "datasize", derive(DataSize))]
// Disallow unknown fields to ensure config files and command-line overrides contain valid keys.
#[serde(deny_unknown_fields)]
pub struct TransactionConfig {
    /// Maximum time to live any transaction can specify.
    pub max_ttl: TimeDiff,
    /// Maximum number of approvals (signatures) allowed in a block across all transactions.
    pub block_max_approval_count: u32,
    /// Maximum possible size in bytes of a block.
    pub max_block_size: u32,
    /// Maximum sum of payment across all transactions included in a block.
    pub block_gas_limit: u64,
    /// Minimum token amount for a native transfer deploy or transaction (a transfer deploy or
    /// transaction received with an transfer amount less than this will be rejected upon receipt).
    pub native_transfer_minimum_motes: u64,
    /// Maximum value to which `transaction_acceptor.timestamp_leeway` can be set in the
    /// config.toml file.
    pub max_timestamp_leeway: TimeDiff,
    /// Configuration values specific to Deploy transactions.
    #[serde(rename = "deploy")]
    pub deploy_config: DeployConfig,
    /// Configuration of the transaction runtime.
    /// Configuration values specific to V1 transactions.
    #[serde(rename = "v1")]
    pub transaction_v1_config: TransactionV1Config,
    /// Configuration values specific to the runtime.
    ///
    /// This is where we specify which runtimes are available.
    #[serde(rename = "enabled_runtime")]
    pub runtime_config: RuntimeConfig,
}

#[cfg(any(all(feature = "std", feature = "testing"), test))]
impl TransactionConfig {
    /// Generates a random instance using a `TestRng`.
    pub fn random(rng: &mut TestRng) -> Self {
        let max_ttl = TimeDiff::from_seconds(rng.gen_range(60..3_600));
        let block_max_approval_count = rng.gen();
        let max_block_size = rng.gen_range(1_000_000..1_000_000_000);
        let block_gas_limit = rng.gen_range(100_000_000_000..1_000_000_000_000_000);
        let native_transfer_minimum_motes =
            rng.gen_range(DEFAULT_MIN_TRANSFER_MOTES..1_000_000_000_000_000);
        let max_timestamp_leeway = TimeDiff::from_seconds(rng.gen_range(0..6));
        let deploy_config = DeployConfig::random(rng);
        let transaction_v1_config: TransactionV1Config = TransactionV1Config::random(rng);
        let runtime_config = RuntimeConfig::random(rng);

        TransactionConfig {
            max_ttl,
            block_max_approval_count,
            max_block_size,
            block_gas_limit,
            native_transfer_minimum_motes,
            max_timestamp_leeway,
            deploy_config,
            transaction_v1_config,
            runtime_config,
        }
    }
}

impl Default for TransactionConfig {
    fn default() -> Self {
        let two_hours = TimeDiff::from_seconds(2 * 60 * 60);
        TransactionConfig {
            max_ttl: two_hours,
            block_max_approval_count: 2600,
            max_block_size: 10_485_760,
            block_gas_limit: 10_000_000_000_000,
            native_transfer_minimum_motes: DEFAULT_MIN_TRANSFER_MOTES,
            max_timestamp_leeway: TimeDiff::from_seconds(5),
            deploy_config: DeployConfig::default(),
            runtime_config: RuntimeConfig {
                vm_casper_v1: true,
                vm_casper_v2: false,
            },
            transaction_v1_config: TransactionV1Config::default(),
        }
    }
}

impl ToBytes for TransactionConfig {
    fn write_bytes(&self, writer: &mut Vec<u8>) -> Result<(), bytesrepr::Error> {
        self.max_ttl.write_bytes(writer)?;
        self.block_max_approval_count.write_bytes(writer)?;
        self.max_block_size.write_bytes(writer)?;
        self.block_gas_limit.write_bytes(writer)?;
        self.native_transfer_minimum_motes.write_bytes(writer)?;
        self.max_timestamp_leeway.write_bytes(writer)?;
        self.deploy_config.write_bytes(writer)?;
        self.runtime_config.write_bytes(writer)?;
        self.transaction_v1_config.write_bytes(writer)
    }

    fn to_bytes(&self) -> Result<Vec<u8>, bytesrepr::Error> {
        let mut buffer = bytesrepr::allocate_buffer(self)?;
        self.write_bytes(&mut buffer)?;
        Ok(buffer)
    }

    fn serialized_length(&self) -> usize {
        self.max_ttl.serialized_length()
            + self.block_max_approval_count.serialized_length()
            + self.max_block_size.serialized_length()
            + self.block_gas_limit.serialized_length()
            + self.native_transfer_minimum_motes.serialized_length()
            + self.max_timestamp_leeway.serialized_length()
            + self.deploy_config.serialized_length()
            + self.runtime_config.serialized_length()
            + self.transaction_v1_config.serialized_length()
    }
}

impl FromBytes for TransactionConfig {
    fn from_bytes(bytes: &[u8]) -> Result<(Self, &[u8]), bytesrepr::Error> {
        let (max_ttl, remainder) = TimeDiff::from_bytes(bytes)?;
        let (block_max_approval_count, remainder) = u32::from_bytes(remainder)?;
        let (max_block_size, remainder) = u32::from_bytes(remainder)?;
        let (block_gas_limit, remainder) = u64::from_bytes(remainder)?;
        let (native_transfer_minimum_motes, remainder) = u64::from_bytes(remainder)?;
        let (max_timestamp_leeway, remainder) = TimeDiff::from_bytes(remainder)?;
        let (deploy_config, remainder) = DeployConfig::from_bytes(remainder)?;
        let (runtime_config, remainder) = RuntimeConfig::from_bytes(remainder)?;
        let (transaction_v1_config, remainder) = TransactionV1Config::from_bytes(remainder)?;

        let config = TransactionConfig {
            max_ttl,
            block_max_approval_count,
            max_block_size,
            block_gas_limit,
            native_transfer_minimum_motes,
            max_timestamp_leeway,
            deploy_config,
            runtime_config,
            transaction_v1_config,
        };
        Ok((config, remainder))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytesrepr_roundtrip() {
        let mut rng = TestRng::new();
        let config = TransactionConfig::random(&mut rng);
        bytesrepr::test_serialization_roundtrip(&config);
    }
}
