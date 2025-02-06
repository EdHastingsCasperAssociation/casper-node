use casper_types::bytesrepr::{self, Bytes, FromBytes, ToBytes};

use crate::{binary_response::BinaryResponse, response_type::PayloadEntity, ResponseType};

use crate::record_id::RecordId;
#[cfg(test)]
use casper_types::testing::TestRng;

/// The binary response along with the original binary request attached.
#[derive(Debug, PartialEq)]
pub struct BinaryResponseAndRequest {
    /// Context of the original request.
    request: Bytes,
    /// The response.
    response: BinaryResponse,
}

impl BinaryResponseAndRequest {
    /// Creates new binary response with the original request attached.
    pub fn new(data: BinaryResponse, request: Bytes) -> Self {
        Self {
            request,
            response: data,
        }
    }

    /// Returns a new binary response with specified data and no original request.
    pub fn new_test_response<A: PayloadEntity + ToBytes>(
        record_id: RecordId,
        data: &A,
    ) -> BinaryResponseAndRequest {
        let response = BinaryResponse::from_raw_bytes(
            ResponseType::from_record_id(record_id, false),
            data.to_bytes().unwrap(),
        );
        Self::new(response, Bytes::from(vec![]))
    }

    /// Returns a new binary response with specified legacy data and no original request.
    pub fn new_legacy_test_response<A: PayloadEntity + serde::Serialize>(
        record_id: RecordId,
        data: &A,
    ) -> BinaryResponseAndRequest {
        let response = BinaryResponse::from_raw_bytes(
            ResponseType::from_record_id(record_id, true),
            bincode::serialize(data).unwrap(),
        );
        Self::new(response, Bytes::from(vec![]))
    }

    /// Returns true if response is success.
    pub fn is_success(&self) -> bool {
        self.response.is_success()
    }

    /// Returns the error code.
    pub fn error_code(&self) -> u16 {
        self.response.error_code()
    }

    #[cfg(test)]
    pub(crate) fn random(rng: &mut TestRng) -> Self {
        let bytes = vec![1; 155];
        Self {
            request: Bytes::from(bytes),
            response: BinaryResponse::random(rng),
        }
    }

    /// Returns serialized bytes representing the original request.
    pub fn request(&self) -> &[u8] {
        &self.request
    }

    /// Returns the inner binary response.
    pub fn response(&self) -> &BinaryResponse {
        &self.response
    }
}

impl ToBytes for BinaryResponseAndRequest {
    fn to_bytes(&self) -> Result<Vec<u8>, bytesrepr::Error> {
        let mut buffer = bytesrepr::allocate_buffer(self)?;
        self.write_bytes(&mut buffer)?;
        Ok(buffer)
    }

    fn write_bytes(&self, writer: &mut Vec<u8>) -> Result<(), bytesrepr::Error> {
        let BinaryResponseAndRequest { request, response } = self;
        request.write_bytes(writer)?;
        response.write_bytes(writer)
    }

    fn serialized_length(&self) -> usize {
        self.request.serialized_length() + self.response.serialized_length()
    }
}

impl FromBytes for BinaryResponseAndRequest {
    fn from_bytes(bytes: &[u8]) -> Result<(Self, &[u8]), bytesrepr::Error> {
        let (request, remainder) = FromBytes::from_bytes(bytes)?;
        let (response, remainder) = FromBytes::from_bytes(remainder)?;

        Ok((BinaryResponseAndRequest { request, response }, remainder))
    }
}

impl From<BinaryResponseAndRequest> for BinaryResponse {
    fn from(response_and_request: BinaryResponseAndRequest) -> Self {
        let BinaryResponseAndRequest { response, .. } = response_and_request;
        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use casper_types::testing::TestRng;

    #[test]
    fn roundtrip() {
        let rng = &mut TestRng::new();
        let bytes = vec![1; 155];
        let response = BinaryResponse::random(rng);
        let val = BinaryResponseAndRequest::new(response, Bytes::from(bytes));
        bytesrepr::test_serialization_roundtrip(&val);
    }

    #[test]
    fn bytesrepr_roundtrip() {
        let rng = &mut TestRng::new();

        let val = BinaryResponseAndRequest::random(rng);
        bytesrepr::test_serialization_roundtrip(&val);
    }
}
