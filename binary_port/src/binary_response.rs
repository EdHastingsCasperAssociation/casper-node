use casper_types::bytesrepr::{self, Bytes, FromBytes, ToBytes};

use crate::{
    binary_response_header::BinaryResponseHeader,
    error_code::ErrorCode,
    response_type::{PayloadEntity, ResponseType},
};

#[cfg(test)]
use casper_types::testing::TestRng;

/// The response used in the binary port protocol.
#[derive(Debug, PartialEq)]
pub struct BinaryResponse {
    /// Header of the binary response.
    header: BinaryResponseHeader,
    /// The response.
    payload: Vec<u8>,
}

impl BinaryResponse {
    /// Creates new empty binary response.
    pub fn new_empty() -> Self {
        Self {
            header: BinaryResponseHeader::new(None),
            payload: vec![],
        }
    }

    /// Creates new binary response with error code.
    pub fn new_error(error: ErrorCode) -> Self {
        BinaryResponse {
            header: BinaryResponseHeader::new_error(error),
            payload: vec![],
        }
    }

    /// Creates new binary response from raw bytes.
    pub fn from_raw_bytes(payload_type: ResponseType, payload: Vec<u8>) -> Self {
        BinaryResponse {
            header: BinaryResponseHeader::new(Some(payload_type)),
            payload,
        }
    }

    /// Creates a new binary response from a value.
    pub fn from_value<V>(val: V) -> Self
    where
        V: ToBytes + PayloadEntity,
    {
        ToBytes::to_bytes(&val).map_or(
            BinaryResponse::new_error(ErrorCode::InternalError),
            |payload| BinaryResponse {
                payload,
                header: BinaryResponseHeader::new(Some(V::RESPONSE_TYPE)),
            },
        )
    }

    /// Creates a new binary response from an optional value.
    pub fn from_option<V>(opt: Option<V>) -> Self
    where
        V: ToBytes + PayloadEntity,
    {
        match opt {
            Some(val) => Self::from_value(val),
            None => Self::new_empty(),
        }
    }

    /// Returns true if response is success.
    pub fn is_success(&self) -> bool {
        self.header.is_success()
    }

    /// Returns the error code.
    pub fn error_code(&self) -> u16 {
        self.header.error_code()
    }

    /// Returns the payload type of the response.
    pub fn returned_data_type_tag(&self) -> Option<u8> {
        self.header.returned_data_type_tag()
    }

    /// Returns true if the response means that data has not been found.
    pub fn is_not_found(&self) -> bool {
        self.header.is_not_found()
    }

    /// Returns the payload.
    pub fn payload(&self) -> &[u8] {
        self.payload.as_ref()
    }

    #[cfg(test)]
    pub(crate) fn random(rng: &mut TestRng) -> Self {
        Self {
            header: BinaryResponseHeader::random(rng),
            payload: rng.random_vec(64..128),
        }
    }
}

impl ToBytes for BinaryResponse {
    fn to_bytes(&self) -> Result<Vec<u8>, bytesrepr::Error> {
        let mut buffer = bytesrepr::allocate_buffer(self)?;
        self.write_bytes(&mut buffer)?;
        Ok(buffer)
    }

    fn write_bytes(&self, writer: &mut Vec<u8>) -> Result<(), bytesrepr::Error> {
        let BinaryResponse { header, payload } = self;

        header.write_bytes(writer)?;
        payload.write_bytes(writer)
    }

    fn serialized_length(&self) -> usize {
        self.header.serialized_length() + self.payload.serialized_length()
    }
}

impl FromBytes for BinaryResponse {
    fn from_bytes(bytes: &[u8]) -> Result<(Self, &[u8]), bytesrepr::Error> {
        let (header, remainder) = FromBytes::from_bytes(bytes)?;
        let (payload, remainder) = Bytes::from_bytes(remainder)?;

        Ok((
            BinaryResponse {
                header,
                payload: payload.into(),
            },
            remainder,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use casper_types::testing::TestRng;

    #[test]
    fn bytesrepr_roundtrip() {
        let rng = &mut TestRng::new();

        let val = BinaryResponse::random(rng);
        bytesrepr::test_serialization_roundtrip(&val);
    }
}
