//! Leaf encoding of OP-Stack `0x7e` deposit transactions. See `tests/fixtures/README.md`.

use alloy::{
    dyn_abi::{DynSolType, DynSolValue},
    network::AnyTransactionReceipt,
    primitives::{Address, B256, Bytes, TxKind, U256},
};
use attestcoin_abi_encoding::{
    abi::{
        TxRxInput, abi_encode,
        op_stack::{DEPOSIT_TX_TYPE, DepositReceiptFields, DepositTransaction},
    },
    common::EncodingVersion,
};
use serde_json::{Value, json};

const BASE_SEPOLIA: &str = include_str!("fixtures/deposit_base_sepolia_46388021.json");
const BASE_SEPOLIA_LEAF: &str = include_str!("fixtures/deposit_base_sepolia_46388021_leaf.hex");
const OP_MAINNET: &str = include_str!("fixtures/deposit_op_mainnet_110000000.json");
const OP_MAINNET_LEAF: &str = include_str!("fixtures/deposit_op_mainnet_110000000_leaf.hex");

struct Fixture {
    tx: DepositTransaction,
    rx: AnyTransactionReceipt,
    rpc_nonce: u64,
}

fn hex_field<T: serde::de::DeserializeOwned>(tx: &Value, key: &str) -> T {
    serde_json::from_value(tx[key].clone()).unwrap_or_else(|e| panic!("field {key}: {e}"))
}

/// What a caller does with an RPC response: read the deposit fields, canonicalize the nonce.
fn load(json: &str) -> Fixture {
    let fixture: Value = serde_json::from_str(json).unwrap();
    let tx = &fixture["tx"];
    let rx: AnyTransactionReceipt = serde_json::from_value(fixture["receipt"].clone()).unwrap();

    let receipt_fields = DepositReceiptFields {
        deposit_nonce: rx
            .other
            .get_deserialized::<alloy::primitives::U64>("depositNonce")
            .map(|v| v.unwrap().to()),
        deposit_receipt_version: rx
            .other
            .get_deserialized::<alloy::primitives::U64>("depositReceiptVersion")
            .map(|v| v.unwrap().to()),
    };
    let deposit = DepositTransaction {
        hash: hex_field(tx, "hash"),
        source_hash: hex_field(tx, "sourceHash"),
        from: hex_field(tx, "from"),
        to: TxKind::Call(hex_field(tx, "to")),
        mint: hex_field(tx, "mint"),
        value: hex_field(tx, "value"),
        gas_limit: hex_field::<alloy::primitives::U64>(tx, "gas").to(),
        is_system_tx: false,
        input: hex_field(tx, "input"),
        nonce: receipt_fields.canonical_nonce().unwrap(),
    };
    Fixture {
        tx: deposit,
        rx,
        rpc_nonce: hex_field::<alloy::primitives::U64>(tx, "nonce").to(),
    }
}

fn encode(tx: DepositTransaction, rx: AnyTransactionReceipt) -> Vec<u8> {
    let result = abi_encode(
        TxRxInput::OpDeposit { tx, rx: rx.inner },
        EncodingVersion::V1,
    )
    .expect("deposit leaf encodes");
    assert_eq!(result.version(), EncodingVersion::V1);
    result.abi().to_vec()
}

/// Splits a leaf into `(txType, decoded chunks)`: common, deposit, receipt.
fn decode(leaf: &[u8]) -> (U256, Vec<Vec<DynSolValue>>) {
    let envelope = DynSolType::Tuple(vec![
        DynSolType::Uint(8),
        DynSolType::Array(Box::new(DynSolType::Bytes)),
    ]);
    let DynSolValue::Tuple(parts) = envelope.abi_decode_sequence(leaf).expect("envelope") else {
        panic!("expected tuple")
    };
    let DynSolValue::Uint(tx_type, 8) = parts[0] else {
        panic!("expected uint8")
    };
    let DynSolValue::Array(raw_chunks) = &parts[1] else {
        panic!("expected chunk array")
    };

    let types = [
        DynSolType::Tuple(vec![
            DynSolType::Uint(64),
            DynSolType::Uint(64),
            DynSolType::Address,
            DynSolType::Bool,
            DynSolType::Address,
            DynSolType::Uint(256),
            DynSolType::Bytes,
        ]),
        DynSolType::Tuple(vec![
            DynSolType::FixedBytes(32),
            DynSolType::Uint(256),
            DynSolType::Bool,
        ]),
        DynSolType::Tuple(vec![
            DynSolType::Uint(8),
            DynSolType::Uint(64),
            DynSolType::Array(Box::new(DynSolType::Tuple(vec![
                DynSolType::Address,
                DynSolType::Array(Box::new(DynSolType::FixedBytes(32))),
                DynSolType::Bytes,
            ]))),
            DynSolType::Bytes,
        ]),
    ];
    assert_eq!(raw_chunks.len(), types.len(), "common, deposit, receipt");
    let chunks = raw_chunks
        .iter()
        .zip(types)
        .map(|(raw, ty)| {
            let DynSolValue::Bytes(bytes) = raw else {
                panic!("chunk is not bytes")
            };
            let DynSolValue::Tuple(fields) = ty.abi_decode_sequence(bytes).expect("chunk") else {
                panic!("expected tuple")
            };
            fields
        })
        .collect();
    (tx_type, chunks)
}

fn expected_leaf(hex_file: &str) -> Vec<u8> {
    alloy::hex::decode(hex_file.trim()).unwrap()
}

#[test]
fn deposit_hash_is_reproduced_by_our_2718_encoding() {
    for json in [BASE_SEPOLIA, OP_MAINNET] {
        let fixture = load(json);
        assert_eq!(fixture.tx.computed_hash(), fixture.tx.hash);
    }
}

