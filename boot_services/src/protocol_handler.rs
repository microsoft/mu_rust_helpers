use core::{ffi::c_void, ops::Deref, ptr::NonNull};

use r_efi::efi;

pub unsafe trait Protocol: Deref<Target = efi::Guid> {
    type Interface;
    fn protocol_guid(&self) -> &'static efi::Guid;
}

pub type Registration = NonNull<c_void>;

#[derive(Debug, Clone, Copy)]
pub enum HandleSearchType {
    AllHandle,
    ByRegisterNotify(Registration),
    ByProtocol(&'static efi::Guid),
}

impl Into<efi::LocateSearchType> for HandleSearchType {
    fn into(self) -> efi::LocateSearchType {
        match self {
            HandleSearchType::AllHandle => efi::ALL_HANDLES,
            HandleSearchType::ByRegisterNotify(_) => efi::BY_REGISTER_NOTIFY,
            HandleSearchType::ByProtocol(_) => efi::BY_PROTOCOL,
        }
    }
}

macro_rules! impl_protocol {
    ($protocol_struct:ident, $protocol_type:ty, $guid:expr) => {
        pub struct $protocol_struct;
        unsafe impl Protocol for $protocol_struct {
            type Interface = $protocol_type;
            fn protocol_guid(&self) -> &'static efi::Guid {
                &$guid
            }
        }
        impl core::ops::Deref for $protocol_struct {
            type Target = r_efi::efi::Guid;
            fn deref(&self) -> &Self::Target {
                &self.protocol_guid()
            }
        }
    };
}

macro_rules! impl_r_efi_protocol {
    ($protocol_struct:ident, $protocol:ident) => {
        impl_protocol! {
          $protocol_struct,
          r_efi::efi::protocols::$protocol::Protocol,
          r_efi::efi::protocols::$protocol::PROTOCOL_GUID
        }
    };
}

impl_r_efi_protocol!(AbsolutePointer, absolute_pointer);
impl_r_efi_protocol!(BlockIo, block_io);
impl_r_efi_protocol!(BusSpecificDriverOverride, bus_specific_driver_override);
impl_r_efi_protocol!(DebugSupport, debug_support);
impl_r_efi_protocol!(DebugPort, debugport);
impl_r_efi_protocol!(Decompress, decompress);
impl_r_efi_protocol!(DevicePath, device_path);
impl_r_efi_protocol!(DevicePathFromText, device_path_from_text);
impl_r_efi_protocol!(DevicePathUtilities, device_path_utilities);
impl_r_efi_protocol!(DiskIo, disk_io);
impl_r_efi_protocol!(DiskIo2, disk_io2);
impl_r_efi_protocol!(DriverBinding, driver_binding);
impl_r_efi_protocol!(DriverDiagnostic2, driver_diagnostics2);
impl_r_efi_protocol!(DriverFamilyOverride, driver_family_override);
// protocol file ???;
impl_r_efi_protocol!(GraphicOutput, graphics_output);
impl_r_efi_protocol!(HiiDatabase, hii_database);
impl_r_efi_protocol!(HiiFont, hii_font);
impl_r_efi_protocol!(HiiFontEx, hii_font_ex);
// protocol hii_package_list ???;
impl_r_efi_protocol!(HiiString, hii_string);
impl_r_efi_protocol!(Ip4, ip4);
impl_r_efi_protocol!(Ip6, ip6);
impl_r_efi_protocol!(LoadFile, load_file);
impl_r_efi_protocol!(LoadFile2, load_file2);
impl_r_efi_protocol!(LoadedImage, loaded_image);
impl_protocol!(
    LoadedImageDevicePath,
    efi::protocols::loaded_image::Protocol,
    efi::protocols::loaded_image_device_path::PROTOCOL_GUID
);
impl_r_efi_protocol!(ManagedNetwork, managed_network);
impl_r_efi_protocol!(MpService, mp_services);
impl_r_efi_protocol!(PciIo, pci_io);
impl_r_efi_protocol!(PlatformDriverOverride, platform_driver_override);
impl_r_efi_protocol!(Rng, rng);
// protocol service_binding ???
impl_r_efi_protocol!(Shell, shell);
impl_r_efi_protocol!(ShellDynamicCommand, shell_dynamic_command);
impl_r_efi_protocol!(ShellParameters, shell_parameters);
impl_r_efi_protocol!(SimpleFileSystem, simple_file_system);
impl_r_efi_protocol!(SimpleNetwork, simple_network);
impl_r_efi_protocol!(SimpleTextInput, simple_text_input);
impl_r_efi_protocol!(SimpleTextInputEx, simple_text_input_ex);
impl_r_efi_protocol!(SimpleTextOutput, simple_text_output);
impl_r_efi_protocol!(Tcp4, tcp4);
impl_r_efi_protocol!(Tcp6, tcp6);
impl_r_efi_protocol!(Timerstamp, timestamp);
impl_r_efi_protocol!(Udp4, udp4);
impl_r_efi_protocol!(Udp6, udp6);
