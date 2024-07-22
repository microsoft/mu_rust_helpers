use core::{
    cell::UnsafeCell,
    fmt::{self, Debug, Display},
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicBool, Ordering},
};

use r_efi::efi;

use boot_services::BootServices;

/// Type use for mutual exclusion of data across Tpl (task priority level)
pub struct TplMutex<'a, T: ?Sized> {
    boot_services: &'a dyn BootServices,
    tpl_lock_level: efi::Tpl,
    lock: AtomicBool,
    data: UnsafeCell<T>,
}

/// RAII implementation of a [TplMutex] lock. When this structure is dropped, the lock will be unlocked.
#[must_use = "if unused the TplMutex will immediately unlock"]
pub struct TplMutexGuard<'a, T: ?Sized> {
    tpl_mutex: &'a TplMutex<'a, T>,
    release_tpl: efi::Tpl,
}

impl<'a, T> TplMutex<'a, T> {
    /// Create an new TplMutex in an unlock state.
    pub const fn new(boot_services: &'a dyn BootServices, tpl_lock_level: efi::Tpl, data: T) -> Self {
        Self { boot_services, tpl_lock_level, lock: AtomicBool::new(false), data: UnsafeCell::new(data) }
    }
}

impl<'a, T: ?Sized> TplMutex<'a, T> {
    /// Attempt to lock the mutex and return a [TplMutexGuard] if the mutex was not locked.
    ///
    /// # Panics
    /// This call will panic if the mutex is already locked.
    pub fn lock(&self) -> TplMutexGuard<T> {
        self.try_lock().map_err(|_| "Re-entrant lock").unwrap()
    }

    /// Attempt to lock the mutex and return [TplMutexGuard] if the mutex was not locked.
    ///
    /// # Errors
    /// If the mutex is already lock, then this call will return [Result::Err()].
    pub fn try_lock(&self) -> Result<TplMutexGuard<T>, ()> {
        self.lock
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .map(|_| TplMutexGuard { release_tpl: self.boot_services.raise_tpl(self.tpl_lock_level), tpl_mutex: &self })
            .map_err(|_| ())
    }
}

impl<T: ?Sized> Drop for TplMutexGuard<'_, T> {
    fn drop(&mut self) {
        self.tpl_mutex.boot_services.restore_tpl(self.release_tpl);
        self.tpl_mutex.lock.store(false, Ordering::Release);
    }
}

impl<'a, T: ?Sized> Deref for TplMutexGuard<'a, T> {
    type Target = T;
    fn deref(&self) -> &'a T {
        // SAFETY:
        // `as_ref` is guarantee to have a valid pointer because it come from a UnsafeCell.
        // This also comply to the aliasing rule because it is the only way to get a reference to the data, thus no other mutable reference to this data exist.
        unsafe { self.tpl_mutex.data.get().as_ref::<'a>().unwrap() }
    }
}

impl<'a, T: ?Sized> DerefMut for TplMutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &'a mut T {
        // SAFETY:
        // `as_ref` is guarantee to have a valid pointer because it come from a UnsafeCell.
        // This also comply to the mutability rule because it is the only way to get a reference to the data, thus no other mutable reference to this data exist.
        unsafe { self.tpl_mutex.data.get().as_mut().unwrap() }
    }
}

impl<'a, T: ?Sized + fmt::Debug> fmt::Debug for TplMutex<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let mut dbg = f.debug_struct("TplMutex");
        match self.try_lock() {
            Ok(guard) => dbg.field("data", &guard),
            Err(()) => dbg.field("data", &format_args!("<locked>")),
        };
        dbg.finish_non_exhaustive()
    }
}

impl<'a, T: ?Sized + fmt::Debug> fmt::Debug for TplMutexGuard<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Debug::fmt(self.deref(), f)
    }
}

impl<'a, T: ?Sized + fmt::Display> fmt::Display for TplMutexGuard<'a, T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Display::fmt(self.deref(), f)
    }
}

unsafe impl<T: ?Sized + Send> Sync for TplMutex<'_, T> {}
unsafe impl<T: ?Sized + Send> Send for TplMutex<'_, T> {}

unsafe impl<T: ?Sized + Sync> Sync for TplMutexGuard<'_, T> {}
unsafe impl<T: ?Sized + Send> Send for TplMutexGuard<'_, T> {}

#[cfg(test)]
mod test {
    use super::*;
    use r_efi::efi::{TPL_APPLICATION, TPL_NOTIFY};

    struct TestTplServices;

    impl BootServices for TestTplServices {
        fn raise_tpl(&self, _tpl: r_efi::efi::Tpl) -> r_efi::efi::Tpl {
            TPL_APPLICATION
        }
        fn restore_tpl(&self, tpl: r_efi::efi::Tpl) {
            assert_eq!(TPL_APPLICATION, tpl)
        }
    }

    #[derive(Debug, Default)]
    struct TestStruct {
        field: u32,
    }
    impl Display for TestStruct {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "{}", &self.field)
        }
    }

    #[test]
    fn test_try_lock() {
        let mutex = TplMutex::new(&TestTplServices, TPL_NOTIFY, 0);

        let guard_result = mutex.try_lock();
        assert!(matches!(guard_result, Ok(_)), "First lock should work.");

        for _ in 0..2 {
            assert!(
                matches!(mutex.try_lock(), Err(())),
                "Try lock should not work when there is already a lock guard."
            );
        }

        drop(guard_result);
        let guard_result = mutex.try_lock();
        assert!(matches!(guard_result, Ok(_)), "Lock should work after the guard has been dropped.");
    }

    #[test]
    #[should_panic(expected = "Re-entrant lock")]
    fn test_that_locking_a_locked_mutex_with_lock_fn_should_panic() {
        let mutex = TplMutex::new(&TestTplServices, TPL_NOTIFY, TestStruct::default());
        let guard_result = mutex.try_lock();
        assert!(matches!(guard_result, Ok(_)));
        let _ = mutex.lock();
    }

    #[test]
    fn test_debug_output_for_tpl_mutex() {
        let mutex = TplMutex::new(&TestTplServices, TPL_NOTIFY, TestStruct::default());
        assert_eq!("TplMutex { data: TestStruct { field: 0 }, .. }", format!("{mutex:?}"));
        let _guard = mutex.lock();
        assert_eq!("TplMutex { data: <locked>, .. }", format!("{mutex:?}"));
    }

    #[test]
    fn test_display_and_debug_output_for_tpl_mutex_guard() {
        let mutex = TplMutex::new(&TestTplServices, TPL_NOTIFY, TestStruct::default());
        let guard = mutex.lock();
        assert_eq!("0", format!("{guard}"));
        assert_eq!("TestStruct { field: 0 }", format!("{guard:?}"));
    }
}
