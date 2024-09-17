#![cfg_attr(all(not(test), not(feature = "mockall")), no_std)]

#[cfg(feature = "global_allocator")]
pub mod global_allocator;

extern crate alloc;

pub mod allocation;
pub mod boxed;
pub mod static_ptr;

#[cfg(any(test, feature = "mockall"))]
use mockall::automock;

use alloc::vec::Vec;
use core::{
    any::{Any, TypeId},
    ffi::c_void,
    marker::PhantomData,
    mem::{self, MaybeUninit},
    option::Option,
    panic, ptr, slice,
    sync::atomic::{AtomicPtr, Ordering},
};
use static_ptr::{StaticPtr, StaticPtrMut};

use r_efi::{efi, protocols::pci_io::Attribute};

use allocation::{AllocType, MemoryMap, MemoryType};
use boxed::RuntimeServicesBox;

/// This is the runtime services used in the UEFI.
/// it wraps an atomic ptr to [`efi::RuntimeServices`]
#[derive(Debug)]
pub struct StandardRuntimeServices<'a> {
    efi_runtime_services: AtomicPtr<efi::RuntimeServices>,
    _lifetime_marker: PhantomData<&'a efi::RuntimeServices>,
}

pub enum RuntimeServicesGetVariableStatus {
    Error(efi::Status),
    BufferTooSmall { data_size: usize, attributes: u32 },
    Success { data_size: usize, attributes: u32 },
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
            panic!("Runtime services is already initialize.")
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
                .expect("Runtime services is not initialize.")
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
        namespace: &efi::Guid,
        name: &str,
        attributes: u32,
        data: &mut T,
    ) -> Result<(), efi::Status>
    where
        T: AsMut<[u8]>,
    {
        unsafe { self.set_variable_unchecked(name, namespace, attributes, data.as_mut()) }
    }

    fn get_variable<T>(
        &self,
        name: &str,
        namespace: &efi::Guid,
        size_hint: Option<usize>,
    ) -> Result<(Option<T>, u32), efi::Status>
    where
        T: TryFrom<Vec<u8>>,
    {
        // Note: We can't simply allocate an empty T because we can't assume
        //       T::try_from will be the same size as T

        let mut data: Vec<u8> = match size_hint {
            Some(s) => Vec::<u8>::with_capacity(s),
            None => {
                let (size, _) = self.get_variable_size_and_attributes(name, namespace)?;
                Vec::<u8>::with_capacity(size)
            }
        };

        // Loop a maximum of two times: If the first iteration has a buffer that's too small
        // run it again with a buffer size that matches the returned size
        let mut allow_try_again = size_hint.is_some();
        loop {
            unsafe {
                match self.get_variable_unchecked(name, namespace, Some(&mut data)) {
                    RuntimeServicesGetVariableStatus::Success { data_size, attributes } => match T::try_from(data) {
                        Ok(d) => return Ok((Some(d), attributes)),
                        Err(_) => return Err(efi::Status::INVALID_PARAMETER),
                    },
                    RuntimeServicesGetVariableStatus::BufferTooSmall { data_size, attributes } => {
                        if allow_try_again {
                            allow_try_again = false;
                            data.reserve_exact(data_size - data.len())
                        } else {
                            return Err(efi::Status::BUFFER_TOO_SMALL);
                        }
                    }
                    RuntimeServicesGetVariableStatus::Error(e) => {
                        return Err(efi::Status::INVALID_PARAMETER);
                    }
                }
            }
        }
    }

    fn get_variable_size_and_attributes(&self, name: &str, namespace: &efi::Guid) -> Result<(usize, u32), efi::Status> {
        // Create a buffer the size T would be if in u8 form

        unsafe {
            match self.get_variable_unchecked(name, namespace, None) {
                RuntimeServicesGetVariableStatus::BufferTooSmall { data_size, attributes } => {
                    Ok((data_size, attributes))
                }
                RuntimeServicesGetVariableStatus::Error(e) => Err(e),
                RuntimeServicesGetVariableStatus::Success { data_size, attributes } => {
                    panic!("GetVariable call with zero-sized buffer returned Success.")
                }
            }
        }
    }

    unsafe fn set_variable_unchecked(
        &self,
        name: &str,
        namespace: &efi::Guid,
        attributes: u32,
        data: &mut [u8],
    ) -> Result<(), efi::Status>;

    unsafe fn get_variable_unchecked(
        &self,
        name: &str,
        namespace: &efi::Guid,
        data: Option<&mut [u8]>,
    ) -> RuntimeServicesGetVariableStatus;
}

