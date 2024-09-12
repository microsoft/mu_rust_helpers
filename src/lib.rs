#![cfg_attr(not(test), no_std)]

extern crate alloc;

#[cfg(feature = "boot_services")]
pub use boot_services;

#[cfg(feature = "guid_helpers")]
pub use guid_helpers;

#[cfg(feature = "tpl_mutex")]
pub use tpl_mutex;
