//! Implementation of the [`BlockExecutionStrategyFactory`] for Scroll.

use crate::{build::ScrollBlockAssembler, receipt::ScrollRethReceiptBuilder, ScrollEvmConfig};
use std::{fmt::Debug, sync::Arc};

use alloy_consensus::{BlockHeader, Header};
use alloy_evm::{block::BlockExecutorFactory, FromRecoveredTx};
use alloy_primitives::{Address, B256};
use reth_chainspec::EthChainSpec;
use reth_evm::execute::{BasicBlockExecutorProvider, BlockExecutionStrategyFactory};
use reth_primitives::SealedBlock;
use reth_primitives_traits::{Block, NodePrimitives, SealedHeader, SignedTransaction};
use reth_scroll_chainspec::{ChainConfig, ScrollChainConfig, ScrollChainSpec};
use reth_scroll_primitives::ScrollReceipt;
use revm::context::TxEnv;
use scroll_alloy_evm::{
    ScrollBlockExecutionCtx, ScrollBlockExecutorFactory, ScrollReceiptBuilder,
    ScrollTransactionIntoTxEnv,
};
use scroll_alloy_hardforks::ScrollHardforks;

impl<ChainSpec, N, R> BlockExecutionStrategyFactory for ScrollEvmConfig<ChainSpec, N, R>
where
    ChainSpec: EthChainSpec + ScrollHardforks + ChainConfig<Config = ScrollChainConfig>,
    N: NodePrimitives<
        Receipt = R::Receipt,
        SignedTx = R::Transaction,
        BlockHeader = Header,
        BlockBody = alloy_consensus::BlockBody<R::Transaction>,
    >,
    ScrollTransactionIntoTxEnv<TxEnv>: FromRecoveredTx<N::SignedTx>,
    R: ScrollReceiptBuilder<Receipt = ScrollReceipt, Transaction: SignedTransaction>,
    Self: Send + Sync + Unpin + Clone + 'static,
{
    type Primitives = N;
    type BlockExecutorFactory = ScrollBlockExecutorFactory<R, Arc<ChainSpec>>;
    type BlockAssembler = ScrollBlockAssembler<ChainSpec>;

    fn block_executor_factory(&self) -> &Self::BlockExecutorFactory {
        &self.executor_factory
    }

    fn block_assembler(&self) -> &Self::BlockAssembler {
        &self.block_assembler
    }

    fn context_for_block<'a>(
        &self,
        block: &'a reth_primitives_traits::SealedBlock<<Self::Primitives as NodePrimitives>::Block>,
    ) -> <Self::BlockExecutorFactory as BlockExecutorFactory>::ExecutionCtx<'a> {
        ScrollBlockExecutionCtx { parent_hash: block.header().parent_hash() }
    }

    fn context_for_next_block(
        &self,
        parent: &SealedHeader<<Self::Primitives as NodePrimitives>::BlockHeader>,
        _attributes: Self::NextBlockEnvCtx,
    ) -> <Self::BlockExecutorFactory as BlockExecutorFactory>::ExecutionCtx<'_> {
        ScrollBlockExecutionCtx { parent_hash: parent.hash() }
    }
}

/// Input for block execution.
#[derive(Debug, Clone, Copy)]
pub struct ScrollBlockExecutionInput {
    /// Block number.
    pub number: u64,
    /// Block timestamp.
    pub timestamp: u64,
    /// Parent block hash.
    pub parent_hash: B256,
    /// Block gas limit.
    pub gas_limit: u64,
    /// Block beneficiary.
    pub beneficiary: Address,
}

impl<B: Block> From<&SealedBlock<B>> for ScrollBlockExecutionInput {
    fn from(block: &SealedBlock<B>) -> Self {
        Self {
            number: block.header().number(),
            timestamp: block.header().timestamp(),
            parent_hash: block.header().parent_hash(),
            gas_limit: block.header().gas_limit(),
            beneficiary: block.header().beneficiary(),
        }
    }
}

/// Helper type with backwards compatible methods to obtain Scroll executor
/// providers.
#[derive(Debug)]
pub struct ScrollExecutorProvider;

