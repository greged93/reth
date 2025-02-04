//! Outcome of a Scroll block building task with payload attributes provided via the Engine API.

use core::iter;

use alloy_eips::eip7685::Requests;
use alloy_primitives::U256;
use alloy_rpc_types_engine::{
    BlobsBundleV1, ExecutionPayloadEnvelopeV2, ExecutionPayloadEnvelopeV3,
    ExecutionPayloadEnvelopeV4, ExecutionPayloadFieldV2, ExecutionPayloadV1, ExecutionPayloadV3,
    PayloadId,
};
use reth_chain_state::ExecutedBlockWithTrieUpdates;
use reth_payload_primitives::BuiltPayload;
use reth_primitives_traits::SealedBlock;
use reth_scroll_primitives::{ScrollBlock, ScrollPrimitives};

/// Contains the built payload.
#[derive(Debug, Clone, Default)]
pub struct ScrollBuiltPayload {
    /// Identifier of the payload
    pub(crate) id: PayloadId,
    /// Block execution data for the payload
    pub(crate) block: ExecutedBlockWithTrieUpdates<ScrollPrimitives>,
    /// The fees of the block
    pub(crate) fees: U256,
}

impl ScrollBuiltPayload {
    /// Initializes the payload with the given initial block.
    pub const fn new(
        id: PayloadId,
        block: ExecutedBlockWithTrieUpdates<ScrollPrimitives>,
        fees: U256,
    ) -> Self {
        Self { id, block, fees }
    }

    /// Returns the identifier of the payload.
    pub const fn id(&self) -> PayloadId {
        self.id
    }

    /// Returns the built block(sealed)
    #[allow(clippy::missing_const_for_fn)]
    pub fn block(&self) -> &SealedBlock<ScrollBlock> {
        self.block.sealed_block()
    }

    /// Fees of the block
    pub const fn fees(&self) -> U256 {
        self.fees
    }
}

impl BuiltPayload for ScrollBuiltPayload {
    type Primitives = ScrollPrimitives;

    fn block(&self) -> &SealedBlock<ScrollBlock> {
        self.block()
    }

    fn fees(&self) -> U256 {
        self.fees
    }

    fn executed_block(&self) -> Option<ExecutedBlockWithTrieUpdates<ScrollPrimitives>> {
        Some(self.block.clone())
    }

    fn requests(&self) -> Option<Requests> {
        None
    }
}

impl BuiltPayload for &ScrollBuiltPayload {
    type Primitives = ScrollPrimitives;

    fn block(&self) -> &SealedBlock<ScrollBlock> {
        (**self).block()
    }

    fn fees(&self) -> U256 {
        (**self).fees()
    }

    fn executed_block(&self) -> Option<ExecutedBlockWithTrieUpdates<ScrollPrimitives>> {
        Some(self.block.clone())
    }

    fn requests(&self) -> Option<Requests> {
        None
    }
}

// V1 engine_getPayloadV1 response
impl From<ScrollBuiltPayload> for ExecutionPayloadV1 {
    fn from(value: ScrollBuiltPayload) -> Self {
        Self::from_block_unchecked(
            value.block().hash(),
            &value.block.into_sealed_block().into_block(),
        )
    }
}

// V2 engine_getPayloadV2 response
impl From<ScrollBuiltPayload> for ExecutionPayloadEnvelopeV2 {
    fn from(value: ScrollBuiltPayload) -> Self {
        let ScrollBuiltPayload { block, fees, .. } = value;

        let block = block.into_sealed_block();
        Self {
            block_value: fees,
            execution_payload: ExecutionPayloadFieldV2::from_block_unchecked(
                block.hash(),
                &block.into_block(),
            ),
        }
    }
}

impl From<ScrollBuiltPayload> for ExecutionPayloadEnvelopeV3 {
    fn from(value: ScrollBuiltPayload) -> Self {
        let ScrollBuiltPayload { block, fees, .. } = value;

        let block = block.into_sealed_block();
        Self {
            execution_payload: ExecutionPayloadV3::from_block_unchecked(
                block.hash(),
                &block.into_block(),
            ),
            block_value: fees,
            // From the engine API spec:
            //
            // > Client software **MAY** use any heuristics to decide whether to set
            // `shouldOverrideBuilder` flag or not. If client software does not implement any
            // heuristic this flag **SHOULD** be set to `false`.
            //
            // Spec:
            // <https://github.com/ethereum/execution-apis/blob/fe8e13c288c592ec154ce25c534e26cb7ce0530d/src/engine/cancun.md#specification-2>
            should_override_builder: false,
            blobs_bundle: BlobsBundleV1::new(iter::empty()),
        }
    }
}
impl From<ScrollBuiltPayload> for ExecutionPayloadEnvelopeV4 {
    fn from(value: ScrollBuiltPayload) -> Self {
        Self { envelope_inner: value.into(), execution_requests: Default::default() }
    }
}
