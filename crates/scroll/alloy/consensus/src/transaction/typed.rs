use crate::{ScrollTxEnvelope, ScrollTxType, TxL1Message};
use alloy_consensus::{Transaction, TxEip1559, TxEip2930, TxLegacy, Typed2718};
use alloy_eips::eip2930::AccessList;
use alloy_primitives::{Address, Bytes, TxKind};
use reth_codecs::{Compact, __private::bytes};
use reth_codecs_derive::generate_tests;

/// The `TypedTransaction` enum represents all Ethereum transaction request types, modified for
/// Scroll
///
/// Its variants correspond to specific allowed transactions:
/// 1. `Legacy` (pre-EIP2718) [`TxLegacy`]
/// 2. `EIP2930` (state access lists) [`TxEip2930`]
/// 3. `EIP1559` [`TxEip1559`]
/// 4. `L1Message` [`TxL1Message`]
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "arbitrary", derive(arbitrary::Arbitrary))]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(
        from = "serde_from::MaybeTaggedTypedTransaction",
        into = "serde_from::TaggedTypedTransaction"
    )
)]
pub enum ScrollTypedTransaction {
    /// Legacy transaction
    Legacy(TxLegacy),
    /// EIP-2930 transaction
    Eip2930(TxEip2930),
    /// EIP-1559 transaction
    Eip1559(TxEip1559),
    /// Scroll L1 message transaction
    L1Message(TxL1Message),
}

impl From<TxLegacy> for ScrollTypedTransaction {
    fn from(tx: TxLegacy) -> Self {
        Self::Legacy(tx)
    }
}

impl From<TxEip2930> for ScrollTypedTransaction {
    fn from(tx: TxEip2930) -> Self {
        Self::Eip2930(tx)
    }
}

impl From<TxEip1559> for ScrollTypedTransaction {
    fn from(tx: TxEip1559) -> Self {
        Self::Eip1559(tx)
    }
}

impl From<TxL1Message> for ScrollTypedTransaction {
    fn from(tx: TxL1Message) -> Self {
        Self::L1Message(tx)
    }
}

impl From<ScrollTxEnvelope> for ScrollTypedTransaction {
    fn from(envelope: ScrollTxEnvelope) -> Self {
        match envelope {
            ScrollTxEnvelope::Legacy(tx) => Self::Legacy(tx.strip_signature()),
            ScrollTxEnvelope::Eip2930(tx) => Self::Eip2930(tx.strip_signature()),
            ScrollTxEnvelope::Eip1559(tx) => Self::Eip1559(tx.strip_signature()),
            ScrollTxEnvelope::L1Message(tx) => Self::L1Message(tx.into_inner()),
        }
    }
}

impl ScrollTypedTransaction {
    /// Return the [`ScrollTxType`] of the inner txn.
    pub const fn tx_type(&self) -> ScrollTxType {
        match self {
            Self::Legacy(_) => ScrollTxType::Legacy,
            Self::Eip2930(_) => ScrollTxType::Eip2930,
            Self::Eip1559(_) => ScrollTxType::Eip1559,
            Self::L1Message(_) => ScrollTxType::L1Message,
        }
    }

    /// Return the inner legacy transaction if it exists.
    pub const fn legacy(&self) -> Option<&TxLegacy> {
        match self {
            Self::Legacy(tx) => Some(tx),
            _ => None,
        }
    }

    /// Return the inner EIP-2930 transaction if it exists.
    pub const fn eip2930(&self) -> Option<&TxEip2930> {
        match self {
            Self::Eip2930(tx) => Some(tx),
            _ => None,
        }
    }

    /// Return the inner EIP-1559 transaction if it exists.
    pub const fn eip1559(&self) -> Option<&TxEip1559> {
        match self {
            Self::Eip1559(tx) => Some(tx),
            _ => None,
        }
    }

    /// Return the inner l1 message if it exists.
    pub const fn l1_message(&self) -> Option<&TxL1Message> {
        match self {
            Self::L1Message(tx) => Some(tx),
            _ => None,
        }
    }
}

impl Typed2718 for ScrollTypedTransaction {
    fn ty(&self) -> u8 {
        match self {
            Self::Legacy(_) => ScrollTxType::Legacy as u8,
            Self::Eip2930(_) => ScrollTxType::Eip2930 as u8,
            Self::Eip1559(_) => ScrollTxType::Eip1559 as u8,
            Self::L1Message(_) => ScrollTxType::L1Message as u8,
        }
    }
}

impl Transaction for ScrollTypedTransaction {
    fn chain_id(&self) -> Option<alloy_primitives::ChainId> {
        match self {
            Self::Legacy(tx) => tx.chain_id(),
            Self::Eip2930(tx) => tx.chain_id(),
            Self::Eip1559(tx) => tx.chain_id(),
            Self::L1Message(tx) => tx.chain_id(),
        }
    }

    fn nonce(&self) -> u64 {
        match self {
            Self::Legacy(tx) => tx.nonce(),
            Self::Eip2930(tx) => tx.nonce(),
            Self::Eip1559(tx) => tx.nonce(),
            Self::L1Message(tx) => tx.nonce(),
        }
    }

