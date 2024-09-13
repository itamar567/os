use x86_64::{
    structures::paging::{
        page::PageRangeInclusive, FrameAllocator, Mapper, Page, PageTableFlags, RecursivePageTable,
        Size4KiB,
    },
    VirtAddr,
};

#[derive(Debug)]
pub struct Stack {
    top: VirtAddr,
    bottom: VirtAddr,
}

#[allow(dead_code)]
impl Stack {
    fn new(top: VirtAddr, bottom: VirtAddr) -> Stack {
        assert!(top > bottom);
        Stack { top, bottom }
    }

    pub fn top(&self) -> VirtAddr {
        self.top
    }

    pub fn bottom(&self) -> VirtAddr {
        self.bottom
    }
}

pub struct StackAllocator {
    range: PageRangeInclusive,
}

impl StackAllocator {
    pub fn new(page_range: PageRangeInclusive) -> Self {
        Self { range: page_range }
    }

    pub fn allocate_stack<A: FrameAllocator<Size4KiB>>(
        &mut self,
        active_table: &mut RecursivePageTable,
        frame_allocator: &mut A,
        size_in_pages: usize,
    ) -> Option<Stack> {
        if size_in_pages == 0 {
            return None;
        }

        // Clone the range, since we only want to change it on success
        let mut range = self.range.clone();

        // Try to allocate the stack pages and a guard page
        let guard_page = range.next();
        let stack_start = range.next();
        let stack_end = if size_in_pages == 1 {
            stack_start
        } else {
            range.nth(size_in_pages - 2)
        };

        match (guard_page, stack_start, stack_end) {
            (Some(_), Some(start), Some(end)) => {
                // Success, update the page range
                self.range = range;

                // Map the stack to physical frames
                for page in Page::range_inclusive(start, end) {
                    unsafe {
                        active_table
                            .map_to(
                                page,
                                frame_allocator
                                    .allocate_frame()
                                    .expect("Failed to allocate frame"),
                                PageTableFlags::WRITABLE | PageTableFlags::PRESENT,
                                frame_allocator,
                            )
                            .unwrap()
                            .flush();
                    }
                }

                Some(Stack::new(
                    end.start_address() + end.size(),
                    start.start_address(),
                ))
            }
            _ => None,
        }
    }
}