impl RuntimeServices for StandardRuntimeServices<'_> {
    unsafe fn set_variable_unchecked(
        &self,
        name: &str,
        namespace: &efi::Guid,
        attributes: u32,
        data: &mut [u8],
    ) -> Result<(), efi::Status> {
        let set_variable = self.efi_runtime_services().set_variable;
        if set_variable as usize == 0 {
            panic!("SetVariable has not initialized in the Runtime Services Table.")
        }

        let status = set_variable(
            name.encode_utf16().collect::<Vec<u16>>().as_mut_ptr(),
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
        name: &str,
        namespace: &efi::Guid,
        data: Option<&mut [u8]>,
    ) -> RuntimeServicesGetVariableStatus {
        let set_variable = self.efi_runtime_services().get_variable;
        if set_variable as usize == 0 {
            panic!("GetVariable has not initialized in the Runtime Services Table.")
        }

        let mut data_size: usize = match data {
            Some(ref d) => d.len(),
            None => 0,
        };
        let mut attributes: u32 = 0;

        let status = set_variable(
            name.encode_utf16().collect::<Vec<u16>>().as_mut_ptr(),
            namespace as *const _ as *mut _,
            ptr::addr_of_mut!(attributes),
            ptr::addr_of_mut!(data_size),
            match data {
                Some(mut d) => ptr::addr_of_mut!(d) as *mut c_void,
                None => 0 as *mut c_void,
            },
        );

        if status == efi::Status::BUFFER_TOO_SMALL {
            return RuntimeServicesGetVariableStatus::BufferTooSmall { data_size: data_size, attributes: attributes };
        } else if status.is_error() {
            return RuntimeServicesGetVariableStatus::Error(status);
        }

        RuntimeServicesGetVariableStatus::Success { data_size: data_size, attributes: attributes }
    }
}

#[cfg(test)]
mod test {
    use efi;

    use super::*;
    use core::{mem::MaybeUninit, sync::atomic::AtomicUsize};

    macro_rules! runtime_services {
    ($($efi_services:ident = $efi_service_fn:ident),*) => {{
      static RUNTIME_SERVICE: StandardRuntimeServices = StandardRuntimeServices::new_uninit();
      let efi_runtime_services = unsafe {
        #[allow(unused_mut)]
        let mut bs = MaybeUninit::<efi::RuntimeServices>::zeroed();
        $(
          bs.assume_init_mut().$efi_services = $efi_service_fn;
        )*
        bs.assume_init()
      };
      RUNTIME_SERVICE.initialize(&efi_runtime_services);
      &RUNTIME_SERVICE
    }};
  }

    #[test]
    #[should_panic(expected = "Runtime services is not initialized.")]
    fn test_that_accessing_uninit_runtime_services_should_panic() {
        let bs = StandardRuntimeServices::new_uninit();
        bs.efi_runtime_services();
    }

    #[test]
    #[should_panic(expected = "Runtime services is already initialized.")]
    fn test_that_initializing_runtime_services_multiple_time_should_panic() {
        let efi_rs = unsafe { MaybeUninit::<efi::RuntimeServices>::zeroed().as_ptr().as_ref().unwrap() };
        let rs = StandardRuntimeServices::new_uninit();
        rs.initialize(efi_rs);
        rs.initialize(efi_rs);
    }
}
