#![cfg_attr(all(not(test), not(feature = "mockall")), no_std)]

extern crate alloc;

pub mod variable_services;

#[cfg(any(test, feature = "mockall"))]
use mockall::automock;

use alloc::vec::Vec;
use core::{
    ffi::c_void,
    marker::PhantomData,
    ptr,
    sync::atomic::{AtomicPtr, Ordering},
};

use r_efi::efi;
use variable_services::{GetVariableStatus, VariableInfo};

/// This is the runtime services used in the UEFI.
/// it wraps an atomic ptr to [`efi::RuntimeServices`]
#[derive(Debug)]
pub struct StandardRuntimeServices<'a> {
    efi_runtime_services: AtomicPtr<efi::RuntimeServices>,
    _lifetime_marker: PhantomData<&'a efi::RuntimeServices>,
}

impl<'a> StandardRuntimeServices<'a> {
    /// Create a new StandardRuntimeServices with the provided [efi::RuntimeServices].
    pub const fn new(efi_runtime_services: &'a efi::RuntimeServices) -> Self {
        // The efi::RuntimeServices is only read, that is why we use a non mutable reference.
        Self {
            efi_runtime_services: AtomicPtr::new(efi_runtime_services as *const _ as *mut _),
            _lifetime_marker: PhantomData,
        }
    }

    /// Create a new StandardRuntimeServices that is uninitialized.
    /// The struct need to be initialize later with [Self::initialize], otherwise, subsequent call will panic.
    pub const fn new_uninit() -> Self {
        Self { efi_runtime_services: AtomicPtr::new(ptr::null_mut()), _lifetime_marker: PhantomData }
    }

    /// Initialize the StandardRuntimeServices with a reference to [efi::RuntimeServices].
    /// # Panics
    /// This function will panic if already initialize.
    pub fn initialize(&'a self, efi_runtime_services: &'a efi::RuntimeServices) {
        if self.efi_runtime_services.load(Ordering::Relaxed).is_null() {
            // The efi::RuntimeServices is only read, that is why we use a non mutable reference.
            self.efi_runtime_services.store(efi_runtime_services as *const _ as *mut _, Ordering::SeqCst)
        } else {
            panic!("Runtime services is already initialized.")
        }
    }

    /// # Panics
    /// This function will panic if it was not initialize.
    fn efi_runtime_services(&self) -> &efi::RuntimeServices {
        // SAFETY: This pointer is assume to be a valid efi::RuntimeServices pointer since the only way to set it was via an efi::RuntimeServices reference.
        unsafe {
            self.efi_runtime_services
                .load(Ordering::SeqCst)
                .as_ref::<'a>()
                .expect("Runtime services is not initialized.")
        }
    }
}

///SAFETY: StandardRuntimeServices uses an atomic ptr to access the RuntimeServices.
unsafe impl Sync for StandardRuntimeServices<'static> {}
///SAFETY: When the lifetime is `'static`, the pointer is guaranteed to stay valid.
unsafe impl Send for StandardRuntimeServices<'static> {}

#[cfg_attr(any(test, feature = "mockall"), automock)]
pub trait RuntimeServices: Sized {
    fn set_variable<T>(
        &self,
        name: &[u16],
        namespace: &efi::Guid,
        attributes: u32,
        data: &mut T,
    ) -> Result<(), efi::Status>
    where
        T: AsMut<[u8]> + 'static,
    {
        if !name.iter().position(|&c| c == 0).is_some() {
            panic!("Name passed into set_variable is not null-terminated.");
        }

        // Keep a local copy of name to unburden the caller of having to pass in a mutable slice
        let mut name_vec = name.to_vec();

        unsafe { self.set_variable_unchecked(name_vec.as_mut_slice(), namespace, attributes, data.as_mut()) }
    }

