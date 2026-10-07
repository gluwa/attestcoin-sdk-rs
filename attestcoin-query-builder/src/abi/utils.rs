use crate::abi::models::FieldMetadata;
use alloy::{
    dyn_abi::{Decoder, DynSolType},
    sol_types::Error,
};

pub(crate) const WORD_SIZE: usize = 32;

#[derive(Debug)]
pub enum ComputeAbiOffsetsError {
    FailedToDecode(Error),
}

pub fn compute_abi_offsets(
    types: Vec<DynSolType>,
    abi: &[u8],
) -> Result<Vec<FieldMetadata>, ComputeAbiOffsetsError> {
    let mut reader = Decoder::new(abi);
    match decode_offset_recursive(&mut reader, types, 0) {
        Ok(result) => Ok(result),
        Err(err) => Err(ComputeAbiOffsetsError::FailedToDecode(err)),
    }
}

fn decode_offset_recursive(
    reader: &mut Decoder,
    types: Vec<DynSolType>,
    base_offset: usize,
) -> Result<Vec<FieldMetadata>, Error> {
    let mut result = Vec::new();
    for sol_type in types {
        match sol_type.clone() {
            DynSolType::Bool
            | DynSolType::Int(_)
            | DynSolType::Uint(_)
            | DynSolType::FixedBytes(_)
            | DynSolType::Address => {
                let relative_offset = reader.offset();
                let absolute_offset = base_offset + relative_offset;
                let word = reader.take_word()?;
                result.push(FieldMetadata {
                    sol_type: sol_type.clone(),
                    offset: absolute_offset,
                    size: Some(WORD_SIZE),
                    is_dynamic: false,
                    value: Some(word.0.to_vec()),
                    children: vec![],
                });
            }
            DynSolType::Bytes | DynSolType::String => {
                let offset = reader.take_offset()?;
                let mut sub_reader = reader.child(offset)?;
                let size = sub_reader.take_offset()?; // take_size() dosen't exist, its the same though.
                let data = sub_reader.take_slice(size)?;
                let dynamic_field_absolute_offset = base_offset + offset + WORD_SIZE;
                result.push(FieldMetadata {
                    sol_type: sol_type.clone(),
                    offset: dynamic_field_absolute_offset,
                    size: Some(size),
                    is_dynamic: true,
                    value: Some(data.to_vec()),
                    children: vec![],
                });
            }
            DynSolType::Array(array_element_sol_type_boxed) => {
                // all array's not fixed length are always dynamic
                let field_dynamic_offset = reader.take_offset()?; // first we get the offset of the array
                let absolute_offset = base_offset + field_dynamic_offset;
                let mut sub_reader = reader.child(field_dynamic_offset)?; // create a sub reader..
                let number_of_elements = sub_reader.take_offset()?; // reader the number of elements..
                // if the child of the array is dynamic, its treated quite differently
                // as if it was a dynamic child.
                let array_element_sol_type_unboxed = *array_element_sol_type_boxed;
                // this can probably be a function on its own D:
                let array_components: Vec<DynSolType> = match array_element_sol_type_unboxed.clone()
                {
                    // Likely a bug. We would want to add `dyn_sol_types` repeatedly for 0..number_of_elements.
                    // It's fine for the number of array_components to end up larger than the number_of_elements.
                    DynSolType::Tuple(dyn_sol_types) => dyn_sol_types,
                    other => {
                        let mut types = Vec::new();
                        for _ in 0..number_of_elements {
                            types.push(other.clone());
                        }
                        types
                    }
                };

                let mut children = Vec::new();
                if is_dynamic(array_element_sol_type_unboxed.clone()) {
                    // if its a dynamic child we need to compute the offsets
                    // for each element.
                    let mut dynamic_array_relative_offsets = Vec::new();
                    for _ in 0..number_of_elements {
                        let child_element_offset = sub_reader.take_offset()?;
                        dynamic_array_relative_offsets.push(child_element_offset);
                    }

                    // STRICT FIX: Only handle bytes/string arrays differently
                    let is_bytes_or_string = matches!(
                        array_element_sol_type_unboxed,
                        DynSolType::Bytes | DynSolType::String
                    );

                    for array_element_offset in dynamic_array_relative_offsets {
                        if is_bytes_or_string {
                            // For bytes/string in arrays, arrayElementOffset is relative to the start of offsets array
                            // Offsets array starts at field_dynamic_offset + WORD_SIZE (after array length prefix)
                            // So absolute position is: base_offset + field_dynamic_offset + WORD_SIZE + array_element_offset
                            let absolute_data_offset = base_offset
                                + field_dynamic_offset
                                + WORD_SIZE
                                + array_element_offset;
                            let mut data_sub_reader = reader.child(absolute_data_offset)?;
                            let length = data_sub_reader.take_offset()?; // Read length
                            let data = data_sub_reader.take_slice(length)?;

                            children.push(FieldMetadata {
                                sol_type: array_element_sol_type_unboxed.clone(),
                                offset: absolute_data_offset + WORD_SIZE, // Absolute position of data start (after length prefix)
                                size: Some(length),
                                is_dynamic: true,
                                value: Some(data.to_vec()),
                                children: vec![],
                            });
                        } else {
                            // Original code path for tuples and other types (unchanged)
                            // the only problem is to calculate the absolute position
                            // its a bit more complicated you need to consider the offset since the begining
                            // so hmm at this point the sub reader has already read all the offset so its cursor is kind of already at the right place.
                            // so the position is hmm
                            let semi_absolute_positon =
                                field_dynamic_offset + array_element_offset + WORD_SIZE;
                            let mut child_element_sub_reader =
                                reader.child(semi_absolute_positon)?;

                            // absolute position since parents..
                            let position = base_offset + semi_absolute_positon;
                            // go decode the children :)
                            let children_of_child = decode_offset_recursive(
                                &mut child_element_sub_reader,
                                array_components.clone(),
                                position,
                            )?;

                            children.push(FieldMetadata {
                                sol_type: array_element_sol_type_unboxed.clone(),
                                offset: position,
                                size: None,
                                is_dynamic: true,
                                value: None,
                                children: children_of_child,
                            });
                        }
                    }
                } else {
                    // in this case it means that the inner type of the dynamic array is not a dynamic type
                    // and what you want to do is to decode them by passing the sub reader down to the
                    // next iteration of the decoder, this way it moves forward properly.
                    children = decode_offset_recursive(
                        &mut sub_reader,
                        array_components,
                        absolute_offset,
                    )?;
                }

                result.push(FieldMetadata {
                    sol_type: sol_type.clone(),
                    offset: absolute_offset,
                    size: None,
                    is_dynamic: true,
                    value: None,
                    children,
                });
            }
            DynSolType::FixedArray(array_element_sol_type_boxed, number_of_elements) => {
                // if the child of the array is dynamic, its treated quite differently
                // as if it was a dynamic child.
                let array_element_sol_type_unboxed = *array_element_sol_type_boxed;
                // this can probably be a function on its own D:
                let array_components: Vec<DynSolType> = match array_element_sol_type_unboxed.clone()
                {
                    // Maybe same issue here as noted in Array case
                    DynSolType::Tuple(dyn_sol_types) => dyn_sol_types,
                    other => {
                        let mut types = Vec::new();
                        for _ in 0..number_of_elements {
                            types.push(other.clone());
                        }
                        types
                    }
                };

                // if its dynamic don't know what to do :D
                if is_dynamic(array_element_sol_type_unboxed.clone()) {
                    // okay so this uses an offset..
                    let offset = reader.take_offset()?;
                    let absolute_offset = base_offset + offset;
                    // we need a sub reader there..
                    let mut sub_reader = reader.child(offset)?;
                    let children = decode_offset_recursive(
                        &mut sub_reader,
                        array_components,
                        absolute_offset,
                    )?;
                    result.push(FieldMetadata {
                        sol_type: sol_type.clone(),
                        offset: absolute_offset,
                        size: None,
                        is_dynamic: true,
                        value: None,
                        children,
                    });
                } else {
                    // easy enough we just recursive.
                    let absolute_offset = base_offset + reader.offset();
                    let children =
                        decode_offset_recursive(reader, array_components, absolute_offset)?;
                    result.push(FieldMetadata {
                        sol_type: sol_type.clone(),
                        offset: absolute_offset,
                        size: None,
                        is_dynamic: false,
                        value: None,
                        children,
                    });
                }
            }
            DynSolType::Tuple(tuple_components) => {
                if is_dynamic(sol_type.clone()) {
                    let offset_of_tuple = base_offset + reader.offset();
                    let offset_of_dynamic_data = reader.take_offset()?;
                    let mut sub_reader = reader.child(offset_of_dynamic_data)?;
                    let children = decode_offset_recursive(
                        &mut sub_reader,
                        tuple_components,
                        offset_of_dynamic_data,
                    )?;
                    result.push(FieldMetadata {
                        sol_type: sol_type.clone(),
                        offset: offset_of_tuple,
                        size: None,
                        is_dynamic: true,
                        value: None,
                        children,
                    });
                } else {
                    let offset_of_tuple = base_offset + reader.offset();
                    let children = decode_offset_recursive(reader, tuple_components, base_offset)?;
                    result.push(FieldMetadata {
                        sol_type: sol_type.clone(),
                        offset: offset_of_tuple,
                        size: None,
                        is_dynamic: false,
                        value: None,
                        children,
                    });
                }
            }
            DynSolType::Function => {
                return Err(Error::custom(
                    "Cannot decode functions, with this decoder..",
                ));
            }
        }
    }
    Ok(result)
}

pub fn is_dynamic(sol_type: DynSolType) -> bool {
    match sol_type {
        DynSolType::Bool => false,
        DynSolType::Int(_) => false,
        DynSolType::Uint(_) => false,
        DynSolType::FixedBytes(_) => false,
        DynSolType::Address => false,
        DynSolType::Function => todo!(),
        DynSolType::Bytes => true,
        DynSolType::String => true,
        DynSolType::Array(_array_children) => true,
        DynSolType::FixedArray(array_children, _array_size) => is_dynamic(*array_children),
        DynSolType::Tuple(tuple_components) => {
            for tuple_component in tuple_components {
                if is_dynamic(tuple_component) {
                    return true;
                }
            }
            false
        }
    }
}

/// Recursively makes offsets absolute by adding baseOffset to field and all children
pub(crate) fn make_offsets_absolute(field: &mut FieldMetadata, base_offset: usize) {
    field.offset += base_offset;
    for child in &mut field.children {
        make_offsets_absolute(child, base_offset);
    }
}
