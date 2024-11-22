use alloc::slice;
use core::{
    mem,
    ops::{Deref, DerefMut},
    ptr,
};

use crate::{allocation::MemoryType, BootServices};

#[derive(Debug)]
pub struct BootServicesBox<'a, T: ?Sized, B: BootServices + ?Sized> {
    ptr: *mut T,
    boot_services: &'a B,
}

impl<'a, T, B: BootServices> BootServicesBox<'a, T, B> {
    pub fn new(value: T, memory_type: MemoryType, boot_services: &'a B) -> Self {
        let size = mem::size_of_val(&value);
        let ptr = boot_services.allocate_pool(memory_type, size).unwrap() as *mut T;
        unsafe { ptr::write(ptr, value) };
        Self { boot_services, ptr }
    }

    pub unsafe fn from_raw(ptr: *mut T, boot_services: &'a B) -> Self {
        Self { boot_services, ptr }
    }

    pub unsafe fn into_raw(self) -> *const T {
        self.ptr as *const T
    }

    pub unsafe fn into_raw_mut(self) -> *mut T {
        self.ptr
    }

    pub fn leak(self) -> &'a mut T {
        let leak = unsafe { self.ptr.as_mut() }.unwrap();
        mem::forget(self);
        leak
    }
}

impl<'a, T, B: BootServices> BootServicesBox<'a, [T], B> {
    pub unsafe fn from_raw_parts_mut(ptr: *mut T, len: usize, boot_services: &'a B) -> Self {
        let ptr = slice::from_raw_parts_mut(ptr, len) as *mut [T];
        Self { boot_services, ptr }
    }
}

impl<T: ?Sized, B: BootServices + ?Sized> Drop for BootServicesBox<'_, T, B> {
    fn drop(&mut self) {
        let _ = self.boot_services.free_pool(self.ptr as *mut u8);
    }
}

impl<T: ?Sized, B: BootServices> Deref for BootServicesBox<'_, T, B> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { self.ptr.as_ref() }.unwrap()
    }
}

impl<T: ?Sized, B: BootServices> DerefMut for BootServicesBox<'_, T, B> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { self.ptr.as_mut() }.unwrap()
    }
}

impl<T: ?Sized, B: BootServices> AsRef<T> for BootServicesBox<'_, T, B> {
    fn as_ref(&self) -> &T {
        self.deref()
    }
}

impl<T: ?Sized, B: BootServices> AsMut<T> for BootServicesBox<'_, T, B> {
    fn as_mut(&mut self) -> &mut T {
        self.deref_mut()
    }
}
