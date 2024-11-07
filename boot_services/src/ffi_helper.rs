use alloc::boxed::Box;
use core::{
    ffi::c_void,
    marker::PhantomData,
    mem::{self, ManuallyDrop},
    ptr,
};

#[derive(Clone, Copy)]
pub struct PtrMetadata<'a, T> {
    pub ptr_value: usize,
    _container: PhantomData<&'a T>,
}

impl<'a, R: CPtr<'a, Type = T>, T> PtrMetadata<'a, R> {
    pub unsafe fn into_original_ptr(self) -> R {
        mem::transmute_copy(&self.ptr_value)
    }
}

pub unsafe trait CPtr<'a>: Sized {
    type Type: Sized;

    fn as_ptr(&self) -> *const Self::Type {
        unsafe { mem::transmute_copy(&self) }
    }

    fn into_ptr(self) -> *const Self::Type {
        let this = ManuallyDrop::new(self);
        this.as_ptr()
    }

    fn metadata(&self) -> PtrMetadata<'a, Self> {
        PtrMetadata { ptr_value: self.as_ptr() as usize, _container: PhantomData }
    }
}
pub unsafe trait CMutPtr<'a>: CPtr<'a> {
    fn as_mut_ptr(&mut self) -> *mut Self::Type {
        self.as_ptr() as *mut _
    }

    fn into_mut_ptr(self) -> *mut Self::Type {
        let mut this = ManuallyDrop::new(self);
        this.as_mut_ptr()
    }
}

pub unsafe trait CRef<'a>: CPtr<'a> {
    fn as_ref(&self) -> &Self::Type {
        unsafe { mem::transmute_copy(&self) }
    }
}

pub unsafe trait CMutRef<'a>: CRef<'a> + CMutPtr<'a> {
    fn as_mut(&mut self) -> &mut Self::Type {
        unsafe { mem::transmute_copy(&self) }
    }
}

// &T
unsafe impl<'a, T> CPtr<'a> for &'a T {
    type Type = T;
}
unsafe impl<'a, T> CRef<'a> for &'a T {}

// &mut T
unsafe impl<'a, T> CPtr<'a> for &'a mut T {
    type Type = T;
}
unsafe impl<'a, T> CRef<'a> for &'a mut T {}
unsafe impl<'a, T> CMutPtr<'a> for &'a mut T {}
unsafe impl<'a, T> CMutRef<'a> for &'a mut T {}

// Box<T>
unsafe impl<'a, T> CPtr<'a> for Box<T> {
    type Type = T;
}
unsafe impl<'a, T> CRef<'a> for Box<T> {}
unsafe impl<'a, T> CMutPtr<'a> for Box<T> {}
unsafe impl<'a, T> CMutRef<'a> for Box<T> {}

// ()
unsafe impl CPtr<'static> for () {
    type Type = c_void;

    fn as_ptr(&self) -> *const Self::Type {
        ptr::null()
    }
}

unsafe impl CMutPtr<'static> for () {
    fn as_mut_ptr(&mut self) -> *mut Self::Type {
        ptr::null_mut()
    }
}

// Option<T>
unsafe impl<'a, R: CPtr<'a, Type = T>, T> CPtr<'a> for Option<R> {
    type Type = T;
}
unsafe impl<'a, R: CMutPtr<'a, Type = T>, T> CMutPtr<'a> for Option<R> {}

// ManuallyDrop<T>
unsafe impl<'a, R: CPtr<'a, Type = T>, T> CPtr<'a> for ManuallyDrop<R> {
    type Type = T;
}
unsafe impl<'a, R: CMutPtr<'a, Type = T>, T> CMutPtr<'a> for ManuallyDrop<R> {}
unsafe impl<'a, R: CRef<'a, Type = T>, T> CRef<'a> for ManuallyDrop<R> {}
unsafe impl<'a, R: CMutRef<'a, Type = T>, T> CMutRef<'a> for ManuallyDrop<R> {}

// impl<'a, T: Sized + 'a> CRef<'a> for &mut T {
//     type Type = T;

//     fn as_ref(&self) -> &Self::Type {
//         self
//     }
// }

// impl<T: Sized + 'static> CRef<'static> for Box<T> {
//     type Type = T;

//     fn as_ref(&self) -> &Self::Type {
//         Box::deref(&self)
//     }
// }

// impl<'a, T: Sized + 'a, R: CRef<'a, Type = T>> CRef<'a> for ManuallyDrop<R> {
//     type Type = T;

//     fn as_ref(&self) -> &Self::Type {
//         R::as_ref(ManuallyDrop::deref(&self))
//     }
// }

// impl<'a, T: Sized + 'a> CMutRef<'a> for &'a mut T {
//     fn as_mut(&mut self) -> &mut <Self as CRef>::Type {
//         self
//     }
// }

// impl<T: Sized + 'static> CMutRef<'static> for Box<T> {
//     fn as_mut(&mut self) -> &mut Self::Type {
//         Box::deref_mut(self)
//     }
// }

// impl<'a, T: Sized + 'a, R: CMutRef<'a, Type = T>> CMutRef<'a> for ManuallyDrop<R> {
//     fn as_mut(&mut self) -> &mut Type {
//         R::as_mut(ManuallyDrop::deref_mut(self))
//     }
// }

// impl<'a, T: Sized + 'a, R: CRef<'a, Type = T>> CPtr<'a> for R {
//     type Type = T;

//     fn as_ptr(&self) -> *const Type {
//         R::as_ref(self) as *const _
//     }
// }
// impl<'a, T: Sized + 'a, R: CRef<'a, Type = T>> CPtr<'a> for Option<R> {
//     type Type = T;

//     fn as_ptr(&self) -> *const Type {
//         match self {
//             Some(r) => R::as_ptr(r),
//             None => ptr::null_mut(),
//         }
//     }
// }

// impl<'a, T: Sized + 'a, R: CMutRef<'a, Type = T>> CMutPtr<'a> for R {
//     fn as_mut_ptr(&mut self) -> *mut Self::Type {
//         R::as_mut(self) as *mut _
//     }
// }

// impl<'a, T: Sized + 'a, R: CMutRef<'a, Type = T>> CMutPtr<'a> for Option<R> {
//     fn as_mut_ptr(&'a mut self) -> *mut <Self as CPtr>::Type {
//         match self {
//             Some(r) => R::as_mut_ptr(r),
//             None => ptr::null_mut(),
//         }
//     }
// }

// impl CPtr<'static> for () {
//     type Type = c_void;

//     fn as_ptr(&self) -> *const Self::Type {
//         ptr::null()
//     }
// }

// impl CMutPtr<'static> for () {
//     fn as_mut_ptr(&mut self) -> *mut Self::Type {
//         ptr::null_mut()
//     }
// }
