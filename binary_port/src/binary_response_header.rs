use crate::{error_code::ErrorCode, response_type::ResponseType};
use casper_types::bytesrepr::{self, FromBytes, ToBytes};

#[cfg(test)]
use casper_types::testing::TestRng;
#[cfg(test)]
use rand::Rng;

/// Header of the binary response.
#[derive(Debug, PartialEq)]
pub struct BinaryResponseHeader {
    binary_response_version: u16,
    error: u16,
    returned_data_type_tag: Option<u8>,
}

impl BinaryResponseHeader {
    pub const BINARY_RESPONSE_VERSION: u16 = 1;
    /// Creates new binary response header representing success.
    pub fn new(returned_data_type: Option<ResponseType>) -> Self {
        Self {
            binary_response_version: Self::BINARY_RESPONSE_VERSION,
            error: ErrorCode::NoError as u16,
            returned_data_type_tag: returned_data_type.map(|ty| ty as u8),
        }
    }

    /// Creates new binary response header representing error.
    pub fn new_error(error: ErrorCode) -> Self {
        Self {
            binary_response_version: Self::BINARY_RESPONSE_VERSION,
            error: error as u16,
            returned_data_type_tag: None,
        }
    }

    /// Returns the type of the returned data.
    pub fn returned_data_type_tag(&self) -> Option<u8> {
        self.returned_data_type_tag
    }

    /// Returns the error code.
    pub fn error_code(&self) -> u16 {
        self.error
    }

    /// Returns true if the response represents success.
    pub fn is_success(&self) -> bool {
        self.error == ErrorCode::NoError as u16
    }

    /// Returns true if the response indicates the data was not found.
    pub fn is_not_found(&self) -> bool {
        self.error == ErrorCode::NotFound as u16
    }

    #[cfg(test)]
    pub(crate) fn random(rng: &mut TestRng) -> Self {
        let error = rng.gen();
        let returned_data_type_tag = if rng.gen() { None } else { Some(rng.gen()) };

        BinaryResponseHeader {
            binary_response_version: Self::BINARY_RESPONSE_VERSION,
            error,
            returned_data_type_tag,
        }
    }
}

impl ToBytes for BinaryResponseHeader {
    fn to_bytes(&self) -> Result<Vec<u8>, bytesrepr::Error> {
        let mut buffer = bytesrepr::allocate_buffer(self)?;
        self.write_bytes(&mut buffer)?;
        Ok(buffer)
    }

    fn write_bytes(&self, writer: &mut Vec<u8>) -> Result<(), bytesrepr::Error> {
        let Self {
            binary_response_version,
            error,
            returned_data_type_tag,
        } = self;

        binary_response_version.write_bytes(writer)?;
        error.write_bytes(writer)?;
        returned_data_type_tag.write_bytes(writer)
    }

    fn serialized_length(&self) -> usize {
        self.binary_response_version.serialized_length()
            + self.error.serialized_length()
            + self.returned_data_type_tag.serialized_length()
    }
}

impl FromBytes for BinaryResponseHeader {
    fn from_bytes(bytes: &[u8]) -> Result<(Self, &[u8]), bytesrepr::Error> {
        let (binary_response_version, remainder) = FromBytes::from_bytes(bytes)?;
        let (error, remainder) = FromBytes::from_bytes(remainder)?;
        let (returned_data_type_tag, remainder) = FromBytes::from_bytes(remainder)?;

        Ok((
            BinaryResponseHeader {
                binary_response_version,
                error,
                returned_data_type_tag,
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

        let val = BinaryResponseHeader::random(rng);
        bytesrepr::test_serialization_roundtrip(&val);
    }
}
