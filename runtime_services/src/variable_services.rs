
use r_efi::efi::{self, Guid};
use fallible_streaming_iterator::FallibleStreamingIterator;
use alloc::vec::Vec;

use crate::RuntimeServices;

#[derive(Debug)]
pub enum GetVariableStatus {
    Error(efi::Status),
    BufferTooSmall { data_size: usize, attributes: u32 },
    Success { data_size: usize, attributes: u32 },
}

#[derive(Debug)]
pub struct VariableInfo {
    pub maximum_variable_storage_size: u64,
    pub remaining_variable_storage_size: u64,
    pub maximum_variable_size: u64,
}

#[derive(Debug)]
pub struct VariableIdentifier {
    name: Vec<u16>,
    namespace: efi::Guid,
}

#[derive(Debug)]
pub struct VariableNameIterator<'a, R: RuntimeServices> {
    rs: &'a R,

    prev: VariableIdentifier,
    current: VariableIdentifier
}

impl<'a, R: RuntimeServices> VariableNameIterator<'a, R> {
    fn new_from_first(
        &self,
        runtime_services: &'a R
    ) -> Self {
        Self {
            rs: &runtime_services,
            prev: VariableIdentifier {
                name: {
                    // Previous name should be an empty string to get the first variable
                    let mut prev_name = Vec::<u16>::with_capacity(1);
                    prev_name.resize(1, 0);
    
                    prev_name
                },
                // When calling with an empty name, the GUID is ignored.
                // We can just set it to zero.
                namespace: Guid::from_bytes(&[0x0; 16])
            },
            current: VariableIdentifier {
                name: Vec::<u16>::new(),
                namespace: Guid::from_bytes(&[0x0; 16])
            }
        }
    }

    fn new_from_variable(
        &self,
        name: &[u16],
        namespace: &efi::Guid,
        runtime_services: &'a R
    ) -> Self {
        Self {
            rs: &runtime_services,
            prev: VariableIdentifier {
                name: name.to_vec(),
                namespace: namespace.clone()
            },
            current: VariableIdentifier {
                name: Vec::<u16>::new(),
                namespace: Guid::from_bytes(&[0x0; 16])
            }
        }
    }
}

impl<'a, R: RuntimeServices> FallibleStreamingIterator for VariableNameIterator<'a, R> {
    type Item = VariableIdentifier;
    type Error = efi::Status;

    fn advance(&mut self) -> Result<(), Self::Error> {
        unsafe {
            self.rs.get_next_variable_name_unchecked(
                &self.prev.name,
                &self.prev.namespace,
                &mut self.current.name,
                &mut self.current.namespace
            )
        }
    }

    fn get(&self) -> Option<&Self::Item> {
        Some(&self.current)
    }
}