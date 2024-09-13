use core::ptr;

use alloc::string::String;

use super::ata::Disk;

#[repr(C)]
struct BiosParameterBlock {
    jmp_short3c_nop: [u8; 3],
    oem_identifier: [u8; 8],
    bytes_per_sector: u16,
    sectors_per_cluster: u8,
    reserved_sectors: u16,
    number_of_tables: u8,
    number_of_root_entries: u16,
    sector_count: u16,
    media_descriptor_type: u8,
    sectors_per_fat: u16,
    sectors_per_track: u16,
    number_of_heads: u16,
    number_of_hidden_sectors: u32,
    large_sector_count: u32,
}

#[repr(C)]
struct ExtendedBootRecord {
    drive_number: u8,
    reserved: u8,
    signature: u8,
    serial: u32,
    label: [u8; 11],
    system_identifier: [u8; 8],
    boot_code: [u8; 448],
    bootable_partition_signature: u16,
}

#[repr(C)]
struct BootRecord {
    bios_parameter_block: BiosParameterBlock,
    extended_boot_record: ExtendedBootRecord,
}

#[repr(C)]
pub struct Fat16 {
    boot_record: BootRecord,
}

impl Fat16 {
    pub fn new(disk: Disk) -> Self {
        let mut target: [u8; 512] = [0; 512];
        disk.read(&mut target, 0, 1);

        unsafe { ptr::read(target.as_ptr() as *const _) }
    }

    pub fn info(&self) -> String {
        let mut info = String::new();
        info.push_str("OEM Identifier: ");
        info.push_str(&String::from_utf8_lossy(
            &self.boot_record.bios_parameter_block.oem_identifier,
        ));
        info.push_str("\n");
        info.push_str("Label: ");
        info.push_str(&String::from_utf8_lossy(
            &self.boot_record.extended_boot_record.label,
        ));

        info
    }
}
