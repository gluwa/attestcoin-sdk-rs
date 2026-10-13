use super::common::{AbiEncodeResult, EncodingVersion};

use alloy::rpc::types::{Transaction, TransactionReceipt};

pub mod op_stack;
mod v1;

pub use op_stack::{DepositReceipt, DepositTransaction};

/// A transaction and its receipt, ready to be encoded into a Merkle leaf.
///
/// The variant selects the leaf layout.
// Built and consumed per call, never stored in bulk, so boxing would only burden callers.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub enum TxRxInput {
    /// A standard Ethereum transaction (types `0x0`–`0x4`) and its receipt. Also the right
    /// variant for the non-deposit transactions of an OP-Stack block.
    Ethereum {
        tx: Transaction,
        rx: TransactionReceipt,
    },
    /// An OP-Stack deposit transaction (type `0x7e`) and its receipt.
    OpDeposit {
        tx: DepositTransaction,
        rx: DepositReceipt,
    },
}

/// Encodes a given transaction and its receipt into ABI format according to the specified
/// encoding version.
///
/// If the encoding process fails, the function returns `None`.
///
/// This function assumes that both the transaction and receipt comply with the ethereum
/// specifications as defined in the `alloy` crate, or, for [`TxRxInput::OpDeposit`], with the
/// OP-Stack specification (see [`op_stack`]).
pub fn abi_encode(input: TxRxInput, version: EncodingVersion) -> Option<AbiEncodeResult> {
    match input {
        TxRxInput::Ethereum { tx, rx } => match version {
            EncodingVersion::V1 => v1::abi_encode(tx, rx),
        },
        TxRxInput::OpDeposit { tx, rx } => {
            op_stack::abi_encode(tx, rx, version).map(|abi| AbiEncodeResult::new(abi, version))
        }
    }
}
