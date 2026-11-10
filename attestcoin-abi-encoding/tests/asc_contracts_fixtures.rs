//! Cross-check against the Solidity decoder's fixtures (gluwa/asc-contracts#47).
//!
//! Each case carries the expected fields, taken from the source RPC transaction and receipt, and
//! `txBytes`, the leaf produced by the reference Rust implementation that `EvmV1Decoder2` decodes
//! in asc-contracts' tests. This crate must produce those exact bytes from those fields.

use alloy::{
    network::AnyTransactionReceipt,
    primitives::{Address, B256, Bytes, TxKind, U64, U256},
};
use attestcoin_abi_encoding::{
    abi::{TxRxInput, abi_encode, op_stack::DepositTransaction},
    common::EncodingVersion,
};
use serde_json::{Value, json};

const FIXTURES: &str = include_str!("fixtures/asc_contracts_op_stack_deposits.json");

fn get<T: serde::de::DeserializeOwned>(value: &Value, key: &str) -> T {
    serde_json::from_value(value[key].clone()).unwrap_or_else(|e| panic!("field {key}: {e}"))
}

#[test]
fn leaves_match_the_solidity_decoder_fixtures() {
    let fixtures: Value = serde_json::from_str(FIXTURES).unwrap();
    let cases = fixtures["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 4, "fixture file changed; review the new cases");

    for case in cases {
        let name: String = get(case, "name");
        let (common, deposit, receipt) = (&case["common"], &case["deposit"], &case["receipt"]);

        let tx = DepositTransaction {
            hash: get::<B256>(case, "transactionHash"),
            source_hash: get(deposit, "sourceHash"),
            from: get::<Address>(common, "from"),
            to: TxKind::Call(get(common, "to")),
            mint: get::<U256>(deposit, "mint"),
            value: get(common, "value"),
            gas_limit: get::<U64>(common, "gasLimit").to(),
            is_system_tx: get(deposit, "isSystemTx"),
            input: get::<Bytes>(common, "data"),
            // The canonical nonce: receipt-authenticated since Canyon, zero before.
            nonce: get::<U64>(common, "nonce").to(),
        };
        assert_eq!(
            tx.computed_hash(),
            tx.hash,
            "{name}: 2718 encoding hashes to the tx hash"
        );

        // Only the fields the leaf reads come from the fixture; the rest just let the RPC type parse.
        let rx: AnyTransactionReceipt = serde_json::from_value(json!({
            "type": "0x7e",
            "transactionHash": tx.hash,
            "transactionIndex": "0x0",
            "blockHash": case["blockHash"],
            "blockNumber": format!("{:#x}", get::<u64>(case, "blockNumber")),
            "from": tx.from,
            "to": common["to"],
            "status": receipt["status"],
            "gasUsed": receipt["gasUsed"],
            "cumulativeGasUsed": receipt["gasUsed"],
            "effectiveGasPrice": "0x0",
            "logsBloom": receipt["logsBloom"],
            "logs": receipt["logs"],
        }))
        .unwrap_or_else(|e| panic!("{name}: receipt: {e}"));

        let leaf = abi_encode(
            TxRxInput::OpDeposit { tx, rx: rx.inner },
            EncodingVersion::V1,
        )
        .unwrap_or_else(|| panic!("{name}: leaf encodes"));
        let expected: Bytes = get(case, "txBytes");
        assert_eq!(
            leaf.abi(),
            expected.as_ref(),
            "{name}: leaf differs from the Rust-produced fixture"
        );
    }
}
