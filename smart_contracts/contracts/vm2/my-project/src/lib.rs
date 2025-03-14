#![cfg_attr(target_arch = "wasm32", no_main)]
#![cfg_attr(target_arch = "wasm32", no_std)]

use casper_sdk::{casper::print, prelude::*};

#[casper(contract_state)]
pub struct Contract;

impl Default for Contract {
    fn default() -> Self {
        panic!("Unable to instantiate contract without a constructor!");
    }
}

#[casper]
impl Contract {
    #[casper(constructor)]
    pub fn new() -> Self {
        Self
    }

    #[casper(constructor)]
    pub fn default() -> Self {
        Self::new()
    }

    pub fn my_entrypoint() {
        print("Hello, Casper!");
    }
}