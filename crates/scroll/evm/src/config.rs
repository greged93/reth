use crate::ScrollEvmConfig;
use std::convert::Infallible;

use alloy_consensus::BlockHeader;
use alloy_evm::FromRecoveredTx;
use reth_chainspec::EthChainSpec;
use reth_evm::{ConfigureEvm, ConfigureEvmEnv, EvmEnv, NextBlockEnvAttributes};
use reth_primitives_traits::NodePrimitives;
use reth_scroll_chainspec::{ChainConfig, ScrollChainConfig};
use revm::{
    context::{BlockEnv, CfgEnv, TxEnv},
    primitives::U256,
};
use revm_scroll::ScrollSpecId;
use scroll_alloy_evm::{ScrollEvmFactory, ScrollTransactionIntoTxEnv};
use scroll_alloy_hardforks::ScrollHardforks;

impl<ChainSpec, N, R> ConfigureEvm for ScrollEvmConfig<ChainSpec, N, R>
where
    ChainSpec: EthChainSpec + ChainConfig<Config = ScrollChainConfig> + ScrollHardforks,
    N: NodePrimitives,
    ScrollTransactionIntoTxEnv<TxEnv>: FromRecoveredTx<N::SignedTx>,
    Self: Send + Sync + Unpin + Clone,
{
    type EvmFactory = ScrollEvmFactory;

    fn evm_factory(&self) -> &Self::EvmFactory {
        self.executor_factory.evm_factory()
    }
}

