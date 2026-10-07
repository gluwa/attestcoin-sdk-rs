use alloy::{dyn_abi::DynSolType, rpc::types::Log};
use alloy_json_abi::{Event, Function};

#[derive(Debug, Clone)]
pub struct FieldMetadata {
    pub sol_type: DynSolType,
    pub offset: usize,
    pub size: Option<usize>,
    pub is_dynamic: bool,
    pub value: Option<Vec<u8>>,
    pub children: Vec<FieldMetadata>,
}

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub enum QueryableFields {
    Type,
    TxChainId,
    TxNonce,
    TxGasPrice,
    TxGasLimit,
    TxFrom,
    TxToIsNull,
    TxTo,
    TxValue,
    TxData,
    TxV,
    TxR,
    TxS,
    TxYParity,
    TxAccessList,
    TxMaxPriorityFeePerGas,
    TxMaxFeePerGas,
    TxMaxFeePerBlobGas,
    TxBlobVersionedHashes,
    RxStatus,
    RxGasUsed,
    RxLogBlooms,
    RxLogs,
    TxSignedAuthorizations,
}

#[derive(Debug)]
pub enum QueryBuilderError {
    FailedToAbiEncode,
    FailedToComputeOffsets,
    MissMatchedLengthDecoding,
    FieldNotPresentInTx,
    FieldIsNotStatic,
    EventSignatureNameProvidedIsNotValidHex,
    FunctionSignatureNameProvidedIsNotValidHex,
    AbiProviderNotInitialized,
    NoAbiFoundForContract(String),
    ContractAbiRetrievalFailed {
        contract_addr: String,
        error_message: String,
    },
    FailedToParseAbi(String, String),
    FailedToFindEventByNameOrSignature(String),
    FailedToDecodeLog(Box<Log>),
    AmbigiousEventMatch(String),
    FailedToFindRxLogsField,
    FailedToFindTxDataField,
    MissingLogInAbiOffsets(usize),
    MissingDataInAbiOffsets,
    TryingToGetSizeOfDynamicType,
    FailedToGetEventDataOffsets(Box<Log>),
    RequestingFunctionArgumentOfAnEmptyCalldataTransaction,
    RequestingFunctionArgumentButNoToAddressPresent,
    FailedToFindFunctionByNameOrSignature(String),
    AmbigiousFunctionMatch(Vec<Function>),
    DataFieldMissingSize,
    DataFieldNotLongEnoughForSignatureExtraction,
    CannotFindArgumentInFunction(Function, String),
    FailedToResolveSolTypesOfMatchedFunction(Function),
    FailedToResolveSolTypesOfMatchedEvent(Event),
    FailedToComputeOffsetsForCalldata,
    MissingDataInCalldataOffsets,
}
