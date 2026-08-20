use core::alloc::Layout;
use core::ptr::NonNull;

const HEAP_SIZE: usize = 64 * 1024;
const SLAB_CLASS_COUNT: usize = 8;
const REGION_SIZE: usize = HEAP_SIZE / SLAB_CLASS_COUNT;
const SLAB_SIZES: [usize; SLAB_CLASS_COUNT] = [16, 32, 64, 128, 256, 512, 1024, 2048];
// Size class 0 is the densest slab region, so it determines the largest bitmap needed.
const MAX_SLOTS_PER_REGION: usize = REGION_SIZE / SLAB_SIZES[0];
const MAX_BITMAP_WORDS: usize = MAX_SLOTS_PER_REGION.div_ceil(64);

#[repr(align(4096))]
struct HeapStorage([u8; HEAP_SIZE]);

impl HeapStorage {
    const fn new() -> Self {
        Self([0; HEAP_SIZE])
    }
}

pub struct SlabAllocator {
    storage: HeapStorage,
    usage_bitmap: [[u64; MAX_BITMAP_WORDS]; SLAB_CLASS_COUNT],
}

impl SlabAllocator {
    pub const fn new() -> Self {
        Self {
            storage: HeapStorage::new(),
            usage_bitmap: [[0; MAX_BITMAP_WORDS]; SLAB_CLASS_COUNT],
        }
    }

    pub fn allocate(&mut self, layout: Layout) -> Option<NonNull<u8>> {
        let class_index = class_index_for(layout)?;
        let slot_index = self.first_free_slot(class_index)?;
        self.set_slot_used(class_index, slot_index);

        let offset = class_index * REGION_SIZE + slot_index * SLAB_SIZES[class_index];
        NonNull::new(self.storage.0.as_mut_ptr().wrapping_add(offset))
    }

    pub fn deallocate(&mut self, ptr: NonNull<u8>, layout: Layout) -> bool {
        let class_index = match class_index_for(layout) {
            Some(index) => index,
            None => return false,
        };

        let base_ptr = self.storage.0.as_ptr() as usize;
        let ptr_addr = ptr.as_ptr() as usize;
        if ptr_addr < base_ptr || ptr_addr >= base_ptr + HEAP_SIZE {
            return false;
        }

        let offset = ptr_addr - base_ptr;
        let expected_region = class_index * REGION_SIZE;
        if offset < expected_region || offset >= expected_region + REGION_SIZE {
            return false;
        }

        let slot_index = (offset - expected_region) / SLAB_SIZES[class_index];
        if slot_index >= self.capacity_for(class_index) {
            return false;
        }

        if !self.is_slot_used(class_index, slot_index) {
            return false;
        }

        self.set_slot_free(class_index, slot_index);
        true
    }

    pub fn available_blocks(&self, class_index: usize) -> Option<usize> {
        if class_index >= SLAB_CLASS_COUNT {
            return None;
        }

        let used_blocks = self.used_blocks(class_index);
        Some(self.capacity_for(class_index).saturating_sub(used_blocks))
    }

    pub fn available_blocks_for_layout(&self, layout: Layout) -> Option<usize> {
        let class_index = class_index_for(layout)?;
        self.available_blocks(class_index)
    }

    fn capacity_for(&self, class_index: usize) -> usize {
        REGION_SIZE / SLAB_SIZES[class_index]
    }

    fn used_blocks(&self, class_index: usize) -> usize {
        self.usage_bitmap[class_index]
            .iter()
            .map(|word| word.count_ones() as usize)
            .sum()
    }

    fn first_free_slot(&self, class_index: usize) -> Option<usize> {
        let capacity = self.capacity_for(class_index);
        for (word_index, word) in self.usage_bitmap[class_index].iter().enumerate() {
            if *word == u64::MAX {
                continue;
            }

            let bit_index = (!*word).trailing_zeros() as usize;
            let slot_index = word_index * 64 + bit_index;
            if slot_index < capacity {
                return Some(slot_index);
            }
        }
        None
    }

    fn is_slot_used(&self, class_index: usize, slot_index: usize) -> bool {
        let word_index = slot_index / 64;
        let bit_index = slot_index % 64;
        if word_index >= MAX_BITMAP_WORDS {
            return false;
        }
        (self.usage_bitmap[class_index][word_index] & (1 << bit_index)) != 0
    }

    fn set_slot_used(&mut self, class_index: usize, slot_index: usize) {
        let word_index = slot_index / 64;
        let bit_index = slot_index % 64;
        if word_index >= MAX_BITMAP_WORDS {
            return;
        }
        self.usage_bitmap[class_index][word_index] |= 1 << bit_index;
    }

    fn set_slot_free(&mut self, class_index: usize, slot_index: usize) {
        let word_index = slot_index / 64;
        let bit_index = slot_index % 64;
        if word_index >= MAX_BITMAP_WORDS {
            return;
        }
        self.usage_bitmap[class_index][word_index] &= !(1 << bit_index);
    }
}

fn class_index_for(layout: Layout) -> Option<usize> {
    SLAB_SIZES.iter().position(|&block_size| {
        block_size >= layout.size()
            && block_size >= layout.align()
            && REGION_SIZE % block_size == 0
    })
}