    fn get_variable<T>(
        &self,
        name: &[u16],
        namespace: &efi::Guid,
        size_hint: Option<usize>,
    ) -> Result<(T, u32), efi::Status>
    where
        T: TryFrom<Vec<u8>> + 'static,
    {
        if !name.iter().position(|&c| c == 0).is_some() {
            panic!("Name passed into get_variable is not null-terminated.");
        }

        // Keep a local copy of name to unburden the caller of having to pass in a mutable slice
        let mut name_vec = name.to_vec();

        // We can't simply allocate an empty buffer of size T because we can't assume
        // the TryFrom representation of T will be the same as T
        let mut data = Vec::<u8>::new();
        if size_hint.is_some() {
            data.resize(size_hint.unwrap(), 0);
        }

        let mut first_attempt = true;
        loop {
            unsafe {
                let status = self.get_variable_unchecked(
                    name_vec.as_mut_slice(),
                    namespace,
                    if data.len() == 0 { None } else { Some(&mut data) },
                );

                match status {
                    GetVariableStatus::Success { data_size: _, attributes } => match T::try_from(data) {
                        Ok(d) => return Ok((d, attributes)),
                        Err(_) => return Err(efi::Status::INVALID_PARAMETER),
                    },
                    GetVariableStatus::BufferTooSmall { data_size, attributes: _ } => {
                        if first_attempt {
                            first_attempt = false;
                            data.resize(data_size, 10);
                        } else {
                            return Err(efi::Status::BUFFER_TOO_SMALL);
                        }
                    }
                    GetVariableStatus::Error(e) => {
                        return Err(e);
                    }
                }
            }
        }
    }

    fn get_variable_size_and_attributes(
        &self,
        name: &[u16],
        namespace: &efi::Guid,
    ) -> Result<(usize, u32), efi::Status> {
        if !name.iter().position(|&c| c == 0).is_some() {
            panic!("Name passed into set_variable is not null-terminated.");
        }

        // Keep a local copy of name to unburden the caller of having to pass in a mutable slice
        let mut name_vec = name.to_vec();

        unsafe {
            match self.get_variable_unchecked(name_vec.as_mut_slice(), namespace, None) {
                GetVariableStatus::BufferTooSmall { data_size, attributes } => Ok((data_size, attributes)),
                GetVariableStatus::Error(e) => Err(e),
                GetVariableStatus::Success { data_size: _, attributes: _ } => {
                    panic!("GetVariable call with zero-sized buffer returned Success.")
                }
            }
        }
    }

    fn get_next_variable_name(
        &self,
        prev_name: &[u16],
        prev_namespace: &efi::Guid,
    ) -> Result<(Vec<u16>, efi::Guid), efi::Status> {
        unsafe { self.get_next_variable_name_unchecked(prev_name, prev_namespace) }
    }

    unsafe fn query_variable_info(&self, attributes: u32) -> Result<VariableInfo, efi::Status> {
        unsafe { self.query_variable_info_unchecked(attributes) }
    }

    unsafe fn set_variable_unchecked(
        &self,
        name: &mut [u16],
        namespace: &efi::Guid,
        attributes: u32,
        data: &mut [u8],
    ) -> Result<(), efi::Status>;

    unsafe fn get_variable_unchecked<'a>(
        &self,
        name: &mut [u16],
        namespace: &efi::Guid,
        data: Option<&'a mut [u8]>,
    ) -> GetVariableStatus;

    unsafe fn get_next_variable_name_unchecked(
        &self,
        prev_name: &[u16],
        prev_namespace: &efi::Guid,
    ) -> Result<(Vec<u16>, efi::Guid), efi::Status>;

    unsafe fn query_variable_info_unchecked(&self, attributes: u32) -> Result<VariableInfo, efi::Status>;
}

