//! Scroll-Reth `eth_` endpoint implementation.

use std::{fmt, sync::Arc};

use alloy_primitives::U256;
use reth_chainspec::{EthChainSpec, EthereumHardforks};
use reth_evm::ConfigureEvm;
use reth_network_api::NetworkInfo;
use reth_node_api::NodePrimitives;
use reth_node_builder::EthApiBuilderCtx;
use reth_primitives::EthPrimitives;
use reth_provider::{
    BlockNumReader, BlockReader, BlockReaderIdExt, CanonStateSubscriptions, ChainSpecProvider,
    NodePrimitivesProvider, ProviderBlock, ProviderHeader, ProviderReceipt, ProviderTx,
    StageCheckpointReader, StateProviderFactory,
};
use reth_rpc::eth::{core::EthApiInner, DevSigner};
use reth_rpc_eth_api::{
    helpers::{
        AddDevSigners, EthApiSpec, EthFees, EthSigner, EthState, LoadBlock, LoadFee, LoadState,
        SpawnBlocking, Trace,
    },
    EthApiTypes, RpcNodeCore, RpcNodeCoreExt,
};
use reth_rpc_eth_types::{EthStateCache, FeeHistoryCache, GasPriceOracle};
use reth_tasks::{
    pool::{BlockingTaskGuard, BlockingTaskPool},
    TaskSpawner,
};
use reth_transaction_pool::TransactionPool;

pub use receipt::ScrollReceiptBuilder;
use scroll_alloy_network::Scroll;

use crate::ScrollEthApiError;

mod block;
mod call;
mod pending_block;
pub mod receipt;
pub mod transaction;

/// Adapter for [`EthApiInner`], which holds all the data required to serve core `eth_` API.
pub type EthApiNodeBackend<N> = EthApiInner<
    <N as RpcNodeCore>::Provider,
    <N as RpcNodeCore>::Pool,
    <N as RpcNodeCore>::Network,
    <N as RpcNodeCore>::Evm,
>;

/// A helper trait with requirements for [`RpcNodeCore`] to be used in [`ScrollEthApi`].
pub trait ScrollNodeCore: RpcNodeCore<Provider: BlockReader> {}
impl<T> ScrollNodeCore for T where T: RpcNodeCore<Provider: BlockReader> {}

/// Scroll-Reth `Eth` API implementation.
///
/// This type provides the functionality for handling `eth_` related requests.
///
/// This wraps a default `Eth` implementation, and provides additional functionality where the
/// scroll spec deviates from the default (ethereum) spec, e.g. transaction forwarding to the
/// receipts, additional RPC fields for transaction receipts.
///
/// This type implements the [`FullEthApi`](reth_rpc_eth_api::helpers::FullEthApi) by implemented
/// all the `Eth` helper traits and prerequisite traits.
#[derive(Clone)]
pub struct ScrollEthApi<N: ScrollNodeCore> {
    /// Gateway to node's core components.
    inner: Arc<ScrollEthApiInner<N>>,
}

impl<N> ScrollEthApi<N>
where
    N: ScrollNodeCore<
        Provider: BlockReaderIdExt
                      + ChainSpecProvider
                      + CanonStateSubscriptions<Primitives = EthPrimitives>
                      + Clone
                      + 'static,
    >,
{
    /// Returns a reference to the [`EthApiNodeBackend`].
    #[allow(clippy::missing_const_for_fn)]
    pub fn eth_api(&self) -> &EthApiNodeBackend<N> {
        self.inner.eth_api()
    }

    /// Build a [`ScrollEthApi`] using [`ScrollEthApiBuildlmn9 ,ner`].
    pub const fn builder() -> ScrollEthApiBuilder {
        ScrollEthApiBuilder::new()
    }
}

impl<N> EthApiTypes for ScrollEthApi<N>
where
    Self: Send + Sync,
    N: ScrollNodeCore,
{
    type Error = ScrollEthApiError;
    type NetworkTypes = Scroll;
    type TransactionCompat = Self;

    fn tx_resp_builder(&self) -> &Self::TransactionCompat {
        self
    }
}

impl<N> RpcNodeCore for ScrollEthApi<N>
where
    N: ScrollNodeCore,
{
    type Provider = N::Provider;
    type Pool = N::Pool;
    type Evm = <N as RpcNodeCore>::Evm;
    type Network = <N as RpcNodeCore>::Network;
    type PayloadBuilder = ();

    #[inline]
    fn pool(&self) -> &Self::Pool {
        self.inner.eth_api.pool()
    }

    #[inline]
    fn evm_config(&self) -> &Self::Evm {
        self.inner.eth_api.evm_config()
    }

    #[inline]
    fn network(&self) -> &Self::Network {
        self.inner.eth_api.network()
    }

    #[inline]
    fn payload_builder(&self) -> &Self::PayloadBuilder {
        &()
    }

    #[inline]
    fn provider(&self) -> &Self::Provider {
        self.inner.eth_api.provider()
    }
}

impl<N> RpcNodeCoreExt for ScrollEthApi<N>
where
    N: ScrollNodeCore,
{
    #[inline]
    fn cache(&self) -> &EthStateCache<ProviderBlock<N::Provider>, ProviderReceipt<N::Provider>> {
        self.inner.eth_api.cache()
    }
}

