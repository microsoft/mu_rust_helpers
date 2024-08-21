use core::ops::{BitOr, BitOrAssign};

use r_efi::efi;

use crate::{boxed::BootServicesBox, BootServices};

#[derive(Debug)]
pub enum AllocType {
  AnyPage,
  MaxAddress(usize),
  Address(usize),
}

#[derive(Debug)]
pub enum MemoryType {
  ReservedMemoryType,
  LoaderCode,
  LoaderData,
  BootServicesCode,
  BootServicesData,
  RuntimeServicesCode,
  RuntimeServicesData,
  ConventionalMemory,
  UnusableMemory,
  ACPIReclaimMemory,
  ACPIMemoryNVS,
  MemoryMappedIO,
  MemoryMappedIOPortSpace,
  PalCode,
  PersistentMemory,
  UnacceptedMemoryType,
}

#[derive(Debug)]
pub struct MemoryMap<'a, B: BootServices> {
  pub descriptors: BootServicesBox<'a, [MemoryDescriptor], B>,
  pub map_key: usize,
  pub descriptor_version: u32,
}

#[derive(Debug)]
pub struct MemoryDescriptor {
  pub memory_type: MemoryType,
  pub physical_start: usize,
  pub virtual_start: usize,
  pub nb_pages: usize,
  pub attribute: MemroyAttribute,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemroyAttribute(u64);

impl MemroyAttribute {
  pub const UC: MemroyAttribute = MemroyAttribute(efi::MEMORY_UC);
  pub const WC: MemroyAttribute = MemroyAttribute(efi::MEMORY_WC);
  pub const WT: MemroyAttribute = MemroyAttribute(efi::MEMORY_WT);
  pub const WB: MemroyAttribute = MemroyAttribute(efi::MEMORY_WB);
  pub const UCE: MemroyAttribute = MemroyAttribute(efi::MEMORY_UCE);
  pub const WP: MemroyAttribute = MemroyAttribute(efi::MEMORY_WP);
  pub const RP: MemroyAttribute = MemroyAttribute(efi::MEMORY_RP);
  pub const XP: MemroyAttribute = MemroyAttribute(efi::MEMORY_XP);
  pub const NV: MemroyAttribute = MemroyAttribute(efi::MEMORY_NV);
  pub const MORE_RELIABLE: MemroyAttribute = MemroyAttribute(efi::MEMORY_MORE_RELIABLE);
  pub const RO: MemroyAttribute = MemroyAttribute(efi::MEMORY_RO);
  pub const SP: MemroyAttribute = MemroyAttribute(efi::MEMORY_SP);
  pub const CPU_CRYPTO: MemroyAttribute = MemroyAttribute(efi::MEMORY_CPU_CRYPTO);
  pub const RUNTIME: MemroyAttribute = MemroyAttribute(efi::MEMORY_RUNTIME);
  pub const ISA_VALID: MemroyAttribute = MemroyAttribute(efi::MEMORY_ISA_VALID);
  pub const ISA_MASK: MemroyAttribute = MemroyAttribute(efi::MEMORY_ISA_MASK);
}

impl BitOr for MemroyAttribute {
  type Output = MemroyAttribute;

  fn bitor(self, rhs: Self) -> Self::Output {
    MemroyAttribute(self.0 | rhs.0)
  }
}

impl BitOrAssign for MemroyAttribute {
  fn bitor_assign(&mut self, rhs: Self) {
    self.0 |= rhs.0
  }
}

impl Into<efi::AllocateType> for AllocType {
  fn into(self) -> efi::AllocateType {
    match self {
      AllocType::AnyPage => efi::ALLOCATE_ANY_PAGES,
      AllocType::MaxAddress(_) => efi::ALLOCATE_MAX_ADDRESS,
      AllocType::Address(_) => efi::ALLOCATE_ADDRESS,
    }
  }
}

impl Into<efi::MemoryType> for MemoryType {
  fn into(self) -> efi::MemoryType {
    match self {
      Self::ReservedMemoryType => efi::RESERVED_MEMORY_TYPE,
      Self::LoaderCode => efi::LOADER_CODE,
      Self::LoaderData => efi::LOADER_DATA,
      Self::BootServicesCode => efi::BOOT_SERVICES_CODE,
      Self::BootServicesData => efi::BOOT_SERVICES_DATA,
      Self::RuntimeServicesCode => efi::RUNTIME_SERVICES_CODE,
      Self::RuntimeServicesData => efi::RUNTIME_SERVICES_DATA,
      Self::ConventionalMemory => efi::CONVENTIONAL_MEMORY,
      Self::UnusableMemory => efi::UNUSABLE_MEMORY,
      Self::ACPIReclaimMemory => efi::ACPI_RECLAIM_MEMORY,
      Self::ACPIMemoryNVS => efi::ACPI_MEMORY_NVS,
      Self::MemoryMappedIO => efi::MEMORY_MAPPED_IO,
      Self::MemoryMappedIOPortSpace => efi::MEMORY_MAPPED_IO_PORT_SPACE,
      Self::PalCode => efi::PAL_CODE,
      Self::PersistentMemory => efi::PERSISTENT_MEMORY,
      Self::UnacceptedMemoryType => efi::UNACCEPTED_MEMORY_TYPE,
    }
  }
}

impl Into<u64> for MemroyAttribute {
  fn into(self) -> u64 {
    self.0
  }
}
