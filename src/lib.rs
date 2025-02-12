#![cfg_attr(not(test), no_std)]

extern crate alloc;

pub mod macros;

#[cfg(feature = "runtime_services")]
pub use runtime_services;

#[cfg(feature = "guid")]
pub use guid;

#[cfg(feature = "uefi_decompress")]
pub use uefi_decompress;

#[cfg(feature = "perf_timer")]
pub use perf_timer;