impl RuntimeServices for StandardRuntimeServices<'_> {
    unsafe fn set_variable_unchecked(
        &self,
        name: &mut [u16],
        namespace: &efi::Guid,
        attributes: u32,
        data: &mut [u8],
    ) -> Result<(), efi::Status> {
        let set_variable = self.efi_runtime_services().set_variable;
        if set_variable as usize == 0 {
            panic!("SetVariable has not initialized in the Runtime Services Table.")
        }

        let status = set_variable(
            name.as_mut_ptr(),
            namespace as *const _ as *mut _,
            attributes,
            data.len(),
            data.as_mut_ptr() as *mut c_void,
        );

        if status.is_error() {
            Err(status)
        } else {
            Ok(())
        }
    }

    unsafe fn get_variable_unchecked(
        &self,
        name: &mut [u16],
        namespace: &efi::Guid,
        data: Option<&mut [u8]>,
    ) -> GetVariableStatus {
        let get_variable = self.efi_runtime_services().get_variable;
        if get_variable as usize == 0 {
            panic!("GetVariable has not initialized in the Runtime Services Table.")
        }

        let mut data_size: usize = match data {
            Some(ref d) => d.len(),
            None => 0,
        };
        let mut attributes: u32 = 0;

        let status = get_variable(
            name.as_mut_ptr(),
            namespace as *const _ as *mut _,
            ptr::addr_of_mut!(attributes),
            ptr::addr_of_mut!(data_size),
            match data {
                Some(d) => d.as_ptr() as *mut c_void,
                None => ptr::null_mut() as *mut c_void,
            },
        );

        if status == efi::Status::BUFFER_TOO_SMALL {
            return GetVariableStatus::BufferTooSmall { data_size: data_size, attributes: attributes };
        } else if status.is_error() {
            return GetVariableStatus::Error(status);
        }

        GetVariableStatus::Success { data_size: data_size, attributes: attributes }
    }

    // Note: Unlike get_variable, a non-null terminated name will return INVALID_PARAMETER per UEFI spec
    unsafe fn get_next_variable_name_unchecked(
        &self,
        prev_name: &[u16],
        prev_namespace: &efi::Guid,
    ) -> Result<(Vec<u16>, efi::Guid), efi::Status> {
        let get_next_variable_name = self.efi_runtime_services().get_next_variable_name;
        if get_next_variable_name as usize == 0 {
            panic!("GetNextVariableName has not initialized in the Runtime Services Table.")
        }

        if prev_name.len() == 0 {
            panic!("Zero-length name passed into get_next_variable_name.");
        }

        let mut name = prev_name.to_vec();

        let mut name_size: usize = name.len();
        let mut namespace: efi::Guid = *prev_namespace;

        let mut first_try: bool = true;
        loop {
            let status = get_next_variable_name(ptr::addr_of_mut!(name_size), name.as_mut_ptr(), ptr::addr_of_mut!(namespace));

            if status == efi::Status::BUFFER_TOO_SMALL && first_try {
                first_try = false;

                assert!(name_size > name.len(), "get_next_variable_name requested smaller buffer.");
                name.resize(name_size, 0);

                // Reset fields which may have been overwritten
                name.splice(0..prev_name.len(), prev_name.iter().cloned());

                namespace = *prev_namespace;
            } else if status.is_error() {
                return Err(status)
            } else {
                name.truncate(
                    name.iter()
                        .position(|&c| c == 0)
                        .expect("Name returned by get_next_variable_name is not null-terminated.")
                        + 1,
                );

                return Ok((name, namespace));
            }
        }
    }

    unsafe fn query_variable_info_unchecked(&self, attributes: u32) -> Result<VariableInfo, efi::Status> {
        let query_variable_info = self.efi_runtime_services().query_variable_info;
        if query_variable_info as usize == 0 {
            panic!("QueryVariableInfo has not initialized in the Runtime Services Table.")
        }

        let mut var_info = VariableInfo {
            maximum_variable_storage_size: 0,
            remaining_variable_storage_size: 0,
            maximum_variable_size: 0,
        };

        let status = query_variable_info(
            attributes,
            ptr::addr_of_mut!(var_info.maximum_variable_storage_size),
            ptr::addr_of_mut!(var_info.remaining_variable_storage_size),
            ptr::addr_of_mut!(var_info.maximum_variable_size),
        );

        if status.is_error() {
            return Err(status);
        } else {
            return Ok(var_info);
        }
    }
}

#[cfg(test)]
mod test {
    use efi;

    use super::*;
    use core::{mem, slice};

    macro_rules! runtime_services {
    ($($efi_services:ident = $efi_service_fn:ident),*) => {{
      static RUNTIME_SERVICE: StandardRuntimeServices = StandardRuntimeServices::new_uninit();
      let efi_runtime_services = unsafe {
        #[allow(unused_mut)]
        let mut rs = mem::MaybeUninit::<efi::RuntimeServices>::zeroed();
        $(
          rs.assume_init_mut().$efi_services = $efi_service_fn;
        )*
        rs.assume_init()
      };
      RUNTIME_SERVICE.initialize(&efi_runtime_services);
      &RUNTIME_SERVICE
    }};
  }

    #[test]
    #[should_panic(expected = "Runtime services is not initialized.")]
    fn test_that_accessing_uninit_runtime_services_should_panic() {
        let rs = StandardRuntimeServices::new_uninit();
        rs.efi_runtime_services();
    }

    #[test]
    #[should_panic(expected = "Runtime services is already initialized.")]
    fn test_that_initializing_runtime_services_multiple_time_should_panic() {
        let efi_rs = unsafe { mem::MaybeUninit::<efi::RuntimeServices>::zeroed().as_ptr().as_ref().unwrap() };
        let rs = StandardRuntimeServices::new_uninit();
        rs.initialize(efi_rs);
        rs.initialize(efi_rs);
    }

