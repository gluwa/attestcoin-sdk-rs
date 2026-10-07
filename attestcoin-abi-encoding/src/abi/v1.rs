use crate::common::{
    AbiEncodeResult, EncodingVersion, compute_v, compute_y_parity, encode_blob_hashes,
};
use alloy::{
    consensus::{
        Signed, Transaction as ConsensusTransaction, TxEip1559, TxEip2930, TxEip4844Variant,
        TxEip7702, TxEnvelope, TxLegacy,
    },
    dyn_abi::DynSolValue,
    eips::eip7702::SignedAuthorization,
    primitives::{Address, B256, U256},
    rpc::types::{AccessListItem, Transaction, TransactionReceipt},
};

fn encode_common_fields(tx: &Transaction) -> DynSolValue {
    let (is_to_null, to) = match tx.to() {
        Some(address) => (false, address),
        None => (true, Address::ZERO),
    };

    DynSolValue::Tuple(vec![
        DynSolValue::Uint(U256::from(tx.nonce()), 64), // Nonce
        DynSolValue::Uint(U256::from(tx.gas_limit()), 64), // Gas limit
        DynSolValue::Address(tx.inner.signer()),       // From address
        DynSolValue::Bool(is_to_null),                 // Is null (true if contract creation)
        DynSolValue::Address(to),                      // To address
        DynSolValue::Uint(tx.value(), 256),            // Value
        DynSolValue::Bytes(tx.input().to_vec()),       // Input data (true = dynamic encoding)
    ])
}

fn encode_receipt_fields(rx: TransactionReceipt) -> DynSolValue {
    let log_blooms = rx.inner.logs_bloom().0.to_vec();
    DynSolValue::Tuple(vec![
        DynSolValue::Uint(U256::from(rx.status()), 8),
        DynSolValue::Uint(U256::from(rx.gas_used), 64),
        DynSolValue::Array(
            rx.inner
                .logs()
                .iter()
                .map(|log| {
                    let topics = DynSolValue::Array(
                        log.topics()
                            .iter()
                            .map(|topic| DynSolValue::FixedBytes(*topic, 32))
                            .collect(),
                    );

                    DynSolValue::Tuple(vec![
                        DynSolValue::Address(log.address()),
                        topics,
                        DynSolValue::Bytes(log.data().data.to_vec()),
                    ])
                })
                .collect(),
        ),
        DynSolValue::Bytes(log_blooms),
    ])
}

fn encode_transaction_type_0(
    tx: Transaction,
    signed_tx: Signed<TxLegacy>,
    rx: TransactionReceipt,
) -> Vec<DynSolValue> {
    // Encoding transaction in 3 chunks
    let chunk_1 = encode_common_fields(&tx);

    let chunk_2 = DynSolValue::Tuple(vec![
        DynSolValue::Uint(U256::from(signed_tx.tx().gas_price), 128), // Gas price
        DynSolValue::Uint(
            U256::from(compute_v(signed_tx.signature(), tx.chain_id())),
            256,
        ), // v (legacy is uint8)
        DynSolValue::FixedBytes(B256::from(signed_tx.signature().r()), 32), // r
        DynSolValue::FixedBytes(B256::from(signed_tx.signature().s()), 32), // s
    ]);

    let chunk_3 = encode_receipt_fields(rx);

    vec![chunk_1, chunk_2, chunk_3]
}

fn encode_transaction_type_1(
    tx: Transaction,
    signed_tx: Signed<TxEip2930>,
    rx: TransactionReceipt,
) -> Vec<DynSolValue> {
    // Encoding transaction in 3 chunks
    let chunk_1 = encode_common_fields(&tx);

    let chunk_2 = DynSolValue::Tuple(vec![
        DynSolValue::Uint(U256::from(signed_tx.tx().chain_id), 64),
        DynSolValue::Uint(U256::from(signed_tx.tx().gas_price), 128), // Gas price
        encode_access_list(&signed_tx.tx().access_list),
        DynSolValue::Uint(U256::from(compute_y_parity(signed_tx.signature())), 8), // y parity
        DynSolValue::FixedBytes(B256::from(signed_tx.signature().r()), 32),        // r
        DynSolValue::FixedBytes(B256::from(signed_tx.signature().s()), 32),        // s
    ]);

    let chunk_3 = encode_receipt_fields(rx);

    vec![chunk_1, chunk_2, chunk_3]
}

fn encode_transaction_type_2(
    tx: Transaction,
    signed_tx: Signed<TxEip1559>,
    rx: TransactionReceipt,
) -> Vec<DynSolValue> {
    // Encoding transaction in 3 chunks
    let chunk_1 = encode_common_fields(&tx);

    let chunk_2 = DynSolValue::Tuple(vec![
        DynSolValue::Uint(U256::from(signed_tx.tx().chain_id), 64),
        DynSolValue::Uint(U256::from(signed_tx.tx().max_priority_fee_per_gas), 128),
        DynSolValue::Uint(U256::from(tx.max_fee_per_gas()), 128),
        encode_access_list(&signed_tx.tx().access_list),
        DynSolValue::Uint(U256::from(compute_y_parity(signed_tx.signature())), 8), // y parity
        DynSolValue::FixedBytes(B256::from(signed_tx.signature().r()), 32),        // r
        DynSolValue::FixedBytes(B256::from(signed_tx.signature().s()), 32),        // s
    ]);

    let chunk_3 = encode_receipt_fields(rx);

    vec![chunk_1, chunk_2, chunk_3]
}

fn encode_transaction_type_3(
    tx: Transaction,
    signed_tx: Signed<TxEip4844Variant>,
    rx: TransactionReceipt,
) -> Vec<DynSolValue> {
    let access_list = match tx.access_list() {
        Some(access_list) => encode_access_list(access_list),
        None => encode_access_list(&[]),
    };

    let (
        chain_id,
        max_priority_fee_per_gas,
        max_fee_per_gas,
        max_fee_per_blob_gas,
        blob_version_hashes,
    ) = match signed_tx.tx() {
        TxEip4844Variant::TxEip4844(tx_ref) => (
            tx_ref.chain_id,
            tx_ref.max_priority_fee_per_gas,
            tx_ref.max_fee_per_gas,
            tx_ref.max_fee_per_blob_gas,
            &tx_ref.blob_versioned_hashes,
        ),
        TxEip4844Variant::TxEip4844WithSidecar(tx_ref) => (
            tx_ref.tx.chain_id,
            tx_ref.tx.max_priority_fee_per_gas,
            tx_ref.tx.max_fee_per_gas,
            tx_ref.tx.max_fee_per_blob_gas,
            &tx_ref.tx.blob_versioned_hashes,
        ),
    };
    let blob_hashes = encode_blob_hashes(blob_version_hashes);

    // Encoding transaction in 4 chunks
    let chunk_1 = encode_common_fields(&tx);

    let chunk_2 = DynSolValue::Tuple(vec![
        DynSolValue::Uint(U256::from(chain_id), 64),
        DynSolValue::Uint(U256::from(max_priority_fee_per_gas), 128),
        DynSolValue::Uint(U256::from(max_fee_per_gas), 128),
        access_list,
    ]);

    let chunk_3 = DynSolValue::Tuple(vec![
        DynSolValue::Uint(U256::from(max_fee_per_blob_gas), 128),
        blob_hashes,
        DynSolValue::Uint(U256::from(compute_y_parity(signed_tx.signature())), 8),
        DynSolValue::FixedBytes(B256::from(signed_tx.signature().r()), 32),
        DynSolValue::FixedBytes(B256::from(signed_tx.signature().s()), 32),
    ]);

    let chunk_4 = encode_receipt_fields(rx);

    vec![chunk_1, chunk_2, chunk_3, chunk_4]
}

