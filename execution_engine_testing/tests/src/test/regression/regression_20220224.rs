use casper_engine_test_support::{
    DeployItemBuilder, ExecuteRequestBuilder, LmdbWasmTestBuilder, DEFAULT_ACCOUNT_ADDR,
    DEFAULT_PAYMENT, LOCAL_GENESIS_REQUEST,
};
use casper_execution_engine::{engine_state, execution::ExecError};
use casper_types::{runtime_args, system::mint, ApiError, RuntimeArgs};

const CONTRACT_REGRESSION_PAYMENT: &str = "regression_payment.wasm";
const CONTRACT_REVERT: &str = "revert.wasm";

#[ignore]
#[test]
fn should_not_transfer_above_approved_limit_in_payment_code() {
    let mut builder = LmdbWasmTestBuilder::default();
    builder.run_genesis(LOCAL_GENESIS_REQUEST.clone());

    let account_hash = *DEFAULT_ACCOUNT_ADDR;
    let deploy_hash: [u8; 32] = [42; 32];
    let payment_args = runtime_args! {
        "amount" => *DEFAULT_PAYMENT,
    };
    let session_args = RuntimeArgs::default();

    let deploy_item = DeployItemBuilder::new()
        .with_address(account_hash)
        .with_session_code(CONTRACT_REVERT, session_args)
        .with_payment_code(CONTRACT_REGRESSION_PAYMENT, payment_args)
        .with_authorization_keys(&[account_hash])
        .with_deploy_hash(deploy_hash)
        .build();

    let exec_request = ExecuteRequestBuilder::from_deploy_item(&deploy_item).build();

    builder.exec(exec_request).expect_failure().commit();

    let error = builder.get_error().expect("should have returned an error");
    assert!(
        matches!(
            error,
            engine_state::Error::Exec(ExecError::Revert(ApiError::Mint(mint_error)))
            if mint_error == mint::Error::UnapprovedSpendingAmount as u8
        ),
        "Expected unapproved spending amount error but received {:?}",
        error
    );
}
