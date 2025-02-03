use core::convert::TryFrom;

use casper_types::{
    bytesrepr::{self, FromBytes, ToBytes},
    Transaction,
};

use crate::get_request::GetRequest;

#[cfg(test)]
use casper_types::testing::TestRng;
#[cfg(test)]
use rand::Rng;

/// The header of a binary request.
#[derive(Debug, PartialEq)]
pub struct BinaryRequestHeader {
    binary_request_version: u16,
    type_tag: u8,
    id: u16,
}

impl BinaryRequestHeader {
    // Defines the current version of the header, in practice defining the current version of the
    // binary port protocol. Requests with mismatched header version will be dropped.
    pub const BINARY_REQUEST_VERSION: u16 = 1;

    /// Creates new binary request header.
    pub fn new(type_tag: BinaryRequestTag, id: u16) -> Self {
        Self {
            binary_request_version: Self::BINARY_REQUEST_VERSION,
            type_tag: type_tag.into(),
            id,
        }
    }

    /// Returns the type tag of the request.
    pub fn type_tag(&self) -> u8 {
        self.type_tag
    }

    /// Returns the request id.
    pub fn id(&self) -> u16 {
        self.id
    }

    /// Returns the header version.
    pub fn version(&self) -> u16 {
        self.binary_request_version
    }

    #[cfg(any(feature = "testing", test))]
    pub fn set_binary_request_version(&mut self, version: u16) {
        self.binary_request_version = version;
    }

    #[cfg(test)]
    pub(crate) fn random(rng: &mut TestRng) -> Self {
        Self {
            binary_request_version: rng.gen(),
            type_tag: BinaryRequestTag::random(rng).into(),
            id: rng.gen(),
        }
    }
}

impl ToBytes for BinaryRequestHeader {
    fn to_bytes(&self) -> Result<Vec<u8>, bytesrepr::Error> {
        let mut buffer = bytesrepr::allocate_buffer(self)?;
        self.write_bytes(&mut buffer)?;
        Ok(buffer)
    }

    fn write_bytes(&self, writer: &mut Vec<u8>) -> Result<(), bytesrepr::Error> {
        self.binary_request_version.write_bytes(writer)?;
        self.type_tag.write_bytes(writer)?;
        self.id.write_bytes(writer)
    }

    fn serialized_length(&self) -> usize {
        self.binary_request_version.serialized_length()
            + self.type_tag.serialized_length()
            + self.id.serialized_length()
    }
}

impl FromBytes for BinaryRequestHeader {
    fn from_bytes(bytes: &[u8]) -> Result<(Self, &[u8]), bytesrepr::Error> {
        let (binary_request_version, remainder) = FromBytes::from_bytes(bytes)?;
        let (type_tag, remainder) = FromBytes::from_bytes(remainder)?;
        let (id, remainder) = FromBytes::from_bytes(remainder)?;
        Ok((
            BinaryRequestHeader {
                binary_request_version,
                type_tag,
                id,
            },
            remainder,
        ))
    }
}

/// A request to the binary access interface.
#[derive(Debug, PartialEq)]

pub enum Command {
    /// Request to get data from the node
    Get(GetRequest),
    /// Request to add a transaction into a blockchain.
    TryAcceptTransaction {
        /// Transaction to be handled.
        transaction: Transaction,
    },
    /// Request to execute a transaction speculatively.
    TrySpeculativeExec {
        /// Transaction to execute.
        transaction: Transaction,
    },
}

impl Command {
    /// Returns the type tag of the request.
    pub fn tag(&self) -> BinaryRequestTag {
        match self {
            Command::Get(_) => BinaryRequestTag::Get,
            Command::TryAcceptTransaction { .. } => BinaryRequestTag::TryAcceptTransaction,
            Command::TrySpeculativeExec { .. } => BinaryRequestTag::TrySpeculativeExec,
        }
    }

    #[cfg(test)]
    pub(crate) fn random(rng: &mut TestRng) -> Self {
        match BinaryRequestTag::random(rng) {
            BinaryRequestTag::Get => Self::Get(GetRequest::random(rng)),
            BinaryRequestTag::TryAcceptTransaction => Self::TryAcceptTransaction {
                transaction: Transaction::random(rng),
            },
            BinaryRequestTag::TrySpeculativeExec => Self::TrySpeculativeExec {
                transaction: Transaction::random(rng),
            },
        }
    }
}

