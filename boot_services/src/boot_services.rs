use core::{
    marker::PhantomData,
    ptr,
    sync::atomic::{AtomicPtr, Ordering},
};

use r_efi::efi::{self, Tpl};

pub trait BootServices {
    /// Raises a task’s priority level and returns its previous level.
    fn raise_tpl(&self, tpl: Tpl) -> Tpl;
    /// Restores a task’s priority level to its previous value.
    fn restore_tpl(&self, tpl: Tpl);
}

#[derive(Debug)]
pub struct StandardBootServices<'a> {
    efi_boot_services: AtomicPtr<efi::BootServices>,
    _lifetime_marker: PhantomData<&'a efi::BootServices>,
}

impl<'a> StandardBootServices<'a> {
    /// Create a new StandardBootServices with the provided [efi::BootServices].
    pub const fn new(efi_boot_services: &'a efi::BootServices) -> Self {
        // The efi::BootServices is only read, that is why we use a non mutable reference.
        Self {
            efi_boot_services: AtomicPtr::new(efi_boot_services as *const _ as *mut _),
            _lifetime_marker: PhantomData,
        }
    }

    /// Create a new StandardBootServices that is uninitialized.
    /// The struct need to be initialize later with [Self::initialize], otherwise, subsequent call will panic.
    pub const fn new_uninit() -> Self {
        Self { efi_boot_services: AtomicPtr::new(ptr::null_mut()), _lifetime_marker: PhantomData }
    }

    /// Initialize the StandardBootServices with a reference to [efi::BootServices].
    /// # Panics
    /// This function will panic if already initialize.
    pub fn initialize(&'a self, efi_boot_services: &'a efi::BootServices) {
        if self.efi_boot_services.load(Ordering::Relaxed).is_null() {
            // The efi::BootServices is only read, that is why we use a non mutable reference.
            self.efi_boot_services.store(efi_boot_services as *const _ as *mut _, Ordering::SeqCst)
        } else {
            panic!("Boot services already initialize.")
        }
    }

    /// # Panics
    /// This function will panic if it was not initialize.
    fn efi_boot_services(&self) -> &efi::BootServices {
        // SAFETY: This pointer is assume to be a valid efi::BootServices pointer since the only way to set it was via an efi::BootServices reference.
        unsafe {
            self.efi_boot_services
                .load(Ordering::SeqCst)
                .as_ref::<'a>()
                .expect("Boot services has not been initialize.")
        }
    }
}

unsafe impl Sync for StandardBootServices<'_> {}
unsafe impl Send for StandardBootServices<'_> {}

impl BootServices for StandardBootServices<'_> {
    fn raise_tpl(&self, new_tpl: efi::Tpl) -> efi::Tpl {
        (self.efi_boot_services().raise_tpl)(new_tpl)
    }

    fn restore_tpl(&self, old_tpl: efi::Tpl) {
        (self.efi_boot_services().restore_tpl)(old_tpl)
    }
}

#[cfg(test)]
mod test {
    use core::mem::MaybeUninit;

    use super::*;

    #[test]
    #[should_panic(expected = "Boot services has not been initialize.")]
    fn test_that_accessing_uninit_boot_services_should_panic() {
        let bs = StandardBootServices::new_uninit();
        bs.efi_boot_services();
    }

    #[test]
    #[should_panic(expected = "Boot services already initialize.")]
    fn test_that_initializing_boot_services_multiple_time_should_panic() {
        let efi_bs = unsafe { MaybeUninit::<efi::BootServices>::zeroed().as_ptr().as_ref().unwrap() };
        let bs = StandardBootServices::new_uninit();
        bs.initialize(efi_bs);
        bs.initialize(efi_bs);
    }
}
