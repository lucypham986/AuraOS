use linked_list_allocator::LockedHeap;
use x86_64::registers::control::Cr3;
use x86_64::structures::paging::{
    FrameAllocator, Mapper, OffsetPageTable, Page, PageTable, PageTableFlags, PhysFrame, Size4KiB,
};
use x86_64::{PhysAddr, VirtAddr};

pub const RESERVED_START_FRAME: usize = 256; // Reserve lower 1 MB (256 * 4KiB)
pub const MAX_FRAMES: usize = 32768; // 128 MB physical memory managed via 4KiB frames

pub const HEAP_START: usize = 0x_4444_4444_0000;
pub const HEAP_SIZE: usize = 100 * 1024; // 100 KiB heap

#[global_allocator]
pub static ALLOCATOR: LockedHeap = LockedHeap::empty();

pub struct BitmapAllocator {
    bitmap: [u64; MAX_FRAMES / 64],
    next_free: usize,
}

impl BitmapAllocator {
    pub const fn new() -> Self {
        Self {
            bitmap: [0; MAX_FRAMES / 64],
            next_free: RESERVED_START_FRAME,
        }
    }

    pub fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        let start = if self.next_free < RESERVED_START_FRAME {
            RESERVED_START_FRAME
        } else {
            self.next_free
        };

        for index in start..MAX_FRAMES {
            let word_idx = index / 64;
            let bit_idx = index % 64;

            if (self.bitmap[word_idx] & (1 << bit_idx)) == 0 {
                self.bitmap[word_idx] |= 1 << bit_idx;
                self.next_free = index + 1;
                let phys_addr = PhysAddr::new((index * 4096) as u64);
                return PhysFrame::from_start_address(phys_addr).ok();
            }
        }
        None
    }

    pub fn deallocate_frame(&mut self, frame: PhysFrame<Size4KiB>) {
        let index = (frame.start_address().as_u64() / 4096) as usize;
        if index >= RESERVED_START_FRAME && index < MAX_FRAMES {
            let word_idx = index / 64;
            let bit_idx = index % 64;
            self.bitmap[word_idx] &= !(1 << bit_idx);
            if index < self.next_free {
                self.next_free = index;
            }
        }
    }
}

unsafe impl FrameAllocator<Size4KiB> for BitmapAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        self.allocate_frame()
    }
}

pub unsafe fn active_level_4_table(physical_memory_offset: VirtAddr) -> &'static mut PageTable {
    let (level_4_table_frame, _) = Cr3::read();
    let phys = level_4_table_frame.start_address();
    let virt = physical_memory_offset + phys.as_u64();
    let page_table_ptr: *mut PageTable = virt.as_mut_ptr();
    &mut *page_table_ptr
}

pub unsafe fn init_page_table(physical_memory_offset: VirtAddr) -> OffsetPageTable<'static> {
    let level_4_table = active_level_4_table(physical_memory_offset);
    OffsetPageTable::new(level_4_table, physical_memory_offset)
}

pub fn init_heap(
    mapper: &mut OffsetPageTable<'static>,
    frame_allocator: &mut BitmapAllocator,
) -> Result<(), ()> {
    let page_range = {
        let heap_start = VirtAddr::new(HEAP_START as u64);
        let heap_end = heap_start + HEAP_SIZE as u64 - 1u64;
        let heap_start_page = Page::containing_address(heap_start);
        let heap_end_page = Page::containing_address(heap_end);
        Page::range_inclusive(heap_start_page, heap_end_page)
    };

    for page in page_range {
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(())?;
        let flags = PageTableFlags::PRESENT | PageTableFlags::WRITABLE;
        unsafe {
            mapper
                .map_to(page, frame, flags, frame_allocator)
                .map_err(|_| ())?
                .flush();
        }
    }

    unsafe {
        ALLOCATOR.lock().init(HEAP_START as *mut u8, HEAP_SIZE);
    }
    Ok(())
}
