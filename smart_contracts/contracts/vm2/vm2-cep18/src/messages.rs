use casper_macros::casper;
use casper_sdk::host::Entity;

#[casper(message)]
pub struct Transfer {
    pub from: Option<Entity>,
    pub to: Entity,
    pub amount: u64,
}

#[casper(message)]
pub struct Approve {
    pub owner: Entity,
    pub spender: Entity,
    pub amount: u64,
}
