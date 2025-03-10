//! This file provides types to allow conversion from an EE `ExecutionResult` into a similar type
//! which can be serialized to a valid binary or JSON representation.
//!
//! It is stored as metadata related to a given transaction, and made available to clients via the
//! JSON-RPC API.

#[cfg(any(feature = "testing", test))]
use alloc::format;
use alloc::{string::String, vec::Vec};

#[cfg(feature = "datasize")]
use datasize::DataSize;
#[cfg(feature = "json-schema")]
use once_cell::sync::Lazy;
#[cfg(any(feature = "testing", test))]
use rand::Rng;
#[cfg(feature = "json-schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::Effects;
#[cfg(feature = "json-schema")]
use super::{TransformKindV2, TransformV2};
#[cfg(any(feature = "testing", test))]
use crate::testing::TestRng;
#[cfg(feature = "json-schema")]
use crate::Key;
use crate::{
    bytesrepr::{self, Error, FromBytes, ToBytes},
    Gas, InitiatorAddr, Transfer, U512,
};

#[cfg(feature = "json-schema")]
static EXECUTION_RESULT: Lazy<ExecutionResultV2> = Lazy::new(|| {
    let key1 = Key::from_formatted_str(
        "account-hash-2c4a11c062a8a337bfc97e27fd66291caeb2c65865dcb5d3ef3759c4c97efecb",
    )
    .unwrap();
    let key2 = Key::from_formatted_str(
        "deploy-af684263911154d26fa05be9963171802801a0b6aff8f199b7391eacb8edc9e1",
    )
    .unwrap();
    let mut effects = Effects::new();
    effects.push(TransformV2::new(key1, TransformKindV2::AddUInt64(8u64)));
    effects.push(TransformV2::new(key2, TransformKindV2::Identity));

    let transfers = vec![Transfer::example().clone()];

    // NOTE: these are arbitrary values for schema and type demonstration,
    // they are not properly derived actual values. Depending on current chainspec
    // settings on a given chain, we may or may not be issuing a refund and if we are
    // the percentage can vary. And the cost is affected by dynamic gas pricing
    // for a given era, within an inclusive range defined in the chainspec.
    // Thus, real values cannot be calculated in a vacuum.
    const LIMIT: u64 = 123_456;
    const CONSUMED: u64 = 100_000;
    const COST: u64 = 246_912;

    const PRICE: u8 = 2;

    let refund = COST.saturating_sub(CONSUMED);

    ExecutionResultV2 {
        initiator: InitiatorAddr::from(crate::PublicKey::example().clone()),
        error_message: None,
        current_price: PRICE,
        limit: Gas::new(LIMIT),
        consumed: Gas::new(CONSUMED),
        cost: U512::from(COST),
        refund: U512::from(refund),
        size_estimate: Transfer::example().serialized_length() as u64,
        transfers,
        effects,
    }
});

/// The result of executing a single transaction.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize, Debug)]
#[cfg_attr(feature = "datasize", derive(DataSize))]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[serde(deny_unknown_fields)]
pub struct ExecutionResultV2 {
    /// Who initiated this transaction.
    pub initiator: InitiatorAddr,
    /// If there is no error message, this execution was processed successfully.
    /// If there is an error message, this execution failed to fully process for the stated reason.
    pub error_message: Option<String>,
    /// The current gas price. I.e. how many motes are charged for each unit of computation.
    pub current_price: u8,
    /// The maximum allowed gas limit for this transaction
    pub limit: Gas,
    /// How much gas was consumed executing this transaction.
    pub consumed: Gas,
    /// How much was paid for this transaction.
    pub cost: U512,
    /// How much unconsumed gas was refunded (if any)?
    pub refund: U512,
    /// A record of transfers performed while executing this transaction.
    pub transfers: Vec<Transfer>,
    /// The size estimate of the transaction
    pub size_estimate: u64,
    /// The effects of executing this transaction.
    pub effects: Effects,
}

