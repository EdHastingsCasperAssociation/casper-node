use casper_sdk::casper_executor_wasm_common::flags::ReturnFlags;

const SCHEMA: &str = r#""{{__CARGO_CASPER_INJECT_SCHEMA_MARKER}}""#;

#[no_mangle]
pub extern "C" fn __casper_schema() {
    let data = SCHEMA.as_bytes();
    casper_return(ReturnFlags::empty(), Some(data));
}