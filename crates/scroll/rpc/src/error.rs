//! RPC errors specific to Scroll.

use alloy_rpc_types_eth::BlockError;
use reth_rpc_eth_api::AsEthApiError;
use reth_rpc_eth_types::EthApiError;

/// Scroll specific errors, that extend [`EthApiError`].
#[derive(Debug, thiserror::Error)]
pub enum ScrollEthApiError {
    /// L1 ethereum error.
    #[error(transparent)]
    Eth(#[from] EthApiError),
}

impl AsEthApiError for ScrollEthApiError {
    fn as_err(&self) -> Option<&EthApiError> {
        match self {
            Self::Eth(err) => Some(err),
        }
    }
}

impl From<ScrollEthApiError> for jsonrpsee_types::error::ErrorObject<'static> {
    fn from(err: ScrollEthApiError) -> Self {
        match err {
            ScrollEthApiError::Eth(err) => err.into(),
        }
    }
}

impl From<BlockError> for ScrollEthApiError {
    fn from(error: BlockError) -> Self {
        Self::Eth(error.into())
    }
}
