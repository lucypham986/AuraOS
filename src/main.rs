#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]

extern crate alloc;

mod ai_runtime;
mod interrupts;
mod ipc;
mod memory;
mod task;
mod virtio_block;
mod zero_copy;

use alloc::boxed::Box;
use alloc::string::ToString;
use uefi::prelude::*;
use uefi::proto::console::gop::GraphicsOutput;
use x86_64::VirtAddr;

/// Panic handler required by `#![no_std]`.  In a production kernel this would
/// print a kernel-panic screen and halt all CPUs.
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}

#[entry]
fn main() -> Status {
    // Initialise UEFI helpers (logger → log crate, allocator).
    uefi::helpers::init().expect("UEFI helpers init failed");

    // -----------------------------------------------------------------------
    // Sprint 1 — GOP framebuffer: fill screen with solid blue
    // -----------------------------------------------------------------------
    if let Ok(gop_handle) = uefi::boot::get_handle_for_protocol::<GraphicsOutput>() {
        if let Ok(mut gop) = uefi::boot::open_protocol_exclusive::<GraphicsOutput>(gop_handle) {
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
                            *fb_ptr.add(pixel_index)     = 255; // Blue
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

    // -----------------------------------------------------------------------
    // Sprint 2 — Interrupts: IDT setup
    // -----------------------------------------------------------------------
    interrupts::init_idt();

    // -----------------------------------------------------------------------
    // Sprint 2 — Memory management: physical bitmap allocator + heap
    // -----------------------------------------------------------------------
    let mut phys_allocator = memory::BitmapAllocator::new();
    if let Some(frame) = phys_allocator.allocate_frame() {
        log::info!("Allocated physical frame at {:?}", frame.start_address());
    }

    let phys_mem_offset = VirtAddr::new(0);
    let mut mapper = unsafe { memory::init_page_table(phys_mem_offset) };
    if memory::init_heap(&mut mapper, &mut phys_allocator).is_ok() {
        log::info!("Kernel Heap Initialized successfully!");
        let heap_val = Box::new(42);
        log::info!("Heap allocation test: Box value = {}", *heap_val);
    }

    // -----------------------------------------------------------------------
    // Sprint 2 — Multitasking: kernel thread scheduler
    // -----------------------------------------------------------------------
    task::init_scheduler();

    {
        let mut sched = task::SCHEDULER.lock();
        if let Some(ref mut s) = *sched {
            fn idle_task() -> ! { loop { core::hint::spin_loop(); } }
            fn worker_task() -> ! { loop { core::hint::spin_loop(); } }

            let idle_id   = s.spawn(idle_task);
            let worker_id = s.spawn(worker_task);

            if let Some(next) = s.schedule() {
                log::info!("Scheduler tick 1: running task id={}", next);
            }
            if let Some(next) = s.schedule() {
                log::info!("Scheduler tick 2: running task id={}", next);
            }

            s.block_task(worker_id);
            log::info!("Task {} blocked.", worker_id);
            s.unblock_task(worker_id);
            log::info!("Task {} unblocked.", worker_id);

            s.exit_task(idle_id);
            log::info!(
                "Scheduler: {} live task(s) after idle exit.",
                s.task_count()
            );
        }
    }

    // -----------------------------------------------------------------------
    // Sprint 3 — IPC Engine
    // -----------------------------------------------------------------------
    ipc::init_ipc();

    {
        use ipc::{Message, Opcode};
        let mut chan = ipc::SYSTEM_CHANNEL.lock();

        let mut ping = Message::new(0, 1, Opcode::Ping);
        ping.write_payload(b"Hello from Ring-0!");
        chan.send(ping);

        let received = chan.recv();
        log::info!(
            "IPC: received {:?} from sender={} ({} bytes)",
            received.opcode,
            received.sender,
            received.data_len
        );
    }

    // -----------------------------------------------------------------------
    // Sprint 3 — VirtIO-Block Driver
    // -----------------------------------------------------------------------
    virtio_block::init_virtio_block();

    {
        use virtio_block::BlockDevice;
        let mut blk = virtio_block::VIRTIO_BLK.lock();
        let mut sector_buf = [0u8; virtio_block::VIRTIO_BLK_SECTOR_SIZE];

        match blk.read_block(0, &mut sector_buf) {
            Ok(()) => log::info!("VirtIO-Block: read sector 0 OK."),
            Err(e) => log::error!("VirtIO-Block: read error: {:?}", e),
        }
        match blk.write_block(0, &sector_buf) {
            Ok(()) => log::info!("VirtIO-Block: write sector 0 OK."),
            Err(e) => log::error!("VirtIO-Block: write error: {:?}", e),
        }
    }

    // -----------------------------------------------------------------------
    // Sprint 3 — AI Micro-Runtime
    // -----------------------------------------------------------------------
    ai_runtime::init_ai_runtime();

    {
        use ai_runtime::{AccelBackend, ModelInfo};
        let mut rt = ai_runtime::AI_RUNTIME.lock();

        rt.load_model(ModelInfo {
            name: "llama3-8b-stub".to_string(),
            param_count: 8_000_000_000,
            weight_bytes: 4 * 1024 * 1024 * 1024, // 4 GiB (symbolic)
            backend: AccelBackend::Cpu,
        }).ok();

        let input  = b"AURA OS inference test input";
        let mut output = [0u8; 64];
        match rt.infer("llama3-8b-stub", input, &mut output) {
            Ok(n)  => log::info!("AI inference OK: {} bytes written.", n),
            Err(e) => log::error!("AI inference error: {:?}", e),
        }
    }

    // -----------------------------------------------------------------------
    // Sprint 3 — Zero-Copy Memory Bridge
    // -----------------------------------------------------------------------
    zero_copy::init_zero_copy();

    {
        use zero_copy::CpuAccess;
        let mut bridge = zero_copy::ZC_BRIDGE.lock();

        if let Ok(handle) = bridge.alloc(2 * 1024 * 1024, CpuAccess::ReadWrite) {
            log::info!("ZC alloc OK: handle={:?}", handle);
            if let Ok(iova) = bridge.map_to_device(handle) {
                log::info!("ZC map_to_device OK: device IOVA = {:#x}", iova);
            }
            bridge.free(handle).ok();
            log::info!("ZC free OK.");
        }
    }

    loop {}
}