    fn gas_limit(&self) -> u64 {
        match self {
            Self::Legacy(tx) => tx.gas_limit(),
            Self::Eip2930(tx) => tx.gas_limit(),
            Self::Eip1559(tx) => tx.gas_limit(),
            Self::L1Message(tx) => tx.gas_limit(),
        }
    }

    fn gas_price(&self) -> Option<u128> {
        match self {
            Self::Legacy(tx) => tx.gas_price(),
            Self::Eip2930(tx) => tx.gas_price(),
            Self::Eip1559(tx) => tx.gas_price(),
            Self::L1Message(tx) => tx.gas_price(),
        }
    }

    fn max_fee_per_gas(&self) -> u128 {
        match self {
            Self::Legacy(tx) => tx.max_fee_per_gas(),
            Self::Eip2930(tx) => tx.max_fee_per_gas(),
            Self::Eip1559(tx) => tx.max_fee_per_gas(),
            Self::L1Message(tx) => tx.max_fee_per_gas(),
        }
    }

    fn max_priority_fee_per_gas(&self) -> Option<u128> {
        match self {
            Self::Legacy(tx) => tx.max_priority_fee_per_gas(),
            Self::Eip2930(tx) => tx.max_priority_fee_per_gas(),
            Self::Eip1559(tx) => tx.max_priority_fee_per_gas(),
            Self::L1Message(tx) => tx.max_priority_fee_per_gas(),
        }
    }

    fn max_fee_per_blob_gas(&self) -> Option<u128> {
        match self {
            Self::Legacy(tx) => tx.max_fee_per_blob_gas(),
            Self::Eip2930(tx) => tx.max_fee_per_blob_gas(),
            Self::Eip1559(tx) => tx.max_fee_per_blob_gas(),
            Self::L1Message(tx) => tx.max_fee_per_blob_gas(),
        }
    }

    fn priority_fee_or_price(&self) -> u128 {
        match self {
            Self::Legacy(tx) => tx.priority_fee_or_price(),
            Self::Eip2930(tx) => tx.priority_fee_or_price(),
            Self::Eip1559(tx) => tx.priority_fee_or_price(),
            Self::L1Message(tx) => tx.priority_fee_or_price(),
        }
    }

    fn to(&self) -> Option<Address> {
        match self {
            Self::Legacy(tx) => tx.to(),
            Self::Eip2930(tx) => tx.to(),
            Self::Eip1559(tx) => tx.to(),
            Self::L1Message(tx) => tx.to(),
        }
    }

    fn kind(&self) -> TxKind {
        match self {
            Self::Legacy(tx) => tx.kind(),
            Self::Eip2930(tx) => tx.kind(),
            Self::Eip1559(tx) => tx.kind(),
            Self::L1Message(tx) => tx.kind(),
        }
    }

    fn value(&self) -> alloy_primitives::U256 {
        match self {
            Self::Legacy(tx) => tx.value(),
            Self::Eip2930(tx) => tx.value(),
            Self::Eip1559(tx) => tx.value(),
            Self::L1Message(tx) => tx.value(),
        }
    }

    fn input(&self) -> &Bytes {
        match self {
            Self::Legacy(tx) => tx.input(),
            Self::Eip2930(tx) => tx.input(),
            Self::Eip1559(tx) => tx.input(),
            Self::L1Message(tx) => tx.input(),
        }
    }

    fn access_list(&self) -> Option<&AccessList> {
        match self {
            Self::Legacy(tx) => tx.access_list(),
            Self::Eip2930(tx) => tx.access_list(),
            Self::Eip1559(tx) => tx.access_list(),
            Self::L1Message(tx) => tx.access_list(),
        }
    }

    fn blob_versioned_hashes(&self) -> Option<&[alloy_primitives::B256]> {
        match self {
            Self::Legacy(tx) => tx.blob_versioned_hashes(),
            Self::Eip2930(tx) => tx.blob_versioned_hashes(),
            Self::Eip1559(tx) => tx.blob_versioned_hashes(),
            Self::L1Message(tx) => tx.blob_versioned_hashes(),
        }
    }

    fn authorization_list(&self) -> Option<&[alloy_eips::eip7702::SignedAuthorization]> {
        match self {
            Self::Legacy(tx) => tx.authorization_list(),
            Self::Eip2930(tx) => tx.authorization_list(),
            Self::Eip1559(tx) => tx.authorization_list(),
            Self::L1Message(tx) => tx.authorization_list(),
        }
    }

    fn is_dynamic_fee(&self) -> bool {
        match self {
            Self::Legacy(tx) => tx.is_dynamic_fee(),
            Self::Eip2930(tx) => tx.is_dynamic_fee(),
            Self::Eip1559(tx) => tx.is_dynamic_fee(),
            Self::L1Message(tx) => tx.is_dynamic_fee(),
        }
    }

