use crate::prelude::fmt::{self, Display, Formatter};

use crate::serializers::borsh::{BorshDeserialize, BorshSerialize};

use crate::abi::{CasperABI, Definition, EnumVariant};

pub type Address = [u8; 32];

pub(crate) const CALL_ERROR_CALLEE_SUCCEEDED: u32 = 0;
pub(crate) const CALL_ERROR_CALLEE_REVERTED: u32 = 1;
pub(crate) const CALL_ERROR_CALLEE_TRAPPED: u32 = 2;
pub(crate) const CALL_ERROR_CALLEE_GAS_DEPLETED: u32 = 3;
pub(crate) const CALL_ERROR_CALLEE_NOT_CALLABLE: u32 = 4;

#[derive(Debug, Copy, Clone, PartialEq, Eq, BorshSerialize, BorshDeserialize)]
#[borsh(crate = "crate::serializers::borsh")]
pub enum CallError {
    CalleeReverted,
    CalleeTrapped,
    CalleeGasDepleted,
    NotCallable,
}

impl Display for CallError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            CallError::CalleeReverted => write!(f, "callee reverted"),
            CallError::CalleeTrapped => write!(f, "callee trapped"),
            CallError::CalleeGasDepleted => write!(f, "callee gas depleted"),
            CallError::NotCallable => write!(f, "not callable"),
        }
    }
}

impl TryFrom<u32> for CallError {
    type Error = ();

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            CALL_ERROR_CALLEE_REVERTED => Ok(Self::CalleeReverted),
            CALL_ERROR_CALLEE_TRAPPED => Ok(Self::CalleeTrapped),
            CALL_ERROR_CALLEE_GAS_DEPLETED => Ok(Self::CalleeGasDepleted),
            CALL_ERROR_CALLEE_NOT_CALLABLE => Ok(Self::NotCallable),
            _ => Err(()),
        }
    }
}

impl CasperABI for CallError {
    fn populate_definitions(_definitions: &mut crate::abi::Definitions) {}

    fn declaration() -> crate::abi::Declaration {
        "CallError".into()
    }

    fn definition() -> Definition {
        Definition::Enum {
            items: vec![
                EnumVariant {
                    name: "CalleeReverted".into(),
                    discriminant: 0,
                    decl: <()>::declaration(),
                },
                EnumVariant {
                    name: "CalleeTrapped".into(),
                    discriminant: 1,
                    decl: <()>::declaration(),
                },
                EnumVariant {
                    name: "CalleeGasDepleted".into(),
                    discriminant: 2,
                    decl: <()>::declaration(),
                },
                EnumVariant {
                    name: "CodeNotFound".into(),
                    discriminant: 3,
                    decl: <()>::declaration(),
                },
            ],
        }
    }
}
