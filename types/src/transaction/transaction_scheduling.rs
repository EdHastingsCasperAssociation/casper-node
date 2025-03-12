use super::serialization::CalltableSerializationEnvelope;
#[cfg(any(feature = "testing", test))]
use crate::testing::TestRng;
use crate::{
    bytesrepr::{
        Error::{self, Formatting},
        FromBytes, ToBytes,
    },
    transaction::serialization::CalltableSerializationEnvelopeBuilder,
    EraId, Timestamp,
};
use alloc::vec::Vec;
use core::fmt::{self, Display, Formatter};
#[cfg(feature = "datasize")]
use datasize::DataSize;
#[cfg(any(feature = "testing", test))]
use rand::Rng;
#[cfg(feature = "json-schema")]
use schemars::JsonSchema;
#[cfg(any(feature = "std", test))]
use serde::{Deserialize, Serialize};

/// The scheduling mode of a [`crate::Transaction`].
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Debug)]
#[cfg_attr(
    any(feature = "std", test),
    derive(Serialize, Deserialize),
    serde(deny_unknown_fields)
)]
#[cfg_attr(feature = "datasize", derive(DataSize))]
#[cfg_attr(
    feature = "json-schema",
    derive(JsonSchema),
    schemars(description = "Scheduling mode of a Transaction.")
)]
pub enum TransactionScheduling {
    /// No special scheduling applied.
    Standard,
}

impl TransactionScheduling {
    fn serialized_field_lengths(&self) -> Vec<usize> {
        match self {
            TransactionScheduling::Standard => {
                vec![crate::bytesrepr::U8_SERIALIZED_LENGTH]
            }
        }
    }

    /// Returns a random `TransactionScheduling`.
    #[cfg(any(feature = "testing", test))]
    pub fn random(rng: &mut TestRng) -> Self {
        match rng.gen_range(0..3) {
            0 => TransactionScheduling::Standard,
            _ => unreachable!(),
        }
    }
}

const TAG_FIELD_INDEX: u16 = 0;

const STANDARD_VARIANT: u8 = 0;

impl ToBytes for TransactionScheduling {
    fn to_bytes(&self) -> Result<Vec<u8>, Error> {
        match self {
            TransactionScheduling::Standard => {
                CalltableSerializationEnvelopeBuilder::new(self.serialized_field_lengths())?
                    .add_field(TAG_FIELD_INDEX, &STANDARD_VARIANT)?
                    .binary_payload_bytes()
            }
        }
    }
    fn serialized_length(&self) -> usize {
        CalltableSerializationEnvelope::estimate_size(self.serialized_field_lengths())
    }
}

impl FromBytes for TransactionScheduling {
    fn from_bytes(bytes: &[u8]) -> Result<(TransactionScheduling, &[u8]), Error> {
        let (binary_payload, remainder) = CalltableSerializationEnvelope::from_bytes(2, bytes)?;
        let window = binary_payload.start_consuming()?.ok_or(Formatting)?;
        window.verify_index(0)?;
        let (tag, window) = window.deserialize_and_maybe_next::<u8>()?;
        let to_ret = match tag {
            STANDARD_VARIANT => {
                if window.is_some() {
                    return Err(Formatting);
                }
                Ok(TransactionScheduling::Standard)
            }
            _ => Err(Formatting),
        };
        to_ret.map(|endpoint| (endpoint, remainder))
    }
}

impl Display for TransactionScheduling {
    fn fmt(&self, formatter: &mut Formatter) -> fmt::Result {
        match self {
            TransactionScheduling::Standard => write!(formatter, "schedule(standard)"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{bytesrepr, gens::transaction_scheduling_arb};
    use proptest::prelude::*;

    #[test]
    fn bytesrepr_roundtrip() {
        let rng = &mut TestRng::new();
        for _ in 0..10 {
            bytesrepr::test_serialization_roundtrip(&TransactionScheduling::random(rng));
        }
    }

    proptest! {
        #[test]
        fn generative_bytesrepr_roundtrip(val in transaction_scheduling_arb()) {
            bytesrepr::test_serialization_roundtrip(&val);
        }
    }
}
