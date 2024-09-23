#![cfg_attr(not(test), no_std)]

extern crate alloc;

#[cfg(feature = "boot_services")]
pub use boot_services;

#[cfg(feature = "runtime_services")]
pub use runtime_services;

#[cfg(feature = "guid")]
pub use guid;

#[cfg(feature = "tpl_mutex")]
pub use tpl_mutex;
