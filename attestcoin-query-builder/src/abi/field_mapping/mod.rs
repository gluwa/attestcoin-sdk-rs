use crate::abi::models::QueryableFields;

use alloy::{consensus::TxType, dyn_abi::DynSolType};
use attestcoin_abi_encoding::common::EncodingVersion;

mod v1;

pub type Field = (QueryableFields, DynSolType);

pub struct Chunk {
    pub fields: Vec<Field>,
}

impl Chunk {
    pub fn common_fields() -> Self {
        Self {
            fields: vec![
                (QueryableFields::TxNonce, DynSolType::Uint(64)),
                (QueryableFields::TxGasLimit, DynSolType::Uint(64)),
                (QueryableFields::TxFrom, DynSolType::Address),
                (QueryableFields::TxToIsNull, DynSolType::Bool),
                (QueryableFields::TxTo, DynSolType::Address),
                (QueryableFields::TxValue, DynSolType::Uint(256)),
                (QueryableFields::TxData, DynSolType::Bytes),
            ],
        }
    }

    pub fn receipt_fields() -> Self {
        Self {
            fields: vec![
                (QueryableFields::RxStatus, DynSolType::Uint(8)),
                (QueryableFields::RxGasUsed, DynSolType::Uint(64)),
                (
                    QueryableFields::RxLogs,
                    DynSolType::Array(
                        DynSolType::Tuple(vec![
                            DynSolType::Address,
                            DynSolType::Array(DynSolType::FixedBytes(32).into()),
                            DynSolType::Bytes,
                        ])
                        .into(),
                    ),
                ),
                (QueryableFields::RxLogBlooms, DynSolType::Bytes),
            ],
        }
    }

    pub fn get_fields(&self) -> Vec<QueryableFields> {
        self.fields.iter().map(|field| field.0.clone()).collect()
    }

    pub fn get_types(&self) -> Vec<DynSolType> {
        self.fields.iter().map(|field| field.1.clone()).collect()
    }

    pub fn get_type(&self) -> DynSolType {
        DynSolType::Tuple(self.fields.iter().map(|field| field.1.clone()).collect())
    }
}

pub struct MappedEncodedFields {
    pub chunks: Vec<Chunk>,
}

impl MappedEncodedFields {
    pub fn get_all_fields(&self) -> Vec<QueryableFields> {
        let mut all_fields = Vec::new();
        for chunk in &self.chunks {
            for field in &chunk.fields {
                all_fields.push(field.0.clone());
            }
        }
        all_fields
    }

    pub fn get_all_types(&self) -> Vec<DynSolType> {
        let mut all_types = Vec::new();
        for chunk in &self.chunks {
            for field in &chunk.fields {
                all_types.push(field.1.clone());
            }
        }
        all_types
    }
}

pub fn get_all_fields_for_transaction(
    tx_type: TxType,
    encoding: EncodingVersion,
) -> MappedEncodedFields {
    match encoding {
        EncodingVersion::V1 => v1::get_mapped_field_for_type(tx_type),
    }
}
