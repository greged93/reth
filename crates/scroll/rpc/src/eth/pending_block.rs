//! Loads Scroll pending block for an RPC response.

use crate::ScrollEthApi;

use alloy_consensus::{BlockHeader, Header};
use alloy_primitives::B256;
use reth_chainspec::EthChainSpec;
use reth_evm::{execute::BlockExecutionStrategyFactory, ConfigureEvmEnv, NextBlockEnvAttributes};
use reth_primitives_traits::{NodePrimitives, SealedHeader};
use reth_provider::{
    BlockReaderIdExt, ChainSpecProvider, ProviderBlock, ProviderHeader, ProviderReceipt,
    ProviderTx, StateProviderFactory,
};
use reth_rpc_eth_api::{
    helpers::{LoadPendingBlock, SpawnBlocking},
    types::RpcTypes,
    EthApiTypes, RpcNodeCore,
};
use reth_rpc_eth_types::{error::FromEvmError, PendingBlock};
use reth_scroll_primitives::{ScrollBlock, ScrollReceipt, ScrollTransactionSigned};
use reth_transaction_pool::{PoolTransaction, TransactionPool};
use scroll_alloy_hardforks::ScrollHardforks;

impl<N> LoadPendingBlock for ScrollEthApi<N>
where
    Self: SpawnBlocking
        + EthApiTypes<
            NetworkTypes: RpcTypes<
                Header = alloy_rpc_types_eth::Header<ProviderHeader<Self::Provider>>,
            >,
            Error: FromEvmError<Self::Evm>,
        >,
    N: RpcNodeCore<
        Provider: BlockReaderIdExt<
            Transaction = ScrollTransactionSigned,
            Block = ScrollBlock,
            Receipt = ScrollReceipt,
            Header = Header,
        > + ChainSpecProvider<ChainSpec: EthChainSpec + ScrollHardforks>
                      + StateProviderFactory,
        Pool: TransactionPool<Transaction: PoolTransaction<Consensus = ProviderTx<N::Provider>>>,
        Evm: BlockExecutionStrategyFactory<
            Primitives: NodePrimitives<
                SignedTx = ProviderTx<Self::Provider>,
                BlockHeader = ProviderHeader<Self::Provider>,
                Receipt = ProviderReceipt<Self::Provider>,
                Block = ProviderBlock<Self::Provider>,
            >,
            NextBlockEnvCtx = NextBlockEnvAttributes,
        >,
    >,
{
    #[inline]
    fn pending_block(
        &self,
    ) -> &tokio::sync::Mutex<
        Option<PendingBlock<ProviderBlock<Self::Provider>, ProviderReceipt<Self::Provider>>>,
    > {
        self.inner.eth_api.pending_block()
    }

    fn next_env_attributes(
        &self,
        parent: &SealedHeader<ProviderHeader<Self::Provider>>,
    ) -> Result<<Self::Evm as ConfigureEvmEnv>::NextBlockEnvCtx, Self::Error> {
        Ok(NextBlockEnvAttributes {
            timestamp: parent.timestamp().saturating_add(12),
            suggested_fee_recipient: parent.beneficiary(),
            prev_randao: B256::random(),
            gas_limit: parent.gas_limit(),
            parent_beacon_block_root: None,
            withdrawals: None,
        })
    }
}
