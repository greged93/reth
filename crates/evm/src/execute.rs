//! Traits for execution.

use crate::{EnvWithHandlerCfg, Evm};
use reth_execution_types::ExecutionOutcome;
use reth_primitives::{Address, BlockNumber, BlockWithSenders, Receipt, Request, U256};
use reth_prune_types::PruneModes;
use revm::{db::BundleState, DatabaseCommit};
use revm_primitives::{db::Database, Account, EVMError, EVMResult, Env, ExecutionResult};
use std::collections::HashMap;

pub use reth_execution_errors::{BlockExecutionError, BlockValidationError};
pub use reth_storage_errors::provider::ProviderError;

/// Trait that transacts an input (e.g. transaction and height) and produces an output
/// (e.g. state changes).
///
/// The trait cannot commit the state changes to the database directly, see [`EvmCommit`].
pub trait EvmTransact {
    /// The database used by the evm.
    type DB: Database;

    /// Transacts with the current env to produce an output without
    /// committing to the underlying database.
    fn transact(&mut self) -> EVMResult<<<Self as EvmTransact>::DB as Database>::Error>;

    /// Returns a mutable reference to the current env.
    fn env_mut(&mut self) -> &mut Env;

    /// Returns a reference to the current env.
    fn env(&self) -> &Env;

    /// Returns the current env with handler cfg.
    fn env_with_handler_cfg(&self) -> EnvWithHandlerCfg;
}

/// Trait that can transact an [`EvmTransact::Env`] and
/// commit state changes to the database.
pub trait EvmCommit: EvmTransact {
    /// The output produced by the evm.
    type EvmOutput;
    /// The error produced by the evm.
    type EvmError;

    /// Commit state changes to the database.
    fn commit(&mut self, output: HashMap<Address, Account>);

    /// Transact using [`EvmTransact::Env`], commit the state changes and
    /// return them.
    fn transact_and_commit(&mut self) -> Result<Self::EvmOutput, Self::EvmError>;
}

impl<'a, C, DB> EvmTransact for Evm<'a, C, DB>
where
    DB: Database + 'a,
{
    type DB = DB;

    fn transact(&mut self) -> EVMResult<<<Self as EvmTransact>::DB as Database>::Error> {
        self.transact()
    }

    fn env_mut(&mut self) -> &mut Env {
        &mut self.context.evm.inner.env
    }

    fn env(&self) -> &Env {
        &self.context.evm.inner.env
    }

    fn env_with_handler_cfg(&self) -> EnvWithHandlerCfg {
        EnvWithHandlerCfg { env: Box::new(self.env().clone()), handler_cfg: self.handler.cfg() }
    }
}

impl<'a, C, DB> EvmCommit for Evm<'a, C, DB>
where
    DB: Database + DatabaseCommit + 'a,
{
    type EvmOutput = ExecutionResult;
    type EvmError = EVMError<DB::Error>;

    fn commit(&mut self, output: HashMap<Address, Account>) {
        self.context.evm.db.commit(output)
    }

    fn transact_and_commit(&mut self) -> Result<Self::EvmOutput, Self::EvmError> {
        self.transact_commit()
    }
}

/// A general purpose executor trait that executes an input (e.g. block) and produces an output
/// (e.g. state changes and receipts).
///
/// This executor does not validate the output, see [`BatchExecutor`] for that.
pub trait Executor<DB> {
    /// The input type for the executor.
    type Input<'a>;
    /// The output type for the executor.
    type Output;
    /// The error type returned by the executor.
    type Error;

    /// Consumes the type and executes the block.
    ///
    /// # Note
    /// Execution happens without any validation of the output. To validate the output, use the
    /// [`BatchExecutor`].
    ///
    /// # Returns
    /// The output of the block execution.
    fn execute(self, input: Self::Input<'_>) -> Result<Self::Output, Self::Error>;
}

/// A general purpose executor that can execute multiple inputs in sequence, validate the outputs,
/// and keep track of the state over the entire batch.
pub trait BatchExecutor<DB> {
    /// The input type for the executor.
    type Input<'a>;
    /// The output type for the executor.
    type Output;
    /// The error type returned by the executor.
    type Error;

    /// Executes the next block in the batch, verifies the output and updates the state internally.
    fn execute_and_verify_one(&mut self, input: Self::Input<'_>) -> Result<(), Self::Error>;

    /// Executes multiple inputs in the batch, verifies the output, and updates the state
    /// internally.
    ///
    /// This method is a convenience function for calling [`BatchExecutor::execute_and_verify_one`]
    /// for each input.
    fn execute_and_verify_many<'a, I>(&mut self, inputs: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Self::Input<'a>>,
    {
        for input in inputs {
            self.execute_and_verify_one(input)?;
        }
        Ok(())
    }

    /// Executes the entire batch, verifies the output, and returns the final state.
    ///
    /// This method is a convenience function for calling [`BatchExecutor::execute_and_verify_many`]
    /// and [`BatchExecutor::finalize`].
    fn execute_and_verify_batch<'a, I>(mut self, batch: I) -> Result<Self::Output, Self::Error>
    where
        I: IntoIterator<Item = Self::Input<'a>>,
        Self: Sized,
    {
        self.execute_and_verify_many(batch)?;
        Ok(self.finalize())
    }

    /// Finishes the batch and return the final state.
    fn finalize(self) -> Self::Output;

    /// Set the expected tip of the batch.
    ///
    /// This can be used to optimize state pruning during execution.
    fn set_tip(&mut self, tip: BlockNumber);

    /// The size hint of the batch's tracked state size.
    ///
    /// This is used to optimize DB commits depending on the size of the state.
    fn size_hint(&self) -> Option<usize>;
}

/// The output of an ethereum block.
///
/// Contains the state changes, transaction receipts, and total gas used in the block.
///
/// TODO(mattsse): combine with `ExecutionOutcome`
#[derive(Debug)]
pub struct BlockExecutionOutput<T> {
    /// The changed state of the block after execution.
    pub state: BundleState,
    /// All the receipts of the transactions in the block.
    pub receipts: Vec<T>,
    /// All the EIP-7685 requests of the transactions in the block.
    pub requests: Vec<Request>,
    /// The total gas used by the block.
    pub gas_used: u64,
}

/// A helper type for ethereum block inputs that consists of a block and the total difficulty.
#[derive(Debug)]
pub struct BlockExecutionInput<'a, Block> {
    /// The block to execute.
    pub block: &'a Block,
    /// The total difficulty of the block.
    pub total_difficulty: U256,
}

impl<'a, Block> BlockExecutionInput<'a, Block> {
    /// Creates a new input.
    pub const fn new(block: &'a Block, total_difficulty: U256) -> Self {
        Self { block, total_difficulty }
    }
}

impl<'a, Block> From<(&'a Block, U256)> for BlockExecutionInput<'a, Block> {
    fn from((block, total_difficulty): (&'a Block, U256)) -> Self {
        Self::new(block, total_difficulty)
    }
}

