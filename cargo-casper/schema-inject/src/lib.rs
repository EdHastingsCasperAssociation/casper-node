use casper_sdk::{casper::ret, casper_executor_wasm_common::flags::ReturnFlags};

const SCHEMA: &str = r#""{{__CARGO_CASPER_INJECT_SCHEMA_MARKER}}""#;

#[no_mangle]
pub extern "C" fn __casper_schema() {
    let data = SCHEMA.as_bytes();
    ret(ReturnFlags::empty(), Some(data));
}