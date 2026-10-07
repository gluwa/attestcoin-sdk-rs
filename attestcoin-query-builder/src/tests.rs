use crate::{
    abi::{models::QueryableFields, query_builder::QueryBuilder},
    test_helpers::{
        ResultField, TestAbiProvider, check_results, get_transaction_and_receipt, get_vrs,
        get_y_parity,
    },
};

use alloy::consensus::Transaction;
use attestcoin_abi_encoding::{abi::abi_encode, common::EncodingVersion};

const ENCODING: EncodingVersion = EncodingVersion::V1;

// Tx/Rx Fields queried in this test: All legacy (type 0) fields except Tx Data
// See abi_encoding_mapping.rs for details
#[tokio::test]
async fn legacy_tx_queried_fields_match_expected() {
    // Get legacy transaction via rpc
    let (tx, rx) = get_transaction_and_receipt(
        "0xc990ce703dd3ca83429c302118f197651678de359c271f205b9083d4aa333aae",
    )
    .await;
    assert!(tx.inner.is_legacy());

    // Encode transaction
    let encoded = abi_encode(tx.clone(), rx.clone(), ENCODING).unwrap();

    let mut query_builder = QueryBuilder::create_from_transaction(tx.clone(), rx.clone(), ENCODING)
        .expect("creating queryable builder should work");

    query_builder.set_abi_provider(Box::new(TestAbiProvider));

    query_builder
        .add_static_field(QueryableFields::Type)
        .unwrap();
    query_builder
        .add_static_field(QueryableFields::TxNonce)
        .unwrap();
    query_builder
        .add_static_field(QueryableFields::TxGasPrice)
        .unwrap();
    query_builder
        .add_static_field(QueryableFields::TxGasLimit)
        .unwrap();
    query_builder
        .add_static_field(QueryableFields::TxFrom)
        .unwrap();
    query_builder
        .add_static_field(QueryableFields::TxTo)
        .unwrap();
    query_builder
        .add_static_field(QueryableFields::TxValue)
        .unwrap();
    query_builder
        .add_static_field(QueryableFields::TxV)
        .unwrap();
    query_builder
        .add_static_field(QueryableFields::TxR)
        .unwrap();
    query_builder
        .add_static_field(QueryableFields::TxS)
        .unwrap();

    let selected_offsets = query_builder.get_selected_offsets();
    let raw = encoded.abi().to_vec();

    let (v, r, s) = get_vrs(&tx);

    let expected_results: Vec<ResultField> = vec![
        ResultField::TxType(tx.inner.tx_type().into()),
        ResultField::TxNonce(tx.nonce()),
        ResultField::TxGasPrice(tx.gas_price().expect("Legacy tx should have this")),
        ResultField::TxGasLimit(tx.gas_limit()),
        ResultField::EthAddress(tx.inner.signer()), // Caller address
        ResultField::EthAddress(tx.to().expect("Should be to field in contract call")), // Contract address in call
        ResultField::TxValue(tx.value()),
        ResultField::TxV(v),
        ResultField::TxR(r),
        ResultField::TxS(s),
    ];

    // Checking that all result data matches expected
    check_results(expected_results, selected_offsets, raw.clone());
}

// Tx/Rx Fields queried in this test:
// - Rx Status
// - Rx Gas Used
// - Rx Log Blooms
#[tokio::test]
async fn queried_receipt_fields_match_expected() {
    // Get legacy transaction via rpc
    let (tx, rx) = get_transaction_and_receipt(
        "0xc990ce703dd3ca83429c302118f197651678de359c271f205b9083d4aa333aae",
    )
    .await;
    assert!(tx.inner.is_legacy());

    // Encode transaction
    let encoded = abi_encode(tx.clone(), rx.clone(), ENCODING).unwrap();

    let mut query_builder = QueryBuilder::create_from_transaction(tx.clone(), rx.clone(), ENCODING)
        .expect("creating queryable builder should work");
    query_builder.set_abi_provider(Box::new(TestAbiProvider));

    query_builder
        .add_static_field(QueryableFields::RxStatus)
        .unwrap();
    query_builder
        .add_static_field(QueryableFields::RxGasUsed)
        .unwrap();
    query_builder
        .add_static_field(QueryableFields::RxLogBlooms)
        .unwrap();

    let selected_offsets = query_builder.get_selected_offsets();
    let raw = encoded.abi().to_vec();
    let expected_results: Vec<ResultField> = vec![
        ResultField::RxStatus(rx.status() as u8),
        ResultField::RxGasUsed(rx.gas_used),
        ResultField::RxLogBlooms(rx.inner.logs_bloom().0.to_vec()),
    ];

    // Checking that all result data matches expected
    check_results(expected_results, selected_offsets, raw.clone());
}

