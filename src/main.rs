#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![feature(alloc_error_handler)]

mod allocator;
mod interrupts;
mod memory;
mod paging;

use core::fmt::Write;
use core::panic::PanicInfo;
use uefi::prelude::*;
use uefi::proto::console::gop::GraphicsOutput;

#[entry]
fn main(_image_handle: Handle, mut system_table: SystemTable<Boot>) -> Status {
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

    let _ = writeln!(system_table.stdout(), "Hello World - AURA OS Kernel");

    // Initialize IDT and PIC hardware interrupt controller
    interrupts::init_idt();
    interrupts::init_pics();
    x86_64::instructions::interrupts::enable();

    // Initialize physical memory bitmap allocator
    let mut phys_allocator = memory::BitmapAllocator::new();
    if let Some(frame) = phys_allocator.allocate_frame() {
        let _ = writeln!(
            system_table.stdout(),
            "Allocated physical frame at {:?}",
            frame.start_address()
        );
    }

    let _ = writeln!(
        system_table.stdout(),
        "Paging and heap groundwork added; activation needs a boot-provided physical memory offset"
    );

    loop {}
}

#[panic_handler]
fn panic(_info: &PanicInfo<'_>) -> ! {
    loop {}
}
