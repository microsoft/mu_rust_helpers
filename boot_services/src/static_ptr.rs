use core::{
    mem::ManuallyDrop,
    ops::{Deref, DerefMut},
    pin::Pin,
    ptr,
};

use alloc::boxed::Box;

/// <div class="warning">
///
/// This should be implemented **only** on type that have the same memory layout as `*mut T` and that can be recreated with [`core::mem::transmute`].
///
/// </div>
pub unsafe trait StaticPtr {
    type Pointee: Sized + 'static;
    fn into_raw(self) -> *const Self::Pointee;
}

/// <div class="warning">
///
/// This should be implemented **only** on type that have the same memory layout as `*mut T` and that can be recreated with [`core::mem::transmute`].
///
/// </div>
pub unsafe trait StaticPtrMut: StaticPtr {
    fn into_raw_mut(self) -> *mut Self::Pointee;
}

// &'static T

unsafe impl<T> StaticPtr for &'static T
where
    T: Sized + Sync,
{
    type Pointee = T;
    fn into_raw(self) -> *const Self::Pointee {
        self as *const T
    }
}

// &'static mut T

unsafe impl<T> StaticPtr for &'static mut T
where
    T: Sized + Sync,
{
    type Pointee = T;
    fn into_raw(self) -> *const Self::Pointee {
        self as *const T
    }
}

unsafe impl<T> StaticPtrMut for &'static mut T
where
    T: Sized + Sync,
{
    fn into_raw_mut(self) -> *mut Self::Pointee {
        self as *mut T
    }
}

// Box<T>

unsafe impl<T> StaticPtr for Box<T>
where
    T: Sized + 'static,
{
    type Pointee = T;
    fn into_raw(self) -> *const Self::Pointee {
        ptr::from_ref(Box::leak(self))
    }
}

unsafe impl<T> StaticPtrMut for Box<T>
where
    T: Sized + 'static,
{
    fn into_raw_mut(self) -> *mut Self::Pointee {
        ptr::from_mut(Box::leak(self))
    }
}

// Option<T>

unsafe impl<T> StaticPtr for Option<T>
where
    T: StaticPtr,
{
    type Pointee = T::Pointee;

    fn into_raw(self) -> *const Self::Pointee {
        Option::map_or(self, ptr::null(), |t| T::into_raw(t))
    }
}

unsafe impl<T> StaticPtrMut for Option<T>
where
    T: StaticPtrMut,
{
    fn into_raw_mut(self) -> *mut Self::Pointee {
        Option::map_or(self, ptr::null_mut(), |t| T::into_raw_mut(t))
    }
}

// ManuallyDrop<T>

unsafe impl<T> StaticPtr for ManuallyDrop<T>
where
    T: StaticPtr,
{
    type Pointee = T::Pointee;

    fn into_raw(self) -> *const Self::Pointee {
        ManuallyDrop::into_inner(self).into_raw()
    }
}

unsafe impl<T> StaticPtrMut for ManuallyDrop<T>
where
    T: StaticPtrMut,
{
    fn into_raw_mut(self) -> *mut Self::Pointee {
        ManuallyDrop::into_inner(self).into_raw_mut()
    }
}

// Pin<T>

unsafe impl<T> StaticPtr for Pin<T>
where
    T: StaticPtr + Deref,
    <T as Deref>::Target: Unpin,
{
    type Pointee = T::Pointee;

    fn into_raw(self) -> *const Self::Pointee {
        Pin::into_inner(self).into_raw()
    }
}

unsafe impl<T> StaticPtrMut for Pin<T>
where
    T: StaticPtrMut + DerefMut,
    <T as Deref>::Target: Unpin,
{
    fn into_raw_mut(self) -> *mut Self::Pointee {
        Pin::into_inner(self).into_raw_mut()
    }
}