impl ToBytes for Command {
    fn to_bytes(&self) -> Result<Vec<u8>, bytesrepr::Error> {
        let mut buffer = bytesrepr::allocate_buffer(self)?;
        self.write_bytes(&mut buffer)?;
        Ok(buffer)
    }

    fn write_bytes(&self, writer: &mut Vec<u8>) -> Result<(), bytesrepr::Error> {
        match self {
            Command::Get(inner) => inner.write_bytes(writer),
            Command::TryAcceptTransaction { transaction } => transaction.write_bytes(writer),
            Command::TrySpeculativeExec { transaction } => transaction.write_bytes(writer),
        }
    }

    fn serialized_length(&self) -> usize {
        match self {
            Command::Get(inner) => inner.serialized_length(),
            Command::TryAcceptTransaction { transaction } => transaction.serialized_length(),
            Command::TrySpeculativeExec { transaction } => transaction.serialized_length(),
        }
    }
}

impl TryFrom<(BinaryRequestTag, &[u8])> for Command {
    type Error = bytesrepr::Error;

    fn try_from((tag, bytes): (BinaryRequestTag, &[u8])) -> Result<Self, Self::Error> {
        let (req, remainder) = match tag {
            BinaryRequestTag::Get => {
                let (get_request, remainder) = FromBytes::from_bytes(bytes)?;
                (Command::Get(get_request), remainder)
            }
            BinaryRequestTag::TryAcceptTransaction => {
                let (transaction, remainder) = FromBytes::from_bytes(bytes)?;
                (Command::TryAcceptTransaction { transaction }, remainder)
            }
            BinaryRequestTag::TrySpeculativeExec => {
                let (transaction, remainder) = FromBytes::from_bytes(bytes)?;
                (Command::TrySpeculativeExec { transaction }, remainder)
            }
        };
        if !remainder.is_empty() {
            return Err(bytesrepr::Error::LeftOverBytes);
        }
        Ok(req)
    }
}

/// The type tag of a binary request.
#[derive(Debug, PartialEq)]
#[repr(u8)]
pub enum BinaryRequestTag {
    /// Request to get data from the node
    Get = 0,
    /// Request to add a transaction into a blockchain.
    TryAcceptTransaction = 1,
    /// Request to execute a transaction speculatively.
    TrySpeculativeExec = 2,
}

impl BinaryRequestTag {
    /// Creates a random `BinaryRequestTag`.
    #[cfg(test)]
    pub fn random(rng: &mut TestRng) -> Self {
        match rng.gen_range(0..3) {
            0 => BinaryRequestTag::Get,
            1 => BinaryRequestTag::TryAcceptTransaction,
            2 => BinaryRequestTag::TrySpeculativeExec,
            _ => unreachable!(),
        }
    }
}

impl TryFrom<u8> for BinaryRequestTag {
    type Error = InvalidBinaryRequestTag;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(BinaryRequestTag::Get),
            1 => Ok(BinaryRequestTag::TryAcceptTransaction),
            2 => Ok(BinaryRequestTag::TrySpeculativeExec),
            _ => Err(InvalidBinaryRequestTag),
        }
    }
}

impl From<BinaryRequestTag> for u8 {
    fn from(value: BinaryRequestTag) -> Self {
        value as u8
    }
}

/// Error raised when trying to convert an invalid u8 into a `BinaryRequestTag`.
pub struct InvalidBinaryRequestTag;

#[cfg(test)]
mod tests {
    use super::*;
    use casper_types::testing::TestRng;

    #[test]
    fn header_bytesrepr_roundtrip() {
        let rng = &mut TestRng::new();

        let val = BinaryRequestHeader::random(rng);
        bytesrepr::test_serialization_roundtrip(&val);
    }

    #[test]
    fn request_bytesrepr_roundtrip() {
        std::env::set_var("CL_TEST_SEED", "8213819b1de2c4be287fe81c4534ad28");
        let rng = &mut TestRng::new();

        let val = Command::random(rng);
        let bytes = val.to_bytes().expect("should serialize");
        assert_eq!(Command::try_from((val.tag(), &bytes[..])), Ok(val));
    }
}