fn encode_transaction_type_4(
    tx: Transaction,
    signed_tx: Signed<TxEip7702>,
    rx: TransactionReceipt,
) -> Vec<DynSolValue> {
    // Encoding transaction in 4 chunks
    let chunk_1 = encode_common_fields(&tx);

    let chunk_2 = DynSolValue::Tuple(vec![
        DynSolValue::Uint(U256::from(signed_tx.tx().chain_id), 64),
        DynSolValue::Uint(U256::from(signed_tx.tx().max_priority_fee_per_gas), 128),
        DynSolValue::Uint(U256::from(signed_tx.tx().max_fee_per_gas), 128),
        encode_access_list(&signed_tx.tx().access_list),
    ]);

    let chunk_3 = DynSolValue::Tuple(vec![
        encode_authorization_list(&signed_tx.tx().authorization_list),
        DynSolValue::Uint(U256::from(compute_y_parity(signed_tx.signature())), 8),
        DynSolValue::FixedBytes(B256::from(signed_tx.signature().r()), 32),
        DynSolValue::FixedBytes(B256::from(signed_tx.signature().s()), 32),
    ]);

    let chunk_4 = encode_receipt_fields(rx);

    vec![chunk_1, chunk_2, chunk_3, chunk_4]
}

fn encode_transaction_with_receipt(
    tx: Transaction,
    rx: TransactionReceipt,
) -> Option<(DynSolValue, Vec<DynSolValue>)> {
    let (type_id, chunks) = match tx.inner.clone_inner() {
        TxEnvelope::Legacy(signed_tx) => (
            DynSolValue::Uint(U256::from(0), 8),
            encode_transaction_type_0(tx, signed_tx, rx),
        ),
        TxEnvelope::Eip2930(signed_tx) => (
            DynSolValue::Uint(U256::from(1), 8),
            encode_transaction_type_1(tx, signed_tx, rx),
        ),
        TxEnvelope::Eip1559(signed_tx) => (
            DynSolValue::Uint(U256::from(2), 8),
            encode_transaction_type_2(tx, signed_tx, rx),
        ),
        TxEnvelope::Eip4844(signed_tx) => (
            DynSolValue::Uint(U256::from(3), 8),
            encode_transaction_type_3(tx, signed_tx, rx),
        ),
        TxEnvelope::Eip7702(signed_tx) => (
            DynSolValue::Uint(U256::from(4), 8),
            encode_transaction_type_4(tx, signed_tx, rx),
        ),
    };

    // If for some reason we fail to encode any chunk, we return None
    let encoded_chunks = chunks
        .into_iter()
        .map(|c| c.abi_encode_sequence().map(DynSolValue::Bytes))
        .collect::<Option<Vec<_>>>()?;

    Some((type_id, encoded_chunks))
}

fn encode_authorization_list(signed_authorizations: &[SignedAuthorization]) -> DynSolValue {
    let mut result = Vec::with_capacity(signed_authorizations.len());
    for signed_authorization in signed_authorizations {
        let signed_authorization_tuple = DynSolValue::Tuple(vec![
            DynSolValue::Uint(U256::from(*signed_authorization.chain_id()), 256),
            DynSolValue::Address(*signed_authorization.address()),
            DynSolValue::Uint(U256::from(signed_authorization.nonce()), 64),
            DynSolValue::Uint(U256::from(signed_authorization.y_parity()), 8),
            DynSolValue::Uint(U256::from(signed_authorization.r()), 256),
            DynSolValue::Uint(U256::from(signed_authorization.s()), 256),
        ]);

        result.push(signed_authorization_tuple);
    }

    DynSolValue::Array(result)
}

fn encode_access_list(access_list: &[AccessListItem]) -> DynSolValue {
    let mut list = Vec::with_capacity(access_list.len());
    for access_list_item in access_list {
        let mut storage_keys = Vec::with_capacity(access_list_item.storage_keys.len());
        for storage_item in &access_list_item.storage_keys {
            storage_keys.push(DynSolValue::FixedBytes(*storage_item, 32));
        }

        // Create the `DynSolValue::Tuple` (address, storage_keys)
        let access_list_tuple = DynSolValue::Tuple(vec![
            DynSolValue::Address(access_list_item.address), // Address
            DynSolValue::Array(storage_keys),
        ]);

        list.push(access_list_tuple);
    }

    DynSolValue::Array(list)
}

pub(super) fn abi_encode(tx: Transaction, rx: TransactionReceipt) -> Option<AbiEncodeResult> {
    if let Some((type_id, encoded_chunks)) = encode_transaction_with_receipt(tx, rx) {
        let tuple = DynSolValue::Tuple(vec![type_id, DynSolValue::Array(encoded_chunks.clone())]);
        let final_bytes = tuple.abi_encode_sequence()?;

        Some(AbiEncodeResult::new(final_bytes, EncodingVersion::V1))
    } else {
        None
    }
}

// These tests are used to ensure that the encoding abi is stable and does not change unexpectedly.
#[cfg(test)]
mod test {
    use super::*;

    use serde_json::json;

    #[test]
    fn type_0_transaction_integrity() {
        let transaction = json!({
            "type": "0x0",
            "nonce": "0x43eb",
            "gasPrice": "0xdf8475800",
            "gas": "0xc350",
            "to": "0xdf190dc7190dfba737d7777a163445b7fff16133",
            "value": "0x6113a84987be800",
            "input": "0x",
            "r": "0x3b08715b4403c792b8c7567edea634088bedcd7f60d9352b1f16c69830f3afd5",
            "s": "0x10b9afb67d2ec8b956f0e1dbc07eb79152904f3a7bf789fc869db56320adfe09",
            "v": "0x1c",
            "hash": "0xe9e91f1ee4b56c0df2e9f06c2b8c27c6076195a88a7b8537ba8313d80e6f124e",
            "blockHash": "0x8e38b4dbf6b11fcc3b9dee84fb7986e29ca0a02cecd8977c161ff7333329681e",
            "blockNumber": "0xf4240",
            "transactionIndex": "0x1",
            "from": "0x32be343b94f860124dc4fee278fdcbd38c102d88"
        });
        let receipt = json!({
            "blockHash": "0x8e38b4dbf6b11fcc3b9dee84fb7986e29ca0a02cecd8977c161ff7333329681e",
            "blockNumber": "0xf4240",
            "contractAddress": null,
            "cumulativeGasUsed": "0xc444",
            "effectiveGasPrice": "0xdf8475800",
            "from": "0x32be343b94f860124dc4fee278fdcbd38c102d88",
            "gasUsed": "0x5208",
            "logs": [],
            "logsBloom": "0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
            "root": "0xfa28ef92787192b577a8628e520b546ab58b72102572e08191ddecd51d0851e5",
            "to": "0xdf190dc7190dfba737d7777a163445b7fff16133",
            "transactionHash": "0xe9e91f1ee4b56c0df2e9f06c2b8c27c6076195a88a7b8537ba8313d80e6f124e",
            "transactionIndex": "0x1",
            "type": "0x0"
        });
        let transaction: Transaction =
            serde_json::from_value(transaction).expect("valid transaction");
        let receipt: TransactionReceipt = serde_json::from_value(receipt).expect("valid receipt");

        let result = abi_encode(transaction, receipt).expect("encoding should succeed");

        let abi = format!("0x{}", hex::encode(result.abi()));
        let expected_abi = String::from(
            "0x000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000001800000000000000000000000000000000000000000000000000000000000000220000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000043eb000000000000000000000000000000000000000000000000000000000000c35000000000000000000000000032be343b94f860124dc4fee278fdcbd38c102d880000000000000000000000000000000000000000000000000000000000000000000000000000000000000000df190dc7190dfba737d7777a163445b7fff1613300000000000000000000000000000000000000000000000006113a84987be80000000000000000000000000000000000000000000000000000000000000000e0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000800000000000000000000000000000000000000000000000000000000df8475800000000000000000000000000000000000000000000000000000000000000001c3b08715b4403c792b8c7567edea634088bedcd7f60d9352b1f16c69830f3afd510b9afb67d2ec8b956f0e1dbc07eb79152904f3a7bf789fc869db56320adfe0900000000000000000000000000000000000000000000000000000000000001c000000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000005208000000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
        );

        assert_eq!(abi, expected_abi);
    }

