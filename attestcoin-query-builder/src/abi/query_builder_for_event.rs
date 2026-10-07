use alloy::{
    dyn_abi::{DecodedEvent, Specifier},
    rpc::types::Log,
};
use alloy_json_abi::Event;

use super::{
    models::{FieldMetadata, QueryBuilderError},
    utils::compute_abi_offsets,
};

pub struct QueryBuilderForEvent {
    field: FieldMetadata,
    log: Log,
    _decoded_event: DecodedEvent,
    event: Event,
    selected_offsets: Vec<(usize, usize)>,
}

impl QueryBuilderForEvent {
    pub(crate) fn new(
        log_field: FieldMetadata,
        log: Log,
        _decoded_event: DecodedEvent,
        event: Event,
    ) -> Self {
        Self {
            field: log_field,
            log,
            _decoded_event,
            event,
            selected_offsets: vec![],
        }
    }

    pub fn add_argument(&mut self, name: &str) -> Result<&mut Self, QueryBuilderError> {
        let mut topic_index: usize = 0;
        let mut data_index: usize = 0;

        for event_input in self.event.inputs.clone() {
            // We add 1 before loop contents to skip index 0, which always holds the
            // event signature.
            if event_input.indexed {
                topic_index += 1;
            }

            if event_input.name == name {
                if event_input.indexed {
                    // if its indexed..
                    // calculate the offset..
                    match self.field.children.get(1) {
                        // Children are 0:address, 1:indexed, and 2:data
                        Some(topics) => {
                            match topics.children.get(topic_index) {
                                Some(subject_topic) => {
                                    // all topics are 32 length :)
                                    // if you want to be extra safe you can always also do
                                    // match subject_topic.size
                                    self.selected_offsets.push((subject_topic.offset, 32));
                                    return Ok(self);
                                }
                                None => {
                                    return Err(QueryBuilderError::MissingDataInAbiOffsets);
                                }
                            }
                        }
                        None => {
                            return Err(QueryBuilderError::MissingDataInAbiOffsets);
                        }
                    }
                } else {
                    // its a data field.
                    let data_field = match self.field.children.get(2) {
                        Some(df) => df,
                        None => {
                            return Err(QueryBuilderError::MissingDataInAbiOffsets);
                        }
                    };

                    // construct a list of body solidity types.
                    let mut body_sol_types = Vec::new();
                    for input in self.event.inputs.clone() {
                        if !input.indexed {
                            match input.resolve() {
                                Ok(st) => {
                                    body_sol_types.push(st);
                                }
                                Err(_) => {
                                    return Err(
                                        QueryBuilderError::FailedToResolveSolTypesOfMatchedEvent(
                                            self.event.clone(),
                                        ),
                                    );
                                }
                            }
                        }
                    }

                    // we have the body solidity types :) of the data field.
                    // we need to compute its offsets, similar to our transaction.
                    let event_data_offsets =
                        match compute_abi_offsets(body_sol_types, &self.log.data().data) {
                            Ok(offsets) => offsets,
                            Err(_) => {
                                return Err(QueryBuilderError::FailedToGetEventDataOffsets(
                                    Box::new(self.log.clone()),
                                ));
                            }
                        };

                    match event_data_offsets.get(data_index) {
                        Some(argument_field) => match argument_field.size {
                            Some(argument_field_size) => {
                                self.selected_offsets.push((
                                    data_field.offset + argument_field.offset,
                                    argument_field_size,
                                ));
                                return Ok(self);
                            }
                            None => {
                                return Err(QueryBuilderError::TryingToGetSizeOfDynamicType);
                            }
                        },
                        None => {
                            return Err(QueryBuilderError::MissingDataInAbiOffsets);
                        }
                    }
                }
            }

            if !event_input.indexed {
                data_index += 1;
            }
        }

        Err(QueryBuilderError::MissingDataInAbiOffsets)
    }

    pub fn add_address(&mut self) -> Result<&mut Self, QueryBuilderError> {
        match self.field.children.first() {
            Some(address_field) => match address_field.size {
                Some(address_field_size) => {
                    self.selected_offsets
                        .push((address_field.offset, address_field_size));
                    Ok(self)
                }
                None => Err(QueryBuilderError::TryingToGetSizeOfDynamicType),
            },
            None => Err(QueryBuilderError::MissingDataInAbiOffsets),
        }
    }

    pub fn add_signature(&mut self) -> Result<&mut Self, QueryBuilderError> {
        // this is the topics..
        match self.field.children.get(1) {
            Some(topics) => match topics.children.first() {
                Some(signature_topic) => match signature_topic.size {
                    Some(size_of_topic) => {
                        let offset_and_size = (signature_topic.offset, size_of_topic);
                        self.selected_offsets.push(offset_and_size);
                        Ok(self)
                    }
                    None => Err(QueryBuilderError::TryingToGetSizeOfDynamicType),
                },
                None => Err(QueryBuilderError::MissingDataInAbiOffsets),
            },
            None => Err(QueryBuilderError::MissingDataInAbiOffsets),
        }
    }

    pub fn get_selected_offsets(self) -> Vec<(usize, usize)> {
        self.selected_offsets.clone()
    }
}
