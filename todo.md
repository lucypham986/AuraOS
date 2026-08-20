# AURA OS — TODO & BUG TRACKER

## SPRINT 1: BARE-METAL BOOTSTRAPPING (GIAI ĐOẠN 1)

### 1. Khởi tạo Dự án & Toolchain
- [x] Khởi tạo Git repository (`git init`) và cấu trúc thư mục dự án.
- [x] Cấu hình `Cargo.toml` ở chế độ `#![no_std]` và target `x86_64-unknown-none`.
- [x] Tạo file `PROJECT_STATE.md` và `TODO.md` trong thư mục gốc.

### 2. UEFI Bootloader & Bare-Metal Output
- [x] Viết UEFI Entry Point bằng Rust (`uefi-rs`).
- [x] Cấu hình xuất dữ liệu ra màn hình Framebuffer cơ bản (Graphics Output Protocol - GOP).
- [x] In dòng chữ "Hello World - AURA OS Kernel" ra màn hình.
- [x] Cấu hình QEMU script (`qemu-system-x86_64`) để test khởi động file `.iso` / `.efi`.

---

## SPRINT 2: KERNEL CORE SUBSYSTEMS (GIAI ĐOẠN 2)
- [x] **Interrupts:** Thiết lập bảng ngắt IDT (Interrupt Descriptor Table) và xử lý ngắt phần cứng (PIC/APIC).
- [x] **Memory Management (Physical):** Triển khai Bitmap Allocator / Buddy Allocator cho RAM vật lý.
- [ ] **Memory Management (Virtual):** Thiết lập Paging (4-level paging x86_64), cấp phát Heap (`Slab Allocator`).
- [ ] **Multitasking:** Triển khai Kernel Threads và Preemptive Scheduler (Round-Robin).

---

## SPRINT 3: MICROKERNEL SERVICES & AI RUNTIME (GIAI ĐOẠN 3)
- [ ] **IPC Engine:** Thiết lập cơ chế truyền tin nhắn siêu tốc giữa Ring 0 và Ring 3.
- [ ] **Drivers Layer:** Viết VirtIO-Block driver (đọc/ghi đĩa) và VirtIO-GPU driver.
- [ ] **AI Micro-Runtime:** Tích hợp ONNX Runtime / `llama.cpp` C++ binding vào User Space.
- [ ] **Zero-Copy Memory:** Thử nghiệm share memory giữa CPU và GPU qua Vulkan Memory Allocator.

---

## DANH SÁCH BUG (BUG TRACKER)
> *Ghi lại các lỗi phát sinh trong quá trình code tại đây để xử lý dứt điểm trước khi làm task mới.*

- [ ] *(Chưa có bug)*
