use alloy::consensus::Transaction as _;
use alloy::eips::eip2930::AccessList;
use alloy::rpc::types::eth::transaction::Transaction;
use alloy::{
    primitives::{Address, B256, U256},
    providers::{Provider, ProviderBuilder},
    rpc::types::TransactionReceipt,
};
use async_trait::async_trait;
use attestcoin_abi_encoding::common::{compute_v, compute_y_parity};
use std::str::FromStr;

use crate::abi::models::QueryBuilderError;
use crate::abi::query_builder::AbiProvider;

pub enum ResultField {
    TxType(u8),
    TxChainId(u64),
    TxNonce(u64),
    TxGasPrice(u128),
    TxMaxPriorityFeePerGas(u128),
    TxMaxFeePerGas(u128),
    TxGasLimit(u64),
    EthAddress(Address),
    TxValue(U256),
    TxAccessList(AccessList),
    TxV(U256),
    TxYParity(u8),
    TxR([u8; 32]),
    TxS([u8; 32]),
    RxStatus(u8),
    RxGasUsed(u64),
    RxLogBlooms(Vec<u8>),
    EventTopic([u8; 32]),
    EventDataField([u8; 32]),
    FunctionSignifier([u8; 4]),
    CallDataField([u8; 32]),
}

pub async fn get_transaction_and_receipt(tx_hash_str: &str) -> (Transaction, TransactionReceipt) {
    // RPC.
    let rpc_url = "https://sepolia-proxy-rpc.creditcoin.network";
    let provider = ProviderBuilder::new().connect_http(rpc_url.parse().unwrap());

    // which transaction.
    let tx_hash = B256::from_str(tx_hash_str).unwrap();

    // get the transaction & receipt.
    let tx = provider
        .get_transaction_by_hash(tx_hash)
        .await
        .unwrap()
        .unwrap();
    let rx = provider
        .get_transaction_receipt(tx_hash)
        .await
        .unwrap()
        .unwrap();
    (tx, rx)
}

pub fn check_results(
    expected_results: Vec<ResultField>,
    result_segments: Vec<(usize, usize)>,
    abi: Vec<u8>,
) {
    assert_eq!(
        expected_results.len(),
        result_segments.len(),
        "Number of expected results doesn't match segment count. Expected results: {}, Result segments: {}",
        expected_results.len(),
        result_segments.len()
    );

    for (field_number, (expected, (offset, size))) in
        expected_results.iter().zip(result_segments).enumerate()
    {
        // Pad expected result according to type
        let expected_padded: Vec<u8> = match expected {
            // All cases where 1 byte is padded to 32 bytes
            ResultField::TxType(value)
            | ResultField::TxYParity(value)
            | ResultField::RxStatus(value) => {
                let mut value_padded: Vec<u8> = vec![0; 31];
                value_padded.push(*value);
                value_padded
            }
            // All cases where 8 bytes are padded to 32 bytes
            ResultField::RxGasUsed(value)
            | ResultField::TxNonce(value)
            | ResultField::TxChainId(value)
            | ResultField::TxGasLimit(value) => {
                let mut value_padded: Vec<u8> = vec![0; 24];
                value_padded.append(&mut Vec::from(value.to_be_bytes()));
                value_padded
            }
            // All cases where 16 bytes are padded to 32 bytes
            ResultField::TxGasPrice(value)
            | ResultField::TxMaxPriorityFeePerGas(value)
            | ResultField::TxMaxFeePerGas(value) => {
                let mut value_padded: Vec<u8> = vec![0; 16];
                value_padded.append(&mut Vec::from(value.to_be_bytes()));
                value_padded
            }
            ResultField::EthAddress(address) => {
                let mut address_padded: Vec<u8> = vec![0; 12];
                address_padded.append(&mut Vec::from(address.0.0));
                address_padded
            }
            ResultField::TxValue(value) => value.to_be_bytes_vec(),
            ResultField::TxAccessList(_list) => {
                //TODO: figure out how access list should look when encoded
                vec![]
            }
            ResultField::TxV(v) => v.to_be_bytes_vec(),
            ResultField::TxR(r) => Vec::from(r),
            ResultField::TxS(s) => Vec::from(s),
            ResultField::RxLogBlooms(blooms) => blooms.clone(),
            ResultField::EventTopic(topic) => Vec::from(topic),
            ResultField::EventDataField(field) => Vec::from(field),
            ResultField::FunctionSignifier(signifier) => Vec::from(signifier),
            ResultField::CallDataField(field) => Vec::from(field),
        };
        // Get segment bytes from abi
        let segment_bytes = &abi[offset..offset + size];
        // Compare, printing which field number and type on error.
        // TODO: Possibly allow field names to be added to ResultFields, just to make errors more intelligible.
        assert_eq!(
            &expected_padded, segment_bytes,
            "Expected and actual didn't match. Field num: {field_number}, Expected: {expected_padded:?}, Actual: {segment_bytes:?}",
        );
    }
}