#[test]
fn canyon_deposit_leaf_matches_reference_bytes() {
    let fixture = load(BASE_SEPOLIA);
    assert_eq!(fixture.tx.nonce, 46_388_024);
    assert_eq!(
        encode(fixture.tx, fixture.rx),
        expected_leaf(BASE_SEPOLIA_LEAF)
    );
}

#[test]
fn pre_canyon_deposit_leaf_matches_reference_bytes_and_uses_zero_nonce() {
    let fixture = load(OP_MAINNET);
    // The RPC reports a nonce, but no header root commits to it: the leaf must not use it.
    assert_ne!(fixture.rpc_nonce, 0);
    assert_eq!(fixture.tx.nonce, 0);

    let leaf = encode(fixture.tx, fixture.rx);
    assert_eq!(leaf, expected_leaf(OP_MAINNET_LEAF));
    let (_, chunks) = decode(&leaf);
    assert_eq!(chunks[0][0], DynSolValue::Uint(U256::ZERO, 64));
}

#[test]
fn leaf_decodes_into_documented_layout() {
    let fixture = load(BASE_SEPOLIA);
    let tx = fixture.tx.clone();
    let gas_used = fixture.rx.gas_used;
    let (tx_type, chunks) = decode(&encode(fixture.tx, fixture.rx));

    assert_eq!(tx_type, U256::from(DEPOSIT_TX_TYPE));
    assert_eq!(tx_type, U256::from(126));

    let [common, deposit, receipt] = &chunks[..] else {
        panic!("three chunks")
    };
    assert_eq!(common[0], DynSolValue::Uint(U256::from(tx.nonce), 64));
    assert_eq!(common[1], DynSolValue::Uint(U256::from(tx.gas_limit), 64));
    assert_eq!(common[2], DynSolValue::Address(tx.from));
    assert_eq!(common[3], DynSolValue::Bool(false));
    assert_eq!(
        common[4],
        DynSolValue::Address(match tx.to {
            TxKind::Call(to) => to,
            TxKind::Create => panic!("fixture is a call"),
        })
    );
    assert_eq!(common[5], DynSolValue::Uint(tx.value, 256));
    assert_eq!(common[6], DynSolValue::Bytes(tx.input.to_vec()));

    assert_eq!(deposit[0], DynSolValue::FixedBytes(tx.source_hash, 32));
    assert_eq!(deposit[1], DynSolValue::Uint(tx.mint, 256));
    assert_eq!(deposit[2], DynSolValue::Bool(false));

    assert_eq!(receipt[0], DynSolValue::Uint(U256::from(1), 8));
    assert_eq!(receipt[1], DynSolValue::Uint(U256::from(gas_used), 64));
    assert_eq!(receipt[2], DynSolValue::Array(vec![]));
}

/// A user deposit (not the L1-attributes one): contract creation, minted ETH, a log.
#[test]
fn contract_creation_mint_and_logs_are_encoded() {
    let Fixture { mut tx, rx, .. } = load(BASE_SEPOLIA);
    tx.to = TxKind::Create;
    tx.mint = U256::from(1_000_000u64);
    tx.value = U256::from(7u64);
    tx.is_system_tx = true;
    tx.input = Bytes::from_static(&[0x60, 0x80]);

    let mut receipt = serde_json::to_value(&rx).unwrap();
    let log_address: Address = "0x1111111111111111111111111111111111111111"
        .parse()
        .unwrap();
    let topic: B256 = B256::repeat_byte(0xab);
    receipt["logs"] = json!([{
        "address": log_address,
        "topics": [topic],
        "data": "0xdeadbeef",
        "blockNumber": "0x2c3d335",
        "transactionIndex": "0x0",
        "logIndex": "0x0",
        "removed": false,
    }]);
    let rx: AnyTransactionReceipt = serde_json::from_value(receipt).unwrap();

    let (_, chunks) = decode(&encode(tx.clone(), rx));
    let [common, deposit, receipt] = &chunks[..] else {
        panic!("three chunks")
    };
    assert_eq!(common[3], DynSolValue::Bool(true), "toIsNull");
    assert_eq!(common[4], DynSolValue::Address(Address::ZERO));
    assert_eq!(common[5], DynSolValue::Uint(U256::from(7u64), 256));
    assert_eq!(common[6], DynSolValue::Bytes(vec![0x60, 0x80]));
    assert_eq!(deposit[1], DynSolValue::Uint(U256::from(1_000_000u64), 256));
    assert_eq!(deposit[2], DynSolValue::Bool(true));
    assert_eq!(
        receipt[2],
        DynSolValue::Array(vec![DynSolValue::Tuple(vec![
            DynSolValue::Address(log_address),
            DynSolValue::Array(vec![DynSolValue::FixedBytes(topic, 32)]),
            DynSolValue::Bytes(vec![0xde, 0xad, 0xbe, 0xef]),
        ])])
    );
}

#[test]
fn canonical_nonce_rules() {
    let nonce = |deposit_nonce, deposit_receipt_version| {
        DepositReceiptFields {
            deposit_nonce,
            deposit_receipt_version,
        }
        .canonical_nonce()
    };
    // Pre-Canyon: unavailable, regardless of provider metadata.
    assert_eq!(nonce(None, None), Ok(0));
    assert_eq!(nonce(Some(9), None), Ok(0));
    // Canyon and later: the receipt-authenticated nonce.
    assert_eq!(nonce(Some(9), Some(1)), Ok(9));
    // A versioned receipt without a nonce is malformed.
    assert!(nonce(None, Some(1)).is_err());
}
