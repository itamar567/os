use spin::Mutex;
use x86_64::instructions::port::{Port, PortReadOnly, PortWriteOnly};

static ATA_INTERRUPT_PORT: Mutex<Port<u8>> = Mutex::new(Port::new(0x3f6));

static SECTOR_COUNT_PORT: Mutex<Port<u16>> = Mutex::new(Port::new(0x1f2));
static LBA_LOW_PORT: Mutex<Port<u8>> = Mutex::new(Port::new(0x1f3));
static LBA_MID_PORT: Mutex<Port<u8>> = Mutex::new(Port::new(0x1f4));
static LBA_HIGH_PORT: Mutex<Port<u8>> = Mutex::new(Port::new(0x1f5));
static DRIVE_PORT: Mutex<Port<u8>> = Mutex::new(Port::new(0x1f6));
static DATA_PORT: Mutex<Port<u32>> = Mutex::new(Port::new(0x1f0));
static STATUS_PORT: Mutex<PortReadOnly<u8>> = Mutex::new(PortReadOnly::new(0x1f7));
static COMMAND_PORT: Mutex<PortWriteOnly<u8>> = Mutex::new(PortWriteOnly::new(0x1f7));

const READ_COMMAND: u8 = 0x20;
//const WRITE = 0x30;
const STATUS_BUSY: u8 = 0b10000000;
const STATUS_READY: u8 = 0b01000000;

pub struct Disk;

impl Disk {
    pub fn read<T>(&self, mut target: *mut T, logical_block_address: u32, amount_of_sectors: u16) {
        // Disable ATA interrupt
        unsafe { ATA_INTERRUPT_PORT.lock().write(2) };

        // Specify drive index, sector amount, and LBA
        unsafe {
            SECTOR_COUNT_PORT.lock().write(amount_of_sectors);
            DRIVE_PORT
                .lock()
                .write((0xE0 | ((logical_block_address >> 24) & 0xF)) as u8); // 0xE0 (master drive) ORed with highest 4 bits of LBA
            LBA_LOW_PORT.lock().write(logical_block_address as u8);
            LBA_LOW_PORT
                .lock()
                .write((logical_block_address >> 8) as u8);
            LBA_LOW_PORT
                .lock()
                .write((logical_block_address >> 16) as u8);
        }

        // Send read command
        unsafe { COMMAND_PORT.lock().write(READ_COMMAND) };

        for _ in 0..amount_of_sectors {
            // A sector is 512 bytes, and each buffer is 4 bytes
            for i in 0..(512 / 4) {
                while self.is_busy() || !self.is_ready() {}

                unsafe {
                    let buffer = DATA_PORT.lock().read();
                    core::ptr::write_unaligned(target as *mut u32, buffer);
                    target = target.byte_add(4);
                };
            }
        }

        self.reset();
    }

    fn is_ready(&self) -> bool {
        let status;
        unsafe {
            status = STATUS_PORT.lock().read();
        }

        (status & STATUS_READY) != 0
    }

    fn is_busy(&self) -> bool {
        let status;
        unsafe {
            status = STATUS_PORT.lock().read();
        }

        (status & STATUS_BUSY) != 0
    }

    fn reset(&self) {
        unsafe {
            ATA_INTERRUPT_PORT.lock().write(6);
            ATA_INTERRUPT_PORT.lock().write(2);
        }
    }
}
