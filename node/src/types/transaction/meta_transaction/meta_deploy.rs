use datasize::DataSize;
use serde::Serialize;

use crate::types::transaction::meta_transaction::lane_id::get_lane_for_non_install_wasm;
#[cfg(test)]
use casper_types::TransactionLaneDefinition;
use casper_types::{
    bytesrepr::ToBytes, system::auction::ARG_AMOUNT, Deploy, ExecutableDeployItem, GasLimited,
    InvalidDeploy, InvalidTransaction, Phase, PricingHandling, PricingMode, TransactionV1Config,
    MINT_LANE_ID, U512,
};
#[derive(Clone, Debug, Serialize, DataSize)]
pub(crate) struct MetaDeploy {
    deploy: Deploy,
    //We need to keep that id here since we can fetch it only from chainspec.
    lane_id: u8,
}

impl MetaDeploy {
    pub(crate) fn from_deploy(
        deploy: Deploy,
        pricing_handling: PricingHandling,
        config: &TransactionV1Config,
    ) -> Result<Self, InvalidTransaction> {
        if deploy.is_transfer() {
            return Ok(MetaDeploy {
                deploy,
                lane_id: MINT_LANE_ID,
            });
        }

        let size_estimation = deploy.serialized_length() as u64;
        let runtime_args_size = (deploy.payment().args().serialized_length()
            + deploy.session().args().serialized_length()) as u64;

        let gas_price_tolerance = deploy.gas_price_tolerance()?;
        let pricing_mode = match pricing_handling {
            PricingHandling::PaymentLimited => {
                let is_standard_payment = deploy.payment().is_standard_payment(Phase::Payment);
                let value = deploy
                    .payment()
                    .args()
                    .get(ARG_AMOUNT)
                    .ok_or(InvalidDeploy::MissingPaymentAmount)?;
                let payment_amount = value
                    .clone()
                    .into_t::<U512>()
                    .map_err(|_| InvalidDeploy::FailedToParsePaymentAmount)?
                    .as_u64();
                PricingMode::PaymentLimited {
                    payment_amount,
                    gas_price_tolerance,
                    standard_payment: is_standard_payment,
                }
            }
            PricingHandling::Fixed => PricingMode::Fixed {
                gas_price_tolerance,
                // TODO: Recheck value before switching to fixed.
                additional_computation_factor: 0,
            },
        };

        let lane_id = get_lane_for_non_install_wasm(
            config,
            &pricing_mode,
            size_estimation,
            runtime_args_size,
        )?;
        println!("{lane_id} for {}", deploy.hash());
        Ok(MetaDeploy { deploy, lane_id })
    }

    pub(crate) fn lane_id(&self) -> u8 {
        self.lane_id
    }

    pub(crate) fn session(&self) -> &ExecutableDeployItem {
        self.deploy.session()
    }

    pub(crate) fn deploy(&self) -> &Deploy {
        &self.deploy
    }
}

#[cfg(test)]
pub(crate) fn calculate_lane_id_of_biggest_wasm(
    wasm_lanes: &[TransactionLaneDefinition],
) -> Option<u8> {
    wasm_lanes
        .iter()
        .max_by(|left, right| {
            left.max_transaction_length
                .cmp(&right.max_transaction_length)
        })
        .map(|definition| definition.id)
}
#[cfg(test)]
mod tests {
    use super::calculate_lane_id_of_biggest_wasm;
    use casper_types::TransactionLaneDefinition;
    #[test]
    fn calculate_lane_id_of_biggest_wasm_should_return_none_on_empty() {
        let wasms = vec![];
        assert!(calculate_lane_id_of_biggest_wasm(&wasms).is_none());
    }

    #[test]
    fn calculate_lane_id_of_biggest_wasm_should_return_biggest() {
        let wasms = vec![
            TransactionLaneDefinition {
                id: 0,
                max_transaction_length: 1,
                max_transaction_args_length: 2,
                max_transaction_gas_limit: 3,
                max_transaction_count: 4,
            },
            TransactionLaneDefinition {
                id: 1,
                max_transaction_length: 10,
                max_transaction_args_length: 2,
                max_transaction_gas_limit: 3,
                max_transaction_count: 4,
            },
        ];
        assert_eq!(calculate_lane_id_of_biggest_wasm(&wasms), Some(1));
        let wasms = vec![
            TransactionLaneDefinition {
                id: 0,
                max_transaction_length: 1,
                max_transaction_args_length: 2,
                max_transaction_gas_limit: 3,
                max_transaction_count: 4,
            },
            TransactionLaneDefinition {
                id: 1,
                max_transaction_length: 10,
                max_transaction_args_length: 2,
                max_transaction_gas_limit: 3,
                max_transaction_count: 4,
            },
            TransactionLaneDefinition {
                id: 2,
                max_transaction_length: 7,
                max_transaction_args_length: 2,
                max_transaction_gas_limit: 3,
                max_transaction_count: 4,
            },
        ];
        assert_eq!(calculate_lane_id_of_biggest_wasm(&wasms), Some(1));

        let wasms = vec![
            TransactionLaneDefinition {
                id: 0,
                max_transaction_length: 1,
                max_transaction_args_length: 2,
                max_transaction_gas_limit: 3,
                max_transaction_count: 4,
            },
            TransactionLaneDefinition {
                id: 1,
                max_transaction_length: 10,
                max_transaction_args_length: 2,
                max_transaction_gas_limit: 3,
                max_transaction_count: 4,
            },
            TransactionLaneDefinition {
                id: 2,
                max_transaction_length: 70,
                max_transaction_args_length: 2,
                max_transaction_gas_limit: 3,
                max_transaction_count: 4,
            },
        ];
        assert_eq!(calculate_lane_id_of_biggest_wasm(&wasms), Some(2));
    }
}