impl<ChainSpec, N, R> ConfigureEvmEnv for ScrollEvmConfig<ChainSpec, N, R>
where
    ChainSpec: EthChainSpec + ChainConfig<Config = ScrollChainConfig> + ScrollHardforks,
    N: NodePrimitives,
    ScrollTransactionIntoTxEnv<TxEnv>: FromRecoveredTx<N::SignedTx>,
    Self: Send + Sync + Unpin + Clone,
{
    type Header = N::BlockHeader;
    type Transaction = N::SignedTx;
    type TxEnv = ScrollTransactionIntoTxEnv<TxEnv>;
    type Error = Infallible;
    type Spec = ScrollSpecId;
    type NextBlockEnvCtx = NextBlockEnvAttributes;

    fn evm_env(&self, header: &Self::Header) -> EvmEnv<Self::Spec> {
        let chain_spec = self.chain_spec();
        let spec_id = self.spec_id_at_timestamp_and_number(header.timestamp(), header.number());

        let cfg_env = CfgEnv::<ScrollSpecId>::default()
            .with_spec(spec_id)
            .with_chain_id(chain_spec.chain().id());

        // get coinbase from chain spec
        let coinbase = if let Some(vault_address) = chain_spec.chain_config().fee_vault_address {
            vault_address
        } else {
            header.beneficiary()
        };

        let block_env = BlockEnv {
            number: header.number(),
            beneficiary: coinbase,
            timestamp: header.timestamp(),
            difficulty: header.difficulty(),
            prevrandao: header.mix_hash(),
            gas_limit: header.gas_limit(),
            basefee: header.base_fee_per_gas().unwrap_or_default(),
            // EIP-4844 excess blob gas of this block, introduced in Cancun
            blob_excess_gas_and_price: None,
        };

        EvmEnv { cfg_env, block_env }
    }

    fn next_evm_env(
        &self,
        parent: &Self::Header,
        attributes: &Self::NextBlockEnvCtx,
    ) -> Result<EvmEnv<Self::Spec>, Self::Error> {
        // ensure we're not missing any timestamp based hardforks
        let spec_id =
            self.spec_id_at_timestamp_and_number(attributes.timestamp, parent.number() + 1);

        let chain_spec = self.chain_spec();

        // configure evm env based on parent block
        let cfg_env = CfgEnv::<ScrollSpecId>::default()
            .with_chain_id(chain_spec.chain().id())
            .with_spec(spec_id);

        // get coinbase from chain spec
        let coinbase = if let Some(vault_address) = chain_spec.chain_config().fee_vault_address {
            vault_address
        } else {
            attributes.suggested_fee_recipient
        };

        let block_env = BlockEnv {
            number: parent.number() + 1,
            beneficiary: coinbase,
            timestamp: attributes.timestamp,
            difficulty: U256::ZERO,
            prevrandao: Some(attributes.prev_randao),
            gas_limit: attributes.gas_limit,
            // calculate basefee based on parent block's gas usage
            // TODO(scroll): update with correct block fee calculation for block building.
            basefee: parent.base_fee_per_gas().unwrap_or_default(),
            blob_excess_gas_and_price: None,
        };

        Ok(EvmEnv { cfg_env, block_env })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ScrollRethReceiptBuilder;
    use alloy_consensus::Header;
    use reth_chainspec::{Head, NamedChain::Scroll};
    use reth_scroll_chainspec::{ScrollChainConfig, ScrollChainSpecBuilder};
    use reth_scroll_primitives::ScrollPrimitives;
    use revm::primitives::B256;
    use revm_primitives::Address;

    #[test]
    fn test_spec_at_head() {
        let config = ScrollEvmConfig::<_, ScrollPrimitives, _>::new(
            ScrollChainSpecBuilder::scroll_mainnet().build(ScrollChainConfig::mainnet()).into(),
            ScrollRethReceiptBuilder::default(),
        );

        // prepare all fork heads
        let curie_head = &Head { number: 7096836, ..Default::default() };
        let bernouilli_head = &Head { number: 5220340, ..Default::default() };
        let pre_bernouilli_head = &Head { number: 0, ..Default::default() };

        // check correct spec id
        assert_eq!(
            config.spec_id_at_timestamp_and_number(curie_head.timestamp, curie_head.number),
            ScrollSpecId::CURIE
        );
        assert_eq!(
            config
                .spec_id_at_timestamp_and_number(bernouilli_head.timestamp, bernouilli_head.number),
            ScrollSpecId::BERNOULLI
        );
        assert_eq!(
            config.spec_id_at_timestamp_and_number(
                pre_bernouilli_head.timestamp,
                pre_bernouilli_head.number
            ),
            ScrollSpecId::SHANGHAI
        );
    }

    #[test]
    fn test_fill_cfg_env() {
        let config = ScrollEvmConfig::<_, ScrollPrimitives, _>::new(
            ScrollChainSpecBuilder::scroll_mainnet().build(ScrollChainConfig::mainnet()).into(),
            ScrollRethReceiptBuilder::default(),
        );

        // curie
        let curie_header = Header { number: 7096836, ..Default::default() };

        // fill cfg env
        let env = config.evm_env(&curie_header);

        // check correct cfg env
        assert_eq!(env.cfg_env.chain_id, Scroll as u64);
        assert_eq!(env.cfg_env.spec, ScrollSpecId::CURIE);

        // bernoulli
        let bernouilli_header = Header { number: 5220340, ..Default::default() };

        // fill cfg env
        let env = config.evm_env(&bernouilli_header);

        // check correct cfg env
        assert_eq!(env.cfg_env.chain_id, Scroll as u64);
        assert_eq!(env.cfg_env.spec, ScrollSpecId::BERNOULLI);

        // pre-bernoulli
        let pre_bernouilli_header = Header { number: 0, ..Default::default() };

        // fill cfg env
        let env = config.evm_env(&pre_bernouilli_header);

        // check correct cfg env
        assert_eq!(env.cfg_env.chain_id, Scroll as u64);
        assert_eq!(env.cfg_env.spec, ScrollSpecId::SHANGHAI);
    }

    #[test]
    fn test_fill_block_env() {
        let config = ScrollEvmConfig::<_, ScrollPrimitives, _>::new(
            ScrollChainSpecBuilder::scroll_mainnet().build(ScrollChainConfig::mainnet()).into(),
            ScrollRethReceiptBuilder::default(),
        );

        // curie header
        let header = Header {
            number: 7096836,
            beneficiary: Address::random(),
            timestamp: 1719994277,
            mix_hash: B256::random(),
            base_fee_per_gas: Some(155157341),
            gas_limit: 10000000,
            ..Default::default()
        };

        // fill block env
        let env = config.evm_env(&header);

        // verify block env correctly updated
        let expected = BlockEnv {
            number: header.number,
            beneficiary: config.chain_spec().config.fee_vault_address.unwrap(),
            timestamp: header.timestamp,
            prevrandao: Some(header.mix_hash),
            difficulty: U256::ZERO,
            basefee: header.base_fee_per_gas.unwrap_or_default(),
            gas_limit: header.gas_limit,
            blob_excess_gas_and_price: None,
        };
        assert_eq!(env.block_env, expected)
    }

    #[test]
    fn test_next_cfg_and_block_env() -> eyre::Result<()> {
        let config = ScrollEvmConfig::<_, ScrollPrimitives, _>::new(
            ScrollChainSpecBuilder::scroll_mainnet().build(ScrollChainConfig::mainnet()).into(),
            ScrollRethReceiptBuilder::default(),
        );

        // pre curie header
        let header = Header {
            number: 7096835,
            beneficiary: Address::random(),
            timestamp: 1719994274,
            mix_hash: B256::random(),
            base_fee_per_gas: None,
            gas_limit: 10000000,
            ..Default::default()
        };

        // curie block attributes
        let attributes = NextBlockEnvAttributes {
            timestamp: 1719994277,
            suggested_fee_recipient: Address::random(),
            prev_randao: B256::random(),
            gas_limit: 10000000,
            parent_beacon_block_root: None,
            withdrawals: None,
        };

        // get next cfg env and block env
        let env = config.next_evm_env(&header, &attributes)?;
        let (cfg_env, block_env, spec) = (env.cfg_env.clone(), env.block_env, env.cfg_env.spec);

        // verify cfg env
        assert_eq!(cfg_env.chain_id, Scroll as u64);
        assert_eq!(spec, ScrollSpecId::CURIE);

        // verify block env
        let expected = BlockEnv {
            number: header.number + 1,
            beneficiary: config.chain_spec().config.fee_vault_address.unwrap(),
            timestamp: attributes.timestamp,
            prevrandao: Some(attributes.prev_randao),
            difficulty: U256::ZERO,
            // TODO(scroll): this shouldn't be 0 at curie fork
            basefee: 0,
            gas_limit: header.gas_limit,
            blob_excess_gas_and_price: None,
        };
        assert_eq!(block_env, expected);

        Ok(())
    }
}
