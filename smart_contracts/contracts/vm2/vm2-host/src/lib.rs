#![cfg_attr(target_arch = "wasm32", no_main)]

use casper_macros::casper;
use casper_sdk::{
    casper_executor_wasm_common::{flags::ReturnFlags, keyspace::Keyspace},
    host::{self, Entity},
    prelude::*,
};

const CURRENT_VERSION: &str = "v1";

// This contract is used to assert that calling host functions consumes gas.
// It is by design that it does nothing other than calling appropriate host functions.

// There is no need for these functions to actually do anything meaningful, and it's alright
// if they short-circuit.

#[casper(contract_state)]
pub struct MinHostWrapper;

impl Default for MinHostWrapper {
    fn default() -> Self {
        panic!("Unable to instantiate contract without a constructor");
    }
}

#[casper]
impl MinHostWrapper {
    #[casper(constructor)]
    pub fn new(with_host_fn_call: String) -> Self {
        let ret = Self;
        match with_host_fn_call.as_str() {
            "get_caller" => {
                ret.get_caller();
            }
            "get_block_time" => {
                ret.get_block_time();
            }
            "get_value" => {
                ret.get_value();
            }
            "get_balance_of" => {
                ret.get_balance_of();
            }
            "call" => {
                ret.call();
            }
            "input" => {
                ret.input();
            }
            "create" => {
                ret.create();
            }
            "print" => {
                ret.print();
            }
            "read" => {
                ret.read();
            }
            "ret" => {
                ret.ret();
            }
            "transfer" => {
                ret.transfer();
            }
            "upgrade" => {
                ret.upgrade();
            }
            "write" => {
                ret.write();
            }
            "write_n_bytes" => {
                ret.write();
            }
            _ => panic!("Unknown host function"),
        }
        ret
    }

    #[casper(constructor)]
    pub fn new_with_write(byte_count: u64) -> Self {
        let ret = Self;
        ret.write_n_bytes(byte_count);
        ret
    }

    #[casper(constructor)]
    pub fn default() -> Self {
        Self
    }

    pub fn version(&self) -> &str {
        CURRENT_VERSION
    }

    pub fn get_caller(&self) -> Entity {
        host::get_caller()
    }

    pub fn get_block_time(&self) -> u64 {
        host::get_block_time()
    }

    pub fn get_value(&self) -> u128 {
        host::get_value()
    }

    pub fn get_balance_of(&self) -> u128 {
        host::get_balance_of(&Entity::Account([0u8; 32]))
    }

    pub fn call(&self) {
        host::casper_call(&[0u8; 32], 0, "", &[]).1.ok();
    }

    pub fn input(&self) {
        host::casper_copy_input();
    }

    pub fn create(&self) {
        host::casper_create(None, 0, None, None, None).ok();
    }

    pub fn print(&self) {
        host::casper_print("");
    }

    pub fn read(&self) {
        host::casper_read(Keyspace::Context(&[]), |_| None).ok();
    }

    pub fn ret(&self) {
        host::casper_return(ReturnFlags::empty(), None);
    }

    pub fn transfer(&self) {
        host::casper_transfer(&[0; 32], 0).ok();
    }

    pub fn upgrade(&self) {
        host::casper_upgrade(&[], None, None).ok();
    }

    pub fn write(&self) {
        host::casper_write(Keyspace::Context(&[]), &[]).ok();
    }

    pub fn write_n_bytes(&self, n: u64) {
        let buffer = vec![0; n as usize];
        host::casper_write(Keyspace::Context(&[0]), &buffer).ok();
    }
}