    const DUMMY_NAME: [u16; 3] = [0x1000, 0x1020, 0x0000];
    const DUMMY_NON_NULL_TERMINATED_NAME: [u16; 3] = [0x1000, 0x1020, 0x1040];
    const DUMMY_EMPTY_NAME: [u16; 1] = [0x0000];
    const DUMMY_ZERO_LENGTH_NAME: [u16; 0] = [];
    const DUMMY_NEXT_NAME: [u16; 5] = [0x1001, 0x1022, 0x1043, 0x1064, 0x0000];
    const DUMMY_UNKNOWN_NAME: [u16; 3] = [0x2000, 0x2020, 0x0000];

    const DUMMY_NODE: [u8; 6] = [0x0, 0x0, 0x0, 0x0, 0x0, 0x0];
    const DUMMY_NAMESPACE: efi::Guid = efi::Guid::from_fields(0, 0, 0, 0, 0, &DUMMY_NODE);
    const DUMMY_NEXT_NAMESPACE: efi::Guid = efi::Guid::from_fields(1, 0, 0, 0, 0, &DUMMY_NODE);

    const DUMMY_ATTRIBUTES: u32 = 0x1234;
    const DUMMY_DATA: u32 = 0xDEADBEEF;
    const DUMMY_DATA_REPR_SIZE: usize = mem::size_of::<u32>();

    #[derive(Debug)]
    struct DummyVariableType {
        pub value: u32,
    }

    impl AsMut<[u8]> for DummyVariableType {
        fn as_mut(&mut self) -> &mut [u8] {
            unsafe { slice::from_raw_parts_mut::<u8>(ptr::addr_of_mut!(self.value) as *mut u8, mem::size_of::<u32>()) }
        }
    }

    impl TryFrom<Vec<u8>> for DummyVariableType {
        type Error = &'static str;

        fn try_from(value: Vec<u8>) -> Result<Self, Self::Error> {
            assert!(value.len() == mem::size_of::<u32>());

            Ok(DummyVariableType { value: u32::from_ne_bytes(value[0..4].try_into().unwrap()) })
        }
    }

    extern "efiapi" fn mock_efi_get_variable(
        name: *mut u16,
        namespace: *mut efi::Guid,
        attributes: *mut u32,
        data_size: *mut usize,
        data: *mut c_void,
    ) -> efi::Status {
        unsafe {
            if DUMMY_UNKNOWN_NAME.iter().enumerate().all(|(i, &c)| *name.offset(i as isize) == c) {
                return efi::Status::NOT_FOUND;
            }

            // Since it's not DUMMY_UNKNOWN_NAME, we're assuming DUMMY_NAME was passed in
            // If name is not equal to DUMMY_NAME, then something must have gone wrong.
            assert_eq!(
                DUMMY_NAME.iter().enumerate().all(|(i, &c)| *name.offset(i as isize) == c),
                true,
                "Variable name does not match expected."
            );

            assert_eq!(*namespace, DUMMY_NAMESPACE);

            *attributes = DUMMY_ATTRIBUTES;

            if *data_size < DUMMY_DATA_REPR_SIZE {
                *data_size = DUMMY_DATA_REPR_SIZE;
                return efi::Status::BUFFER_TOO_SMALL;
            }

            *data_size = DUMMY_DATA_REPR_SIZE;
            *(data as *mut u32) = DUMMY_DATA;
        }

        return efi::Status::SUCCESS;
    }

    extern "efiapi" fn mock_efi_set_variable(
        name: *mut u16,
        namespace: *mut efi::Guid,
        attributes: u32,
        data_size: usize,
        data: *mut c_void,
    ) -> efi::Status {
        unsafe {
            // Invalid parameter is returned if name is empty (first character is 0)
            if *name == 0 {
                return efi::Status::INVALID_PARAMETER;
            }

            if DUMMY_UNKNOWN_NAME.iter().enumerate().all(|(i, &c)| *name.offset(i as isize) == c) {
                return efi::Status::NOT_FOUND;
            }

            // Since it's not DUMMY_UNKNOWN_NAME, we're assuming DUMMY_NAME was passed in
            // If name is not equal to DUMMY_NAME, then something must have gone wrong.
            assert_eq!(
                DUMMY_NAME.iter().enumerate().all(|(i, &c)| *name.offset(i as isize) == c),
                true,
                "Variable name does not match expected."
            );

            assert_eq!(*namespace, DUMMY_NAMESPACE);
            assert_eq!(attributes, DUMMY_ATTRIBUTES);
            assert_eq!(data_size, DUMMY_DATA_REPR_SIZE);
            assert_eq!(*(data as *mut u32), DUMMY_DATA);
        }

        return efi::Status::SUCCESS;
    }