// Tx/Rx Fields queried in this test:
// - Log 1 (Transfer event):
//     - Event addr (contract addr)
//     - Event index 0 (signature)
//     - Event index 1 (from address)
//     - Event index 2 (to, burn address)
//     - Event data field 0 (burned amount)
#[tokio::test]
async fn event_builder_queried_fields_match_expected() {
    // Get legacy transaction via rpc
    let (tx, rx) = get_transaction_and_receipt(
        "0xc990ce703dd3ca83429c302118f197651678de359c271f205b9083d4aa333aae",
    )
    .await;
    assert!(tx.inner.is_legacy());

    // Encode transaction
    let encoded = abi_encode(tx.clone(), rx.clone(), ENCODING).unwrap();

    println!("Encoded ABI: {:?}", encoded.abi());

    let mut query_builder = QueryBuilder::create_from_transaction(tx.clone(), rx.clone(), ENCODING)
        .expect("creating queryable builder should work");
    query_builder.set_abi_provider(Box::new(TestAbiProvider));

    // just to keep it simple for now
    // i'll just say i care about the event index 1
    // which is the transfer event from, to (zero address), amount
    query_builder
        .event_builder(
            "Transfer".into(),
            |_log, _event, log_index| log_index == 1,
            false, // We only want the transfer at log index 1. There has been a catastrophic error if we find more than one log at that index
            |builder| {
                // fun no? :)
                builder
                    .add_address()
                    .unwrap()
                    .add_signature()
                    .unwrap()
                    .add_argument("from")
                    .unwrap()
                    .add_argument("to")
                    .unwrap()
                    .add_argument("value")
                    .unwrap();

                Ok(())
            },
        )
        .await
        .expect("should have matched an event and constructed offsets");

    let selected_offsets = query_builder.get_selected_offsets();
    let raw = encoded.abi().to_vec();

    let expected_results: Vec<ResultField> = vec![
        ResultField::EthAddress(rx.inner.logs()[1].address()), // Contract address in event
        ResultField::EventTopic(rx.inner.logs()[1].topic0().unwrap().0), // Event signature
        ResultField::EventTopic(rx.inner.logs()[1].topics()[1].0), // Event from address
        ResultField::EventTopic(rx.inner.logs()[1].topics()[2].0), // Event to address
        ResultField::EventDataField(
            rx.inner.logs()[1].data().data[..]
                .try_into()
                .expect("Data should contain 1 32 byte field for this transaction"),
        ), // Event value
    ];

    // Checking that all result data matches expected
    check_results(expected_results, selected_offsets, raw.clone());
}

// Tx/Rx Fields queried in this test:
// - Call Data:
//     - Function signature
//     - data field 0 (burned amount)
#[tokio::test]
async fn function_builder_queried_fields_match_expected() {
    // Get legacy transaction via rpc
    let (tx, rx) = get_transaction_and_receipt(
        "0xc990ce703dd3ca83429c302118f197651678de359c271f205b9083d4aa333aae",
    )
    .await;
    assert!(tx.inner.is_legacy());

    // Encode transaction
    let encoded = abi_encode(tx.clone(), rx.clone(), ENCODING).unwrap();

    let mut query_builder = QueryBuilder::create_from_transaction(tx.clone(), rx.clone(), ENCODING)
        .expect("creating queryable builder should work");
    query_builder.set_abi_provider(Box::new(TestAbiProvider));

    query_builder
        .function_builder("burn".into(), |b| {
            b.add_signature()
                .unwrap()
                .add_argument("value".into())
                .unwrap();
            Ok(())
        })
        .await
        .expect("should be able to query some calldata segments");

    let selected_offsets = query_builder.get_selected_offsets();
    let raw = encoded.abi().to_vec();

    let expected_results: Vec<ResultField> = vec![
        ResultField::FunctionSignifier(
            tx.inner.as_legacy().unwrap().tx().input[0..4]
                .try_into()
                .expect("4 bytes is right slice length"),
        ),
        ResultField::CallDataField(
            tx.inner.as_legacy().unwrap().tx().input[4..36]
                .try_into()
                .expect("32 bytes is right len"),
        ), // Burn value in function call
    ];

    // Checking that all result data matches expected
    check_results(expected_results, selected_offsets, raw.clone());
}

// Tx/Rx Fields queried in this test: All fields newly added in type 1
// - Tx Chain Id
// - Tx y parity
#[tokio::test]
async fn type_1_tx_queried_fields_match_expected() {
    // Get legacy transaction via rpc
    let (tx, rx) = get_transaction_and_receipt(
        "0x5c8c6d8c61bd8109ce02717db62b12554c097d156b66e30ff64864b5d4b1c041",
    )
    .await;
    assert!(tx.inner.is_eip2930());

    // Encode transaction
    let encoded = abi_encode(tx.clone(), rx.clone(), ENCODING).unwrap();

    let mut query_builder = QueryBuilder::create_from_transaction(tx.clone(), rx.clone(), ENCODING)
        .expect("creating queryable builder should work");
    query_builder.set_abi_provider(Box::new(TestAbiProvider));

    query_builder
        .add_static_field(QueryableFields::TxChainId)
        .unwrap();
    query_builder
        .add_static_field(QueryableFields::TxYParity)
        .unwrap();

    let selected_offsets = query_builder.get_selected_offsets();
    let raw = encoded.abi().to_vec();

    let expected_results: Vec<ResultField> = vec![
        ResultField::TxChainId(tx.chain_id().unwrap()),
        ResultField::TxYParity(get_y_parity(&tx)),
    ];

    // Checking that all result data matches expected
    check_results(expected_results, selected_offsets, raw.clone());
}

