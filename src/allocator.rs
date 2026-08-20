use core::alloc::{GlobalAlloc, Layout};
use core::ptr::null_mut;

use spin::{Mutex, MutexGuard};

#[global_allocator]
static ALLOCATOR: Locked<BumpAllocator> = Locked::new(BumpAllocator::new());

pub struct Locked<A> {
    inner: Mutex<A>,
}

impl<A> Locked<A> {
    pub const fn new(inner: A) -> Self {
        Self {
            inner: Mutex::new(inner),
        }
    }

    pub fn lock(&self) -> MutexGuard<'_, A> {
        self.inner.lock()
    }
}

pub struct BumpAllocator {
    heap_start: usize,
    heap_end: usize,
    next: usize,
}

impl BumpAllocator {
    pub const fn new() -> Self {
        Self {
            heap_start: 0,
            heap_end: 0,
            next: 0,
        }
    }

    pub unsafe fn init(&mut self, heap_start: usize, heap_size: usize) {
        self.heap_start = heap_start;
        self.heap_end = heap_start + heap_size;
        self.next = heap_start;
    }

    fn allocate(&mut self, layout: Layout) -> *mut u8 {
        let Some(alloc_start) = align_up(self.next, layout.align()) else {
            return null_mut();
        };
        let Some(alloc_end) = alloc_start.checked_add(layout.size()) else {
            return null_mut();
        };

        if alloc_end > self.heap_end {
            null_mut()
        } else {
            self.next = alloc_end;
            alloc_start as *mut u8
        }
    }

    // Early-boot bump allocation never reclaims individual allocations.
    unsafe fn deallocate(&mut self, _ptr: *mut u8, _layout: Layout) {}
}

unsafe impl GlobalAlloc for Locked<BumpAllocator> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.lock().allocate(layout)
    }

    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {
        self.lock().deallocate(_ptr, _layout);
    }
}

pub unsafe fn init(heap_start: usize, heap_size: usize) {
    ALLOCATOR.lock().init(heap_start, heap_size);
}

#[alloc_error_handler]
fn alloc_error_handler(_layout: Layout) -> ! {
    // Early boot does not yet have a safe global reporting path for OOM.
    loop {}
}

fn align_up(addr: usize, align: usize) -> Option<usize> {
    if align == 0 || !align.is_power_of_two() {
        return None;
    }

    match addr.checked_add(align - 1) {
        Some(aligned) => Some(aligned & !(align - 1)),
        None => None,
    }
}
