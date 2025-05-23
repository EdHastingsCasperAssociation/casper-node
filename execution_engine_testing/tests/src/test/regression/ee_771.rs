use casper_engine_test_support::{
    ExecuteRequestBuilder, LmdbWasmTestBuilder, DEFAULT_ACCOUNT_ADDR, LOCAL_GENESIS_REQUEST,
};
use casper_types::RuntimeArgs;

const CONTRACT_EE_771_REGRESSION: &str = "ee_771_regression.wasm";

#[ignore]
#[test]
fn should_run_ee_771_regression() {
    let exec_request = ExecuteRequestBuilder::standard(
        *DEFAULT_ACCOUNT_ADDR,
        CONTRACT_EE_771_REGRESSION,
        RuntimeArgs::default(),
    )
    .build();

    let mut builder = LmdbWasmTestBuilder::default();
    builder
        .run_genesis(LOCAL_GENESIS_REQUEST.clone())
        .exec(exec_request)
        .commit();

    let exec_result = builder
        .get_exec_result_owned(0)
        .expect("should have a response");

    let error = exec_result.error().expect("should have error");
    assert_eq!(
        format!("{}", error),
        "Function not found: functiondoesnotexist"
    );
}