/// A type that can create a new executor for block execution.
pub trait BlockExecutorProvider: Send + Sync + Clone + Unpin + 'static {
    /// An executor that can execute a single block given a database.
    ///
    /// # Verification
    ///
    /// The on [`Executor::execute`] the executor is expected to validate the execution output of
    /// the input, this includes:
    /// - Cumulative gas used must match the input's gas used.
    /// - Receipts must match the input's receipts root.
    ///
    /// It is not expected to validate the state trie root, this must be done by the caller using
    /// the returned state.
    type Executor<DB: Database<Error = ProviderError>>: for<'a> Executor<
        DB,
        Input<'a> = BlockExecutionInput<'a, BlockWithSenders>,
        Output = BlockExecutionOutput<Receipt>,
        Error = BlockExecutionError,
    >;

    /// An executor that can execute a batch of blocks given a database.
    type BatchExecutor<DB: Database<Error = ProviderError>>: for<'a> BatchExecutor<
        DB,
        Input<'a> = BlockExecutionInput<'a, BlockWithSenders>,
        Output = ExecutionOutcome,
        Error = BlockExecutionError,
    >;

    /// Creates a new executor for single block execution.
    ///
    /// This is used to execute a single block and get the changed state.
    fn executor<DB>(&self, db: DB) -> Self::Executor<DB>
    where
        DB: Database<Error = ProviderError>;

    /// Creates a new batch executor with the given database and pruning modes.
    ///
    /// Batch executor is used to execute multiple blocks in sequence and keep track of the state
    /// during historical sync which involves executing multiple blocks in sequence.
    ///
    /// The pruning modes are used to determine which parts of the state should be kept during
    /// execution.
    fn batch_executor<DB>(&self, db: DB, prune_modes: PruneModes) -> Self::BatchExecutor<DB>
    where
        DB: Database<Error = ProviderError>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use reth_primitives::Block;
    use revm::db::{CacheDB, EmptyDBTyped};
    use std::marker::PhantomData;

    #[derive(Clone, Default)]
    struct TestExecutorProvider;

    impl BlockExecutorProvider for TestExecutorProvider {
        type Executor<DB: Database<Error = ProviderError>> = TestExecutor<DB>;
        type BatchExecutor<DB: Database<Error = ProviderError>> = TestExecutor<DB>;

        fn executor<DB>(&self, _db: DB) -> Self::Executor<DB>
        where
            DB: Database<Error = ProviderError>,
        {
            TestExecutor(PhantomData)
        }

        fn batch_executor<DB>(&self, _db: DB, _prune_modes: PruneModes) -> Self::BatchExecutor<DB>
        where
            DB: Database<Error = ProviderError>,
        {
            TestExecutor(PhantomData)
        }
    }

    struct TestExecutor<DB>(PhantomData<DB>);

    impl<DB> Executor<DB> for TestExecutor<DB> {
        type Input<'a> = BlockExecutionInput<'a, BlockWithSenders>;
        type Output = BlockExecutionOutput<Receipt>;
        type Error = BlockExecutionError;

        fn execute(self, _input: Self::Input<'_>) -> Result<Self::Output, Self::Error> {
            Err(BlockExecutionError::msg("execution unavailable for tests"))
        }
    }

    impl<DB> BatchExecutor<DB> for TestExecutor<DB> {
        type Input<'a> = BlockExecutionInput<'a, BlockWithSenders>;
        type Output = ExecutionOutcome;
        type Error = BlockExecutionError;

        fn execute_and_verify_one(&mut self, _input: Self::Input<'_>) -> Result<(), Self::Error> {
            Ok(())
        }

        fn finalize(self) -> Self::Output {
            todo!()
        }

        fn set_tip(&mut self, _tip: BlockNumber) {
            todo!()
        }

        fn size_hint(&self) -> Option<usize> {
            None
        }
    }

    #[test]
    fn test_provider() {
        let provider = TestExecutorProvider;
        let db = CacheDB::<EmptyDBTyped<ProviderError>>::default();
        let executor = provider.executor(db);
        let block = Block {
            header: Default::default(),
            body: vec![],
            ommers: vec![],
            withdrawals: None,
            requests: None,
        };
        let block = BlockWithSenders::new(block, Default::default()).unwrap();
        let _ = executor.execute(BlockExecutionInput::new(&block, U256::ZERO));
    }
}
