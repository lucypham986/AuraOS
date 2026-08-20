use crate::memory::BitmapAllocator;
use x86_64::structures::paging::{PageTableFlags, PhysFrame, Size4KiB};
use x86_64::{PhysAddr, VirtAddr};

const ENTRIES_PER_TABLE: usize = 512;
// Reserve space for one root table plus up to 63 subordinate tables during early boot.
const MAX_PAGE_TABLES: usize = 64;
const PAGE_SIZE: u64 = 4096;
#[derive(Clone, Copy)]
struct PageTableEntry {
    addr: u64,
    flags: PageTableFlags,
    next_table: Option<usize>,
}

impl PageTableEntry {
    const fn empty() -> Self {
        Self {
            addr: 0,
            flags: PageTableFlags::empty(),
            next_table: None,
        }
    }

    fn is_present(&self) -> bool {
        self.flags.contains(PageTableFlags::PRESENT)
    }
}

#[derive(Clone, Copy)]
struct PageTable {
    entries: [PageTableEntry; ENTRIES_PER_TABLE],
}

impl PageTable {
    const fn new() -> Self {
        Self {
            entries: [PageTableEntry::empty(); ENTRIES_PER_TABLE],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PageMappingError {
    AlreadyMapped,
    TablePoolExhausted,
    OutOfPhysicalFrames,
}

pub struct VirtualMemoryManager {
    tables: [PageTable; MAX_PAGE_TABLES],
    // Entry 0 is the software-managed root table; child tables get physical frames on demand.
    table_frames: [Option<PhysFrame<Size4KiB>>; MAX_PAGE_TABLES],
    used_tables: usize,
    mapped_pages: usize,
}

impl VirtualMemoryManager {
    pub const fn new() -> Self {
        Self {
            tables: [PageTable::new(); MAX_PAGE_TABLES],
            table_frames: [None; MAX_PAGE_TABLES],
            used_tables: 1,
            mapped_pages: 0,
        }
    }

    pub fn map_page(
        &mut self,
        virt_addr: VirtAddr,
        frame: PhysFrame<Size4KiB>,
        flags: PageTableFlags,
        frame_allocator: &mut BitmapAllocator,
    ) -> Result<(), PageMappingError> {
        let indices = table_indices(virt_addr);
        let mut table_index = 0;

        for &index in &indices[..3] {
            let next_table = match self.tables[table_index].entries[index].next_table {
                Some(next) => next,
                None => {
                    let next = self.allocate_table(frame_allocator)?;
                    let table_phys_addr = self.table_addr(next).as_u64();
                    let entry = &mut self.tables[table_index].entries[index];
                    entry.addr = table_phys_addr;
                    entry.flags = inherited_table_flags(flags);
                    entry.next_table = Some(next);
                    next
                }
            };
            table_index = next_table;
        }

        let leaf_entry = &mut self.tables[table_index].entries[indices[3]];
        if leaf_entry.is_present() {
            return Err(PageMappingError::AlreadyMapped);
        }

        leaf_entry.addr = frame.start_address().as_u64();
        leaf_entry.flags = flags | PageTableFlags::PRESENT;
        leaf_entry.next_table = None;
        self.mapped_pages += 1;
        Ok(())
    }

    pub fn unmap_page(&mut self, virt_addr: VirtAddr) -> Option<PhysFrame<Size4KiB>> {
        let indices = table_indices(virt_addr);
        let mut table_index = 0;

        for &index in &indices[..3] {
            match self.tables[table_index].entries[index].next_table {
                Some(next) => table_index = next,
                None => return None,
            }
        }

        let leaf_entry = &mut self.tables[table_index].entries[indices[3]];
        if !leaf_entry.is_present() {
            return None;
        }

        let frame = Some(
            PhysFrame::from_start_address(PhysAddr::new(leaf_entry.addr))
                .expect("leaf page entries must contain 4 KiB-aligned frame addresses"),
        );
        *leaf_entry = PageTableEntry::empty();
        self.mapped_pages = self.mapped_pages.saturating_sub(1);
        frame
    }

    pub fn translate_addr(&self, virt_addr: VirtAddr) -> Option<PhysAddr> {
        let indices = table_indices(virt_addr);
        let mut table_index = 0;

        for &index in &indices[..3] {
            table_index = self.tables[table_index].entries[index].next_table?;
        }

        let leaf_entry = &self.tables[table_index].entries[indices[3]];
        if !leaf_entry.is_present() {
            return None;
        }

        Some(PhysAddr::new(
            leaf_entry.addr + (virt_addr.as_u64() & (PAGE_SIZE - 1)),
        ))
    }

    pub fn mapped_pages(&self) -> usize {
        self.mapped_pages
    }

    fn allocate_table(
        &mut self,
        frame_allocator: &mut BitmapAllocator,
    ) -> Result<usize, PageMappingError> {
        if self.used_tables >= MAX_PAGE_TABLES {
            return Err(PageMappingError::TablePoolExhausted);
        }

        let table_index = self.used_tables;
        let frame = frame_allocator
            .allocate_frame()
            .ok_or(PageMappingError::OutOfPhysicalFrames)?;
        self.tables[table_index] = PageTable::new();
        self.table_frames[table_index] = Some(frame);
        self.used_tables += 1;
        Ok(table_index)
    }

    fn table_addr(&self, table_index: usize) -> PhysAddr {
        self.table_frames[table_index]
            .map(|frame| frame.start_address())
            .expect("allocated page table must have a backing physical frame")
    }
}

fn table_indices(addr: VirtAddr) -> [usize; 4] {
    let raw = addr.as_u64();
    [
        ((raw >> 39) & 0x1ff) as usize,
        ((raw >> 30) & 0x1ff) as usize,
        ((raw >> 21) & 0x1ff) as usize,
        ((raw >> 12) & 0x1ff) as usize,
    ]
}

fn inherited_table_flags(flags: PageTableFlags) -> PageTableFlags {
    let mut table_flags = PageTableFlags::PRESENT;
    if flags.contains(PageTableFlags::WRITABLE) {
        table_flags |= PageTableFlags::WRITABLE;
    }
    if flags.contains(PageTableFlags::USER_ACCESSIBLE) {
        table_flags |= PageTableFlags::USER_ACCESSIBLE;
    }
    table_flags
}
