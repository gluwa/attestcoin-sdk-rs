use super::common::{AbiEncodeResult, EncodingVersion};

use alloy::rpc::types::{Transaction, TransactionReceipt};

mod v1;

/// Encodes a given ethereum transaction and its receipt into ABI format
/// according to the specified encoding version.
///
/// If the encoding process fails, the function returns `None`.
///
/// This function assumes that both the transaction and receipt comply with the ethereum specifications
/// as defined in the `alloy` crate.
pub fn abi_encode(
    tx: Transaction,
    rx: TransactionReceipt,
    version: EncodingVersion,
) -> Option<AbiEncodeResult> {
    match version {
        EncodingVersion::V1 => v1::abi_encode(tx, rx),
    }
}
