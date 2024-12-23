use alloy_rpc_types_engine::{ExecutionPayload, ExecutionPayloadSidecar, PayloadError};
use reth_ethereum_engine_primitives::{EthEngineTypes, EthPayloadAttributes};
use reth_node_api::PayloadValidator;
use reth_node_builder::{
    rpc::EngineValidatorBuilder, AddOnsContext, EngineApiMessageVersion,
    EngineObjectValidationError, EngineTypes, EngineValidator, FullNodeComponents,
    PayloadOrAttributes,
};
use reth_node_types::NodeTypesWithEngine;
use reth_primitives::{Block, EthPrimitives, SealedBlock, SealedBlockFor};
use reth_scroll_chainspec::ScrollChainSpec;

/// Builder for [`ScrollEngineValidator`].
#[derive(Debug, Default, Clone)]
pub struct ScrollEngineValidatorBuilder;

impl<Node, Types> EngineValidatorBuilder<Node> for ScrollEngineValidatorBuilder
where
    Types: NodeTypesWithEngine<
        ChainSpec = ScrollChainSpec,
        Primitives = EthPrimitives,
        Engine = EthEngineTypes,
    >,
    Node: FullNodeComponents<Types = Types>,
    ScrollEngineValidator: EngineValidator<Types::Engine>,
{
    type Validator = ScrollEngineValidator;

    async fn build(self, _ctx: &AddOnsContext<'_, Node>) -> eyre::Result<Self::Validator> {
        Ok(ScrollEngineValidator)
    }
}

/// Noop engine validator used as default for Scroll.
#[derive(Debug, Clone)]
pub struct ScrollEngineValidator;

impl<Types> EngineValidator<Types> for ScrollEngineValidator
where
    Types: EngineTypes<PayloadAttributes = EthPayloadAttributes>,
{
    fn validate_version_specific_fields(
        &self,
        _version: EngineApiMessageVersion,
        _payload_or_attrs: PayloadOrAttributes<'_, EthPayloadAttributes>,
    ) -> Result<(), EngineObjectValidationError> {
        Ok(())
    }

    fn ensure_well_formed_attributes(
        &self,
        _version: EngineApiMessageVersion,
        _attributes: &EthPayloadAttributes,
    ) -> Result<(), EngineObjectValidationError> {
        Ok(())
    }
}

impl PayloadValidator for ScrollEngineValidator {
    type Block = Block;

    fn ensure_well_formed_payload(
        &self,
        payload: ExecutionPayload,
        sidecar: ExecutionPayloadSidecar,
    ) -> Result<SealedBlockFor<Self::Block>, PayloadError> {
        let expected_hash = payload.block_hash();

        // First parse the block
        let sealed_block = try_into_block(payload, &sidecar)?.seal_slow();

        // Ensure the hash included in the payload matches the block hash
        if expected_hash != sealed_block.hash() {
            return Err(PayloadError::BlockHash {
                execution: sealed_block.hash(),
                consensus: expected_hash,
            })
        }

        if self.is_cancun_active_at_timestamp(sealed_block.timestamp) {
            if sealed_block.header.blob_gas_used.is_none() {
                // cancun active but blob gas used not present
                return Err(PayloadError::PostCancunBlockWithoutBlobGasUsed)
            }
            if sealed_block.header.excess_blob_gas.is_none() {
                // cancun active but excess blob gas not present
                return Err(PayloadError::PostCancunBlockWithoutExcessBlobGas)
            }
            if sidecar.cancun().is_none() {
                // cancun active but cancun fields not present
                return Err(PayloadError::PostCancunWithoutCancunFields)
            }
        } else {
            if sealed_block.body.has_eip4844_transactions() {
                // cancun not active but blob transactions present
                return Err(PayloadError::PreCancunBlockWithBlobTransactions)
            }
            if sealed_block.header.blob_gas_used.is_some() {
                // cancun not active but blob gas used present
                return Err(PayloadError::PreCancunBlockWithBlobGasUsed)
            }
            if sealed_block.header.excess_blob_gas.is_some() {
                // cancun not active but excess blob gas present
                return Err(PayloadError::PreCancunBlockWithExcessBlobGas)
            }
            if sidecar.cancun().is_some() {
                // cancun not active but cancun fields present
                return Err(PayloadError::PreCancunWithCancunFields)
            }
        }

        let shanghai_active = self.is_shanghai_active_at_timestamp(sealed_block.timestamp);
        if !shanghai_active && sealed_block.body.withdrawals.is_some() {
            // shanghai not active but withdrawals present
            return Err(PayloadError::PreShanghaiBlockWithWithdrawals)
        }

        if !self.is_prague_active_at_timestamp(sealed_block.timestamp) &&
            sealed_block.body.has_eip7702_transactions()
        {
            return Err(PayloadError::PrePragueBlockWithEip7702Transactions)
        }

        // EIP-4844 checks
        self.ensure_matching_blob_versioned_hashes(
            &sealed_block,
            &sidecar.cancun().cloned().into(),
        )?;

        Ok(sealed_block)
    }
}
