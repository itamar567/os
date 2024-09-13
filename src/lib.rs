#![no_std]
#![feature(panic_info_message)]
#![feature(abi_x86_interrupt)]

extern crate alloc;
extern crate linked_list_allocator;
extern crate multiboot2;
extern crate pc_keyboard;
extern crate pic8259;
extern crate spin;
extern crate x86_64;

#[macro_use]
mod vga_buffer;
mod disk;
mod interrupts;
mod memory;

use core::panic::PanicInfo;

use alloc::string::String;
use linked_list_allocator::LockedHeap;
use multiboot2::{BootInformation, BootInformationHeader};
use spin::Once;
use x86_64::{
    instructions::hlt,
    registers::{
        control::{Cr0, Cr0Flags},
        model_specific::{Efer, EferFlags},
    },
};

#[global_allocator]
static HEAP_ALLOCATOR: LockedHeap = LockedHeap::empty();

static BOOT_INFO: Once<BootInformation> = Once::new();

#[no_mangle]
extern "C" fn rust_main(multiboot_info_address: usize) {
    // Get the boot information from multiboot
    let boot_info = BOOT_INFO.call_once(|| unsafe {
        multiboot2::BootInformation::load(multiboot_info_address as *const BootInformationHeader)
            .expect("Failed to parse boot information")
    });

    // Enable the `No Execute Enable` bit
    unsafe {
        Efer::update(|flags| *flags |= EferFlags::NO_EXECUTE_ENABLE);
    }
    // Enable write protection
    unsafe {
        Cr0::update(|flags| *flags |= Cr0Flags::WRITE_PROTECT);
    };

    // Initialize the memory
    let mut memory_controller = unsafe { memory::init(&boot_info) };

    interrupts::init(&mut memory_controller);

    println!("{}", disk::FILESYSTEM.lock().info());

    loop {
        hlt();
    }
}

#[panic_handler]
#[no_mangle]
fn panic_fmt(info: &PanicInfo) -> ! {
    print!("\nKernel panic at ");
    if let Some(location) = info.location() {
        print!("{}:{}", location.file(), location.line());
    } else {
        print!("unknown");
    }
    println!(":");

    if let Some(message) = info.message() {
        println!("  {}", message);
    }

    loop {
        hlt();
    }
}
