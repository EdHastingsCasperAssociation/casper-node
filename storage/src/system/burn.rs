use std::{cell::RefCell, convert::TryFrom, rc::Rc};
use thiserror::Error;

use casper_types::{
    bytesrepr::FromBytes,
    system::{mint, mint::Error as MintError},
    AccessRights, CLType, CLTyped, CLValue, CLValueError, Key, RuntimeArgs, RuntimeFootprint,
    StoredValue, StoredValueTypeMismatch, URef, U512,
};

use crate::{
    global_state::{error::Error as GlobalStateError, state::StateReader},
    tracking_copy::{TrackingCopy, TrackingCopyError, TrackingCopyExt},
};

/// Burn error.
#[derive(Clone, Error, Debug)]
pub enum BurnError {
    /// Invalid key variant.
    #[error("Invalid key {0}")]
    UnexpectedKeyVariant(Key),
    /// Type mismatch error.
    #[error("{}", _0)]
    TypeMismatch(StoredValueTypeMismatch),
    /// Forged reference error.
    #[error("Forged reference: {}", _0)]
    ForgedReference(URef),
    /// Invalid access.
    #[error("Invalid access rights: {}", required)]
    InvalidAccess {
        /// Required access rights of the operation.
        required: AccessRights,
    },
    /// Error converting a CLValue.
    #[error("{0}")]
    CLValue(CLValueError),
    /// Invalid purse.
    #[error("Invalid purse")]
    InvalidPurse,
    /// Invalid argument.
    #[error("Invalid argument")]
    InvalidArgument,
    /// Missing argument.
    #[error("Missing argument")]
    MissingArgument,
    /// Invalid purse.
    #[error("Attempt to transfer amount 0")]
    AttemptToBurnZero,
    /// Invalid operation.
    #[error("Invalid operation")]
    InvalidOperation,
    /// Disallowed transfer attempt (private chain).
    #[error("Either the source or the target must be an admin (private chain).")]
    RestrictedBurnAttempted,
    /// Could not determine if target is an admin (private chain).
    #[error("Unable to determine if the target of a transfer is an admin")]
    UnableToVerifyTargetIsAdmin,
    /// Tracking copy error.
    #[error("{0}")]
    TrackingCopy(TrackingCopyError),
    /// Mint error.
    #[error("{0}")]
    Mint(MintError),
}

impl From<GlobalStateError> for BurnError {
    fn from(gse: GlobalStateError) -> Self {
        BurnError::TrackingCopy(TrackingCopyError::Storage(gse))
    }
}

impl From<TrackingCopyError> for BurnError {
    fn from(tce: TrackingCopyError) -> Self {
        BurnError::TrackingCopy(tce)
    }
}

/// Mint's burn arguments.
///
/// A struct has a benefit of static typing, which is helpful while resolving the arguments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BurnArgs {
    source: URef,
    amount: U512,
}

impl BurnArgs {
    /// Creates new transfer arguments.
    pub fn new(source: URef, amount: U512) -> Self {
        Self { source, amount }
    }

    /// Returns `source` field.
    pub fn source(&self) -> URef {
        self.source
    }

    /// Returns `amount` field.
    pub fn amount(&self) -> U512 {
        self.amount
    }
}

impl TryFrom<BurnArgs> for RuntimeArgs {
    type Error = CLValueError;

    fn try_from(burn_args: BurnArgs) -> Result<Self, Self::Error> {
        let mut runtime_args = RuntimeArgs::new();

        runtime_args.insert(mint::ARG_SOURCE, burn_args.source)?;
        runtime_args.insert(mint::ARG_AMOUNT, burn_args.amount)?;

        Ok(runtime_args)
    }
}

/// State of a builder of a `BurnArgs`.
///
/// Purpose of this builder is to resolve native burn args into [`BurnTargetMode`] and a
/// [`BurnArgs`] instance to execute actual token burn on the mint contract.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BurnRuntimeArgsBuilder {
    inner: RuntimeArgs,
}

impl BurnRuntimeArgsBuilder {
    /// Creates new burn args builder.
    ///
    /// Takes an incoming runtime args that represents native burn's arguments.
    pub fn new(imputed_runtime_args: RuntimeArgs) -> BurnRuntimeArgsBuilder {
        BurnRuntimeArgsBuilder {
            inner: imputed_runtime_args,
        }
    }