// Tx/Rx Fields queried in this test: All fields newly added in type 2
// - TxMaxPriorityFeePerGas
// - TxMaxFeePerGas
// - TxAccessList
#[tokio::test]
async fn type_2_tx_queried_fields_match_expected() {
    // Get legacy transaction via rpc
    let (tx, rx) = get_transaction_and_receipt(
        "0xdfba59b94bac3da5af5d0fa8b81ae3199069fa6f38002be58c14e94a051e0642",
    )
    .await;
    assert!(tx.inner.is_eip1559());

    // Encode transaction
    let encoded = abi_encode(tx.clone(), rx.clone(), ENCODING).unwrap();

    let mut query_builder = QueryBuilder::create_from_transaction(tx.clone(), rx.clone(), ENCODING)
        .expect("creating queryable builder should work");
    query_builder.set_abi_provider(Box::new(TestAbiProvider));

    query_builder
        .add_static_field(QueryableFields::TxMaxPriorityFeePerGas)
        .unwrap();
    query_builder
        .add_static_field(QueryableFields::TxMaxFeePerGas)
        .unwrap();
    //query_builder
    //    .add_static_field(QueryableFields::TxAccessList)
    //    .unwrap();

    let selected_offsets = query_builder.get_selected_offsets();
    let raw = encoded.abi().to_vec();

    let expected_results: Vec<ResultField> = vec![
        ResultField::TxMaxPriorityFeePerGas(
            tx.max_priority_fee_per_gas()
                .expect("max_priority_fee_per_gas should not be None"),
        ),
        ResultField::TxMaxFeePerGas(tx.max_fee_per_gas()),
        // TODO: Properly test access list
        //ResultField::TxAccessList(tx.access_list().unwrap()),
    ];

    // Checking that all result data matches expected
    check_results(expected_results, selected_offsets, raw.clone());
}

// There are no new fields in type 3 transactions for which we want to support bridging.
// We instead make sure the query builder works for some type 2 transaction fields.
// - TxMaxPriorityFeePerGas
// - TxMaxFeePerGas
#[tokio::test]
async fn type_3_tx_queried_fields_match_expected() {
    // Get legacy transaction via rpc
    let (tx, rx) = get_transaction_and_receipt(
        "0x085d2fe01372711005b053a1b0d081c13cde19b6ddb77cae847e0d11a0a0cafe",
    )
    .await;
    assert!(tx.inner.is_eip4844());

    // Encode transaction
    let encoded = abi_encode(tx.clone(), rx.clone(), ENCODING).unwrap();

    let mut query_builder = QueryBuilder::create_from_transaction(tx.clone(), rx.clone(), ENCODING)
        .expect("creating queryable builder should work");
    query_builder.set_abi_provider(Box::new(TestAbiProvider));

    query_builder
        .add_static_field(QueryableFields::TxMaxPriorityFeePerGas)
        .unwrap();
    query_builder
        .add_static_field(QueryableFields::TxMaxFeePerGas)
        .unwrap();

    let selected_offsets = query_builder.get_selected_offsets();
    let raw = encoded.abi().to_vec();

    let expected_results: Vec<ResultField> = vec![
        ResultField::TxMaxPriorityFeePerGas(
            tx.max_priority_fee_per_gas()
                .expect("max_priority_fee_per_gas should not be None"),
        ),
        ResultField::TxMaxFeePerGas(tx.max_fee_per_gas()),
    ];

    // Checking that all result data matches expected
    check_results(expected_results, selected_offsets, raw.clone());
}

#[tokio::test]
async fn type_4_tx_queried_fields_match_expected() {
    // Get legacy transaction via rpc
    let (tx, rx) = get_transaction_and_receipt(
        "0x2dce846c932bcf50a9dd180e61a09818009da734f2e6761cf9e825f19077f05a",
    )
    .await;
    assert!(tx.inner.is_eip7702());

    // Encode transaction
    let encoded = abi_encode(tx.clone(), rx.clone(), ENCODING).unwrap();

    let mut query_builder = QueryBuilder::create_from_transaction(tx.clone(), rx.clone(), ENCODING)
        .expect("creating queryable builder should work");
    query_builder.set_abi_provider(Box::new(TestAbiProvider));

    //query_builder
    //    .add_static_field(QueryableFields::TxAuthorizationList)
    //    .unwrap();

    let selected_offsets = query_builder.get_selected_offsets();
    let raw = encoded.abi().to_vec();

    let expected_results: Vec<ResultField> = vec![
        // TODO: Properly test access list
        //ResultField::TxAccessList(tx.authorization_list().unwrap()),
    ];

    // Checking that all result data matches expected
    check_results(expected_results, selected_offsets, raw.clone());
}
