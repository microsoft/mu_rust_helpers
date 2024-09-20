use core::mem;

use alloc::vec::Vec;
use fallible_streaming_iterator::FallibleStreamingIterator;
use r_efi::efi::{self, Guid};

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

//// Provides a fallible streaming iterator over UEFI variable names.
///
/// Will produce an EFI status on error.
///
/// # Examples
///
/// ### Iterating through all UEFI variable names
/// ```ignore
/// let mut iter = VariableNameIterator::new_from_first(runtime_services);
/// while let Some(variable_identifier) = iter.next()? {
///     some_function(variable_identifier.name, variable_identifier.namespace);
/// }
/// ```
///
/// ### Iterating through UEFI variable names, starting with a known one
/// ```ignore
/// let mut iter = VariableNameIterator::new_from_variable(
///     &SOME_VARIABLE_NAME,
///     &SOME_VARIABLE_NAMESPACE,
///     runtime_services
/// );
///
/// while let Some(variable_identifier) = iter.next()? {
///     some_function(variable_identifier.name, variable_identifier.namespace);
/// }
/// ```
#[derive(Debug)]
pub struct VariableNameIterator<'a, R: RuntimeServices> {
    rs: &'a R,

    current: VariableIdentifier,
    next: VariableIdentifier,
    finished: bool,
}

impl<'a, R: RuntimeServices> VariableNameIterator<'a, R> {
    /// Produce a new iterator from the beginning of the UEFI variable list
    pub fn new_from_first(runtime_services: &'a R) -> Self {
        Self {
            rs: &runtime_services,
            current: VariableIdentifier {
                name: {
                    // Previous name should be an empty string to get the first variable
                    let mut prev_name = Vec::<u16>::with_capacity(1);
                    prev_name.resize(1, 0);

                    prev_name
                },
                // When calling with an empty name, the GUID is ignored.
                // We can just set it to zero.
                namespace: Guid::from_bytes(&[0x0; 16]),
            },
            next: VariableIdentifier { name: Vec::<u16>::new(), namespace: Guid::from_bytes(&[0x0; 16]) },
            finished: false,
        }
    }

    /// Produce a new iterator, starting from a given variable
    pub fn new_from_variable(name: &[u16], namespace: &efi::Guid, runtime_services: &'a R) -> Self {
        Self {
            rs: &runtime_services,
            current: VariableIdentifier { name: name.to_vec(), namespace: namespace.clone() },
            next: VariableIdentifier { name: Vec::<u16>::new(), namespace: Guid::from_bytes(&[0x0; 16]) },
            finished: false,
        }
    }
}

impl<'a, R: RuntimeServices> FallibleStreamingIterator for VariableNameIterator<'a, R> {
    type Item = VariableIdentifier;
    type Error = efi::Status;

    fn advance(&mut self) -> Result<(), Self::Error> {
        unsafe {
            let status = self.rs.get_next_variable_name_unchecked(
                &self.current.name,
                &self.current.namespace,
                &mut self.next.name,
                &mut self.next.namespace,
            );

            mem::swap(&mut self.current, &mut self.next);

            if status.is_err() && status.unwrap_err() == efi::Status::NOT_FOUND {
                self.finished = true;
                return Ok(());
            } else {
                return status;
            }
        }
    }

    fn get(&self) -> Option<&Self::Item> {
        if self.finished {
            None
        } else {
            Some(&self.current)
        }
    }
}

#[cfg(test)]
mod test {
    use efi;

    use super::*;
    use crate::StandardRuntimeServices;
    use core::mem;

    use crate::test::*;

    #[test]
    fn test_variable_name_iterator_from_first() {
        let rs: &StandardRuntimeServices<'_> =
            runtime_services!(get_next_variable_name = mock_efi_get_next_variable_name);

        let mut iter = VariableNameIterator::new_from_first(rs);

        // Make sure the first result corresponds to DUMMY_FIRST_NAME
        let mut status = iter.next();
        assert!(status.is_ok());
        assert!(status.unwrap().is_some());
        let mut variable_identifier = status.unwrap().unwrap();
        assert_eq!(variable_identifier.name, DUMMY_FIRST_NAME);
        assert_eq!(variable_identifier.namespace, DUMMY_FIRST_NAMESPACE);

        // Make sure the second result corresponds to DUMMY_SECOND_NAME
        status = iter.next();
        assert!(status.is_ok());
        assert!(status.unwrap().is_some());
        variable_identifier = status.unwrap().unwrap();
        assert_eq!(variable_identifier.name, DUMMY_SECOND_NAME);
        assert_eq!(variable_identifier.namespace, DUMMY_SECOND_NAMESPACE);

        // Make sure the third result indicates we've reached the end
        status = iter.next();
        assert!(status.is_ok());
        assert!(status.unwrap().is_none());
    }

    #[test]
    fn test_variable_name_iterator_from_second() {
        let rs: &StandardRuntimeServices<'_> =
            runtime_services!(get_next_variable_name = mock_efi_get_next_variable_name);

        let mut iter = VariableNameIterator::new_from_variable(&DUMMY_FIRST_NAME, &DUMMY_FIRST_NAMESPACE, rs);

        // Make sure the first result corresponds to DUMMY_SECOND_NAME
        let mut status = iter.next();
        assert!(status.is_ok());
        assert!(status.unwrap().is_some());
        let variable_identifier = status.unwrap().unwrap();
        assert_eq!(variable_identifier.name, DUMMY_SECOND_NAME);
        assert_eq!(variable_identifier.namespace, DUMMY_SECOND_NAMESPACE);

        // Make sure the second result indicates we've reached the end
        status = iter.next();
        assert!(status.is_ok());
        assert!(status.unwrap().is_none());
    }
}
