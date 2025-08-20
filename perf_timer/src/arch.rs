use core::sync::atomic::AtomicU64;

#[cfg(target_arch = "x86_64")]
pub use x64::X64 as Arch;

#[cfg(target_arch = "aarch64")]
pub use aarch64::Aarch64 as Arch;

// QEMU uses the ACPI frequency when CPUID-based frequency determination is not available.
const DEFAULT_ACPI_TIMER_FREQUENCY: u64 = 3579545;

static PERF_FREQUENCY: AtomicU64 = AtomicU64::new(0);

pub trait ArchFunctionality {
    /// Value of the counter.
    fn cpu_count() -> u64;
    /// Value in Hz of how often the counter increment.
    fn perf_frequency() -> u64;
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
    use core::{
        arch::x86_64::{self, CpuidResult},
        sync::atomic::Ordering,
    };

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

        fn perf_frequency() -> u64 {
            let cached = PERF_FREQUENCY.load(Ordering::Relaxed);
            if cached != 0 {
                return cached;
            }

            let hypervisor_leaf = unsafe { x86_64::__cpuid(0x1) };
            let is_vm = (hypervisor_leaf.ecx & (1 << 31)) != 0;

            if is_vm {
                log::warn!("Running in a VM - CPUID-based frequency may not be reliable.");
            }

            let CpuidResult {
                eax, // Ratio of TSC frequency to Core Crystal Clock frequency, denominator.
                ebx, // Ratio of TSC frequency to Core Crystal Clock frequency, numerator.
                ecx, // Core Crystal Clock frequency, in units of Hz.
                ..
            } = unsafe { x86_64::__cpuid(0x15) };

            // If not a VM, attempt to use CPUID leaf 0x15
            if !is_vm && ecx != 0 && eax != 0 && ebx != 0 {
                let frequency = (ecx as u64 * ebx as u64) / eax as u64;
                PERF_FREQUENCY.store(frequency, Ordering::Relaxed);
                log::trace!("Used CPUID leaf 0x15 to determine CPU frequency: {}", frequency);
                return frequency;
            }

            // If VM or CPUID 0x15 fails, attempt to use CPUID 0x16
            // Based on testing in QEMU, leaf 0x16 is generally more reliable on VMs
            let CpuidResult { eax, .. } = unsafe { x86_64::__cpuid(0x16) };
            if eax != 0 {
                // Leaf 0x16 gives the frequency in MHz.
                let frequency = (eax * 1_000_000) as u64;
                PERF_FREQUENCY.store(frequency, Ordering::Relaxed);
                log::trace!("Used CPUID leaf 0x16 to determine CPU frequency: {}", frequency);
                return frequency;
            }

            log::warn!("Unable to determine CPU frequency using CPUID leaves, using default ACPI timer frequency");

            PERF_FREQUENCY.store(DEFAULT_ACPI_TIMER_FREQUENCY, Ordering::Relaxed);
            DEFAULT_ACPI_TIMER_FREQUENCY
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

        fn perf_frequency() -> u64 {
            registers::CNTFRQ_EL0.get()
        }
    }
}
