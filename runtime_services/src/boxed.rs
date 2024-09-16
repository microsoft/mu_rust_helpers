use alloc::slice;
use core::{
    mem,
    ops::{Deref, DerefMut},
    ptr,
};

use crate::{allocation::MemoryType, RuntimeServices};

#[derive(Debug)]
pub struct RuntimeServicesBox<'a, T: ?Sized, B: RuntimeServices> {
    ptr: *mut T,
    runtime_services: &'a B,
}

impl<'a, T, B: RuntimeServices> RuntimeServicesBox<'a, T, B> {
/*
    pub fn new(value: T, memory_type: MemoryType, runtime_services: &'a B) -> Self {
        let size = mem::size_of_val(&value);
        let ptr = runtime_services.allocate_pool(memory_type, size).unwrap() as *mut T;
        unsafe { ptr::write(ptr, value) };
        Self { runtime_services, ptr }
    }
*/
    pub unsafe fn from_raw(ptr: *mut T, runtime_services: &'a B) -> Self {
        Self { runtime_services, ptr }
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

impl<'a, T, B: RuntimeServices> RuntimeServicesBox<'a, [T], B> {
    pub unsafe fn from_raw_parts(ptr: *mut T, len: usize, runtime_services: &'a B) -> Self {
        let ptr = slice::from_raw_parts_mut(ptr, len) as *mut [T];
        Self { runtime_services, ptr }
    }
}

impl<T: ?Sized, B: RuntimeServices> Drop for RuntimeServicesBox<'_, T, B> {

    fn drop(&mut self) {
        //let _ = self.runtime_services.free_pool(self.ptr as *mut u8);
    }
}

impl<T: ?Sized, B: RuntimeServices> Deref for RuntimeServicesBox<'_, T, B> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { self.ptr.as_ref() }.unwrap()
    }
}

impl<T: ?Sized, B: RuntimeServices> DerefMut for RuntimeServicesBox<'_, T, B> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { self.ptr.as_mut() }.unwrap()
    }
}

impl<T: ?Sized, B: RuntimeServices> AsRef<T> for RuntimeServicesBox<'_, T, B> {
    fn as_ref(&self) -> &T {
        self.deref()
    }
}

impl<T: ?Sized, B: RuntimeServices> AsMut<T> for RuntimeServicesBox<'_, T, B> {
    fn as_mut(&mut self) -> &mut T {
        self.deref_mut()
    }
}
