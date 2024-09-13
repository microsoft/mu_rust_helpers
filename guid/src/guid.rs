#![cfg_attr(target_os = "uefi", no_std)]

use r_efi::efi;
use uuid::uuid;

/// Macro for creating an `efi::Guid` from string representation.
/// This is a wrapper for `uuid!` from the uuid crate.
#[macro_export]
macro_rules! guid {
    ($guid_str:expr) => {
        efi::Guid::from_bytes(&uuid!($guid_str).to_bytes_le())
    };
}

/// Macro for printing an `efi::Guid` as a string.
#[macro_export]
macro_rules! guid_fmt {
    ($guid_object:expr) => {
        format_args!("{:X}", uuid::Uuid::from_bytes_le(*$guid_object.as_bytes()))
    };
}

/// Macro for creating a `uuid::Uuid` from an `efi::Guid`.
#[macro_export]
macro_rules! guid_to_uuid {
    ($guid_object:expr) => {
        uuid::Uuid::from_bytes_le(*$guid_object.as_bytes())
    };
}

const ZERO_GUID_STR: &str = "00000000-0000-0000-0000-000000000000";

pub const ZERO: efi::Guid = guid!(ZERO_GUID_STR);

/// Rust equivalent to `gEfiCallerIdGuid` from AutoGen.c
/// The EDK2 build system will populate the `FILE_GUID` environment variable with the module INF GUID.
/// A zero-GUID is generated as a backup to support various test cases where the EDK2 build system is not present.
pub const CALLER_ID: efi::Guid = guid!(match option_env!("FILE_GUID") {
    Some(guid_str) => guid_str,
    None => ZERO_GUID_STR,
});

#[cfg(test)]
mod tests {
    use r_efi::efi;
    use uuid::uuid;

    use crate::{CALLER_ID, ZERO, ZERO_GUID_STR};

    const MS_WHEA_RSC_DATA_TYPE_GUID_FROM_MACRO: efi::Guid = guid!("91DEEA05-8C0A-4DCD-B91E-F21CA0C68405");
    const ADVANCED_LOGGER_PROTOCOL_GUID_FROM_MACRO: efi::Guid = guid!("434F695C-EF26-4A12-9EBA-DDEF0097497C");
    const ADVANCED_LOGGER_PROTOCOL_GUID_FROM_FIELDS: efi::Guid =
        efi::Guid::from_fields(0x434f695c, 0xef26, 0x4a12, 0x9e, 0xba, &[0xdd, 0xef, 0x00, 0x97, 0x49, 0x7c]);

    #[test]
    fn test_guid_macro() {
        // `guid!` output is equivalent to `efi::Guid::from_fields()`
        assert_eq!(ADVANCED_LOGGER_PROTOCOL_GUID_FROM_MACRO, ADVANCED_LOGGER_PROTOCOL_GUID_FROM_FIELDS);
        // `CALLER_ID` is a zero-GUID when `FILE_GUID` is undefined
        assert_eq!(CALLER_ID, ZERO);
        // Zero-GUID is actually zeroes
        assert_eq!(*ZERO.as_bytes(), [0u8; 16]);
        // `guid!` is generating different output for different input
        assert_ne!(ADVANCED_LOGGER_PROTOCOL_GUID_FROM_MACRO, MS_WHEA_RSC_DATA_TYPE_GUID_FROM_MACRO);
    }

    #[test]
    fn test_guid_string_macro() {
        assert_eq!(
            "434F695C-EF26-4A12-9EBA-DDEF0097497C",
            format!("{}", guid_fmt!(guid!("434F695C-EF26-4A12-9EBA-DDEF0097497C")))
        );
        println!("Print GUID as string: {}", guid_fmt!(ADVANCED_LOGGER_PROTOCOL_GUID_FROM_MACRO));
        println!("Print GUID as string: {}", guid_fmt!(ADVANCED_LOGGER_PROTOCOL_GUID_FROM_FIELDS));
    }

    #[test]
    fn test_guid_to_uuid_macro() {
        assert_eq!(
            guid_to_uuid!(guid!("434F695C-EF26-4A12-9EBA-DDEF0097497C")),
            uuid!("434F695C-EF26-4A12-9EBA-DDEF0097497C")
        );
        assert_ne!(guid_to_uuid!(guid!("434F695C-EF26-4A12-9EBA-DDEF0097497C")), uuid!(ZERO_GUID_STR));
    }
}
