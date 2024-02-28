use core::fmt::{self, Formatter};

use crate::Deploy;
#[cfg(feature = "datasize")]
use datasize::DataSize;
#[cfg(feature = "json-schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The category of a Transaction.
#[derive(
    Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Serialize, Deserialize, Debug, Default,
)]
#[cfg_attr(feature = "datasize", derive(DataSize))]
#[cfg_attr(
    feature = "json-schema",
    derive(JsonSchema),
    schemars(description = "Session kind of a V1 Transaction.")
)]
#[serde(deny_unknown_fields)]
#[repr(u8)]
pub enum TransactionCategory {
    /// Standard transaction (the default).
    #[default]
    Standard = 0,
    /// Native mint interaction.
    Mint = 1,
    /// Native auction interaction.
    Auction = 2,
    /// Install or Upgrade.
    InstallUpgrade = 3,
}

impl fmt::Display for TransactionCategory {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            TransactionCategory::Standard => write!(f, "Standard"),
            TransactionCategory::Mint => write!(f, "Mint"),
            TransactionCategory::Auction => write!(f, "Auction"),
            TransactionCategory::InstallUpgrade => write!(f, "InstallUpgrade"),
        }
    }
}

impl From<&Deploy> for TransactionCategory {
    fn from(value: &Deploy) -> Self {
        if value.is_transfer() {
            TransactionCategory::Mint
        } else {
            TransactionCategory::Standard
        }
    }
}