impl ExecutionResultV2 {
    // This method is not intended to be used by third party crates.
    #[doc(hidden)]
    #[cfg(feature = "json-schema")]
    pub fn example() -> &'static Self {
        &EXECUTION_RESULT
    }

    /// Returns a random `ExecutionResultV2`.
    #[cfg(any(feature = "testing", test))]
    pub fn random(rng: &mut TestRng) -> Self {
        let effects = Effects::random(rng);

        let transfer_count = rng.gen_range(0..6);
        let mut transfers = vec![];
        for _ in 0..transfer_count {
            transfers.push(Transfer::random(rng))
        }

        let limit = Gas::new(rng.gen::<u64>());
        let gas_price = rng.gen_range(1..6);
        // cost = the limit * the price
        let cost = limit.value() * U512::from(gas_price);
        let range = limit.value().as_u64();

        // can range from 0 to limit
        let consumed = limit
            .checked_sub(Gas::new(rng.gen_range(0..=range)))
            .expect("consumed");

        // this assumes 100% refund ratio
        let refund = cost.saturating_sub(consumed.value());

        let size_estimate = rng.gen();

        ExecutionResultV2 {
            initiator: InitiatorAddr::random(rng),
            effects,
            transfers,
            current_price: gas_price,
            cost,
            limit,
            consumed,
            refund,
            size_estimate,
            error_message: if rng.gen() {
                Some(format!("Error message {}", rng.gen::<u64>()))
            } else {
                None
            },
        }
    }
}

impl ToBytes for ExecutionResultV2 {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        let mut buffer = bytesrepr::allocate_buffer(self)?;
        self.write_bytes(&mut buffer)?;
        Ok(buffer)
    }

    fn serialized_length(&self) -> usize {
        self.initiator.serialized_length()
            + self.error_message.serialized_length()
            + self.limit.serialized_length()
            + self.consumed.serialized_length()
            + self.cost.serialized_length()
            + self.transfers.serialized_length()
            + self.size_estimate.serialized_length()
            + self.effects.serialized_length()
            + self.refund.serialized_length()
            + self.current_price.serialized_length()
    }

    fn write_bytes(&self, writer: &mut Vec<u8>) -> Result<(), Error> {
        self.initiator.write_bytes(writer)?; // initiator should logically be first
        self.error_message.write_bytes(writer)?;
        self.limit.write_bytes(writer)?;
        self.consumed.write_bytes(writer)?;
        self.cost.write_bytes(writer)?;
        self.transfers.write_bytes(writer)?;
        self.size_estimate.write_bytes(writer)?;
        self.effects.write_bytes(writer)?;
        self.refund.write_bytes(writer)?;
        self.current_price.write_bytes(writer)
    }
}

impl FromBytes for ExecutionResultV2 {
    fn from_bytes(bytes: &[u8]) -> Result<(Self, &[u8]), Error> {
        let (initiator, remainder) = InitiatorAddr::from_bytes(bytes)?;
        let (error_message, remainder) = Option::<String>::from_bytes(remainder)?;
        let (limit, remainder) = Gas::from_bytes(remainder)?;
        let (consumed, remainder) = Gas::from_bytes(remainder)?;
        let (cost, remainder) = U512::from_bytes(remainder)?;
        let (transfers, remainder) = Vec::<Transfer>::from_bytes(remainder)?;
        let (size_estimate, remainder) = FromBytes::from_bytes(remainder)?;
        let (effects, remainder) = Effects::from_bytes(remainder)?;
        // refund && current_price were added after 2.0 was upgraded into on
        // DevNet and IntegrationNet, thus the bytes repr must be appended and optional
        let (refund, remainder) = match U512::from_bytes(remainder) {
            Ok((ret, rem)) => (ret, rem),
            Err(_) => {
                let rem: &[u8] = &[];
                (U512::zero(), rem)
            }
        };
        let (current_price, remainder) = match u8::from_bytes(remainder) {
            Ok((ret, rem)) => (ret, rem),
            Err(_) => {
                let ret = {
                    let div = cost.checked_div(limit.value()).unwrap_or_default();
                    if div > U512::from(u8::MAX) {
                        u8::MAX
                    } else {
                        div.as_u32() as u8
                    }
                };

                let rem: &[u8] = &[];
                (ret, rem)
            }
        };
        let execution_result = ExecutionResultV2 {
            initiator,
            error_message,
            current_price,
            limit,
            consumed,
            cost,
            refund,
            transfers,
            size_estimate,
            effects,
        };
        Ok((execution_result, remainder))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytesrepr_roundtrip() {
        let rng = &mut TestRng::new();
        for _ in 0..10 {
            let execution_result = ExecutionResultV2::random(rng);
            bytesrepr::test_serialization_roundtrip(&execution_result);
        }
    }
}
