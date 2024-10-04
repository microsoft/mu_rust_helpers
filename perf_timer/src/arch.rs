use core::u64;

#[cfg(target_arch = "x86_64")]
pub use x64::X64 as Arch;

#[cfg(target_arch = "aarch64")]
pub use aarch64::Aarch64 as Arch;

pub trait ArchFunctionality {
    /// Value of the counter.
    fn cpu_count() -> u64;
    /// Value in Hz of how often the counter increment.
    fn cpu_count_frequency() -> u64;
    /// Value the performance counter starts with when it rolls over.
    fn cpu_count_start() -> u64 {
        0
    }
    /// Value that the performance counter ends with before it rolls over.
    fn cpu_count_end() -> u64 {
        u64::MAX
    }
}

#[cfg(target_arch = "x86_64")]
pub(crate) mod x64 {
    use super::*;
    use core::arch::x86_64::{self, CpuidResult};

    pub struct X64;
    impl ArchFunctionality for X64 {
        fn cpu_count() -> u64 {
            #[cfg(feature = "validate_cpu_features")]
            {
                // TSC support in bit 4.
                if (unsafe { x86_64::__cpuid(0x01) }.edx & 0x10) != 0x10 {
                    panic!("CPU does not support TSC");
                }
                // Invariant TSC support in bit 8.
                if (unsafe { x86_64::__cpuid(0x80000007) }.edx & 0x100) != 0x100 {
                    panic!("CPU does not support Invariant TSC");
                }
            }
            unsafe { x86_64::_rdtsc() }
        }

        fn cpu_count_frequency() -> u64 {
            // https://en.wikipedia.org/wiki/CPUID
            let CpuidResult {
                eax, // Ratio of TSC frequency to Core Crystal Clock frequency, denominator.
                ebx, // Ratio of TSC frequency to Core Crystal Clock frequency, numerator.
                ecx, // Core Crystal Clock frequency, in units of Hz.
                ..
            } = unsafe { x86_64::__cpuid(0x15) };

            #[cfg(feature = "validate_cpu_features")]
            if ecx == 0 {
                panic!("CPU does not support CPUID-based frequency determination");
            }

            (ecx * (ebx / eax)) as u64
        }
    }
}

#[cfg(target_arch = "aarch64")]
pub(crate) mod aarch64 {
    use super::*;
    use aarch64_cpu::registers::{self, Readable};
    pub struct Aarch64;
    impl ArchFunctionality for Aarch64 {
        fn cpu_count() -> u64 {
            registers::CNTPCT_EL0.get()
        }

        fn cpu_count_frequency() -> u64 {
            registers::CNTFRQ_EL0.get()
        }
    }
}
