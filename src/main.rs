#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

use core::alloc::Layout;
use core::panic::PanicInfo;

mod heap;
mod interrupts;
mod memory;
mod paging;

use spin::Once;
use uefi::prelude::*;
use uefi::proto::console::gop::GraphicsOutput;
use x86_64::structures::paging::PageTableFlags;
use x86_64::VirtAddr;

static LOGGER: Once<uefi::logger::Logger> = Once::new();

#[entry]
fn main(_image_handle: Handle, mut system_table: SystemTable<Boot>) -> Status {
    init_logger(&mut system_table);

    let boot_services = system_table.boot_services();

    if let Ok(gop_handle) = boot_services.get_handle_for_protocol::<GraphicsOutput>() {
        if let Ok(mut gop) = boot_services.open_protocol_exclusive::<GraphicsOutput>(gop_handle) {
            let mode = gop.current_mode_info();
            let mut fb = gop.frame_buffer();
            let (width, height) = mode.resolution();
            let stride = mode.stride();

            for y in 0..height {
                for x in 0..width {
                    let pixel_index = (y * stride + x) * 4;
                    if pixel_index + 3 < fb.size() {
                        unsafe {
                            let fb_ptr = fb.as_mut_ptr();
                            *fb_ptr.add(pixel_index) = 255;     // Blue
                            *fb_ptr.add(pixel_index + 1) = 0;   // Green
                            *fb_ptr.add(pixel_index + 2) = 0;   // Red
                            *fb_ptr.add(pixel_index + 3) = 0;   // Reserved
                        }
                    }
                }
            }
        }
    }

    log::info!("Hello World - AURA OS Kernel");

    // Initialize IDT and PIC hardware interrupt controller
    interrupts::init_idt();
    interrupts::init_pics();
    x86_64::instructions::interrupts::enable();

    // Initialize physical memory bitmap allocator
    let mut phys_allocator = memory::BitmapAllocator::new();
    let mut virtual_memory = paging::VirtualMemoryManager::new();

    if let Some(frame) = phys_allocator.allocate_frame() {
        let kernel_page = VirtAddr::new(0x4444_0000);
        if virtual_memory
            .map_page(
                kernel_page,
                frame,
                PageTableFlags::PRESENT | PageTableFlags::WRITABLE,
                &mut phys_allocator,
            )
            .is_ok()
        {
            if let Some(mapped_phys) = virtual_memory.translate_addr(kernel_page) {
                log::info!(
                    "Mapped virtual page {:?} to physical frame {:?}",
                    kernel_page,
                    mapped_phys
                );
            }

            log::info!("Active virtual mappings: {}", virtual_memory.mapped_pages());
            if let Some(unmapped_frame) = virtual_memory.unmap_page(kernel_page) {
                phys_allocator.deallocate_frame(unmapped_frame);
            }
        }
    }

    let mut kernel_heap = heap::SlabAllocator::new();
    let heap_layout = Layout::from_size_align(128, 16).unwrap();
    if let Some(block) = kernel_heap.allocate(heap_layout) {
        log::info!("Allocated heap block at {:?}", block);
        if let Some(available_blocks) = kernel_heap.available_blocks_for_layout(heap_layout) {
            log::info!("Remaining 128-byte slab blocks: {}", available_blocks);
        }
        if !kernel_heap.deallocate(block, heap_layout) {
            log::error!("Failed to release heap block {:?}", block);
        }
    }

    loop {}
}

fn init_logger(system_table: &mut SystemTable<Boot>) {
    let logger = LOGGER.call_once(|| unsafe { uefi::logger::Logger::new(system_table.stdout()) });
    let _ = log::set_logger(logger).map(|()| log::set_max_level(log::LevelFilter::Info));
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}