    extern "efiapi" fn mock_efi_get_next_variable_name(
        name_size: *mut usize,
        name: *mut u16,
        namespace: *mut efi::Guid,
    ) -> efi::Status {
        // Ensure the name and namespace are as expected
        unsafe {
            // Return invalid parameter if the name isn't null-terminated per UEFI spec
            if !slice::from_raw_parts(name, *name_size).iter().position(|&c| c == 0).is_some() {
                return efi::Status::INVALID_PARAMETER;
            }

            if DUMMY_UNKNOWN_NAME.iter().enumerate().all(|(i, &c)| *name.offset(i as isize) == c) {
                return efi::Status::NOT_FOUND;
            }

            // Since it's not DUMMY_UNKNOWN_NAME, we're assuming DUMMY_NAME was passed in
            // If name is not equal to DUMMY_NAME, then something must have gone wrong.
            assert_eq!(
                DUMMY_NAME.iter().enumerate().all(|(i, &c)| *name.offset(i as isize) == c),
                true,
                "Variable name does not match expected."
            );
            assert_eq!(*namespace, DUMMY_NAMESPACE);

            if *name_size < DUMMY_NEXT_NAME.len() {
                *name_size = DUMMY_NEXT_NAME.len();
                return efi::Status::BUFFER_TOO_SMALL;
            }

            *name_size = DUMMY_NEXT_NAME.len();
            ptr::copy_nonoverlapping(DUMMY_NEXT_NAME.as_ptr(), name, DUMMY_NEXT_NAME.len());
            *namespace = DUMMY_NEXT_NAMESPACE;
        }

        return efi::Status::SUCCESS;
    }

    #[test]
    fn test_get_variable() {
        let rs: &StandardRuntimeServices<'_> = runtime_services!(get_variable = mock_efi_get_variable);

        let status = rs.get_variable::<DummyVariableType>(&DUMMY_NAME, &DUMMY_NAMESPACE, None);

        assert!(status.is_ok());
        let (data, attributes) = status.unwrap();
        assert_eq!(attributes, DUMMY_ATTRIBUTES);
        assert_eq!(data.value, DUMMY_DATA);
    }

    #[test]
    #[should_panic(expected = "Name passed into get_variable is not null-terminated.")]
    fn test_get_variable_non_terminated() {
        let rs: &StandardRuntimeServices<'_> = runtime_services!(get_variable = mock_efi_get_variable);

        let _ = rs.get_variable::<DummyVariableType>(&DUMMY_NON_NULL_TERMINATED_NAME, &DUMMY_NAMESPACE, None);
    }

    #[test]
    fn test_get_variable_low_size_hint() {
        let rs: &StandardRuntimeServices<'_> = runtime_services!(get_variable = mock_efi_get_variable);

        let status = rs.get_variable::<DummyVariableType>(&DUMMY_NAME, &DUMMY_NAMESPACE, Some(1));

        assert!(status.is_ok());
        let (data, attributes) = status.unwrap();
        assert_eq!(attributes, DUMMY_ATTRIBUTES);
        assert_eq!(data.value, DUMMY_DATA);
    }

    #[test]
    fn test_get_variable_not_found() {
        let rs: &StandardRuntimeServices<'_> = runtime_services!(get_variable = mock_efi_get_variable);

        let status = rs.get_variable::<DummyVariableType>(&DUMMY_UNKNOWN_NAME, &DUMMY_NAMESPACE, Some(1));

        assert!(status.is_err());
        assert_eq!(status.unwrap_err(), efi::Status::NOT_FOUND);
    }

    #[test]
    fn test_get_variable_size_and_attributes() {
        let rs: &StandardRuntimeServices<'_> = runtime_services!(get_variable = mock_efi_get_variable);

        let status = rs.get_variable_size_and_attributes(&DUMMY_NAME, &DUMMY_NAMESPACE);

        assert!(status.is_ok());
        let (size, attributes) = status.unwrap();
        assert_eq!(size, DUMMY_DATA_REPR_SIZE);
        assert_eq!(attributes, DUMMY_ATTRIBUTES);
    }

