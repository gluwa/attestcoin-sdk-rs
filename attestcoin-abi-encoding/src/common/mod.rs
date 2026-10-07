use alloy::dyn_abi::DynSolValue;
use alloy::primitives::{FixedBytes, U256};
use alloy::signers::Signature;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EncodingVersion {
    V1 = 1,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct AbiEncodeResult {
    abi: Vec<u8>,
    version: EncodingVersion,
}

impl AbiEncodeResult {
    pub fn new(abi: Vec<u8>, version: EncodingVersion) -> Self {
        Self { abi, version }
    }

    pub fn abi(&self) -> &[u8] {
        &self.abi
    }

    pub fn version(&self) -> EncodingVersion {
        self.version
    }
}

pub fn compute_v(signature: &Signature, chain_id: Option<u64>) -> U256 {
    let parity = signature.v() as u64; // Get y_parity (boolean as 0 or 1)

    match chain_id {
        Some(id) => U256::from(35 + 2 * id + parity), // Corrected EIP-155 format
        None => U256::from(27 + parity),              // Legacy format
    }
}

pub fn compute_y_parity(signature: &Signature) -> u8 {
    if signature.v() { 1 } else { 0 }
}

pub fn encode_blob_hashes(blob_hashes: &[FixedBytes<32>]) -> DynSolValue {
    let mut result = Vec::new();
    for hash in blob_hashes {
        result.push(DynSolValue::FixedBytes(*hash, 32));
    }
    DynSolValue::Array(result)
}
