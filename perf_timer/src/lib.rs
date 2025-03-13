#![cfg_attr(not(test), no_std)]

mod arch;

use core::time::Duration;

pub use arch::{Arch, ArchFunctionality};

/// This struct is used to calculate the duration between two instant.
///
/// # Example
/// ```no_run
/// use perf_timer::Instant;
///
/// let start = Instant::now();
///
/// // ...
///
/// let duration = start.elapsed();
/// ```
pub struct Instant {
    cpu_count: u64,
    frequency: u64,
}

impl Instant {
    /// Create a new instant.
    pub fn now() -> Self {
        Self::from_cpu_count(Arch::cpu_count())
    }

    /// Create a new instant from a cpu count.
    pub fn from_cpu_count(cpu_count: u64) -> Self {
        Self { cpu_count, frequency: Arch::perf_frequency() }
    }

    /// Create a new instant from the start of the counter.
    pub fn beginning() -> Self {
        Self { cpu_count: Arch::cpu_count_start(), frequency: Arch::perf_frequency() }
    }

    /// Return the amount of time from `earlier` adn this instant.
    ///
    /// # Panic
    /// This function will panic if earlier is not in the past.
    pub fn duration_since(&self, earlier: &Self) -> Duration {
        if earlier.cpu_count > self.cpu_count {
            panic!("earlier not in the past.");
        }
        let diff = (self.cpu_count - earlier.cpu_count) as f64;
        Duration::from_secs_f64(diff / self.frequency as f64)
    }

    /// Return the amount of time that elapsed since now and this instant.
    pub fn elapsed(&self) -> Duration {
        Instant::now().duration_since(self)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::thread;

    #[ignore = "Register / instruction return nonsense in the Azure pipeline vm."]
    #[test]
    fn test_instant() {
        let ns = 1_000_000_000;

        let start = Instant::now();
        thread::sleep(Duration::from_nanos(ns));
        let duration = start.elapsed();

        let precision = (duration.as_nanos() as u64 - ns) as f64 / ns as f64 * 100_f64;
        assert!(precision < 0.1, "precision is: {precision}");
    }
}
