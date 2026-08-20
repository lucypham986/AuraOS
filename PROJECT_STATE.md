# AURA OS — PROJECT STATE & SYSTEM SPECIFICATIONS

## 1. TỔNG QUAN DỰ ÁN
- **Tên dự án:** AURA OS (AI-Native, Cross-Platform Microkernel Operating System)
- **Mục tiêu:** Hệ điều hành đa nền tảng (PC, Laptop, Mobile, IoT Edge), tích hợp sâu phân hệ AI ở cấp hệ thống và tương thích ứng dụng đa nền tảng.
- **Ngôn ngữ chính:** Rust (chế độ `#![no_std]`, Zero Garbage Collection, Memory-Safe).
- **Ngôn ngữ phụ trợ:** Assembly (x86_64 / AArch64) cho Bootstrapping, Interrupts, Context Switching.
- **An ninh Chuỗi cung ứng:** Reproducible Builds (SHA-256), `cargo vendor` audit thủ công, tự Bootstrap trình biên dịch qua `mrustc`.

---

## 2. KIẾN TRÚC & PHẦN CỨNG (CORE ARCHITECTURE)
- **Kernel Type:** Microkernel (Ring 0 chỉ giữ Memory Management, IPC, Scheduler; Drivers/Filesystem/Network chạy ở Ring 3 User Space).
- **CPU Architecture Support:** x86_64, ARM64/AArch64, RISC-V.
- **Bootloader Standard:** UEFI (`BOOTX64.EFI` / `BOOTAA64.EFI`) hoặc Limine Bootloader.
- **HAL & I/O:** VirtIO Standard (tối ưu hóa cho QEMU/KVM và Bare-metal).
- **Power & Scheduling:** Preemptive Multitasking (MLFQ Scheduler), cơ chế App Freezing khi chạy pin.

---

## 3. PHÂN HỆ AI (AI-NATIVE SUBSYSTEM)
- **System Service Level:** AI Engine chạy như một dịch vụ cốt lõi ở Ring 3 (System Service).
- **Memory Access:** Zero-Copy Unified Memory Bridge giữa CPU, GPU, NPU.
- **OOM Prevention:** GPUDirect Storage Paging (Swap VRAM quá tải sang NVMe SSD).
- **Hardware Acceleration:** CUDA/NVLink (Nvidia), ROCm (AMD), Metal (Apple), DirectML, Qualcomm NPU API.
- **System Engines:** Built-in Vector DB (Rust-based), Distributed AI Compute Bus (P2P GPU Sharing qua Wi-Fi 7 / Ethernet).

---

## 4. HỆ THỐNG FILE & CẤU TRÚC THƯ MỤC
- **File System (`AuraFS`):** Copy-on-Write (CoW), mã hóa AES-XTS.
  - Max File Size: 16 Exabytes | Max Partition Size: 1 Zettabyte.
- **Định dạng Build Output:**
  - Kernel core: `kernel.elf`
  - Installer/Image: `.iso` (PC/VM), `.img` / `.bin` (Mobile/Embedded).
- **Cấu trúc Thư mục Hệ thống (Atomic & Flat Structure):**
  - `@boot/`        : Bootloader, kernel.elf, initramfs.
  - `@system/`      : [READ-ONLY] Drivers, Microkernel Services, Runtimes.
  - `@ai/`          : LLM Weights, KV Cache, Vector Store, Acceleration Layers.
  - `@apps/`        : Ứng dụng cài đặt độc lập.
  - `@users/`       : Dữ liệu người dùng (User Profiles).

---

## 5. GIAO DIỆN (GUI) & TƯƠNG THÍCH ỨNG DỤNG
- **Compositor:** `AuraCompositor` (Wayland Protocol bằng Rust, Render trực tiếp qua Vulkan API).
- **UI Framework:** Declarative Native UI (Iced / Slint) với Adaptive UI (chuyển đổi linh hoạt giữa Mobile Touch UI và Desktop Window UI).
- **Terminal:** Rust-native, GPU Vulkan Acceleration, Shell mặc định Nushell/Fish + AI CLI Agent (`? <prompt>`).
- **Lớp Tương thích (App Runtimes):**
  - **Linux Apps:** 100% Native (POSIX/ABI Subsystem).
  - **Android Apps (APK):** Waydroid / LXC Container.
  - **Windows Apps (.exe):** Wine / Proton Subsystem tối ưu ở cấp Kernel.
  - **iOS Apps (.ipa):** Không hỗ trợ.

---

## 6. MÔI TRƯỜNG PHÁT TRIỂN DỰ ÁN
- **Version Control:** Git (`git init`).
- **Quy tắc Thảo luận Chat:** *Một Task - Một Session Chat* (Copy-paste file này ở đầu mỗi phiên để AI nắm trọn ngữ cảnh).