    /// Checks if a purse exists.
    fn purse_exists<R>(&self, uref: URef, tracking_copy: Rc<RefCell<TrackingCopy<R>>>) -> bool
    where
        R: StateReader<Key, StoredValue, Error = GlobalStateError>,
    {
        let key = match tracking_copy
            .borrow_mut()
            .get_purse_balance_key(uref.into())
        {
            Ok(key) => key,
            Err(_) => return false,
        };
        tracking_copy
            .borrow_mut()
            .get_available_balance(key)
            .is_ok()
    }

    /// Resolves the source purse of the burn.
    ///
    /// User can optionally pass a "source" argument which should refer to an [`URef`] existing in
    /// user's named keys. When the "source" argument is missing then user's main purse is assumed.
    ///
    /// Returns resolved [`URef`].
    fn resolve_source_uref<R>(
        &self,
        account: &RuntimeFootprint,
        tracking_copy: Rc<RefCell<TrackingCopy<R>>>,
    ) -> Result<URef, BurnError>
    where
        R: StateReader<Key, StoredValue, Error = GlobalStateError>,
    {
        let imputed_runtime_args = &self.inner;
        let arg_name = mint::ARG_SOURCE;
        let uref = match imputed_runtime_args.get(arg_name) {
            Some(cl_value) if *cl_value.cl_type() == CLType::URef => {
                self.map_cl_value::<URef>(cl_value)?
            }
            Some(cl_value) if *cl_value.cl_type() == CLType::Option(CLType::URef.into()) => {
                let Some(uref): Option<URef> = self.map_cl_value(cl_value)? else {
                    return account.main_purse().ok_or(BurnError::InvalidOperation);
                };
                uref
            }
            Some(_) => return Err(BurnError::InvalidArgument),
            None => return account.main_purse().ok_or(BurnError::InvalidOperation), /* if no source purse passed use account
                                                                                     * main purse */
        };
        if account
            .main_purse()
            .ok_or(BurnError::InvalidOperation)?
            .addr()
            == uref.addr()
        {
            return Ok(uref);
        }

        let normalized_uref = Key::URef(uref).normalize();
        let maybe_named_key = account
            .named_keys()
            .keys()
            .find(|&named_key| named_key.normalize() == normalized_uref);

        match maybe_named_key {
            Some(Key::URef(found_uref)) => {
                if found_uref.is_writeable() {
                    // it is a URef and caller has access but is it a purse URef?
                    if !self.purse_exists(found_uref.to_owned(), tracking_copy) {
                        return Err(BurnError::InvalidPurse);
                    }

                    Ok(uref)
                } else {
                    Err(BurnError::InvalidAccess {
                        required: AccessRights::WRITE,
                    })
                }
            }
            Some(key) => Err(BurnError::TypeMismatch(StoredValueTypeMismatch::new(
                "Key::URef".to_string(),
                key.type_string(),
            ))),
            None => Err(BurnError::ForgedReference(uref)),
        }
    }

    /// Resolves amount.
    ///
    /// User has to specify "amount" argument that could be either a [`U512`] or a u64.
    fn resolve_amount(&self) -> Result<U512, BurnError> {
        let imputed_runtime_args = &self.inner;

        let amount = match imputed_runtime_args.get(mint::ARG_AMOUNT) {
            Some(amount_value) if *amount_value.cl_type() == CLType::U512 => {
                self.map_cl_value(amount_value)?
            }
            Some(amount_value) if *amount_value.cl_type() == CLType::U64 => {
                let amount: u64 = self.map_cl_value(amount_value)?;
                U512::from(amount)
            }
            Some(_) => return Err(BurnError::InvalidArgument),
            None => return Err(BurnError::MissingArgument),
        };

        if amount.is_zero() {
            return Err(BurnError::AttemptToBurnZero);
        }

        Ok(amount)
    }

    /// Creates new [`BurnArgs`] instance.
    pub fn build<R>(
        self,
        from: &RuntimeFootprint,
        tracking_copy: Rc<RefCell<TrackingCopy<R>>>,
    ) -> Result<BurnArgs, BurnError>
    where
        R: StateReader<Key, StoredValue, Error = GlobalStateError>,
    {
        let source = self.resolve_source_uref(from, Rc::clone(&tracking_copy))?;
        let amount = self.resolve_amount()?;
        Ok(BurnArgs { source, amount })
    }

    fn map_cl_value<T: CLTyped + FromBytes>(&self, cl_value: &CLValue) -> Result<T, BurnError> {
        cl_value.clone().into_t().map_err(BurnError::CLValue)
    }
}
