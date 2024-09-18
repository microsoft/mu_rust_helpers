use r_efi::efi;

#[derive(Debug)]
pub enum GetVariableStatus {
    Error(efi::Status),
    BufferTooSmall { data_size: usize, attributes: u32 },
    Success { data_size: usize, attributes: u32 },
}

#[derive(Debug)]
pub struct VariableInfo {
    pub maximum_variable_storage_size: u64,
    pub remaining_variable_storage_size: u64,
    pub maximum_variable_size: u64,
}
