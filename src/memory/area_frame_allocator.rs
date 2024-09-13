use multiboot2::MemoryArea;
use x86_64::{
    structures::paging::{FrameAllocator, PageSize, PhysFrame, Size4KiB},
    PhysAddr,
};

pub struct AreaFrameAllocator<'a> {
    next_free_frame: PhysFrame,
    current_area: Option<&'a MemoryArea>,
    areas: &'a [MemoryArea],
    kernel_start: PhysFrame,
    kernel_end: PhysFrame,
    multiboot_start: PhysFrame,
    multiboot_end: PhysFrame,
}

impl AreaFrameAllocator<'_> {
    pub fn new(
        kernel_start: PhysAddr,
        kernel_end: PhysAddr,
        multiboot_start: PhysAddr,
        multiboot_end: PhysAddr,
        memory_areas: &[MemoryArea],
    ) -> AreaFrameAllocator {
        let mut allocator = AreaFrameAllocator {
            // We skip the frame at `0x0` to avoid `translate` functions thinking the entry
            // pointing to it is unused
            next_free_frame: PhysFrame::from_start_address(PhysAddr::new(Size4KiB::SIZE)).unwrap(),
            current_area: None,
            areas: memory_areas,
            kernel_start: PhysFrame::containing_address(kernel_start),
            kernel_end: PhysFrame::containing_address(kernel_end),
            multiboot_start: PhysFrame::containing_address(multiboot_start),
            multiboot_end: PhysFrame::containing_address(multiboot_end),
        };
        allocator.choose_next_area();

        allocator
    }

    fn choose_next_area(&mut self) {
        self.current_area = self
            .areas
            .iter()
            .filter(|area| {
                PhysFrame::containing_address(PhysAddr::new(area.end_address() - 1))
                    >= self.next_free_frame
            })
            .min_by_key(|area| area.start_address());

        if let Some(current_area) = self.current_area {
            let start_frame =
                PhysFrame::containing_address(PhysAddr::new(current_area.start_address()));

            if self.next_free_frame < start_frame {
                self.next_free_frame = start_frame;
            }
        }
    }
}

unsafe impl FrameAllocator<Size4KiB> for AreaFrameAllocator<'_> {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        if let Some(area) = self.current_area {
            let frame = self.next_free_frame.clone();

            // the last frame of the current area
            let current_area_last_frame =
                PhysFrame::containing_address(PhysAddr::new(area.end_address() - 1));

            if frame > current_area_last_frame {
                // all frames of current area are used, switch to next area
                self.choose_next_area();
            } else if frame >= self.kernel_start && frame <= self.kernel_end {
                // `frame` is used by the kernel
                self.next_free_frame =
                    PhysFrame::from_start_address(self.kernel_end.start_address() + Size4KiB::SIZE)
                        .unwrap();
            } else if frame >= self.multiboot_start && frame <= self.multiboot_end {
                // `frame` is used by the multiboot information structure
                self.next_free_frame = PhysFrame::from_start_address(
                    self.multiboot_end.start_address() + Size4KiB::SIZE,
                )
                .unwrap();
            } else {
                // frame is unused, increment `next_free_frame` and return it
                self.next_free_frame = PhysFrame::from_start_address(
                    self.next_free_frame.start_address() + Size4KiB::SIZE,
                )
                .unwrap();
                return Some(frame);
            }
            // `frame` was not valid, try it again with the updated `next_free_frame`
            self.allocate_frame()
        } else {
            None // no free frames left
        }
    }
}
