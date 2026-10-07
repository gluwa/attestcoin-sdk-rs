use crate::abi::{
    field_mapping::{Chunk, MappedEncodedFields},
    models::QueryableFields,
};

use alloy::{consensus::TxType, dyn_abi::DynSolType};

fn get_mapped_fields_for_type0() -> MappedEncodedFields {
    MappedEncodedFields {
        chunks: vec![
            Chunk::common_fields(),
            Chunk {
                fields: vec![
                    (QueryableFields::TxGasPrice, DynSolType::Uint(128)),
                    (QueryableFields::TxV, DynSolType::Uint(256)),
                    (QueryableFields::TxR, DynSolType::FixedBytes(32)),
                    (QueryableFields::TxS, DynSolType::FixedBytes(32)),
                ],
            },
            Chunk::receipt_fields(),
        ],
    }
}

fn get_mapped_fields_for_type1() -> MappedEncodedFields {
    MappedEncodedFields {
        chunks: vec![
            Chunk::common_fields(),
            Chunk {
                fields: vec![
                    (QueryableFields::TxChainId, DynSolType::Uint(64)),
                    (QueryableFields::TxGasPrice, DynSolType::Uint(128)),
                    (
                        QueryableFields::TxAccessList,
                        DynSolType::Array(
                            DynSolType::Tuple(vec![
                                DynSolType::Address,
                                DynSolType::Array(DynSolType::FixedBytes(32).into()),
                            ])
                            .into(),
                        ),
                    ),
                    (QueryableFields::TxYParity, DynSolType::Uint(8)),
                    (QueryableFields::TxR, DynSolType::FixedBytes(32)),
                    (QueryableFields::TxS, DynSolType::FixedBytes(32)),
                ],
            },
            Chunk::receipt_fields(),
        ],
    }
}

fn get_mapped_fields_for_type2() -> MappedEncodedFields {
    MappedEncodedFields {
        chunks: vec![
            Chunk::common_fields(),
            Chunk {
                fields: vec![
                    (QueryableFields::TxChainId, DynSolType::Uint(64)),
                    (
                        QueryableFields::TxMaxPriorityFeePerGas,
                        DynSolType::Uint(128),
                    ),
                    (QueryableFields::TxMaxFeePerGas, DynSolType::Uint(128)),
                    (
                        QueryableFields::TxAccessList,
                        DynSolType::Array(
                            DynSolType::Tuple(vec![
                                DynSolType::Address,
                                DynSolType::Array(DynSolType::FixedBytes(32).into()),
                            ])
                            .into(),
                        ),
                    ),
                    (QueryableFields::TxYParity, DynSolType::Uint(8)),
                    (QueryableFields::TxR, DynSolType::FixedBytes(32)),
                    (QueryableFields::TxS, DynSolType::FixedBytes(32)),
                ],
            },
            Chunk::receipt_fields(),
        ],
    }
}

fn get_mapped_fields_for_type3() -> MappedEncodedFields {
    MappedEncodedFields {
        chunks: vec![
            Chunk::common_fields(),
            Chunk {
                fields: vec![
                    (QueryableFields::TxChainId, DynSolType::Uint(64)),
                    (
                        QueryableFields::TxMaxPriorityFeePerGas,
                        DynSolType::Uint(128),
                    ),
                    (QueryableFields::TxMaxFeePerGas, DynSolType::Uint(128)),
                    (
                        QueryableFields::TxAccessList,
                        DynSolType::Array(
                            DynSolType::Tuple(vec![
                                DynSolType::Address,
                                DynSolType::Array(DynSolType::FixedBytes(32).into()),
                            ])
                            .into(),
                        ),
                    ),
                ],
            },
            Chunk {
                fields: vec![
                    (QueryableFields::TxMaxFeePerBlobGas, DynSolType::Uint(128)),
                    (
                        QueryableFields::TxBlobVersionedHashes,
                        DynSolType::Array(DynSolType::FixedBytes(32).into()),
                    ),
                    (QueryableFields::TxYParity, DynSolType::Uint(8)),
                    (QueryableFields::TxR, DynSolType::FixedBytes(32)),
                    (QueryableFields::TxS, DynSolType::FixedBytes(32)),
                ],
            },
            Chunk::receipt_fields(),
        ],
    }
}

fn get_mapped_fields_for_type4() -> MappedEncodedFields {
    MappedEncodedFields {
        chunks: vec![
            Chunk::common_fields(),
            Chunk {
                fields: vec![
                    (QueryableFields::TxChainId, DynSolType::Uint(64)),
                    (
                        QueryableFields::TxMaxPriorityFeePerGas,
                        DynSolType::Uint(128),
                    ),
                    (QueryableFields::TxMaxFeePerGas, DynSolType::Uint(128)),
                    (
                        QueryableFields::TxAccessList,
                        DynSolType::Array(
                            DynSolType::Tuple(vec![
                                DynSolType::Address,
                                DynSolType::Array(DynSolType::FixedBytes(32).into()),
                            ])
                            .into(),
                        ),
                    ),
                ],
            },
            Chunk {
                fields: vec![
                    (
                        QueryableFields::TxSignedAuthorizations,
                        // compare to encode_authorization_list
                        DynSolType::Array(
                            DynSolType::Tuple(vec![
                                DynSolType::Uint(256),
                                DynSolType::Address,
                                DynSolType::Uint(64),
                                DynSolType::Uint(8),
                                DynSolType::Uint(256),
                                DynSolType::Uint(256),
                            ])
                            .into(),
                        ),
                    ),
                    (QueryableFields::TxYParity, DynSolType::Uint(8)),
                    (QueryableFields::TxR, DynSolType::FixedBytes(32)),
                    (QueryableFields::TxS, DynSolType::FixedBytes(32)),
                ],
            },
            Chunk::receipt_fields(),
        ],
    }
}

pub(super) fn get_mapped_field_for_type(transaction_type: TxType) -> MappedEncodedFields {
    match transaction_type {
        TxType::Legacy => get_mapped_fields_for_type0(),
        TxType::Eip2930 => get_mapped_fields_for_type1(),
        TxType::Eip1559 => get_mapped_fields_for_type2(),
        TxType::Eip4844 => get_mapped_fields_for_type3(),
        TxType::Eip7702 => get_mapped_fields_for_type4(),
    }
}
