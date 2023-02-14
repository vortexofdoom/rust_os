use x86_64::{
    structures::paging::{
        Page, 
        PhysFrame, 
        PageTable, 
        OffsetPageTable,
        Mapper,
        Size4KiB,
        FrameAllocator,
    },
    PhysAddr,
    VirtAddr,
};

use bootloader::bootinfo::{
    MemoryMap,
    MemoryRegionType,
};

pub struct BootInfoFrameAllocator {
    memory_map: &'static MemoryMap,
    next: usize,
}

impl BootInfoFrameAllocator {
    /// Create a FrameAllocator from the passed memory map.
    /// 
    /// This function is unsafe because the caller must guarantee that the passed memory map is valid.
    /// The main requirement is that all frames that are marked as `USABLE` in it are actually unused.
    pub unsafe fn init(memory_map: &'static MemoryMap) -> Self {
        BootInfoFrameAllocator { 
            memory_map, 
            next: 0 
        }
    }

    /// Returns an iterator over the usable frames specified in the memory map.
    fn usable_frames(&self) -> impl Iterator<Item = PhysFrame> {
        self.memory_map
            .iter()
            .filter(|r| r.region_type == MemoryRegionType::Usable)
            .map(|r| r.range.start_addr()..r.range.end_addr())
            .flat_map(|r| r.step_by(4096))
            .map(|addr| PhysFrame::containing_address(PhysAddr::new(addr)))
    }
}

/// Initialize a new OffsetPageTable
/// 
/// This function is unsafe because the caller must guarantee that the complete physical memory is mapped to virtual memory
/// at the passed `physical_memory_offset`. Also, this function must only be called once to avoid aliasing `&mut` references.
pub unsafe fn init(phys_mem_offset: VirtAddr) -> OffsetPageTable<'static> {
    let lvl_4_tbl = active_lvl_4_tbl(phys_mem_offset);
    OffsetPageTable::new(lvl_4_tbl, phys_mem_offset)
}

/// Returns a mutable reference to the active level 4 table.
/// 
/// This function is unsafe because the caller must guarantee that the complete phisical memory
/// is mapped to virtual memory at the passed `physical_memory_offset`.
/// Also, this function must only be called once to avoid aliasing `&mut` refences (which is UB).
unsafe fn active_lvl_4_tbl(phys_mem_offset: VirtAddr) -> &'static mut PageTable {
    let (lvl_4_tbl_frame, _) = x86_64::registers::control::Cr3::read();

    let phys = lvl_4_tbl_frame.start_address();
    let virt = phys_mem_offset + phys.as_u64();
    let page_tbl_ptr: *mut PageTable = virt.as_mut_ptr();

    &mut *page_tbl_ptr
}

pub fn create_example_mapping(
    page: Page,
    mapper: &mut OffsetPageTable,
    frame_allocator: &mut impl FrameAllocator<Size4KiB>,
) {
    use x86_64::structures::paging::PageTableFlags as Flags;

    let frame = PhysFrame::containing_address(PhysAddr::new(0xb8000));
    let flags = Flags::PRESENT | Flags::WRITABLE;

    let map_to_result = unsafe {
        mapper.map_to(page, frame, flags, frame_allocator)
    };
    map_to_result.expect("map_to failed").flush();
}

unsafe impl FrameAllocator<Size4KiB> for BootInfoFrameAllocator {
    fn allocate_frame(&mut self) -> Option<PhysFrame<Size4KiB>> {
        let frame = self.usable_frames().nth(self.next);
        self.next += 1;
        frame
    }
}