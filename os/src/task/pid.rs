use alloc::vec::Vec;
use lazy_static::lazy_static;

use crate::{
    config::{KERNEL_STACK_SIZE, PAGE_SIZE, TRAMPOLINE},
    mm::{MapPermission, VirtAddr, KERNEL_SPACE},
    sync::UPSafeCell,
};

lazy_static! {
    static ref PID_ALLOCATOR: UPSafeCell<RecycleAllocator> =
        unsafe { UPSafeCell::new(RecycleAllocator::new()) };
    static ref KSTACK_ALLOCATOR: UPSafeCell<RecycleAllocator> =
        unsafe { UPSafeCell::new(RecycleAllocator::new()) };
}

/// A handle to a process ID.
pub struct PidHandle(pub usize);

impl Drop for PidHandle {
    fn drop(&mut self) {
        PID_ALLOCATOR.exclusive_access().dealloc(self.0);
    }
}

struct RecycleAllocator {
    current: usize,
    recycled: Vec<usize>,
}

impl RecycleAllocator {
    fn new() -> Self {
        RecycleAllocator {
            current: 0,
            recycled: Vec::new(),
        }
    }

    fn alloc(&mut self) -> usize {
        if let Some(pid) = self.recycled.pop() {
            pid
        } else {
            self.current += 1;
            self.current - 1
        }
    }

    fn dealloc(&mut self, pid: usize) {
        assert!(pid < self.current);
        assert!(!self.recycled.contains(&pid));
        self.recycled.push(pid);
    }
}

/// 内核栈
pub struct KernelStack(pub usize);

impl KernelStack {
    /// 在栈顶压入一个值
    pub fn push_on_top<T>(&self, value: T) -> *mut T
    where
        T: Sized,
    {
        let kstack_top = self.get_top();
        let ptr_mut = kstack_top as *mut T;
        unsafe {
            *ptr_mut = value;
        }
        ptr_mut
    }

    /// 获取栈顶
    pub fn get_top(&self) -> usize {
        let (_, kstack_top) = kernel_stack_position(self.0);
        kstack_top
    }
}

/// Return (bottom, top) of a kernel stack in kernel space.
fn kernel_stack_position(app_id: usize) -> (usize, usize) {
    let top = TRAMPOLINE - app_id * (KERNEL_STACK_SIZE + PAGE_SIZE);
    let bottom = top - KERNEL_STACK_SIZE;
    (bottom, top)
}

impl Drop for KernelStack {
    fn drop(&mut self) {
        let (kstack_bottom, _) = kernel_stack_position(self.0);
        KERNEL_SPACE
            .exclusive_access()
            .remove_area_with_start_vpn(VirtAddr::from(kstack_bottom).into());
        KSTACK_ALLOCATOR.exclusive_access().dealloc(self.0);
    }
}

/// 分配一个新进程ID
///
/// 返回：PidHandle
pub fn pid_alloc() -> PidHandle {
    PidHandle(PID_ALLOCATOR.exclusive_access().alloc())
}

/// Allocate a new kernel stack.
pub fn kstack_alloc() -> KernelStack {
    // 先分配kstack_id
    let kstack_id = KSTACK_ALLOCATOR.exclusive_access().alloc();
    let (kstack_bottom, kstack_top) = kernel_stack_position(kstack_id);
    // 分配物理页
    KERNEL_SPACE.exclusive_access().insert_framed_area(
        kstack_bottom.into(),
        kstack_top.into(),
        MapPermission::R | MapPermission::W,
    );
    KernelStack(kstack_id)
}