    #[test]
    fn type_1_transaction_integrity() {
        let transaction = json!({
            "type": "0x1",
            "chainId": "0x1",
            "nonce": "0x3",
            "gasPrice": "0x4bbe66a6b",
            "gas": "0x2dc6c0",
            "to": "0xac1d3d7a8878e655cbb063d58e453540641f4117",
            "value": "0x0",
            "accessList": [],
            "input": "0xc9567bf9",
            "r": "0xe8b501902f80aa4e595c33b0ea633ff42f05fbaafaa81bce322dc12ed63aadc",
            "s": "0x461898ba0f164b032f98a06f4a3fb8f3d7fc5e738ce2925c62c91c450f476d2e",
            "yParity": "0x1",
            "v": "0x1",
            "hash": "0xf725a062432b1e0405abcf69de62d0d2cbf2a15a17f56b77b8444e5aabf3e3de",
            "blockHash": "0xc6b6d85da551576ef3408cff2304d33162fd6260f34e83cdc42dcec4a70687ff",
            "blockNumber": "0x142723b",
            "transactionIndex": "0x0",
            "from": "0x1f1e9426d6b7d100746b09956ad90a0796e19aa3"
        });
        let receipt = json!({
            "blockHash": "0xc6b6d85da551576ef3408cff2304d33162fd6260f34e83cdc42dcec4a70687ff",
            "blockNumber": "0x142723b",
            "contractAddress": null,
            "cumulativeGasUsed": "0x7f9b",
            "effectiveGasPrice": "0x4bbe66a6b",
            "from": "0x1f1e9426d6b7d100746b09956ad90a0796e19aa3",
            "gasUsed": "0x7f9b",
            "logs": [
                {
                "address": "0xac1d3d7a8878e655cbb063d58e453540641f4117",
                "topics": [
                    "0xb3da2db3dfc3778f99852546c6e9ab39ec253f9de7b0847afec61bd27878e923",
                    "0x00000000000000000000000000000000000000000000000000000000672bf8d3"
                ],
                "data": "0x",
                "blockNumber": "0x142723b",
                "transactionHash": "0xf725a062432b1e0405abcf69de62d0d2cbf2a15a17f56b77b8444e5aabf3e3de",
                "transactionIndex": "0x0",
                "blockHash": "0xc6b6d85da551576ef3408cff2304d33162fd6260f34e83cdc42dcec4a70687ff",
                "logIndex": "0x0",
                "removed": false
                }
            ],
            "logsBloom": "0xc0000000000000000000000000000000000000080000010000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000010000000000000000080000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000",
            "status": "0x1",
            "to": "0xac1d3d7a8878e655cbb063d58e453540641f4117",
            "transactionHash": "0xf725a062432b1e0405abcf69de62d0d2cbf2a15a17f56b77b8444e5aabf3e3de",
            "transactionIndex": "0x0",
            "type": "0x1"
        });
        let transaction: Transaction =
            serde_json::from_value(transaction).expect("valid transaction");
        let receipt: TransactionReceipt = serde_json::from_value(receipt).expect("valid receipt");

        let result = abi_encode(transaction, receipt).expect("encoding should succeed");

        let abi = format!("0x{}", hex::encode(result.abi()));
        let expected_abi = String::from(
            "0x000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000001a000000000000000000000000000000000000000000000000000000000000002a00000000000000000000000000000000000000000000000000000000000000120000000000000000000000000000000000000000000000000000000000000000300000000000000000000000000000000000000000000000000000000002dc6c00000000000000000000000001f1e9426d6b7d100746b09956ad90a0796e19aa30000000000000000000000000000000000000000000000000000000000000000000000000000000000000000ac1d3d7a8878e655cbb063d58e453540641f4117000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000e00000000000000000000000000000000000000000000000000000000000000004c9567bf90000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000e0000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000004bbe66a6b00000000000000000000000000000000000000000000000000000000000000c000000000000000000000000000000000000000000000000000000000000000010e8b501902f80aa4e595c33b0ea633ff42f05fbaafaa81bce322dc12ed63aadc461898ba0f164b032f98a06f4a3fb8f3d7fc5e738ce2925c62c91c450f476d2e000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000002c000000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000007f9b000000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000001a000000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000020000000000000000000000000ac1d3d7a8878e655cbb063d58e453540641f4117000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000000c00000000000000000000000000000000000000000000000000000000000000002b3da2db3dfc3778f99852546c6e9ab39ec253f9de7b0847afec61bd27878e92300000000000000000000000000000000000000000000000000000000672bf8d300000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000100c0000000000000000000000000000000000000080000010000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000010000000000000000080000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000",
        );

        assert_eq!(abi, expected_abi);
    }

    #[test]
    fn type_2_transaction_integrity() {
        let transaction = json!({
            "blockHash": "0xf11da6e3bbcea68e3240e095c385a9782ae19fe2420ca69fd297ecbefb2760aa",
            "blockNumber": "0x168d87f",
            "from": "0xdadb0d80178819f2319190d340ce9a924f783711",
            "gas": "0x6af2",
            "gasPrice": "0x179e331a",
            "maxFeePerGas": "0x179e331a",
            "maxPriorityFeePerGas": "0x0",
            "hash": "0xd4be304337067e55c08bcffde2f2825da04da33535f119b422441990cd31bd97",
            "input": "0x",
            "nonce": "0x1914bc",
            "to": "0x73f7b1184b5cd361cc0f7654998953e2a251dd58",
            "transactionIndex": "0xec",
            "value": "0x78b3da703b6a70",
            "type": "0x2",
            "accessList": [],
            "chainId": "0x1",
            "v": "0x1",
            "r": "0x6de9aa950d074830a2339250e6b353d0a8afc886aa3e1649cd0080435a9ac9a4",
            "s": "0x6fb1de15d8511a73808ea299b37ffc66802b142405465937d6b76c36b142d880",
            "yParity": "0x1"
        });
        let receipt = json!({
            "blockHash": "0xf11da6e3bbcea68e3240e095c385a9782ae19fe2420ca69fd297ecbefb2760aa",
            "blockNumber": "0x168d87f",
            "contractAddress": null,
            "cumulativeGasUsed": "0x12668ef",
            "effectiveGasPrice": "0x179e331a",
            "from": "0xdadb0d80178819f2319190d340ce9a924f783711",
            "gasUsed": "0x6af2",
            "logs": [
                {
                "address": "0x73f7b1184b5cd361cc0f7654998953e2a251dd58",
                "topics": [
                    "0x85177f287940f2f05425a4029951af0e047a7f9c4eaa9a6e6917bcd869f86695",
                    "0x000000000000000000000000dadb0d80178819f2319190d340ce9a924f783711"
                ],
                "data": "0x0000000000000000000000000000000000000000000000000078b3da703b6a70",
                "blockNumber": "0x168d87f",
                "transactionHash": "0xd4be304337067e55c08bcffde2f2825da04da33535f119b422441990cd31bd97",
                "transactionIndex": "0xec",
                "blockHash": "0xf11da6e3bbcea68e3240e095c385a9782ae19fe2420ca69fd297ecbefb2760aa",
                "logIndex": "0x1e1",
                "removed": false
                }
            ],
            "logsBloom": "0x00000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000040000000000080000000000000001000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000080000000002000800000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
            "status": "0x1",
            "to": "0x73f7b1184b5cd361cc0f7654998953e2a251dd58",
            "transactionHash": "0xd4be304337067e55c08bcffde2f2825da04da33535f119b422441990cd31bd97",
            "transactionIndex": "0xec",
            "type": "0x2"
        });
        let transaction: Transaction =
            serde_json::from_value(transaction).expect("valid transaction");
        let receipt: TransactionReceipt = serde_json::from_value(receipt).expect("valid receipt");

        let result = abi_encode(transaction, receipt).expect("encoding should succeed");

        let abi = format!("0x{}", hex::encode(result.abi()));
        let expected_abi = String::from(
            "0x0000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000004000000000000000000000000000000000000000000000000000000000000000030000000000000000000000000000000000000000000000000000000000000060000000000000000000000000000000000000000000000000000000000000018000000000000000000000000000000000000000000000000000000000000002a0000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000001914bc0000000000000000000000000000000000000000000000000000000000006af2000000000000000000000000dadb0d80178819f2319190d340ce9a924f783711000000000000000000000000000000000000000000000000000000000000000000000000000000000000000073f7b1184b5cd361cc0f7654998953e2a251dd580000000000000000000000000000000000000000000000000078b3da703b6a7000000000000000000000000000000000000000000000000000000000000000e0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000179e331a00000000000000000000000000000000000000000000000000000000000000e000000000000000000000000000000000000000000000000000000000000000016de9aa950d074830a2339250e6b353d0a8afc886aa3e1649cd0080435a9ac9a46fb1de15d8511a73808ea299b37ffc66802b142405465937d6b76c36b142d880000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000002e000000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000006af2000000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000001c00000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000002000000000000000000000000073f7b1184b5cd361cc0f7654998953e2a251dd58000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000000c0000000000000000000000000000000000000000000000000000000000000000285177f287940f2f05425a4029951af0e047a7f9c4eaa9a6e6917bcd869f86695000000000000000000000000dadb0d80178819f2319190d340ce9a924f78371100000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000078b3da703b6a70000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000040000000000080000000000000001000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000080000000002000800000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
        );

        assert_eq!(abi, expected_abi);
    }

