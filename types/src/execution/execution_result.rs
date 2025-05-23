use alloc::{boxed::Box, string::String, vec::Vec};

#[cfg(feature = "datasize")]
use datasize::DataSize;
#[cfg(any(feature = "testing", test))]
use rand::distributions::Distribution;
#[cfg(any(feature = "testing", test))]
use rand::Rng;
#[cfg(feature = "json-schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tracing::error;

use super::{ExecutionResultV1, ExecutionResultV2};
#[cfg(any(feature = "testing", test))]
use crate::testing::TestRng;
use crate::{
    bytesrepr::{self, FromBytes, ToBytes, U8_SERIALIZED_LENGTH},
    Transfer, U512,
};

const V1_TAG: u8 = 0;
const V2_TAG: u8 = 1;

/// The versioned result of executing a single deploy.
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize, Debug)]
#[cfg_attr(feature = "datasize", derive(DataSize))]
#[cfg_attr(feature = "json-schema", derive(JsonSchema))]
#[serde(deny_unknown_fields)]
pub enum ExecutionResult {
    /// Version 1 of execution result type.
    #[serde(rename = "Version1")]
    V1(ExecutionResultV1),
    /// Version 2 of execution result type.
    #[serde(rename = "Version2")]
    V2(Box<ExecutionResultV2>),
}

impl ExecutionResult {
    /// Returns cost.
    pub fn cost(&self) -> U512 {
        match self {
            ExecutionResult::V1(result) => result.cost(),
            ExecutionResult::V2(result) => result.cost,
        }
    }

    /// Returns consumed amount.
    pub fn consumed(&self) -> U512 {
        match self {
            ExecutionResult::V1(result) => result.cost(),
            ExecutionResult::V2(result) => result.consumed.value(),
        }
    }

    /// Returns refund amount.
    pub fn refund(&self) -> Option<U512> {
        match self {
            ExecutionResult::V1(_) => None,
            ExecutionResult::V2(result) => Some(result.refund),
        }
    }

    /// Returns a random ExecutionResult.
    #[cfg(any(feature = "testing", test))]
    pub fn random(rng: &mut TestRng) -> Self {
        if rng.gen_bool(0.5) {
            Self::V1(rand::distributions::Standard.sample(rng))
        } else {
            Self::V2(Box::new(ExecutionResultV2::random(rng)))
        }
    }

    /// Returns the error message, if any.
    pub fn error_message(&self) -> Option<String> {
        match self {
            ExecutionResult::V1(v1) => match v1 {
                ExecutionResultV1::Failure { error_message, .. } => Some(error_message.clone()),
                ExecutionResultV1::Success { .. } => None,
            },
            ExecutionResult::V2(v2) => v2.error_message.clone(),
        }
    }

    /// Returns transfers, if any.
    pub fn transfers(&self) -> Vec<Transfer> {
        match self {
            ExecutionResult::V1(_) => {
                vec![]
            }
            ExecutionResult::V2(execution_result) => execution_result.transfers.clone(),
        }
    }
}

impl From<ExecutionResultV1> for ExecutionResult {
    fn from(value: ExecutionResultV1) -> Self {
        ExecutionResult::V1(value)
    }
}

impl From<ExecutionResultV2> for ExecutionResult {
    fn from(value: ExecutionResultV2) -> Self {
        ExecutionResult::V2(Box::new(value))
    }
}

impl ToBytes for ExecutionResult {
    fn to_bytes(&self) -> Result<Vec<u8>, bytesrepr::Error> {
        let mut buffer = bytesrepr::allocate_buffer(self)?;
        self.write_bytes(&mut buffer)?;
        Ok(buffer)
    }

    fn serialized_length(&self) -> usize {
        U8_SERIALIZED_LENGTH
            + match self {
                ExecutionResult::V1(result) => result.serialized_length(),
                ExecutionResult::V2(result) => result.serialized_length(),
            }
    }

    fn write_bytes(&self, writer: &mut Vec<u8>) -> Result<(), bytesrepr::Error> {
        match self {
            ExecutionResult::V1(result) => {
                V1_TAG.write_bytes(writer)?;
                result.write_bytes(writer)
            }
            ExecutionResult::V2(result) => {
                V2_TAG.write_bytes(writer)?;
                result.write_bytes(writer)
            }
        }
    }
}

impl FromBytes for ExecutionResult {
    fn from_bytes(bytes: &[u8]) -> Result<(Self, &[u8]), bytesrepr::Error> {
        if bytes.is_empty() {
            error!("FromBytes for ExecutionResult: bytes length should not be 0");
        }
        let (tag, remainder) = match u8::from_bytes(bytes) {
            Ok((tag, rem)) => (tag, rem),
            Err(err) => {
                error!(%err, "FromBytes for ExecutionResult");
                return Err(err);
            }
        };
        match tag {
            V1_TAG => {
                let (result, remainder) = ExecutionResultV1::from_bytes(remainder)?;
                Ok((ExecutionResult::V1(result), remainder))
            }
            V2_TAG => {
                let (result, remainder) = ExecutionResultV2::from_bytes(remainder)?;
                Ok((ExecutionResult::V2(Box::new(result)), remainder))
            }
            _ => {
                error!(%tag, rem_len = remainder.len(), "FromBytes for ExecutionResult: unknown tag");
                Err(bytesrepr::Error::Formatting)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use rand::Rng;

    use super::*;
    use crate::testing::TestRng;

    #[test]
    fn bytesrepr_roundtrip() {
        let rng = &mut TestRng::new();
        let execution_result = ExecutionResult::V1(rng.gen());
        bytesrepr::test_serialization_roundtrip(&execution_result);
        let execution_result = ExecutionResult::from(ExecutionResultV2::random(rng));
        bytesrepr::test_serialization_roundtrip(&execution_result);
    }

    #[test]
    fn bincode_roundtrip() {
        let rng = &mut TestRng::new();
        let execution_result = ExecutionResult::V1(rng.gen());
        let serialized = bincode::serialize(&execution_result).unwrap();
        let deserialized = bincode::deserialize(&serialized).unwrap();
        assert_eq!(execution_result, deserialized);

        let execution_result = ExecutionResult::from(ExecutionResultV2::random(rng));
        let serialized = bincode::serialize(&execution_result).unwrap();
        let deserialized = bincode::deserialize(&serialized).unwrap();
        assert_eq!(execution_result, deserialized);
    }

    #[test]
    fn json_roundtrip() {
        let rng = &mut TestRng::new();
        let execution_result = ExecutionResult::V1(rng.gen());
        let serialized = serde_json::to_string(&execution_result).unwrap();
        let deserialized = serde_json::from_str(&serialized).unwrap();
        assert_eq!(execution_result, deserialized);

        let execution_result = ExecutionResult::from(ExecutionResultV2::random(rng));
        let serialized = serde_json::to_string(&execution_result).unwrap();
        println!("{:#}", serialized);
        let deserialized = serde_json::from_str(&serialized).unwrap();
        assert_eq!(execution_result, deserialized);
    }
}
