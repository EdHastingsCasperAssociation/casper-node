use casper_macros::casper;

use crate::{
    casper,
    collections::{sorted_vector::SortedVector, Map, Set},
    types::Address,
};

/// A role is a unique identifier for a specific permission or set of permissions.
pub type Role = [u8; 32];

const ROLES_PREFIX: &str = "roles";

#[casper(path = "crate")]
pub struct AccessControlState {
    roles: Map<Address, SortedVector<Role>>,
}

impl Default for AccessControlState {
    fn default() -> Self {
        Self {
            roles: Map::new(ROLES_PREFIX),
        }
    }
}

#[casper(path = "crate", export = true)]
pub trait AccessControl {
    /// The state of the contract, which contains the roles.
    #[casper(private)]
    fn state(&self) -> &AccessControlState;
    /// The mutable state of the contract, which allows modifying the roles.
    #[casper(private)]
    fn state_mut(&mut self) -> &mut AccessControlState;

    /// Checks if the given account has the specified role.
    fn has_role(&self, account: Address, role: [u8; 32]) -> bool {
        match self.state().roles.get(&account) {
            Some(roles) => roles.contains(&role),
            None => false,
        }
    }

    fn grant_role(&mut self, account: Address, role: [u8; 32]) {
        match self.state_mut().roles.get(&account) {
            Some(mut roles) => {
                if roles.contains(&role) {
                    return;
                }
                roles.push(role);
            }
            None => {
                let mut roles =
                    SortedVector::new(format!("roles-{}", base16::encode_lower(&account)));
                roles.push(role);
                self.state_mut().roles.insert(&account, &roles);
            }
        }
    }

    fn revoke_role(&mut self, account: Address, role: [u8; 32]) {
        if let Some(mut roles) = self.state_mut().roles.get(&account) {
            roles.retain(|r| r != &role);
        }
    }
}
