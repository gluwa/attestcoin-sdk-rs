//! OP-Stack specifics: the `0x7e` **deposit transaction** and its Merkle leaf.
//!
//! OP-Stack rollups (Base, OP Mainnet, …) are Ethereum-equivalent except for one extra
//! transaction type. Every L2 block opens with a deposit transaction that writes the L1 block
//! attributes into the `L1Block` predeploy, and user deposits through the `OptimismPortal` are
//! delivered the same way. Deposit transactions are *unsigned* — they are derived from L1 — and
//! carry three fields no L1 type has: `sourceHash`, `mint` and `isSystemTx`.
//!
//! alloy's `Ethereum` network types reject the `0x7e` type, so this module has its own
//! [`DepositTransaction`] type. Parsing it out of an RPC response is the caller's job.
//!
//! # Specification references
//!
//! - Deposit transaction type and RLP layout: OP-Stack specs, `deposits.md`
//!   (`0x7E || rlp([sourceHash, from, to, mint, value, gas, isSystemTx, data])`).
//! - Receipt encoding: `op-geth` `core/types/receipt.go`, `Receipts.EncodeIndex`.
//!
//! # Leaf layout (`txType = 126`)
//!
//! Three chunks, mirroring the V1 layout for types `0x0`–`0x2` so a decoder that already
//! understands the common and receipt chunks needs only the middle one:
//!
//! | chunk | fields |
//! |-------|--------|
//! | `chunks[0]` common | `(uint64 nonce, uint64 gasLimit, address from, bool toIsNull, address to, uint256 value, bytes data)` |
//! | `chunks[1]` deposit | `(bytes32 sourceHash, uint256 mint, bool isSystemTx)` |
//! | `chunks[2]` receipt | `(uint8 status, uint64 gasUsed, (address,bytes32[],bytes)[] logs, bytes logsBloom)` |
//!
//! `nonce` is the receipt's `depositNonce` when `depositReceiptVersion` is present (Canyon
//! and later), so it is authenticated by the header's `receiptsRoot`. Before Canyon neither the
//! transaction root nor the receipt root commits to a nonce: the leaf uses **zero**, meaning
//! unavailable, even if RPC metadata reports a nonzero execution nonce. Do not use that
//! historical zero to derive a contract-creation address. There are no signature fields:
//! deposits are unsigned.

use crate::common::EncodingVersion;
use alloy::{
    consensus::Eip658Value,
    dyn_abi::DynSolValue,
    network::AnyReceiptEnvelope,
    primitives::{Address, B256, Bytes, TxKind, U256, keccak256},
    rlp::{BufMut, Encodable, Header},
    rpc::types::{Log as RpcLog, TransactionReceipt},
};

/// EIP-2718 type byte of an OP-Stack deposit transaction.
pub const DEPOSIT_TX_TYPE: u8 = 0x7e;

/// A deposit receipt reports a `depositReceiptVersion` but no `depositNonce`; op-geth never
/// produces this shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MissingDepositNonce;

impl std::fmt::Display for MissingDepositNonce {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("deposit receipt has depositReceiptVersion but no depositNonce")
    }
}

impl std::error::Error for MissingDepositNonce {}

/// Deposit-specific receipt fields (`depositNonce` / `depositReceiptVersion`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct DepositReceiptFields {
    /// Sequencer-assigned deposit nonce (Regolith and later).
    pub deposit_nonce: Option<u64>,
    /// Receipt encoding version (Canyon and later, currently always `1`).
    pub deposit_receipt_version: Option<u64>,
}

impl DepositReceiptFields {
    /// The nonce that goes into the leaf.
    ///
    /// Only Canyon and later receipt roots commit to the nonce. Earlier metadata must not
    /// influence an attestation leaf, even when a provider happens to include it, so those
    /// receipts canonicalize to zero.
    pub fn canonical_nonce(self) -> Result<u64, MissingDepositNonce> {
        match self.deposit_receipt_version {
            Some(_) => self.deposit_nonce.ok_or(MissingDepositNonce),
            None => Ok(0),
        }
    }
}

/// An OP-Stack deposit transaction as it appears in an L2 block.
///
/// Field names follow the OP-Stack spec. `nonce` is not part of the transaction's consensus
/// encoding; set it from [`DepositReceiptFields::canonical_nonce`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DepositTransaction {
    /// Transaction hash.
    pub hash: B256,
    /// Uniquely identifies the L1 origin of this deposit (user-deposited and L1-info deposits
    /// derive it differently).
    pub source_hash: B256,
    /// L2 sender. For the L1-attributes deposit this is the fixed depositor address.
    pub from: Address,
    /// Call target, or contract creation.
    pub to: TxKind,
    /// ETH minted on L2 as part of this deposit.
    pub mint: U256,
    /// ETH value transferred from `from` to `to`.
    pub value: U256,
    /// Gas limit.
    pub gas_limit: u64,
    /// Whether the deposit is exempt from L2 gas accounting (always `false` since Regolith, but
    /// still part of the encoding).
    pub is_system_tx: bool,
    /// Calldata.
    pub input: Bytes,
    /// Receipt-backed nonce since Canyon; zero (unavailable) before Canyon.
    pub nonce: u64,
}

