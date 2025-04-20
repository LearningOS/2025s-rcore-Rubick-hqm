//! Memory management implementation
//!
//! SV39 page-based virtual-memory architecture for RV64 systems, and
//! everything about memory management, like frame allocator, page table,
//! map area and memory set, is implemented here.
//!
//! Every task or process has a memory_set to control its virtual memory.

mod address;
mod frame_allocator;
mod heap_allocator;
mod memory_set;
mod page_table;

use address::VPNRange;
pub use address::{PhysAddr, PhysPageNum, StepByOne, VirtAddr, VirtPageNum};
pub use frame_allocator::{frame_alloc, frame_dealloc, FrameTracker};
pub use memory_set::remap_test;
pub use memory_set::{kernel_token, MapPermission, MemorySet, KERNEL_SPACE};
pub use page_table::{
    translate_pte, translate_str, translated_byte_buffer, translated_refmut, PageTableEntry,
    UserBuffer,
};
pub use page_table::{PTEFlags, PageTable};

/// initiate heap allocator, frame allocator and kernel space
pub fn init() {
    heap_allocator::init_heap();
    // heap_allocator::heap_test();
    frame_allocator::init_frame_allocator();
    // frame_allocator::frame_allocator_test();
    KERNEL_SPACE.exclusive_access().activate();
}
