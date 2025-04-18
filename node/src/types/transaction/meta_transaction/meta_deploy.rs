use datasize::DataSize;
use once_cell::sync::OnceCell;
use serde::Serialize;

#[cfg(test)]
use casper_types::TransactionLaneDefinition;
use casper_types::{
    calculate_lane_id_for_deploy, Deploy, ExecutableDeployItem, InitiatorAddr, InvalidTransaction,
    PricingHandling, TransactionV1Config,
};
#[derive(Clone, Debug, Serialize, DataSize)]
pub(crate) struct MetaDeploy {
    deploy: Deploy,
    //We need to keep this id here since we can fetch it only from chainspec.
    lane_id: u8,
    #[data_size(skip)]
    #[serde(skip)]
    initiator_addr: OnceCell<InitiatorAddr>,
}

impl MetaDeploy {
    pub(crate) fn from_deploy(
        deploy: Deploy,
        pricing_handling: PricingHandling,
        config: &TransactionV1Config,
    ) -> Result<Self, InvalidTransaction> {
        let lane_id = calculate_lane_id_for_deploy(&deploy, pricing_handling, config)
            .map_err(InvalidTransaction::Deploy)?;
        let initiator_addr = OnceCell::new();
        Ok(MetaDeploy {
            deploy,
            lane_id,
            initiator_addr,
        })
    }

    pub(crate) fn initiator_addr(&self) -> &InitiatorAddr {
        self.initiator_addr
            .get_or_init(|| InitiatorAddr::PublicKey(self.deploy.account().clone()))
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