impl<N> EthApiSpec for ScrollEthApi<N>
where
    N: ScrollNodeCore<
        Provider: ChainSpecProvider<ChainSpec: EthereumHardforks>
                      + BlockNumReader
                      + StageCheckpointReader,
        Network: NetworkInfo,
    >,
{
    type Transaction = ProviderTx<Self::Provider>;

    #[inline]
    fn starting_block(&self) -> U256 {
        self.inner.eth_api.starting_block()
    }

    #[inline]
    fn signers(&self) -> &parking_lot::RwLock<Vec<Box<dyn EthSigner<ProviderTx<Self::Provider>>>>> {
        self.inner.eth_api.signers()
    }
}

impl<N> SpawnBlocking for ScrollEthApi<N>
where
    Self: Send + Sync + Clone + 'static,
    N: ScrollNodeCore,
{
    #[inline]
    fn io_task_spawner(&self) -> impl TaskSpawner {
        self.inner.eth_api.task_spawner()
    }

    #[inline]
    fn tracing_task_pool(&self) -> &BlockingTaskPool {
        self.inner.eth_api.blocking_task_pool()
    }

    #[inline]
    fn tracing_task_guard(&self) -> &BlockingTaskGuard {
        self.inner.eth_api.blocking_task_guard()
    }
}

impl<N> LoadFee for ScrollEthApi<N>
where
    Self: LoadBlock<Provider = N::Provider>,
    N: ScrollNodeCore<
        Provider: BlockReaderIdExt
                      + ChainSpecProvider<ChainSpec: EthChainSpec + EthereumHardforks>
                      + StateProviderFactory,
    >,
{
    #[inline]
    fn gas_oracle(&self) -> &GasPriceOracle<Self::Provider> {
        self.inner.eth_api.gas_oracle()
    }

    #[inline]
    fn fee_history_cache(&self) -> &FeeHistoryCache {
        self.inner.eth_api.fee_history_cache()
    }
}

impl<N> LoadState for ScrollEthApi<N> where
    N: ScrollNodeCore<
        Provider: StateProviderFactory + ChainSpecProvider<ChainSpec: EthereumHardforks>,
        Pool: TransactionPool,
    >
{
}

impl<N> EthState for ScrollEthApi<N>
where
    Self: LoadState + SpawnBlocking,
    N: ScrollNodeCore,
{
    #[inline]
    fn max_proof_window(&self) -> u64 {
        self.inner.eth_api.eth_proof_window()
    }
}

impl<N> EthFees for ScrollEthApi<N>
where
    Self: LoadFee,
    N: ScrollNodeCore,
{
}

impl<N> Trace for ScrollEthApi<N>
where
    Self: RpcNodeCore<Provider: BlockReader>
        + LoadState<
            Evm: ConfigureEvm<
                Header = ProviderHeader<Self::Provider>,
                Transaction = ProviderTx<Self::Provider>,
            >,
        >,
    N: ScrollNodeCore,
{
}

impl<N> AddDevSigners for ScrollEthApi<N>
where
    N: ScrollNodeCore,
{
    fn with_dev_accounts(&self) {
        *self.inner.eth_api.signers().write() = DevSigner::random_signers(20)
    }
}

impl<N: ScrollNodeCore> fmt::Debug for ScrollEthApi<N> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ScrollEthApi").finish_non_exhaustive()
    }
}

/// Container type `ScrollEthApi`
#[allow(missing_debug_implementations)]
struct ScrollEthApiInner<N: ScrollNodeCore> {
    /// Gateway to node's core components.
    eth_api: EthApiNodeBackend<N>,
}

impl<N: ScrollNodeCore> ScrollEthApiInner<N> {
    /// Returns a reference to the [`EthApiNodeBackend`].
    const fn eth_api(&self) -> &EthApiNodeBackend<N> {
        &self.eth_api
    }
}

/// A type that knows how to build a [`ScrollEthApi`].
#[derive(Debug, Default)]
pub struct ScrollEthApiBuilder {}

impl ScrollEthApiBuilder {
    /// Creates a [`ScrollEthApiBuilder`] instance from [`EthApiBuilderCtx`].
    pub const fn new() -> Self {
        Self {}
    }
}

impl ScrollEthApiBuilder {
    /// Builds an instance of [`ScrollEthApi`]
    pub fn build<N>(self, ctx: &EthApiBuilderCtx<N>) -> ScrollEthApi<N>
    where
        N: ScrollNodeCore<
            Provider: BlockReaderIdExt<
                Block = <<N::Provider as NodePrimitivesProvider>::Primitives as NodePrimitives>::Block,
                Receipt = <<N::Provider as NodePrimitivesProvider>::Primitives as NodePrimitives>::Receipt,
            > + ChainSpecProvider
            + CanonStateSubscriptions
            + Clone
            + 'static,
        >,
    {
        let blocking_task_pool =
            BlockingTaskPool::build().expect("failed to build blocking task pool");

        let inner = EthApiInner::new(
            ctx.provider.clone(),
            ctx.pool.clone(),
            ctx.network.clone(),
            ctx.cache.clone(),
            ctx.new_gas_price_oracle(),
            ctx.config.rpc_gas_cap,
            ctx.config.rpc_max_simulate_blocks,
            ctx.config.eth_proof_window,
            blocking_task_pool,
            ctx.new_fee_history_cache(),
            ctx.evm_config.clone(),
            ctx.executor.clone(),
            ctx.config.proof_permits,
        );

        ScrollEthApi { inner: Arc::new(ScrollEthApiInner { eth_api: inner }) }
    }
}
