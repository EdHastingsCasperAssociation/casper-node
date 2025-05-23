use casper_engine_test_support::{
    ExecuteRequestBuilder, LmdbWasmTestBuilder, DEFAULT_ACCOUNT_ADDR, LOCAL_GENESIS_REQUEST,
};
use casper_types::runtime_args;

const CONTRACT_GET_BLOCKTIME: &str = "get_blocktime.wasm";
const ARG_KNOWN_BLOCK_TIME: &str = "known_block_time";

#[ignore]
#[test]
fn should_run_get_blocktime_contract() {
    let block_time: u64 = 42;

    let exec_request = ExecuteRequestBuilder::standard(
        *DEFAULT_ACCOUNT_ADDR,
        CONTRACT_GET_BLOCKTIME,
        runtime_args! { ARG_KNOWN_BLOCK_TIME => block_time },
    )
    .with_block_time(block_time)
    .build();
    LmdbWasmTestBuilder::default()
        .run_genesis(LOCAL_GENESIS_REQUEST.clone())
        .exec(exec_request)
        .commit()
        .expect_success();
}