pub fn get_vrs(tx: &Transaction) -> (U256, [u8; 32], [u8; 32]) {
    let signed_tx = tx
        .inner
        .as_legacy()
        .expect("Already checked that tx is legacy");
    let signature = signed_tx.signature();
    let chain_id = tx.chain_id();
    let v = compute_v(signature, chain_id);
    let r: [u8; 32] = signature.r().to_be_bytes::<32>()[0..32].try_into().unwrap();
    let s: [u8; 32] = signature.s().to_be_bytes::<32>()[0..32].try_into().unwrap();
    (v, r, s)
}

pub fn get_y_parity(tx: &Transaction) -> u8 {
    let signature = tx.inner.signature();
    compute_y_parity(signature)
}

pub struct TestAbiProvider;

#[async_trait]
impl AbiProvider for TestAbiProvider {
    async fn get_abi(&self, _contract_address: String) -> Result<String, QueryBuilderError> {
        // hard coded G-CRE's ABI
        let json_str = r#"[{"constant":false,"inputs":[{"name":"tokenHolders","type":"address[]"},{"name":"amounts","type":"uint256[]"}],"name":"recordSales730Days","outputs":[],"payable":false,"stateMutability":"nonpayable","type":"function"},{"constant":true,"inputs":[],"name":"VestingStartDate","outputs":[{"name":"","type":"uint256"}],"payable":false,"stateMutability":"view","type":"function"},{"constant":true,"inputs":[],"name":"name","outputs":[{"name":"","type":"string"}],"payable":false,"stateMutability":"view","type":"function"},{"constant":false,"inputs":[{"name":"spender","type":"address"},{"name":"value","type":"uint256"}],"name":"approve","outputs":[{"name":"success","type":"bool"}],"payable":false,"stateMutability":"nonpayable","type":"function"},{"constant":true,"inputs":[{"name":"tokenHolder","type":"address"}],"name":"vestedBalanceOf","outputs":[{"name":"balance","type":"uint256"}],"payable":false,"stateMutability":"view","type":"function"},{"constant":true,"inputs":[],"name":"totalSupply","outputs":[{"name":"amount","type":"uint256"}],"payable":false,"stateMutability":"view","type":"function"},{"constant":true,"inputs":[{"name":"tokenHolder","type":"address"}],"name":"purchasedBalanceOf365Days","outputs":[{"name":"balance","type":"uint256"}],"payable":false,"stateMutability":"view","type":"function"},{"constant":false,"inputs":[{"name":"value","type":"uint256"},{"name":"sighash","type":"string"}],"name":"exchange","outputs":[{"name":"success","type":"bool"}],"payable":false,"stateMutability":"nonpayable","type":"function"},{"constant":false,"inputs":[{"name":"from","type":"address"},{"name":"to","type":"address"},{"name":"value","type":"uint256"}],"name":"transferFrom","outputs":[{"name":"success","type":"bool"}],"payable":false,"stateMutability":"nonpayable","type":"function"},{"constant":true,"inputs":[{"name":"tokenHolder","type":"address"}],"name":"vestedBalanceOf183Days","outputs":[{"name":"balance","type":"uint256"}],"payable":false,"stateMutability":"view","type":"function"},{"constant":true,"inputs":[],"name":"decimals","outputs":[{"name":"","type":"uint8"}],"payable":false,"stateMutability":"view","type":"function"},{"constant":false,"inputs":[{"name":"tokenHolders","type":"address[]"},{"name":"amounts","type":"uint256[]"}],"name":"recordSales1095Days","outputs":[],"payable":false,"stateMutability":"nonpayable","type":"function"},{"constant":true,"inputs":[{"name":"tokenHolder","type":"address"}],"name":"vestedBalanceOf365Days","outputs":[{"name":"balance","type":"uint256"}],"payable":false,"stateMutability":"view","type":"function"},{"constant":false,"inputs":[{"name":"value","type":"uint256"}],"name":"burn","outputs":[{"name":"success","type":"bool"}],"payable":false,"stateMutability":"nonpayable","type":"function"},{"constant":true,"inputs":[{"name":"tokenHolder","type":"address"}],"name":"purchasedBalanceOf2190Days","outputs":[{"name":"balance","type":"uint256"}],"payable":false,"stateMutability":"view","type":"function"},{"constant":false,"inputs":[{"name":"tokenHolders","type":"address[]"},{"name":"amounts","type":"uint256[]"}],"name":"recordSales183Days","outputs":[],"payable":false,"stateMutability":"nonpayable","type":"function"},{"constant":false,"inputs":[{"name":"tokenHolder","type":"address"},{"name":"numCoins","type":"uint256"}],"name":"recordSale365Days","outputs":[],"payable":false,"stateMutability":"nonpayable","type":"function"},{"constant":true,"inputs":[{"name":"tokenHolder","type":"address"}],"name":"vestedBalanceOf730Days","outputs":[{"name":"balance","type":"uint256"}],"payable":false,"stateMutability":"view","type":"function"},{"constant":true,"inputs":[{"name":"tokenHolder","type":"address"}],"name":"purchasedBalanceOf","outputs":[{"name":"balance","type":"uint256"}],"payable":false,"stateMutability":"view","type":"function"},{"constant":true,"inputs":[{"name":"owner","type":"address"}],"name":"balanceOf","outputs":[{"name":"balance","type":"uint256"}],"payable":false,"stateMutability":"view","type":"function"},{"constant":false,"inputs":[],"name":"finalizeSales","outputs":[],"payable":false,"stateMutability":"nonpayable","type":"function"},{"constant":false,"inputs":[{"name":"from","type":"address"},{"name":"value","type":"uint256"}],"name":"burnFrom","outputs":[{"name":"success","type":"bool"}],"payable":false,"stateMutability":"nonpayable","type":"function"},{"constant":true,"inputs":[{"name":"tokenHolder","type":"address"}],"name":"vestedBalanceOf2190Days","outputs":[{"name":"balance","type":"uint256"}],"payable":false,"stateMutability":"view","type":"function"},{"constant":true,"inputs":[{"name":"tokenHolder","type":"address"}],"name":"purchasedBalanceOf730Days","outputs":[{"name":"balance","type":"uint256"}],"payable":false,"stateMutability":"view","type":"function"},{"constant":true,"inputs":[],"name":"symbol","outputs":[{"name":"","type":"string"}],"payable":false,"stateMutability":"view","type":"function"},{"constant":false,"inputs":[{"name":"tokenHolder","type":"address"},{"name":"numCoins","type":"uint256"}],"name":"recordSale183Days","outputs":[],"payable":false,"stateMutability":"nonpayable","type":"function"},{"constant":false,"inputs":[{"name":"to","type":"address"},{"name":"value","type":"uint256"}],"name":"transfer","outputs":[{"name":"success","type":"bool"}],"payable":false,"stateMutability":"nonpayable","type":"function"},{"constant":true,"inputs":[],"name":"creditcoinSalesLimit","outputs":[{"name":"","type":"uint256"}],"payable":false,"stateMutability":"view","type":"function"},{"constant":true,"inputs":[{"name":"tokenHolder","type":"address"}],"name":"vestedBalanceOf1095Days","outputs":[{"name":"balance","type":"uint256"}],"payable":false,"stateMutability":"view","type":"function"},{"constant":true,"inputs":[],"name":"creditcoinLimitInFrac","outputs":[{"name":"","type":"uint256"}],"payable":false,"stateMutability":"view","type":"function"},{"constant":false,"inputs":[{"name":"tokenHolder","type":"address"},{"name":"numCoins","type":"uint256"}],"name":"recordSale2190Days","outputs":[],"payable":false,"stateMutability":"nonpayable","type":"function"},{"constant":false,"inputs":[{"name":"tokenHolder","type":"address"},{"name":"numCoins","type":"uint256"}],"name":"recordSale730Days","outputs":[],"payable":false,"stateMutability":"nonpayable","type":"function"},{"constant":true,"inputs":[{"name":"tokenHolder","type":"address"}],"name":"purchasedBalanceOf1095Days","outputs":[{"name":"balance","type":"uint256"}],"payable":false,"stateMutability":"view","type":"function"},{"constant":true,"inputs":[{"name":"owner","type":"address"},{"name":"spender","type":"address"}],"name":"allowance","outputs":[{"name":"remaining","type":"uint256"}],"payable":false,"stateMutability":"view","type":"function"},{"constant":false,"inputs":[],"name":"startVesting","outputs":[],"payable":false,"stateMutability":"nonpayable","type":"function"},{"constant":false,"inputs":[{"name":"tokenHolders","type":"address[]"},{"name":"amounts","type":"uint256[]"}],"name":"recordSales2190Days","outputs":[],"payable":false,"stateMutability":"nonpayable","type":"function"},{"constant":true,"inputs":[{"name":"tokenHolder","type":"address"}],"name":"purchasedBalanceOf183Days","outputs":[{"name":"balance","type":"uint256"}],"payable":false,"stateMutability":"view","type":"function"},{"constant":true,"inputs":[],"name":"IsSalesFinalized","outputs":[{"name":"","type":"bool"}],"payable":false,"stateMutability":"view","type":"function"},{"constant":false,"inputs":[{"name":"tokenHolders","type":"address[]"},{"name":"amounts","type":"uint256[]"}],"name":"recordSales365Days","outputs":[],"payable":false,"stateMutability":"nonpayable","type":"function"},{"constant":false,"inputs":[{"name":"tokenHolder","type":"address"},{"name":"numCoins","type":"uint256"}],"name":"recordSale1095Days","outputs":[],"payable":false,"stateMutability":"nonpayable","type":"function"},{"inputs":[{"name":"creditcoinFoundation","type":"address"},{"name":"devCost","type":"address"}],"payable":false,"stateMutability":"nonpayable","type":"constructor"},{"payable":true,"stateMutability":"payable","type":"fallback"},{"anonymous":false,"inputs":[{"indexed":true,"name":"from","type":"address"},{"indexed":false,"name":"value","type":"uint256"},{"indexed":true,"name":"sighash","type":"string"}],"name":"Exchange","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"name":"from","type":"address"},{"indexed":false,"name":"value","type":"uint256"}],"name":"Burnt","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"name":"from","type":"address"},{"indexed":true,"name":"to","type":"address"},{"indexed":false,"name":"value","type":"uint256"}],"name":"Transfer","type":"event"},{"anonymous":false,"inputs":[{"indexed":true,"name":"owner","type":"address"},{"indexed":true,"name":"spender","type":"address"},{"indexed":false,"name":"value","type":"uint256"}],"name":"Approval","type":"event"}]"#;

        Ok(json_str.into())
    }
}