    #[test]
    fn type_3_transaction_integrity() {
        let transaction = json!({
            "type": "0x3",
            "chainId": "0x1",
            "nonce": "0x837e4",
            "gas": "0x36a2b",
            "maxFeePerGas": "0x213f9eed72",
            "maxPriorityFeePerGas": "0x3b9aca00",
            "to": "0x1c479675ad559dc151f6ec7ed3fbf8cee79582b6",
            "value": "0x0",
            "accessList": [
                {
                    "address": "0x1c479675ad559dc151f6ec7ed3fbf8cee79582b6",
                    "storageKeys": [
                        "0x0000000000000000000000000000000000000000000000000000000000000000",
                        "0x0000000000000000000000000000000000000000000000000000000000000001",
                        "0x000000000000000000000000000000000000000000000000000000000000000a",
                        "0xb53127684a568b3173ae13b9f8a6016e243e63b6e8ee1178d6a717850b5d6103",
                        "0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc",
                        "0xa10aa54071443520884ed767b0684edf43acec528b7da83ab38ce60126562660"
                    ]
                },
                {
                    "address": "0x8315177ab297ba92a06054ce80a67ed4dbd7ed3a",
                    "storageKeys": [
                        "0x0000000000000000000000000000000000000000000000000000000000000006",
                        "0x0000000000000000000000000000000000000000000000000000000000000007",
                        "0x0000000000000000000000000000000000000000000000000000000000000009",
                        "0x000000000000000000000000000000000000000000000000000000000000000a",
                        "0xb53127684a568b3173ae13b9f8a6016e243e63b6e8ee1178d6a717850b5d6103",
                        "0x360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbc",
                        "0xa66cc928b5edb82af9bd49922954155ab7b0942694bea4ce44661d9a873fc679",
                        "0xa66cc928b5edb82af9bd49922954155ab7b0942694bea4ce44661d9a873fc67a",
                        "0xf652222313e28459528d920b65115c16c04f3efc82aaedc97be59f3f3792b181"
                    ]
                },
                {
                    "address": "0xe64a54e2533fd126c2e452c5fab544d80e2e4eb5",
                    "storageKeys": [
                        "0x0000000000000000000000000000000000000000000000000000000000000004",
                        "0x0000000000000000000000000000000000000000000000000000000000000005",
                        "0xe85fd79f89ff278fc57d40aecb7947873df9f0beac531c8f71a98f630e1eab62",
                        "0x7686888b19bb7b75e46bb1aa328b65150743f4899443d722f0adf8e252ccda41"
                    ]
                }
            ],
            "blobVersionedHashes": [
                "0x014527d555d949b3afcfa246e16eb0e0aef9e9da60b7a0266f1da43b3fd8e8cf",
                "0x016d80efa350ab1fc156b505ab619bee3f6245b8f7d4d60bf11c9d8b0105b02f",
                "0x0176b14180ebfaa132142ff163eb2aaf2985af7da011d195e39fe8b0faf1e960",
                "0x0134da09304a6a66b691bc48d351b976203cd419778d142f19e68e904f07a5ae",
                "0x0181b4581a9fc316eadc58e4d6d362e316e259643913339a3e46b7c9d742ac30",
                "0x0112fa6c9dfaceaff1868ef19d01c4a1da99e6e02162fe7dacf94ec441da6977"
            ],
            "maxFeePerBlobGas": "0xa",
            "input": "0x3e5aa082000000000000000000000000000000000000000000000000000000000008fff2000000000000000000000000000000000000000000000000000000000016a443000000000000000000000000e64a54e2533fd126c2e452c5fab544d80e2e4eb5000000000000000000000000000000000000000000000000000000000aafdc87000000000000000000000000000000000000000000000000000000000aafde27",
            "r": "0x4dfd139f20fdefc834fbdce2e120ca8ed1a4688d8843df8fc2de1df8c6d0a0f3",
            "s": "0x6ec282ea1c2c55e467425c380c17f0f8ef664d8e2de8a39b18d6c83b8d6a9afa",
            "yParity": "0x1",
            "v": "0x1",
            "hash": "0x2ea19986a6866b6efd2ac292fa8132b0bbf1fcc478560525ce43d6c300323652",
            "blockHash": "0x885dbb7ca37d235e2a211e04668cd382f54872292e2adf50d61d65785fa2b713",
            "blockNumber": "0x12c135c",
            "transactionIndex": "0x53",
            "from": "0xc1b634853cb333d3ad8663715b08f41a3aec47cc",
            "gasPrice": "0x38367e339"
        });
        let receipt = json!({
            "blobGasPrice": "0x1",
            "blobGasUsed": "0xc0000",
            "blockHash": "0x885dbb7ca37d235e2a211e04668cd382f54872292e2adf50d61d65785fa2b713",
            "blockNumber": "0x12c135c",
            "contractAddress": null,
            "cumulativeGasUsed": "0x765f8c",
            "effectiveGasPrice": "0x38367e339",
            "from": "0xc1b634853cb333d3ad8663715b08f41a3aec47cc",
            "gasUsed": "0x29d5f",
            "logs": [
                {
                "address": "0x1c479675ad559dc151f6ec7ed3fbf8cee79582b6",
                "topics": [
                    "0x7394f4a19a13c7b92b5bb71033245305946ef78452f7b4986ac1390b5df4ebd7",
                    "0x000000000000000000000000000000000000000000000000000000000008fff2",
                    "0x10a6a070fc96f51a7f92541aa7fd47cb481cade711489bc1d37fb42273929f95",
                    "0xb4c9f1185b38503ab4b0fcd2f4aff24d39475e5f1df61bb58aa48e7f6d09f709"
                ],
                "data": "0x8d4eaf080d90bc8d1c6bf254f597300e3a04cbc0b17f6f0cc8cb5edd74eefa1b000000000000000000000000000000000000000000000000000000000016a44300000000000000000000000000000000000000000000000000000000661cb14f00000000000000000000000000000000000000000000000000000000661e05cf00000000000000000000000000000000000000000000000000000000012bfcdc00000000000000000000000000000000000000000000000000000000012c139c0000000000000000000000000000000000000000000000000000000000000003",
                "blockNumber": "0x12c135c",
                "transactionHash": "0x2ea19986a6866b6efd2ac292fa8132b0bbf1fcc478560525ce43d6c300323652",
                "transactionIndex": "0x53",
                "blockHash": "0x885dbb7ca37d235e2a211e04668cd382f54872292e2adf50d61d65785fa2b713",
                "logIndex": "0xc9",
                "removed": false
                },
                {
                "address": "0x8315177ab297ba92a06054ce80a67ed4dbd7ed3a",
                "topics": [
                    "0x5e3c1311ea442664e8b1611bfabef659120ea7a0a2cfc0667700bebc69cbffe1",
                    "0x000000000000000000000000000000000000000000000000000000000016a449",
                    "0x7a9be4783a8a7f47297c0edaea411e7bb2e21f7c6b4a83826c5467e1b095ae6a"
                ],
                "data": "0x0000000000000000000000001c479675ad559dc151f6ec7ed3fbf8cee79582b6000000000000000000000000000000000000000000000000000000000000000d000000000000000000000000c1b634853cb333d3ad8663715b08f41a3aec47ccdb32956884f3752604ba67b6ba9ec6827e03e2cfa750f14de667e291564ccc610000000000000000000000000000000000000000000000000000000347cd193900000000000000000000000000000000000000000000000000000000661e02cf",
                "blockNumber": "0x12c135c",
                "transactionHash": "0x2ea19986a6866b6efd2ac292fa8132b0bbf1fcc478560525ce43d6c300323652",
                "transactionIndex": "0x53",
                "blockHash": "0x885dbb7ca37d235e2a211e04668cd382f54872292e2adf50d61d65785fa2b713",
                "logIndex": "0xca",
                "removed": false
                },
                {
                "address": "0x1c479675ad559dc151f6ec7ed3fbf8cee79582b6",
                "topics": [
                    "0xff64905f73a67fb594e0f940a8075a860db489ad991e032f48c81123eb52d60b",
                    "0x000000000000000000000000000000000000000000000000000000000016a449"
                ],
                "data": "0x0000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000009c00000000000000000000000000000000000000000000000000000000661e02cfc1b634853cb333d3ad8663715b08f41a3aec47cc0160b2ad9e7e658fe60b7898ce1f34ed816ad9ebfd9f19b085e84af59c974ee8000000000000000000000000000000000000000000000000000000000008fff20000000000000000000000000000000000000000000000000000000347cd1939000000000000000000000000",
                "blockNumber": "0x12c135c",
                "transactionHash": "0x2ea19986a6866b6efd2ac292fa8132b0bbf1fcc478560525ce43d6c300323652",
                "transactionIndex": "0x53",
                "blockHash": "0x885dbb7ca37d235e2a211e04668cd382f54872292e2adf50d61d65785fa2b713",
                "logIndex": "0xcb",
                "removed": false
                },
                {
                "address": "0xe64a54e2533fd126c2e452c5fab544d80e2e4eb5",
                "topics": [
                    "0xd0224505f828ccfcbc56ca0590d97442e239a7aa770f712948fd6388356b20de",
                    "0x000000000000000000000000c1b634853cb333d3ad8663715b08f41a3aec47cc",
                    "0x0000000000000000000000001c479675ad559dc151f6ec7ed3fbf8cee79582b6",
                    "0x0000000000000000000000000000000000000000000000000000000000000001"
                ],
                "data": "0x0000000000000000000000000000000000000000000000000000000000034710000000000000000000000000000000000000000000000000000000038367e33900000000000000000000000000000000000000000000000000097f51a8b059e2",
                "blockNumber": "0x12c135c",
                "transactionHash": "0x2ea19986a6866b6efd2ac292fa8132b0bbf1fcc478560525ce43d6c300323652",
                "transactionIndex": "0x53",
                "blockHash": "0x885dbb7ca37d235e2a211e04668cd382f54872292e2adf50d61d65785fa2b713",
                "logIndex": "0xcc",
                "removed": false
                }
            ],
            "logsBloom": "0x0400000008000000000000000000000040000080000000000000000040000000000000000000000000000000000000000100000012100000100000001004000000001000000000000000080000000000000000000004001000000000000000000000000000000000000000000000000000000000000000000000000008000000000000000000000002000000000000000000100000000000400000000000000080000002000000000000000001000002006000100400008000000000000000000000000000000000000000000000000000000400000000018400100008004000000000000000800000000000000000000000000000208000000021000000c040",
            "status": "0x1",
            "to": "0x1c479675ad559dc151f6ec7ed3fbf8cee79582b6",
            "transactionHash": "0x2ea19986a6866b6efd2ac292fa8132b0bbf1fcc478560525ce43d6c300323652",
            "transactionIndex": "0x53",
            "type": "0x3"
        });
        let transaction: Transaction =
            serde_json::from_value(transaction).expect("valid transaction");
        let receipt: TransactionReceipt = serde_json::from_value(receipt).expect("valid receipt");

        let result = abi_encode(transaction, receipt).expect("encoding should succeed");

        let abi = format!("0x{}", hex::encode(result.abi()));
        let expected_abi = String::from(
            "0x00000000000000000000000000000000000000000000000000000000000000030000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000000800000000000000000000000000000000000000000000000000000000000000260000000000000000000000000000000000000000000000000000000000000070000000000000000000000000000000000000000000000000000000000000008a000000000000000000000000000000000000000000000000000000000000001c000000000000000000000000000000000000000000000000000000000000837e40000000000000000000000000000000000000000000000000000000000036a2b000000000000000000000000c1b634853cb333d3ad8663715b08f41a3aec47cc00000000000000000000000000000000000000000000000000000000000000000000000000000000000000001c479675ad559dc151f6ec7ed3fbf8cee79582b6000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000e000000000000000000000000000000000000000000000000000000000000000a43e5aa082000000000000000000000000000000000000000000000000000000000008fff2000000000000000000000000000000000000000000000000000000000016a443000000000000000000000000e64a54e2533fd126c2e452c5fab544d80e2e4eb5000000000000000000000000000000000000000000000000000000000aafdc87000000000000000000000000000000000000000000000000000000000aafde270000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000004800000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000003b9aca00000000000000000000000000000000000000000000000000000000213f9eed72000000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000000030000000000000000000000000000000000000000000000000000000000000060000000000000000000000000000000000000000000000000000000000000018000000000000000000000000000000000000000000000000000000000000003000000000000000000000000001c479675ad559dc151f6ec7ed3fbf8cee79582b60000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000000600000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000ab53127684a568b3173ae13b9f8a6016e243e63b6e8ee1178d6a717850b5d6103360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbca10aa54071443520884ed767b0684edf43acec528b7da83ab38ce601265626600000000000000000000000008315177ab297ba92a06054ce80a67ed4dbd7ed3a00000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000000009000000000000000000000000000000000000000000000000000000000000000600000000000000000000000000000000000000000000000000000000000000070000000000000000000000000000000000000000000000000000000000000009000000000000000000000000000000000000000000000000000000000000000ab53127684a568b3173ae13b9f8a6016e243e63b6e8ee1178d6a717850b5d6103360894a13ba1a3210667c828492db98dca3e2076cc3735a920a3ca505d382bbca66cc928b5edb82af9bd49922954155ab7b0942694bea4ce44661d9a873fc679a66cc928b5edb82af9bd49922954155ab7b0942694bea4ce44661d9a873fc67af652222313e28459528d920b65115c16c04f3efc82aaedc97be59f3f3792b181000000000000000000000000e64a54e2533fd126c2e452c5fab544d80e2e4eb50000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000005e85fd79f89ff278fc57d40aecb7947873df9f0beac531c8f71a98f630e1eab627686888b19bb7b75e46bb1aa328b65150743f4899443d722f0adf8e252ccda410000000000000000000000000000000000000000000000000000000000000180000000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000a000000000000000000000000000000000000000000000000000000000000000014dfd139f20fdefc834fbdce2e120ca8ed1a4688d8843df8fc2de1df8c6d0a0f36ec282ea1c2c55e467425c380c17f0f8ef664d8e2de8a39b18d6c83b8d6a9afa0000000000000000000000000000000000000000000000000000000000000006014527d555d949b3afcfa246e16eb0e0aef9e9da60b7a0266f1da43b3fd8e8cf016d80efa350ab1fc156b505ab619bee3f6245b8f7d4d60bf11c9d8b0105b02f0176b14180ebfaa132142ff163eb2aaf2985af7da011d195e39fe8b0faf1e9600134da09304a6a66b691bc48d351b976203cd419778d142f19e68e904f07a5ae0181b4581a9fc316eadc58e4d6d362e316e259643913339a3e46b7c9d742ac300112fa6c9dfaceaff1868ef19d01c4a1da99e6e02162fe7dacf94ec441da6977000000000000000000000000000000000000000000000000000000000000094000000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000029d5f00000000000000000000000000000000000000000000000000000000000000800000000000000000000000000000000000000000000000000000000000000820000000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000000800000000000000000000000000000000000000000000000000000000000000280000000000000000000000000000000000000000000000000000000000000044000000000000000000000000000000000000000000000000000000000000006000000000000000000000000001c479675ad559dc151f6ec7ed3fbf8cee79582b60000000000000000000000000000000000000000000000000000000000000060000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000000047394f4a19a13c7b92b5bb71033245305946ef78452f7b4986ac1390b5df4ebd7000000000000000000000000000000000000000000000000000000000008fff210a6a070fc96f51a7f92541aa7fd47cb481cade711489bc1d37fb42273929f95b4c9f1185b38503ab4b0fcd2f4aff24d39475e5f1df61bb58aa48e7f6d09f70900000000000000000000000000000000000000000000000000000000000000e08d4eaf080d90bc8d1c6bf254f597300e3a04cbc0b17f6f0cc8cb5edd74eefa1b000000000000000000000000000000000000000000000000000000000016a44300000000000000000000000000000000000000000000000000000000661cb14f00000000000000000000000000000000000000000000000000000000661e05cf00000000000000000000000000000000000000000000000000000000012bfcdc00000000000000000000000000000000000000000000000000000000012c139c00000000000000000000000000000000000000000000000000000000000000030000000000000000000000008315177ab297ba92a06054ce80a67ed4dbd7ed3a000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000000e000000000000000000000000000000000000000000000000000000000000000035e3c1311ea442664e8b1611bfabef659120ea7a0a2cfc0667700bebc69cbffe1000000000000000000000000000000000000000000000000000000000016a4497a9be4783a8a7f47297c0edaea411e7bb2e21f7c6b4a83826c5467e1b095ae6a00000000000000000000000000000000000000000000000000000000000000c00000000000000000000000001c479675ad559dc151f6ec7ed3fbf8cee79582b6000000000000000000000000000000000000000000000000000000000000000d000000000000000000000000c1b634853cb333d3ad8663715b08f41a3aec47ccdb32956884f3752604ba67b6ba9ec6827e03e2cfa750f14de667e291564ccc610000000000000000000000000000000000000000000000000000000347cd193900000000000000000000000000000000000000000000000000000000661e02cf0000000000000000000000001c479675ad559dc151f6ec7ed3fbf8cee79582b6000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000000c00000000000000000000000000000000000000000000000000000000000000002ff64905f73a67fb594e0f940a8075a860db489ad991e032f48c81123eb52d60b000000000000000000000000000000000000000000000000000000000016a44900000000000000000000000000000000000000000000000000000000000000e00000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000009c00000000000000000000000000000000000000000000000000000000661e02cfc1b634853cb333d3ad8663715b08f41a3aec47cc0160b2ad9e7e658fe60b7898ce1f34ed816ad9ebfd9f19b085e84af59c974ee8000000000000000000000000000000000000000000000000000000000008fff20000000000000000000000000000000000000000000000000000000347cd1939000000000000000000000000000000000000000000000000e64a54e2533fd126c2e452c5fab544d80e2e4eb5000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000001000000000000000000000000000000000000000000000000000000000000000004d0224505f828ccfcbc56ca0590d97442e239a7aa770f712948fd6388356b20de000000000000000000000000c1b634853cb333d3ad8663715b08f41a3aec47cc0000000000000000000000001c479675ad559dc151f6ec7ed3fbf8cee79582b6000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000600000000000000000000000000000000000000000000000000000000000034710000000000000000000000000000000000000000000000000000000038367e33900000000000000000000000000000000000000000000000000097f51a8b059e200000000000000000000000000000000000000000000000000000000000001000400000008000000000000000000000040000080000000000000000040000000000000000000000000000000000000000100000012100000100000001004000000001000000000000000080000000000000000000004001000000000000000000000000000000000000000000000000000000000000000000000000008000000000000000000000002000000000000000000100000000000400000000000000080000002000000000000000001000002006000100400008000000000000000000000000000000000000000000000000000000400000000018400100008004000000000000000800000000000000000000000000000208000000021000000c040",
        );

        assert_eq!(abi, expected_abi);
    }

