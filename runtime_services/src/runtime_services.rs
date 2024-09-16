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
    ptr,
    sync::atomic::{AtomicPtr, Ordering},
};
use static_ptr::{StaticPtr, StaticPtrMut};

use r_efi::efi;

use allocation::{AllocType, MemoryMap, MemoryType};
use boxed::RuntimeServicesBox;

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
            panic!("Runtime services is already initialize.")
        }
    }

    /// # Panics
    /// This function will panic if it was not initialize.
    fn efi_runtime_services(&self) -> &efi::RuntimeServices {
        // SAFETY: This pointer is assume to be a valid efi::RuntimeServices pointer since the only way to set it was via an efi::RuntimeServices reference.
        unsafe {
            self.efi_runtime_services.load(Ordering::SeqCst).as_ref::<'a>().expect("Runtime services is not initialize.")
        }
    }
}

///SAFETY: StandardRuntimeServices uses an atomic ptr to access the RuntimeServices.
unsafe impl Sync for StandardRuntimeServices<'static> {}
///SAFETY: When the lifetime is `'static`, the pointer is guaranteed to stay valid.
unsafe impl Send for StandardRuntimeServices<'static> {}

#[cfg_attr(any(test, feature = "mockall"), automock)]
pub trait RuntimeServices: Sized {

}

impl RuntimeServices for StandardRuntimeServices<'_> {



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
        let efi_bs = unsafe { MaybeUninit::<efi::RuntimeServices>::zeroed().as_ptr().as_ref().unwrap() };
        let bs = StandardRuntimeServices::new_uninit();
        bs.initialize(efi_bs);
        bs.initialize(efi_bs);
    }

}