impl DepositTransaction {
    /// RLP payload length of `[sourceHash, from, to, mint, value, gas, isSystemTx, data]`.
    fn rlp_payload_length(&self) -> usize {
        self.source_hash.length()
            + self.from.length()
            + self.to.length()
            + self.mint.length()
            + self.value.length()
            + self.gas_limit.length()
            + self.is_system_tx.length()
            + self.input.length()
    }

    /// EIP-2718 encoding: `0x7e || rlp([...])`.
    pub fn encode_2718(&self, out: &mut dyn BufMut) {
        out.put_u8(DEPOSIT_TX_TYPE);
        Header {
            list: true,
            payload_length: self.rlp_payload_length(),
        }
        .encode(out);
        self.source_hash.encode(out);
        self.from.encode(out);
        self.to.encode(out);
        self.mint.encode(out);
        self.value.encode(out);
        self.gas_limit.encode(out);
        self.is_system_tx.encode(out);
        self.input.encode(out);
    }

    /// Owned EIP-2718 encoding.
    pub fn encoded_2718(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(1 + self.rlp_payload_length() + 3);
        self.encode_2718(&mut out);
        out
    }

    /// Hash of the EIP-2718 encoding. Equals [`Self::hash`] for a well-formed deposit, which
    /// lets callers verify an RPC-reported hash against the fields they parsed.
    pub fn computed_hash(&self) -> B256 {
        keccak256(self.encoded_2718())
    }
}

/// A deposit receipt as returned by an `AnyNetwork` RPC.
pub type DepositReceipt = TransactionReceipt<AnyReceiptEnvelope<RpcLog>>;

/// ABI-encode the Merkle leaf for a (deposit tx, receipt) pair. See the [module docs](self) for
/// the layout. Returns `None` only if `alloy`'s dynamic ABI encoder refuses a value, which cannot
/// happen for the fixed shapes built here; the `Option` keeps parity with the Ethereum encoder.
pub(super) fn abi_encode(
    tx: DepositTransaction,
    rx: DepositReceipt,
    version: EncodingVersion,
) -> Option<Vec<u8>> {
    // Matching exhaustively makes a future encoding version a compile error here instead of a
    // silently V1-shaped deposit leaf.
    match version {
        EncodingVersion::V1 => abi_encode_v1(&tx, &rx),
    }
}

fn abi_encode_v1(tx: &DepositTransaction, rx: &DepositReceipt) -> Option<Vec<u8>> {
    let (is_to_null, to) = match tx.to {
        TxKind::Call(addr) => (false, addr),
        TxKind::Create => (true, Address::ZERO),
    };

    let common = DynSolValue::Tuple(vec![
        DynSolValue::Uint(U256::from(tx.nonce), 64),
        DynSolValue::Uint(U256::from(tx.gas_limit), 64),
        DynSolValue::Address(tx.from),
        DynSolValue::Bool(is_to_null),
        DynSolValue::Address(to),
        DynSolValue::Uint(tx.value, 256),
        DynSolValue::Bytes(tx.input.to_vec()),
    ]);

    let deposit = DynSolValue::Tuple(vec![
        DynSolValue::FixedBytes(tx.source_hash, 32),
        DynSolValue::Uint(tx.mint, 256),
        DynSolValue::Bool(tx.is_system_tx),
    ]);

    let consensus = &rx.inner.inner;
    let status: u8 = match consensus.receipt.status {
        Eip658Value::Eip658(ok) => u8::from(ok),
        // Deposits post-date Byzantium by years; a post-state receipt cannot occur on an
        // OP-Stack chain. Treat as success=1 only if the coerced status says so.
        Eip658Value::PostState(_) => u8::from(consensus.receipt.status.coerce_status()),
    };
    let logs = DynSolValue::Array(
        consensus
            .receipt
            .logs
            .iter()
            .map(|log| {
                DynSolValue::Tuple(vec![
                    DynSolValue::Address(log.address()),
                    DynSolValue::Array(
                        log.topics()
                            .iter()
                            .map(|topic| DynSolValue::FixedBytes(*topic, 32))
                            .collect(),
                    ),
                    DynSolValue::Bytes(log.data().data.to_vec()),
                ])
            })
            .collect(),
    );
    let receipt = DynSolValue::Tuple(vec![
        DynSolValue::Uint(U256::from(status), 8),
        DynSolValue::Uint(U256::from(rx.gas_used), 64),
        logs,
        DynSolValue::Bytes(consensus.logs_bloom.0.to_vec()),
    ]);

    let chunks = [common, deposit, receipt]
        .into_iter()
        .map(|chunk| chunk.abi_encode_sequence().map(DynSolValue::Bytes))
        .collect::<Option<Vec<_>>>()?;

    DynSolValue::Tuple(vec![
        DynSolValue::Uint(U256::from(DEPOSIT_TX_TYPE), 8),
        DynSolValue::Array(chunks),
    ])
    .abi_encode_sequence()
}