    #[test]
    fn type_4_transaction_integrity() {
        let transaction = json!({
            "type": "0x4",
            "chainId": "0x1",
            "nonce": "0xddf",
            "gas": "0x142aec",
            "maxFeePerGas": "0xb8511c2",
            "maxPriorityFeePerGas": "0x2faf080",
            "to": "0x0000000071727de22e5e9d8baf0edac6f37da032",
            "value": "0x0",
            "accessList": [],
            "authorizationList": [
                {
                    "chainId": "0x1",
                    "address": "0xd2e28229f6f2c235e57de2ebc727025a1d0530fb",
                    "nonce": "0x52",
                    "yParity": "0x1",
                    "r": "0xdbee6cb8128fe31de9c9a2dd20e4fe4a70f20ee64428a3c09d9a9d25a0c6e1c7",
                    "s": "0x7314d0c97d81179a461f2e236510026953553155935ede0fba3645567fb175d8"
                }
            ],
            "input": "0x765e827f000000000000000000000000000000000000000000000000000000000000004000000000000000000000000048c16a1dc402a1e463474f40202497beb9a11deb000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000200000000000000000000000004cd27786173e00743114202f1455c54e3536cdfb000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001200000000000000000000000000000000000000000000000000000000000000140000000000000000000000000000124f800000000000000000000000000077c60000000000000000000000000000000000000000000000000000000000001b544000000000000000000000000039387000000000000000000000000000e0d0a290000000000000000000000000000000000000000000000000000000000000a000000000000000000000000000000000000000000000000000000000000000ae00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000088426da7d880000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000000120000000000000000000000000dac17f958d2ee523a2206206994597c13d831ec7000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000600000000000000000000000000000000000000000000000000000000000000044095ea7b30000000000000000000000007d3201fa7a85c0a5f9fa1c0c6b9d0b784368d2ac00000000000000000000000000000000000000000000000000000000000dea59000000000000000000000000000000000000000000000000000000000000000000000000000000003c11f6265ddec22f4d049dde480615735f451646000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000600000000000000000000000000000000000000000000000000000000000000684049639fb0000000000000000000000000000000000000000000000000000000000000004000000000000000000000000dac17f958d2ee523a2206206994597c13d831ec7000000000000000000000000eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee000000000000000000000000000000000000000000000000000000000112dcac000000000000000000000000000000000000000000000000000f5b0557048d4000000000000000000000000000000000000000000000000000000000000000c0000000000000000000000000000000000000000000000000000000000000058812aa3caf0000000000000000000000005141b82f5ffda4c6fe1e372978f1c5427640a190000000000000000000000000dac17f958d2ee523a2206206994597c13d831ec7000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc20000000000000000000000005141b82f5ffda4c6fe1e372978f1c5427640a190000000000000000000000000a7ca2c8673bcfa5a26d8ceec2887f2cc2b0db22a000000000000000000000000000000000000000000000000000000000112dcac000000000000000000000000000000000000000000000000000f5b0557048d40000000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000001400000000000000000000000000000000000000000000000000000000000000160000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000003ee0000000000000000000000000000000000000000000000000000000003d05120111111125421ca6dc452d289314280a0f8842a65dac17f958d2ee523a2206206994597c13d831ec7012456a75868f1fa3d165d765dca08fc1df1889d6b89d53ac5bf210c7d50612c24bc68ca5b5100000000000000000000000051c72848c68a965f66fa7a88855f9f7784502a7f00000000000000000000000051c72848c68a965f66fa7a88855f9f7784502a7f0000000000000000000000008b543dff08ed4ba13ee96f533638ef54591aee71000000000000000000000000dac17f958d2ee523a2206206994597c13d831ec7000000000000000000000000000000000000000000000000000f8f1862c313dd000000000000000000000000000000000000000000000000000000000112dcac02000000000000000000000000c74bb8020068ef4829372978f1c5427640a19000000000000000000000000000000000000000000000000000000000000001800000000000000000000000000000000000000000000000000000000000000000280001600000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000041ee1566ef57bc0730d826a0d8009c9f76e16d9208ab35045bc6527775ee37a2c679a9c7fff265764ed3be9b1b16d4cf83a85686c13f7539f7f170be22d24969961c0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001741111111254eeb25477b68fb85ed929f73a9605820000014000000140000001400000014000000140000001400000014000000140000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2000000000000000000000000000000000000000000000000000f8f1862c313dd00000000000000000000000000000000000000000000000000000000c74bb8020000000000000000000000000000000000000000000000000000000068ef48296c68df299ed9d19722811dfcb0a18a058845e20f88bc98d5156c6f700b7ebad90000000000000000000000000000000000000000000000000000000000000120000000000000000000000000000000000000000000000000000000000000004174ddf31a490ba2d579b6e8a91d2683a889f344702e90843f0121ef0a65f52d11684755d830c2fddcf5afbfbd46e1b793cccaca2321a94b0abc1c3ca9b22729c31c00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000c4e3736f000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000b57d3201fa7a85c0a5f9fa1c0c6b9d0b784368d2ac000000000000000000000000000061a800000000000000000000000000005daa000068ef493a000000000000dac17f958d2ee523a2206206994597c13d831ec70000000000000000000000000000000000000000000000000000000000089ba50f051a985ce8f84587f99cbbd0d951ff1186d097109771073b3a260a7c8ef2790227c8dd9fd61ad61f863ede2bc7b5e809b253b9949509f92274d961760dd6101c000000000000000000000000000000000000000000000000000000000000000000000000000000000000417080bcb677d9195f0a53a1c89bd0c5bac797bd96445315c3675382b956b0a1ee246b351f3493e0a16a0468437ff491adb01e2c3a28f88fe47df36b78c71ff20f1c00000000000000000000000000000000000000000000000000000000000000",
            "r": "0x60bf6ed8d222cd3422d7331c1684e7a6827c40f6e68a21a7d69fd87bd2dce6b6",
            "s": "0x6be34d4773afde685b19086963e06af351da89c0796f4f14acdc941b547ad9e4",
            "yParity": "0x1",
            "v": "0x1",
            "hash": "0x32a7f4b178110a2f0f2ff1d611b081b5659f34546b647714d0d76adea3c32da0",
            "blockHash": "0x63029255b1ffdf8eeb04a60c482a20f2294fc3d7ac4d4138817fe0c7bcfc1a72",
            "blockNumber": "0x167d337",
            "transactionIndex": "0xe1",
            "from": "0x48c16a1dc402a1e463474f40202497beb9a11deb",
            "gasPrice": "0x92efa54"
        });
        let receipt = json!({
            "blockHash": "0x63029255b1ffdf8eeb04a60c482a20f2294fc3d7ac4d4138817fe0c7bcfc1a72",
            "blockNumber": "0x167d337",
            "contractAddress": null,
            "cumulativeGasUsed": "0x12d059d",
            "effectiveGasPrice": "0x92efa54",
            "from": "0x48c16a1dc402a1e463474f40202497beb9a11deb",
            "gasUsed": "0x62a37",
            "logs": [
                {
                "address": "0x0000000071727de22e5e9d8baf0edac6f37da032",
                "topics": [
                    "0xbb47ee3e183a558b1a2ff0874b079f3fc5478b7454eacf2bfc5af2ff5878f972"
                ],
                "data": "0x",
                "blockNumber": "0x167d337",
                "transactionHash": "0x32a7f4b178110a2f0f2ff1d611b081b5659f34546b647714d0d76adea3c32da0",
                "transactionIndex": "0xe1",
                "blockHash": "0x63029255b1ffdf8eeb04a60c482a20f2294fc3d7ac4d4138817fe0c7bcfc1a72",
                "logIndex": "0x1ff",
                "removed": false
                },
                {
                "address": "0x0000000071727de22e5e9d8baf0edac6f37da032",
                "topics": [
                    "0xf62676f440ff169a3a9afdbf812e89e7f95975ee8e5c31214ffdef631c5f4792",
                    "0x24a2f504d977eb9364fb9ab406675744e37df5d939547f3f72604b46023b964c",
                    "0x0000000000000000000000004cd27786173e00743114202f1455c54e3536cdfb"
                ],
                "data": "0x000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000000064ad7954bc000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000047939f4240000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
                "blockNumber": "0x167d337",
                "transactionHash": "0x32a7f4b178110a2f0f2ff1d611b081b5659f34546b647714d0d76adea3c32da0",
                "transactionIndex": "0xe1",
                "blockHash": "0x63029255b1ffdf8eeb04a60c482a20f2294fc3d7ac4d4138817fe0c7bcfc1a72",
                "logIndex": "0x200",
                "removed": false
                },
                {
                "address": "0x0000000071727de22e5e9d8baf0edac6f37da032",
                "topics": [
                    "0x49628fd1471006c1482da88028e9ce4dbb080b815c9b0344d39e5a8e6ec1419f",
                    "0x24a2f504d977eb9364fb9ab406675744e37df5d939547f3f72604b46023b964c",
                    "0x0000000000000000000000004cd27786173e00743114202f1455c54e3536cdfb",
                    "0x0000000000000000000000007d3201fa7a85c0a5f9fa1c0c6b9d0b784368d2ac"
                ],
                "data": "0x00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000045ade7e680000000000000000000000000000000000000000000000000000000000000072000",
                "blockNumber": "0x167d337",
                "transactionHash": "0x32a7f4b178110a2f0f2ff1d611b081b5659f34546b647714d0d76adea3c32da0",
                "transactionIndex": "0xe1",
                "blockHash": "0x63029255b1ffdf8eeb04a60c482a20f2294fc3d7ac4d4138817fe0c7bcfc1a72",
                "logIndex": "0x201",
                "removed": false
                }
            ],
            "logsBloom": "0x00000100000000000000000000000000000000000000000000000000000000000008200000000000000000010000000100000000000000000000020000000000000000000000000000000000000000000040000000200000000000000000000000000000000800000000000000000000000000000000000000800000000000000040000000000000000000000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000100002000000000000000000400000400001000000000000000000000400000000000000000000000000000000000000000000000000000000080000000800000000000000000080",
            "status": "0x1",
            "to": "0x0000000071727de22e5e9d8baf0edac6f37da032",
            "transactionHash": "0x32a7f4b178110a2f0f2ff1d611b081b5659f34546b647714d0d76adea3c32da0",
            "transactionIndex": "0xe1",
            "type": "0x4"
        });
        let transaction: Transaction =
            serde_json::from_value(transaction).expect("valid transaction");
        let receipt: TransactionReceipt = serde_json::from_value(receipt).expect("valid receipt");

        let result = abi_encode(transaction, receipt).expect("encoding should succeed");

        let abi = format!("0x{}", hex::encode(result.abi()));
        let expected_abi = String::from(
            "0x00000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000040000000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000000800000000000000000000000000000000000000000000000000000000000000da00000000000000000000000000000000000000000000000000000000000000e600000000000000000000000000000000000000000000000000000000000000fe00000000000000000000000000000000000000000000000000000000000000d000000000000000000000000000000000000000000000000000000000000000ddf0000000000000000000000000000000000000000000000000000000000142aec00000000000000000000000048c16a1dc402a1e463474f40202497beb9a11deb00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000071727de22e5e9d8baf0edac6f37da032000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000e00000000000000000000000000000000000000000000000000000000000000be4765e827f000000000000000000000000000000000000000000000000000000000000004000000000000000000000000048c16a1dc402a1e463474f40202497beb9a11deb000000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000200000000000000000000000004cd27786173e00743114202f1455c54e3536cdfb000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001200000000000000000000000000000000000000000000000000000000000000140000000000000000000000000000124f800000000000000000000000000077c60000000000000000000000000000000000000000000000000000000000001b544000000000000000000000000039387000000000000000000000000000e0d0a290000000000000000000000000000000000000000000000000000000000000a000000000000000000000000000000000000000000000000000000000000000ae00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000088426da7d880000000000000000000000000000000000000000000000000000000000000020000000000000000000000000000000000000000000000000000000000000000200000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000000120000000000000000000000000dac17f958d2ee523a2206206994597c13d831ec7000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000600000000000000000000000000000000000000000000000000000000000000044095ea7b30000000000000000000000007d3201fa7a85c0a5f9fa1c0c6b9d0b784368d2ac00000000000000000000000000000000000000000000000000000000000dea59000000000000000000000000000000000000000000000000000000000000000000000000000000003c11f6265ddec22f4d049dde480615735f451646000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000600000000000000000000000000000000000000000000000000000000000000684049639fb0000000000000000000000000000000000000000000000000000000000000004000000000000000000000000dac17f958d2ee523a2206206994597c13d831ec7000000000000000000000000eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee000000000000000000000000000000000000000000000000000000000112dcac000000000000000000000000000000000000000000000000000f5b0557048d4000000000000000000000000000000000000000000000000000000000000000c0000000000000000000000000000000000000000000000000000000000000058812aa3caf0000000000000000000000005141b82f5ffda4c6fe1e372978f1c5427640a190000000000000000000000000dac17f958d2ee523a2206206994597c13d831ec7000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc20000000000000000000000005141b82f5ffda4c6fe1e372978f1c5427640a190000000000000000000000000a7ca2c8673bcfa5a26d8ceec2887f2cc2b0db22a000000000000000000000000000000000000000000000000000000000112dcac000000000000000000000000000000000000000000000000000f5b0557048d40000000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000001400000000000000000000000000000000000000000000000000000000000000160000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000003ee0000000000000000000000000000000000000000000000000000000003d05120111111125421ca6dc452d289314280a0f8842a65dac17f958d2ee523a2206206994597c13d831ec7012456a75868f1fa3d165d765dca08fc1df1889d6b89d53ac5bf210c7d50612c24bc68ca5b5100000000000000000000000051c72848c68a965f66fa7a88855f9f7784502a7f00000000000000000000000051c72848c68a965f66fa7a88855f9f7784502a7f0000000000000000000000008b543dff08ed4ba13ee96f533638ef54591aee71000000000000000000000000dac17f958d2ee523a2206206994597c13d831ec7000000000000000000000000000000000000000000000000000f8f1862c313dd000000000000000000000000000000000000000000000000000000000112dcac02000000000000000000000000c74bb8020068ef4829372978f1c5427640a19000000000000000000000000000000000000000000000000000000000000001800000000000000000000000000000000000000000000000000000000000000000280001600000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000041ee1566ef57bc0730d826a0d8009c9f76e16d9208ab35045bc6527775ee37a2c679a9c7fff265764ed3be9b1b16d4cf83a85686c13f7539f7f170be22d24969961c0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001741111111254eeb25477b68fb85ed929f73a9605820000014000000140000001400000014000000140000001400000014000000140000000000000000000000000c02aaa39b223fe8d0a0e5c4f27ead9083c756cc2000000000000000000000000000000000000000000000000000f8f1862c313dd00000000000000000000000000000000000000000000000000000000c74bb8020000000000000000000000000000000000000000000000000000000068ef48296c68df299ed9d19722811dfcb0a18a058845e20f88bc98d5156c6f700b7ebad90000000000000000000000000000000000000000000000000000000000000120000000000000000000000000000000000000000000000000000000000000004174ddf31a490ba2d579b6e8a91d2683a889f344702e90843f0121ef0a65f52d11684755d830c2fddcf5afbfbd46e1b793cccaca2321a94b0abc1c3ca9b22729c31c00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000c4e3736f000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000b57d3201fa7a85c0a5f9fa1c0c6b9d0b784368d2ac000000000000000000000000000061a800000000000000000000000000005daa000068ef493a000000000000dac17f958d2ee523a2206206994597c13d831ec70000000000000000000000000000000000000000000000000000000000089ba50f051a985ce8f84587f99cbbd0d951ff1186d097109771073b3a260a7c8ef2790227c8dd9fd61ad61f863ede2bc7b5e809b253b9949509f92274d961760dd6101c000000000000000000000000000000000000000000000000000000000000000000000000000000000000417080bcb677d9195f0a53a1c89bd0c5bac797bd96445315c3675382b956b0a1ee246b351f3493e0a16a0468437ff491adb01e2c3a28f88fe47df36b78c71ff20f1c000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000a000000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000002faf080000000000000000000000000000000000000000000000000000000000b8511c20000000000000000000000000000000000000000000000000000000000000080000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000001600000000000000000000000000000000000000000000000000000000000000080000000000000000000000000000000000000000000000000000000000000000160bf6ed8d222cd3422d7331c1684e7a6827c40f6e68a21a7d69fd87bd2dce6b66be34d4773afde685b19086963e06af351da89c0796f4f14acdc941b547ad9e400000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000000001000000000000000000000000d2e28229f6f2c235e57de2ebc727025a1d0530fb00000000000000000000000000000000000000000000000000000000000000520000000000000000000000000000000000000000000000000000000000000001dbee6cb8128fe31de9c9a2dd20e4fe4a70f20ee64428a3c09d9a9d25a0c6e1c77314d0c97d81179a461f2e236510026953553155935ede0fba3645567fb175d8000000000000000000000000000000000000000000000000000000000000066000000000000000000000000000000000000000000000000000000000000000010000000000000000000000000000000000000000000000000000000000062a370000000000000000000000000000000000000000000000000000000000000080000000000000000000000000000000000000000000000000000000000000054000000000000000000000000000000000000000000000000000000000000000030000000000000000000000000000000000000000000000000000000000000060000000000000000000000000000000000000000000000000000000000000012000000000000000000000000000000000000000000000000000000000000003000000000000000000000000000000000071727de22e5e9d8baf0edac6f37da032000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000000a00000000000000000000000000000000000000000000000000000000000000001bb47ee3e183a558b1a2ff0874b079f3fc5478b7454eacf2bfc5af2ff5878f97200000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000071727de22e5e9d8baf0edac6f37da032000000000000000000000000000000000000000000000000000000000000006000000000000000000000000000000000000000000000000000000000000000e00000000000000000000000000000000000000000000000000000000000000003f62676f440ff169a3a9afdbf812e89e7f95975ee8e5c31214ffdef631c5f479224a2f504d977eb9364fb9ab406675744e37df5d939547f3f72604b46023b964c0000000000000000000000004cd27786173e00743114202f1455c54e3536cdfb00000000000000000000000000000000000000000000000000000000000000e0000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000000000000000000000000000000000000064ad7954bc000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000047939f42400000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000071727de22e5e9d8baf0edac6f37da03200000000000000000000000000000000000000000000000000000000000000600000000000000000000000000000000000000000000000000000000000000100000000000000000000000000000000000000000000000000000000000000000449628fd1471006c1482da88028e9ce4dbb080b815c9b0344d39e5a8e6ec1419f24a2f504d977eb9364fb9ab406675744e37df5d939547f3f72604b46023b964c0000000000000000000000004cd27786173e00743114202f1455c54e3536cdfb0000000000000000000000007d3201fa7a85c0a5f9fa1c0c6b9d0b784368d2ac000000000000000000000000000000000000000000000000000000000000008000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000045ade7e680000000000000000000000000000000000000000000000000000000000000072000000000000000000000000000000000000000000000000000000000000000010000000100000000000000000000000000000000000000000000000000000000000008200000000000000000010000000100000000000000000000020000000000000000000000000000000000000000000040000000200000000000000000000000000000000800000000000000000000000000000000000000800000000000000040000000000000000000000000000000000000000000000000000000000000000000000000000000400000000000000000000000000000100002000000000000000000400000400001000000000000000000000400000000000000000000000000000000000000000000000000000000080000000800000000000000000080",
        );

        assert_eq!(abi, expected_abi);
    }
}
