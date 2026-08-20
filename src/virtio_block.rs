//! Sprint 3 — VirtIO-Block Driver: read/write disk via the VirtIO 1.x Block device protocol.
//!
//! This module defines the `BlockDevice` abstraction trait and provides a
//! `VirtioBlockDevice` stub that models the MMIO-mapped VirtIO block device
//! registers and virtqueue descriptor ring as specified in the VirtIO 1.2
//! specification (section 5.2).  The stub validates the contract of the driver
//! interface and logs all I/O requests; full DMA ring processing would be
//! added here once the VirtIO transport layer is wired up to real QEMU
//! VirtIO-Block MMIO BARs.

use spin::Mutex;

// ---------------------------------------------------------------------------
// BlockDevice trait
// ---------------------------------------------------------------------------

/// Errors that the block layer may return.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockError {
    /// The requested block index is past the end of the device.
    OutOfRange,
    /// The supplied buffer has the wrong size (must equal `block_size()`).
    BadBufferSize,
    /// The underlying hardware reported an error.
    HardwareError,
    /// The device has not been initialised yet.
    NotInitialized,
}

/// Generic abstraction over a block-addressable storage device.
pub trait BlockDevice {
    /// Read one logical block identified by `block_id` into `buf`.
    fn read_block(&mut self, block_id: u64, buf: &mut [u8]) -> Result<(), BlockError>;

    /// Write `buf` as one logical block at position `block_id`.
    fn write_block(&mut self, block_id: u64, buf: &[u8]) -> Result<(), BlockError>;

    /// Size of a single logical block in bytes (typically 512 or 4096).
    fn block_size(&self) -> usize;

    /// Total number of logical blocks exposed by the device.
    fn block_count(&self) -> u64;
}

// ---------------------------------------------------------------------------
// VirtIO-Block MMIO register layout (VirtIO 1.2, §4.2.2)
// ---------------------------------------------------------------------------

/// VirtIO block device status flags.
#[repr(u8)]
#[allow(dead_code)]
pub enum VirtioStatus {
    Acknowledge = 1,
    Driver = 2,
    DriverOk = 4,
    FeaturesOk = 8,
    Failed = 128,
}

/// VirtIO block request type codes (VirtIO 1.2, §5.2.6).
#[repr(u32)]
#[allow(dead_code)]
pub enum ReqType {
    In = 0,  // Read (device → driver)
    Out = 1, // Write (driver → device)
    Flush = 4,
}

/// VirtIO block request header placed in the virtqueue descriptor.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VirtioBlkReqHeader {
    pub req_type: u32,
    pub reserved: u32,
    pub sector: u64,
}

// ---------------------------------------------------------------------------
// VirtIO-Block device stub
// ---------------------------------------------------------------------------

/// Logical block size assumed by the stub driver (512 bytes, matching QEMU default).
pub const VIRTIO_BLK_SECTOR_SIZE: usize = 512;

/// Simulated total capacity of the stub device (64 MiB → 131 072 sectors).
pub const VIRTIO_BLK_TOTAL_SECTORS: u64 = 131_072;

/// VirtIO-Block driver stub.
///
/// In a production kernel this struct would hold:
/// * The MMIO base address mapped into the kernel virtual address space.
/// * The virtqueue descriptor table, available ring, and used ring.
/// * DMA-coherent bounce buffers for request/response transfers.
///
/// For Sprint 3 the driver logs every request and returns success so that
/// the rest of the stack (filesystem, IPC, AI runtime) can be tested
/// end-to-end before real QEMU MMIO is wired up.
pub struct VirtioBlockDevice {
    initialized: bool,
    /// Simulated backing store — one page of zeroed bytes used to return
    /// consistent reads without hardware access.
    scratch: [u8; VIRTIO_BLK_SECTOR_SIZE],
}

impl VirtioBlockDevice {
    pub const fn new() -> Self {
        Self {
            initialized: false,
            scratch: [0u8; VIRTIO_BLK_SECTOR_SIZE],
        }
    }

    /// Negotiate capabilities and mark the device as ready.
    pub fn init(&mut self) {
        // In a real driver: MMIO feature negotiation, virtqueue setup, status update.
        self.initialized = true;
        log::info!(
            "VirtIO-Block driver initialized: {} sectors × {} B = {} KiB",
            VIRTIO_BLK_TOTAL_SECTORS,
            VIRTIO_BLK_SECTOR_SIZE,
            VIRTIO_BLK_TOTAL_SECTORS * VIRTIO_BLK_SECTOR_SIZE as u64 / 1024,
        );
    }
}

impl BlockDevice for VirtioBlockDevice {
    fn block_size(&self) -> usize {
        VIRTIO_BLK_SECTOR_SIZE
    }

    fn block_count(&self) -> u64 {
        VIRTIO_BLK_TOTAL_SECTORS
    }

    fn read_block(&mut self, block_id: u64, buf: &mut [u8]) -> Result<(), BlockError> {
        if !self.initialized {
            return Err(BlockError::NotInitialized);
        }
        if block_id >= VIRTIO_BLK_TOTAL_SECTORS {
            return Err(BlockError::OutOfRange);
        }
        if buf.len() != VIRTIO_BLK_SECTOR_SIZE {
            return Err(BlockError::BadBufferSize);
        }
        // Stub: copy zeroed scratch sector into caller's buffer.
        buf.copy_from_slice(&self.scratch);
        log::debug!("VirtIO-Block READ  sector {}", block_id);
        Ok(())
    }

    fn write_block(&mut self, block_id: u64, buf: &[u8]) -> Result<(), BlockError> {
        if !self.initialized {
            return Err(BlockError::NotInitialized);
        }
        if block_id >= VIRTIO_BLK_TOTAL_SECTORS {
            return Err(BlockError::OutOfRange);
        }
        if buf.len() != VIRTIO_BLK_SECTOR_SIZE {
            return Err(BlockError::BadBufferSize);
        }
        // Stub: absorb the write without persisting it.
        log::debug!("VirtIO-Block WRITE sector {} ({} bytes)", block_id, buf.len());
        Ok(())
    }
}

/// Global VirtIO-Block driver instance.
pub static VIRTIO_BLK: Mutex<VirtioBlockDevice> = Mutex::new(VirtioBlockDevice::new());

/// Initialise the global VirtIO-Block driver.
pub fn init_virtio_block() {
    VIRTIO_BLK.lock().init();
}
