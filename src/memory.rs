use x86_64::structures::paging::{FrameAllocator, PhysFrame, Size4KiB};
use x86_64::PhysAddr;

pub const RESERVED_START_FRAME: usize = 256; // Reserve lower 1 MB (256 * 4KiB)
pub const MAX_FRAMES: usize = 32768; // 128 MB physical memory managed via 4KiB frames

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