impl ScrollExecutorProvider {
    /// Creates a new default scroll executor provider.
    pub fn scroll(chain_spec: Arc<ScrollChainSpec>) -> BasicBlockExecutorProvider<ScrollEvmConfig> {
        BasicBlockExecutorProvider::new(ScrollEvmConfig::new(
            chain_spec,
            ScrollRethReceiptBuilder::default(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use crate::{ScrollEvmConfig, ScrollRethReceiptBuilder};
    use std::{convert::Infallible, sync::Arc};

    use alloy_consensus::{Block, BlockBody, Header};
    use alloy_evm::{
        block::{BlockExecutionResult, BlockExecutor},
        Evm,
    };
    use reth_chainspec::MIN_TRANSACTION_GAS;
    use reth_evm::ConfigureEvm;
    use reth_primitives_traits::{NodePrimitives, RecoveredBlock, SignedTransaction};
    use reth_scroll_chainspec::{ScrollChainConfig, ScrollChainSpec, ScrollChainSpecBuilder};
    use reth_scroll_primitives::{
        ScrollBlock, ScrollPrimitives, ScrollReceipt, ScrollTransactionSigned,
    };
    use revm::{
        bytecode::Bytecode,
        database::{
            states::{bundle_state::BundleRetention, StorageSlot},
            EmptyDBTyped, State,
        },
        inspector::NoOpInspector,
        primitives::{Address, TxKind, B256, U256},
        state::AccountInfo,
    };
    use scroll_alloy_consensus::{ScrollTransactionReceipt, ScrollTxType, ScrollTypedTransaction};
    use scroll_alloy_evm::{
        curie::{
            BLOB_SCALAR_SLOT, COMMIT_SCALAR_SLOT, CURIE_L1_GAS_PRICE_ORACLE_BYTECODE,
            CURIE_L1_GAS_PRICE_ORACLE_STORAGE, IS_CURIE_SLOT, L1_BLOB_BASE_FEE_SLOT,
            L1_GAS_PRICE_ORACLE_ADDRESS,
        },
        ScrollBlockExecutor, ScrollEvm,
    };
    use scroll_alloy_hardforks::ScrollHardforks;

    const BLOCK_GAS_LIMIT: u64 = 10_000_000;
    const SCROLL_CHAIN_ID: u64 = 534352;
    const NOT_CURIE_BLOCK_NUMBER: u64 = 7096835;
    const CURIE_BLOCK_NUMBER: u64 = 7096837;

    const L1_BASE_FEE_SLOT: U256 = U256::from_limbs([1, 0, 0, 0]);
    const OVER_HEAD_SLOT: U256 = U256::from_limbs([2, 0, 0, 0]);
    const SCALAR_SLOT: U256 = U256::from_limbs([3, 0, 0, 0]);

    fn state() -> State<EmptyDBTyped<Infallible>> {
        let db = EmptyDBTyped::<Infallible>::new();
        State::builder().with_database(db).with_bundle_update().without_state_clear().build()
    }

    #[allow(clippy::type_complexity)]
    fn executor<'a>(
        block: &RecoveredBlock<ScrollBlock>,
        state: &'a mut State<EmptyDBTyped<Infallible>>,
    ) -> ScrollBlockExecutor<
        ScrollEvm<&'a mut State<EmptyDBTyped<Infallible>>, NoOpInspector>,
        ScrollRethReceiptBuilder,
        Arc<ScrollChainSpec>,
    > {
        let chain_spec =
            Arc::new(ScrollChainSpecBuilder::scroll_mainnet().build(ScrollChainConfig::mainnet()));
        let evm_config = ScrollEvmConfig::scroll(chain_spec.clone());

        let evm = evm_config.evm_for_block(state, block.header());
        let receipt_builder = ScrollRethReceiptBuilder::default();
        ScrollBlockExecutor::new(evm, chain_spec, receipt_builder)
    }

    fn block(
        number: u64,
        transactions: Vec<ScrollTransactionSigned>,
    ) -> RecoveredBlock<<ScrollPrimitives as NodePrimitives>::Block> {
        let senders = transactions.iter().map(|t| t.recover_signer().unwrap()).collect();
        RecoveredBlock::new_unhashed(
            Block {
                header: Header { number, gas_limit: BLOCK_GAS_LIMIT, ..Default::default() },
                body: BlockBody { transactions, ..Default::default() },
            },
            senders,
        )
    }

    fn transaction(typ: ScrollTxType, gas_limit: u64) -> ScrollTransactionSigned {
        let transaction = match typ {
            ScrollTxType::Legacy => ScrollTypedTransaction::Legacy(alloy_consensus::TxLegacy {
                to: TxKind::Call(Address::ZERO),
                chain_id: Some(SCROLL_CHAIN_ID),
                gas_limit,
                ..Default::default()
            }),
            ScrollTxType::Eip2930 => ScrollTypedTransaction::Eip2930(alloy_consensus::TxEip2930 {
                to: TxKind::Call(Address::ZERO),
                chain_id: SCROLL_CHAIN_ID,
                gas_limit,
                ..Default::default()
            }),
            ScrollTxType::Eip1559 => ScrollTypedTransaction::Eip1559(alloy_consensus::TxEip1559 {
                to: TxKind::Call(Address::ZERO),
                chain_id: SCROLL_CHAIN_ID,
                gas_limit,
                ..Default::default()
            }),
            ScrollTxType::L1Message => {
                ScrollTypedTransaction::L1Message(scroll_alloy_consensus::TxL1Message {
                    sender: Address::random(),
                    to: Address::ZERO,
                    gas_limit,
                    ..Default::default()
                })
            }
        };

        let pk = B256::random();
        let signature = reth_primitives::sign_message(pk, transaction.signature_hash()).unwrap();
        ScrollTransactionSigned::new_unhashed(transaction, signature)
    }

    fn execute_transaction(
        tx_type: ScrollTxType,
        block_number: u64,
        expected_l1_fee: U256,
        expected_error: Option<&str>,
    ) -> eyre::Result<()> {
        // prepare transaction
        let transaction = transaction(tx_type, MIN_TRANSACTION_GAS);
        let block = block(block_number, vec![transaction.clone()]);

        // init strategy
        let mut state = state();
        let mut strategy = executor(&block, &mut state);

        // determine l1 gas oracle storage
        let l1_gas_oracle_storage = if strategy.spec().is_curie_active_at_block(block_number) {
            vec![
                (L1_BLOB_BASE_FEE_SLOT, U256::from(1000)),
                (OVER_HEAD_SLOT, U256::from(1000)),
                (SCALAR_SLOT, U256::from(1000)),
                (L1_BLOB_BASE_FEE_SLOT, U256::from(10000)),
                (COMMIT_SCALAR_SLOT, U256::from(1000)),
                (BLOB_SCALAR_SLOT, U256::from(10000)),
                (IS_CURIE_SLOT, U256::from(1)),
            ]
        } else {
            vec![
                (L1_BASE_FEE_SLOT, U256::from(1000)),
                (OVER_HEAD_SLOT, U256::from(1000)),
                (SCALAR_SLOT, U256::from(1000)),
            ]
        }
        .into_iter()
        .collect();

        // load accounts in state
        strategy.evm_mut().db_mut().insert_account_with_storage(
            L1_GAS_PRICE_ORACLE_ADDRESS,
            Default::default(),
            l1_gas_oracle_storage,
        );
        for add in block.senders() {
            strategy
                .evm_mut()
                .db_mut()
                .insert_account(*add, AccountInfo { balance: U256::MAX, ..Default::default() });
        }

        // execute and verify output
        let res = strategy
            .execute_transaction(transaction.try_into_recovered().unwrap().as_recovered_ref());

        // check for error or execution outcome
        let output = strategy.apply_post_execution_changes()?;
        if let Some(error) = expected_error {
            assert!(res.unwrap_err().to_string().contains(error));
        } else {
            let BlockExecutionResult { receipts, .. } = output;
            let inner = alloy_consensus::Receipt {
                cumulative_gas_used: MIN_TRANSACTION_GAS,
                status: true.into(),
                ..Default::default()
            };
            let into_scroll_receipt = |inner: alloy_consensus::Receipt| {
                ScrollTransactionReceipt::new(inner, expected_l1_fee)
            };
            let receipt = match tx_type {
                ScrollTxType::Legacy => ScrollReceipt::Legacy(into_scroll_receipt(inner)),
                ScrollTxType::Eip2930 => ScrollReceipt::Eip2930(into_scroll_receipt(inner)),
                ScrollTxType::Eip1559 => ScrollReceipt::Eip1559(into_scroll_receipt(inner)),
                ScrollTxType::L1Message => ScrollReceipt::L1Message(inner),
            };
            let expected = vec![receipt];

            assert_eq!(receipts, expected);
        }

        Ok(())
    }

    #[test]
    fn test_apply_pre_execution_changes_curie_block() -> eyre::Result<()> {
        // init curie transition block
        let curie_block = block(7096836, vec![]);

        // init strategy
        let mut state = state();
        let mut strategy = executor(&curie_block, &mut state);

        // apply pre execution change
        strategy.apply_pre_execution_changes()?;

        // take bundle
        let state = strategy.evm_mut().db_mut();
        state.merge_transitions(BundleRetention::Reverts);
        let bundle = state.take_bundle();

        // assert oracle contract contains updated bytecode
        let oracle = bundle.state.get(&L1_GAS_PRICE_ORACLE_ADDRESS).unwrap().clone();
        let bytecode = Bytecode::new_raw(CURIE_L1_GAS_PRICE_ORACLE_BYTECODE);
        assert_eq!(oracle.info.unwrap().code.unwrap(), bytecode);

        // check oracle contract contains storage changeset
        let mut storage = oracle.storage.into_iter().collect::<Vec<(U256, StorageSlot)>>();
        storage.sort_by(|(a, _), (b, _)| a.cmp(b));
        for (got, expected) in storage.into_iter().zip(CURIE_L1_GAS_PRICE_ORACLE_STORAGE) {
            assert_eq!(got.0, expected.0);
            assert_eq!(got.1, StorageSlot { present_value: expected.1, ..Default::default() });
        }

        Ok(())
    }

    #[test]
    fn test_apply_pre_execution_changes_not_curie_block() -> eyre::Result<()> {
        // init block
        let not_curie_block = block(7096837, vec![]);

        // init strategy
        let mut state = state();
        let mut strategy = executor(&not_curie_block, &mut state);

        // apply pre execution change
        strategy.apply_pre_execution_changes()?;

        // take bundle
        let state = strategy.evm_mut().db_mut();
        state.merge_transitions(BundleRetention::Reverts);
        let bundle = state.take_bundle();

        // assert oracle contract is empty
        let oracle = bundle.state.get(&L1_GAS_PRICE_ORACLE_ADDRESS);
        assert!(oracle.is_none());

        Ok(())
    }

    #[test]
    fn test_execute_transactions_exceeds_block_gas_limit() -> eyre::Result<()> {
        // prepare transaction exceeding block gas limit
        let transaction = transaction(ScrollTxType::Legacy, BLOCK_GAS_LIMIT + 1);
        let block = block(7096837, vec![transaction.clone()]);

        // init strategy
        let mut state = state();
        let mut strategy = executor(&block, &mut state);

        // execute and verify error
        let res = strategy.execute_transaction(
            transaction.try_into_recovered().expect("failed to recover tx").as_recovered_ref(),
        );
        assert_eq!(
            res.unwrap_err().to_string(),
            "transaction gas limit 10000001 is more than blocks available gas 10000000"
        );

        Ok(())
    }

    #[test]
    fn test_execute_transactions_l1_message() -> eyre::Result<()> {
        // Execute l1 message on curie block
        let expected_l1_fee = U256::ZERO;
        execute_transaction(ScrollTxType::L1Message, CURIE_BLOCK_NUMBER, expected_l1_fee, None)?;
        Ok(())
    }

    #[test]
    fn test_execute_transactions_legacy_curie_fork() -> eyre::Result<()> {
        // Execute legacy transaction on curie block
        let expected_l1_fee = U256::from(10);
        execute_transaction(ScrollTxType::Legacy, CURIE_BLOCK_NUMBER, expected_l1_fee, None)?;
        Ok(())
    }

    #[test]
    fn test_execute_transactions_legacy_not_curie_fork() -> eyre::Result<()> {
        // Execute legacy before curie block
        let expected_l1_fee = U256::from(2);
        execute_transaction(ScrollTxType::Legacy, NOT_CURIE_BLOCK_NUMBER, expected_l1_fee, None)?;
        Ok(())
    }

    #[test]
    fn test_execute_transactions_eip2930_curie_fork() -> eyre::Result<()> {
        // Execute eip2930 transaction on curie block
        let expected_l1_fee = U256::from(10);
        execute_transaction(ScrollTxType::Eip2930, CURIE_BLOCK_NUMBER, expected_l1_fee, None)?;
        Ok(())
    }

    #[test]
    fn test_execute_transactions_eip2930_not_curie_fork() -> eyre::Result<()> {
        // Execute eip2930 transaction before curie block
        execute_transaction(
            ScrollTxType::Eip2930,
            NOT_CURIE_BLOCK_NUMBER,
            U256::ZERO,
            Some("Eip2930 is not supported"),
        )?;
        Ok(())
    }

    #[test]
    fn test_execute_transactions_eip1559_curie_fork() -> eyre::Result<()> {
        // Execute eip1559 transaction on curie block
        let expected_l1_fee = U256::from(10);
        execute_transaction(ScrollTxType::Eip1559, CURIE_BLOCK_NUMBER, expected_l1_fee, None)?;
        Ok(())
    }

    #[test]
    fn test_execute_transactions_eip_not_curie_fork() -> eyre::Result<()> {
        // Execute eip1559 transaction before curie block
        execute_transaction(
            ScrollTxType::Eip1559,
            NOT_CURIE_BLOCK_NUMBER,
            U256::ZERO,
            Some("Eip1559 is not supported"),
        )?;
        Ok(())
    }
}
