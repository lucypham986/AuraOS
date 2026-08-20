//! Sprint 3 — Zero-Copy Unified Memory Bridge.
//!
//! Provides a shared-memory abstraction that allows the same physical pages to
//! be mapped into both the CPU virtual address space and a GPU/NPU device
//! address space without any explicit DMA copy.  This is the foundation of
//! AuraOS's "Zero-Copy Unified Memory" design goal (see PROJECT_STATE.md §3).
//!
//! **Current implementation:** establishes the public API surface and the
//! `SharedBuffer` descriptor type.  The actual IOMMU page-table manipulation
//! and Vulkan External Memory / CUDA Unified Memory device-side mapping are
//! provided by backend modules that will be implemented in Sprint 4 alongside
//! the full GPU driver stack.

extern crate alloc;

use alloc::vec::Vec;
use spin::Mutex;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZcError {
    /// No contiguous physical pages could be allocated.
    AllocationFailed,
    /// The IOMMU mapping into the device address space failed.
    IommuMapFailed,
    /// The buffer handle is unknown or has already been freed.
    InvalidHandle,
    /// The subsystem has not been initialised.
    NotInitialized,
}

// ---------------------------------------------------------------------------
// Buffer descriptor
// ---------------------------------------------------------------------------

/// Opaque handle identifying a zero-copy shared buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BufHandle(u64);

/// CPU-side access mode for a shared buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CpuAccess {
    ReadOnly,
    WriteOnly,
    ReadWrite,
}

/// Descriptor of a zero-copy shared memory region.
#[derive(Debug, Clone)]
pub struct SharedBuffer {
    /// Opaque handle used to reference this buffer in subsequent API calls.
    pub handle: BufHandle,
    /// Size of the allocation in bytes.
    pub size: usize,
    /// Physical base address of the allocation (page-aligned).
    pub phys_base: u64,
    /// CPU virtual address through which the kernel accesses the buffer.
    pub virt_addr: u64,
    /// Device-side IOVA (I/O Virtual Address) as seen by the GPU/NPU.
    ///
    /// `None` until `map_to_device` has been called.
    pub device_iova: Option<u64>,
    /// Requested CPU access mode.
    pub cpu_access: CpuAccess,
}

// ---------------------------------------------------------------------------
// Zero-copy memory bridge
// ---------------------------------------------------------------------------

/// The Zero-Copy Unified Memory Bridge.
///
/// Manages a registry of `SharedBuffer` descriptors and co-ordinates the
/// physical memory allocator (via `memory::BitmapAllocator`), the CPU page
/// table (`memory::OffsetPageTable`), and the IOMMU device-address remapper.
pub struct ZeroCopyBridge {
    initialized: bool,
    buffers: Vec<SharedBuffer>,
    next_handle: u64,
}

impl ZeroCopyBridge {
    pub const fn new() -> Self {
        Self {
            initialized: false,
            buffers: Vec::new(),
            next_handle: 1,
        }
    }

    /// Initialise the bridge and probe IOMMU capabilities.
    pub fn init(&mut self) {
        // TODO: detect Intel VT-d / AMD-Vi / ARM SMMU via ACPI DMAR table.
        self.initialized = true;
        log::info!(
            "Zero-Copy Memory Bridge initialized (IOMMU backend: stub — Sprint 4)."
        );
    }

    /// Allocate `size` bytes of physically contiguous memory and create a
    /// CPU mapping for it.  The buffer is not yet visible to any device.
    ///
    /// Returns a `BufHandle` identifying the allocation.
    pub fn alloc(
        &mut self,
        size: usize,
        cpu_access: CpuAccess,
    ) -> Result<BufHandle, ZcError> {
        if !self.initialized {
            return Err(ZcError::NotInitialized);
        }
        if size == 0 {
            return Err(ZcError::AllocationFailed);
        }

        // Round size up to page boundary.
        let aligned_size = (size + 0xFFF) & !0xFFF;

        // Stub: we don't actually call into the physical allocator here to
        // avoid circular dependency.  The real implementation would call
        // `memory::BitmapAllocator::allocate_frame` in a loop and map the
        // resulting frames into the kernel heap range.
        let handle = BufHandle(self.next_handle);
        self.next_handle += 1;

        let buf = SharedBuffer {
            handle,
            size: aligned_size,
            // Placeholder addresses — real values come from the physical allocator.
            phys_base: 0x0000_DEAD_BEEF_0000 + handle.0 * 0x1000,
            virt_addr: 0x0000_CAFE_0000_0000 + handle.0 * 0x1000,
            device_iova: None,
            cpu_access,
        };

        log::debug!(
            "ZC alloc: handle={:?} size={} KiB phys={:#x}",
            handle,
            aligned_size / 1024,
            buf.phys_base
        );
        self.buffers.push(buf);
        Ok(handle)
    }

    /// Map an existing buffer into the device address space via the IOMMU.
    ///
    /// After this call `SharedBuffer::device_iova` is populated and the GPU /
    /// NPU can access the same physical pages the CPU uses — zero copies.
    pub fn map_to_device(&mut self, handle: BufHandle) -> Result<u64, ZcError> {
        if !self.initialized {
            return Err(ZcError::NotInitialized);
        }
        let buf = self
            .buffers
            .iter_mut()
            .find(|b| b.handle == handle)
            .ok_or(ZcError::InvalidHandle)?;

        if let Some(iova) = buf.device_iova {
            // Already mapped — return existing IOVA.
            return Ok(iova);
        }

        // Stub: assign a deterministic IOVA.  The real path programs the IOMMU
        // page-table (Intel VT-d DMAR or ARM SMMU stage-2) with the physical
        // address ranges collected during `alloc`.
        let iova = 0x8000_0000_0000u64 + handle.0 * 0x1000;
        buf.device_iova = Some(iova);

        log::debug!(
            "ZC map_to_device: handle={:?} iova={:#x}",
            handle,
            iova
        );
        Ok(iova)
    }

    /// Release a shared buffer and unmap it from both CPU and device address spaces.
    pub fn free(&mut self, handle: BufHandle) -> Result<(), ZcError> {
        if !self.initialized {
            return Err(ZcError::NotInitialized);
        }
        let pos = self
            .buffers
            .iter()
            .position(|b| b.handle == handle)
            .ok_or(ZcError::InvalidHandle)?;

        let buf = self.buffers.remove(pos);
        log::debug!("ZC free: handle={:?} size={} KiB", handle, buf.size / 1024);
        Ok(())
    }

    /// Look up the descriptor for an existing buffer.
    pub fn query(&self, handle: BufHandle) -> Option<&SharedBuffer> {
        self.buffers.iter().find(|b| b.handle == handle)
    }
}

/// Global zero-copy memory bridge instance.
pub static ZC_BRIDGE: Mutex<ZeroCopyBridge> = Mutex::new(ZeroCopyBridge::new());

/// Initialise the zero-copy memory subsystem.
pub fn init_zero_copy() {
    ZC_BRIDGE.lock().init();
}
