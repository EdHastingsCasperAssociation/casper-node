use casper_macros::casper;
use casper_sdk::host::Entity;

#[casper(message)]
pub struct Transfer {
    pub from: Entity,
    pub to: Entity,
    pub amount: u64,
}
