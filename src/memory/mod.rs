mod area_frame_allocator;
mod stack_allocator;

use multiboot2::{BootInformation, ElfSectionFlags};
use x86_64::{
    instructions::tlb,
    registers::control::Cr3,
    structures::paging::{
        FrameAllocator, Mapper, Page, PageTable, PageTableFlags, PhysFrame, RecursivePageTable,
        Size4KiB,
    },
    PhysAddr, VirtAddr,
};

use crate::HEAP_ALLOCATOR;

pub use self::area_frame_allocator::AreaFrameAllocator;
use self::stack_allocator::{Stack, StackAllocator};

const HEAP_START: *mut u8 = 0o_000_001_000_000_0000 as *mut u8;
const HEAP_SIZE: usize = 100 * 1024; // 100 KiB

const P4: *mut PageTable = 0xffffffff_fffff000 as *mut _;

unsafe fn get_active_page_table() -> RecursivePageTable<'static> {
    RecursivePageTable::new(&mut *P4).unwrap()
}

pub struct MemoryController {
    active_page_table: RecursivePageTable<'static>,
    frame_allocator: AreaFrameAllocator<'static>,
    stack_allocator: StackAllocator,
}

impl MemoryController {
    pub fn allocate_stack(&mut self, size_in_pages: usize) -> Option<Stack> {
        self.stack_allocator.allocate_stack(
            &mut self.active_page_table,
            &mut self.frame_allocator,
            size_in_pages,
        )
    }
}

/// Initialize the memory
///
/// SAFTEY: This function should only be called once
pub unsafe fn init(boot_info: &'static BootInformation) -> MemoryController {
    let memory_map_tag = boot_info.memory_map_tag().expect("Memory map tag required");
    let elf_sections_tag = boot_info.elf_sections().expect("ELF-sections tag required");

    // Calculate the kernel and multiboot information addresses
    let kernel_start = elf_sections_tag
        .clone()
        .filter(|s| s.is_allocated())
        .map(|s| s.start_address())
        .min()
        .unwrap();
    let kernel_end = elf_sections_tag
        .filter(|s| s.is_allocated())
        .map(|s| s.end_address())
        .max()
        .unwrap();

    println!("Kernel address: {:#x}-{:#x}", kernel_start, kernel_end);
    println!(
        "Multiboot information address: {:#x}-{:#x}",
        boot_info.start_address(),
        boot_info.end_address()
    );

    // Create the allocator
    let mut frame_allocator = AreaFrameAllocator::new(
        PhysAddr::new(kernel_start),
        PhysAddr::new(kernel_end),
        PhysAddr::new(boot_info.start_address() as u64),
        PhysAddr::new(boot_info.end_address() as u64),
        memory_map_tag.memory_areas(),
    );

    // Remap the kernel
    unsafe { remap_kernel(&mut frame_allocator, &boot_info) };

    let mut active_page_table = get_active_page_table();

    // Map the heap
    let heap_start_page = Page::containing_address(VirtAddr::new(HEAP_START as u64));
    let heap_end_page =
        Page::containing_address(VirtAddr::new(HEAP_START as u64 + HEAP_SIZE as u64 - 1));

    for page in Page::range_inclusive(heap_start_page, heap_end_page) {
        active_page_table
            .map_to(
                page,
                frame_allocator
                    .allocate_frame()
                    .expect("Failed to allocate heap frame"),
                PageTableFlags::WRITABLE | PageTableFlags::PRESENT,
                &mut frame_allocator,
            )
            .unwrap()
            .flush();
    }

    // Initialize the heap allocator
    unsafe { HEAP_ALLOCATOR.lock().init(HEAP_START, HEAP_SIZE) };

    let stack_allocator = stack_allocator::StackAllocator::new(Page::range_inclusive(
        heap_end_page + 1,
        heap_end_page + 101,
    ));

    MemoryController {
        active_page_table,
        frame_allocator,
        stack_allocator,
    }
}

/// Remap the kernel to a new page table, and activate the new page table
///
/// SAFTEY: This function replaces the active page table, and therefore should only be called once
/// when setting up a new page table
unsafe fn remap_kernel<A: FrameAllocator<Size4KiB>>(
    frame_allocator: &mut A,
    boot_info: &BootInformation,
) {
    let new_table_frame = frame_allocator
        .allocate_frame()
        .expect("No frames available");
    let new_table = &mut *(new_table_frame.start_address().as_u64() as *mut PageTable);
    // Clear the table
    new_table.zero();
    // Set up recursive mapping
    new_table[511].set_addr(
        new_table_frame.start_address(),
        PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
    );

    // Save the address of the currently active page table for later
    let original_page_table_address;

    {
        // Overwrite recursive mapping
        let mut active_page_table = get_active_page_table();
        // The table is recursively mapped, so the last entry points to its physical address
        original_page_table_address = active_page_table.level_4_table()[511].addr();
        active_page_table.level_4_table()[511].set_addr(
            new_table_frame.start_address(),
            PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
        );
        tlb::flush_all();

        let elf_sections_tag = boot_info.elf_sections().expect("Memory map tag required");
        for section in elf_sections_tag {
            if !section.is_allocated() {
                // Section is not loaded to memory
                continue;
            }

            let mut flags = PageTableFlags::empty();

            if section.flags().contains(ElfSectionFlags::ALLOCATED) {
                // section is loaded to memory
                flags = flags | PageTableFlags::PRESENT;
            }
            if section.flags().contains(ElfSectionFlags::WRITABLE) {
                flags = flags | PageTableFlags::WRITABLE;
            }
            if !section.flags().contains(ElfSectionFlags::EXECUTABLE) {
                flags = flags | PageTableFlags::NO_EXECUTE;
            }

            let start_frame =
                PhysFrame::<Size4KiB>::from_start_address(PhysAddr::new(section.start_address()))
                    .expect("Kernel sections not aligned");
            let end_frame = PhysFrame::containing_address(PhysAddr::new(section.end_address() - 1));
            // Identity map the new table
            for frame in PhysFrame::range_inclusive(start_frame, end_frame) {
                active_page_table
                    .identity_map(frame, flags, frame_allocator)
                    .unwrap()
                    .flush();
            }
        }

        // identity map the VGA text buffer
        let vga_buffer_frame: PhysFrame<Size4KiB> =
            PhysFrame::containing_address(PhysAddr::new(0xb8000));
        active_page_table
            .identity_map(
                vga_buffer_frame,
                PageTableFlags::WRITABLE | PageTableFlags::PRESENT,
                frame_allocator,
            )
            .unwrap()
            .flush();

        let multiboot_start =
            PhysFrame::containing_address(PhysAddr::new(boot_info.start_address() as u64));
        let multiboot_end =
            PhysFrame::containing_address(PhysAddr::new(boot_info.end_address() as u64 - 1));
        for frame in PhysFrame::<Size4KiB>::range_inclusive(multiboot_start, multiboot_end) {
            active_page_table
                .identity_map(frame, PageTableFlags::PRESENT, frame_allocator)
                .unwrap()
                .flush();
        }
    }

    Cr3::write(new_table_frame, Cr3::read().1);

    // Turn the original p4 page into a guard page
    let old_p4_page =
        Page::<Size4KiB>::containing_address(VirtAddr::new(original_page_table_address.as_u64()));
    get_active_page_table()
        .unmap(old_p4_page)
        .unwrap()
        .1
        .flush();
    println!("Guard page at {:#x}", old_p4_page.start_address());
}