    #[test]
    fn test_set_variable() {
        let rs: &StandardRuntimeServices<'_> = runtime_services!(set_variable = mock_efi_set_variable);

        let mut data = DummyVariableType { value: DUMMY_DATA };

        let status = rs.set_variable::<DummyVariableType>(&DUMMY_NAME, &DUMMY_NAMESPACE, DUMMY_ATTRIBUTES, &mut data);

        assert!(status.is_ok());
    }

    #[test]
    #[should_panic(expected = "Name passed into set_variable is not null-terminated.")]
    fn test_set_variable_non_terminated() {
        let rs: &StandardRuntimeServices<'_> = runtime_services!(set_variable = mock_efi_set_variable);

        let mut data = DummyVariableType { value: DUMMY_DATA };

        let _ = rs.set_variable::<DummyVariableType>(
            &DUMMY_NON_NULL_TERMINATED_NAME,
            &DUMMY_NAMESPACE,
            DUMMY_ATTRIBUTES,
            &mut data,
        );
    }

    #[test]
    fn test_set_variable_empty_name() {
        let rs: &StandardRuntimeServices<'_> = runtime_services!(set_variable = mock_efi_set_variable);

        let mut data = DummyVariableType { value: DUMMY_DATA };

        let status =
            rs.set_variable::<DummyVariableType>(&DUMMY_EMPTY_NAME, &DUMMY_NAMESPACE, DUMMY_ATTRIBUTES, &mut data);

        assert!(status.is_err());
        assert_eq!(status.unwrap_err(), efi::Status::INVALID_PARAMETER);
    }

    #[test]
    fn test_set_variable_not_found() {
        let rs: &StandardRuntimeServices<'_> = runtime_services!(set_variable = mock_efi_set_variable);

        let mut data = DummyVariableType { value: DUMMY_DATA };

        let status = rs.set_variable::<DummyVariableType>(&DUMMY_UNKNOWN_NAME, &DUMMY_NAMESPACE, DUMMY_ATTRIBUTES, &mut data);

        assert!(status.is_err());
        assert_eq!(status.unwrap_err(), efi::Status::NOT_FOUND);
    }

    #[test]
    fn test_get_next_variable_name() {
        // Ensure we are testing a growing name buffer
        assert!(DUMMY_NEXT_NAME.len() > DUMMY_NAME.len());

        let rs: &StandardRuntimeServices<'_> =
            runtime_services!(get_next_variable_name = mock_efi_get_next_variable_name);

        let status = rs.get_next_variable_name(&DUMMY_NAME, &DUMMY_NAMESPACE);

        assert!(status.is_ok());

        let (next_name, next_guid) = status.unwrap();

        assert_eq!(next_name, DUMMY_NEXT_NAME);
        assert_eq!(next_guid, DUMMY_NEXT_NAMESPACE);
    }

    #[test]
    fn test_get_next_variable_name_non_terminated() {
        let rs: &StandardRuntimeServices<'_> =
            runtime_services!(get_next_variable_name = mock_efi_get_next_variable_name);

        let status = rs.get_next_variable_name(&DUMMY_NON_NULL_TERMINATED_NAME, &DUMMY_NAMESPACE);

        assert!(status.is_err());
        assert_eq!(status.unwrap_err(), efi::Status::INVALID_PARAMETER);
    }

    #[test]
    #[should_panic(expected = "Zero-length name passed into get_next_variable_name.")]
    fn test_get_next_variable_name_zero_length_name() {
        let rs: &StandardRuntimeServices<'_> =
            runtime_services!(get_next_variable_name = mock_efi_get_next_variable_name);

        let _ = rs.get_next_variable_name(&DUMMY_ZERO_LENGTH_NAME, &DUMMY_NAMESPACE);
    }

    #[test]
    fn test_get_next_variable_name_not_found() {
        let rs: &StandardRuntimeServices<'_> =
            runtime_services!(get_next_variable_name = mock_efi_get_next_variable_name);

        let status = rs.get_next_variable_name(&DUMMY_UNKNOWN_NAME, &DUMMY_NAMESPACE);

        assert!(status.is_err());
        assert_eq!(status.unwrap_err(), efi::Status::NOT_FOUND);
    }

}