    fn effective_gas_price(&self, base_fee: Option<u64>) -> u128 {
        match self {
            Self::Legacy(tx) => tx.effective_gas_price(base_fee),
            Self::Eip2930(tx) => tx.effective_gas_price(base_fee),
            Self::Eip1559(tx) => tx.effective_gas_price(base_fee),
            Self::L1Message(tx) => tx.effective_gas_price(base_fee),
        }
    }

    fn is_create(&self) -> bool {
        match self {
            Self::Legacy(tx) => tx.is_create(),
            Self::Eip2930(tx) => tx.is_create(),
            Self::Eip1559(tx) => tx.is_create(),
            Self::L1Message(tx) => tx.is_create(),
        }
    }
}

impl Compact for ScrollTypedTransaction {
    fn to_compact<B>(&self, out: &mut B) -> usize
    where
        B: bytes::BufMut + AsMut<[u8]>,
    {
        let identifier = self.tx_type().to_compact(out);
        match self {
            Self::Legacy(tx) => tx.to_compact(out),
            Self::Eip2930(tx) => tx.to_compact(out),
            Self::Eip1559(tx) => tx.to_compact(out),
            Self::L1Message(tx) => tx.to_compact(out),
        };
        identifier
    }

    fn from_compact(buf: &[u8], identifier: usize) -> (Self, &[u8]) {
        let (tx_type, buf) = ScrollTxType::from_compact(buf, identifier);
        match tx_type {
            ScrollTxType::Legacy => {
                let (tx, buf) = Compact::from_compact(buf, buf.len());
                (Self::Legacy(tx), buf)
            }
            ScrollTxType::Eip2930 => {
                let (tx, buf) = Compact::from_compact(buf, buf.len());
                (Self::Eip2930(tx), buf)
            }
            ScrollTxType::Eip1559 => {
                let (tx, buf) = Compact::from_compact(buf, buf.len());
                (Self::Eip1559(tx), buf)
            }
            ScrollTxType::L1Message => {
                let (tx, buf) = Compact::from_compact(buf, buf.len());
                (Self::L1Message(tx), buf)
            }
        }
    }
}

generate_tests!(
    #[compact]
    ScrollTypedTransaction,
    ScrollTypedTransactionTests
);

#[cfg(feature = "serde")]
mod serde_from {
    //! NB: Why do we need this?
    //!
    //! Because the tag may be missing, we need an abstraction over tagged (with
    //! type) and untagged (always legacy). This is
    //! [`MaybeTaggedTypedTransaction`].
    //!
    //! The tagged variant is [`TaggedTypedTransaction`], which always has a
    //! type tag.
    //!
    //! We serialize via [`TaggedTypedTransaction`] and deserialize via
    //! [`MaybeTaggedTypedTransaction`].
    use super::*;

    #[derive(Debug, serde::Deserialize)]
    #[serde(untagged)]
    pub(crate) enum MaybeTaggedTypedTransaction {
        Tagged(TaggedTypedTransaction),
        Untagged(TxLegacy),
    }

    #[derive(Debug, serde::Serialize, serde::Deserialize)]
    #[serde(tag = "type")]
    pub(crate) enum TaggedTypedTransaction {
        /// `Legacy` transaction
        #[serde(rename = "0x00", alias = "0x0")]
        Legacy(TxLegacy),
        /// `EIP-2930` transaction
        #[serde(rename = "0x01", alias = "0x1")]
        Eip2930(TxEip2930),
        /// `EIP-1559` transaction
        #[serde(rename = "0x02", alias = "0x2")]
        Eip1559(TxEip1559),
        /// `L1Message` transaction
        #[serde(
            rename = "0x7e",
            alias = "0x7E",
            serialize_with = "crate::serde_l1_message_tx_rpc"
        )]
        L1Message(TxL1Message),
    }

    impl From<MaybeTaggedTypedTransaction> for ScrollTypedTransaction {
        fn from(value: MaybeTaggedTypedTransaction) -> Self {
            match value {
                MaybeTaggedTypedTransaction::Tagged(tagged) => tagged.into(),
                MaybeTaggedTypedTransaction::Untagged(tx) => Self::Legacy(tx),
            }
        }
    }

    impl From<TaggedTypedTransaction> for ScrollTypedTransaction {
        fn from(value: TaggedTypedTransaction) -> Self {
            match value {
                TaggedTypedTransaction::Legacy(signed) => Self::Legacy(signed),
                TaggedTypedTransaction::Eip2930(signed) => Self::Eip2930(signed),
                TaggedTypedTransaction::Eip1559(signed) => Self::Eip1559(signed),
                TaggedTypedTransaction::L1Message(tx) => Self::L1Message(tx),
            }
        }
    }

    impl From<ScrollTypedTransaction> for TaggedTypedTransaction {
        fn from(value: ScrollTypedTransaction) -> Self {
            match value {
                ScrollTypedTransaction::Legacy(signed) => Self::Legacy(signed),
                ScrollTypedTransaction::Eip2930(signed) => Self::Eip2930(signed),
                ScrollTypedTransaction::Eip1559(signed) => Self::Eip1559(signed),
                ScrollTypedTransaction::L1Message(tx) => Self::L1Message(tx),
            }
        }
    }
}
