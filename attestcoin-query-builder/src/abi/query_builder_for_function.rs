use alloy::{consensus::Transaction as _, dyn_abi::Specifier, rpc::types::Transaction};
use alloy_json_abi::Function;

use crate::abi::utils::compute_abi_offsets;

use super::models::{FieldMetadata, QueryBuilderError};

const FUNCTION_SIGNATURE_SIZE: usize = 4;

pub struct QueryBuilderForFunction {
    selected_offsets: Vec<(usize, usize)>,
    matched_function: Function,
    tx: Transaction,
    data_field: FieldMetadata,
}

impl QueryBuilderForFunction {
    pub(crate) fn new(
        matched_function: Function,
        tx: Transaction,
        data_field: FieldMetadata,
    ) -> Self {
        Self {
            selected_offsets: vec![],
            matched_function,
            tx,
            data_field,
        }
    }

    pub fn get_selected_offsets(self) -> Vec<(usize, usize)> {
        self.selected_offsets.clone()
    }

    pub fn add_signature(&mut self) -> Result<&mut Self, QueryBuilderError> {
        if let Some(size) = self.data_field.size {
            if size >= FUNCTION_SIGNATURE_SIZE {
                self.selected_offsets
                    .push((self.data_field.offset, FUNCTION_SIGNATURE_SIZE));
                Ok(self)
            } else {
                Err(QueryBuilderError::DataFieldNotLongEnoughForSignatureExtraction)
            }
        } else {
            Err(QueryBuilderError::DataFieldMissingSize)
        }
    }

    pub fn add_argument(&mut self, name: String) -> Result<&mut Self, QueryBuilderError> {
        let mut found_argument_index: Option<usize> = None;
        for (argument_index, argument) in self.matched_function.inputs.iter().enumerate() {
            if argument.name().eq(&name) {
                found_argument_index = Some(argument_index);
                break;
            }
        }

        let data_size = match self.data_field.size {
            Some(s) => s,
            None => return Err(QueryBuilderError::DataFieldMissingSize),
        };

        let matched_argument_index = match found_argument_index {
            Some(ma) => ma,
            None => {
                return Err(QueryBuilderError::CannotFindArgumentInFunction(
                    self.matched_function.clone(),
                    name,
                ));
            }
        };

        // we have the types :)
        let mut calldata_sol_types = Vec::new();
        for input in self.matched_function.inputs.clone() {
            match input.resolve() {
                Ok(st) => {
                    calldata_sol_types.push(st);
                }
                Err(_) => {
                    return Err(QueryBuilderError::FailedToResolveSolTypesOfMatchedFunction(
                        self.matched_function.clone(),
                    ));
                }
            }
        }

        // now we need to decode the contract call, but only from the slice of FUNCTION_SIGNITURE_SIZE...onwards..
        let data = self.tx.inner.input().as_ref();
        let sliced_data = &data[FUNCTION_SIGNATURE_SIZE..data_size];

        // compute the offsets :)
        let data_computed_offsets = match compute_abi_offsets(calldata_sol_types, sliced_data) {
            Ok(offsets) => offsets,
            Err(_) => return Err(QueryBuilderError::FailedToComputeOffsetsForCalldata),
        };

        match data_computed_offsets.get(matched_argument_index) {
            Some(field) => match field.size {
                Some(size) => {
                    self.selected_offsets.push((
                        self.data_field.offset + FUNCTION_SIGNATURE_SIZE + field.offset,
                        size,
                    ));
                    Ok(self)
                }
                None => Err(QueryBuilderError::TryingToGetSizeOfDynamicType),
            },
            None => Err(QueryBuilderError::MissingDataInCalldataOffsets),
        }
    }
}
