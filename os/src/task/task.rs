//! Types related to task management
use core::cell::RefMut;

use alloc::sync::{Arc, Weak};
use alloc::vec::Vec;

use super::pid::KernelStack;
use super::{kstack_alloc, pid_alloc, PidHandle, TaskContext};
use crate::config::TRAP_CONTEXT_BASE;
use crate::mm::{MemorySet, PhysPageNum, VirtAddr, KERNEL_SPACE};
use crate::sync::UPSafeCell;
use crate::trap::{trap_handler, TrapContext};

/// 最大优先级
pub const MAX_PRIORITY: u32 = 255;

/// 任务控制块
/// 不可变部分直接存在当前struct中
/// 可变部分放在Inner中
pub struct TaskControlBlock {
    /// 进程ID
    pid: PidHandle,
    /// 内核栈
    kernel_stack: KernelStack,
    inner: UPSafeCell<TaskControlBlockInner>,
}

impl TaskControlBlock {
    /// 返回inner的独有引用
    pub fn inner_exclusive_access(&self) -> RefMut<'_, TaskControlBlockInner> {
        self.inner.exclusive_access()
    }
    /// 获取pid
    pub fn get_pid(&self) -> usize {
        self.pid.0
    }

    /// 创建一个新任务
    ///
    /// 目前仅用于init进程
    /// 1. 分配进程地址空间
    /// 2. 填充陷阱上下文
    /// 3. 填充PCB其他字段
    pub fn new(elf_data: &[u8]) -> Self {
        let (memory_set, user_sp, entry_point) = MemorySet::from_elf(elf_data);
        let trap_cx_ppn = memory_set
            .translate(VirtAddr::from(TRAP_CONTEXT_BASE).into())
            .unwrap()
            .ppn();
        let pid = pid_alloc();
        let kernel_stack = kstack_alloc();
        let kstack_top = kernel_stack.get_top();
        let task_control_block = Self {
            pid,
            kernel_stack,
            inner: unsafe {
                UPSafeCell::new(TaskControlBlockInner {
                    trap_cx_ppn,
                    base_size: user_sp,
                    task_cx: TaskContext::goto_trap_return(kstack_top),
                    task_status: TaskStatus::Ready,
                    memory_set,
                    parent: None,
                    children: Vec::new(),
                    exit_code: 0,
                    heap_bottom: user_sp,
                    program_brk: user_sp,
                    stride: TaskStride::default(),
                })
            },
        };
        let trap_cx = task_control_block.inner_exclusive_access().get_trap_cx();
        *trap_cx = TrapContext::app_init_context(
            entry_point,
            user_sp,
            KERNEL_SPACE.exclusive_access().token(),
            kstack_top,
            trap_handler as usize,
        );

        task_control_block
    }
    /// 用一个新程序替换当前程序
    ///
    /// 1. 替换新程序的地址空间
    /// 2. 替换陷阱上下文
    pub fn exec(&self, elf_data: &[u8]) {
        let (memory_set, user_sp, entry_point) = MemorySet::from_elf(elf_data);
        let trap_cx_ppn = memory_set
            .translate(VirtAddr::from(TRAP_CONTEXT_BASE).into())
            .unwrap()
            .ppn();
        let mut inner = self.inner_exclusive_access();
        inner.memory_set = memory_set;
        inner.trap_cx_ppn = trap_cx_ppn;
        inner.base_size = user_sp;
        let trap_cx = inner.get_trap_cx();
        *trap_cx = TrapContext::app_init_context(
            entry_point,
            user_sp,
            KERNEL_SPACE.exclusive_access().token(),
            self.kernel_stack.get_top(),
            trap_handler as usize,
        );
    }
    /// fork出一个新进程
    pub fn fork(self: &Arc<Self>) -> Arc<Self> {
        let mut parent_inner = self.inner_exclusive_access();
        let memory_set = MemorySet::from_existed_user(&parent_inner.memory_set);
        let trap_cx_ppn = memory_set
            .translate(VirtAddr::from(TRAP_CONTEXT_BASE).into())
            .unwrap()
            .ppn();
        let pid = pid_alloc();
        let kernel_stack = kstack_alloc();
        let kstack_top = kernel_stack.get_top();
        let task_control_block = Arc::new(Self {
            pid,
            kernel_stack,
            inner: unsafe {
                UPSafeCell::new(TaskControlBlockInner {
                    trap_cx_ppn,
                    base_size: parent_inner.base_size,
                    task_cx: TaskContext::goto_trap_return(kstack_top),
                    task_status: TaskStatus::Ready,
                    memory_set,
                    parent: Some(Arc::downgrade(self)),
                    children: Vec::new(),
                    exit_code: 0,
                    heap_bottom: parent_inner.heap_bottom,
                    program_brk: parent_inner.program_brk,
                    stride: parent_inner.stride,
                })
            },
        });

        // 将新进程添加到父进程的子进程列表中
        parent_inner.children.push(task_control_block.clone());
        // 修改子进程的trap_cx
        let trap_cx = task_control_block.inner_exclusive_access().get_trap_cx();
        // 仅将kernel_sp指向新的内核栈
        // 其他字段保持不变
        trap_cx.kernel_sp = kstack_top;

        task_control_block
    }

    /// 通过elf数据创建一个新任务
    pub fn spawn(self: &Arc<Self>, elf_data: &[u8]) -> Arc<Self> {
        let new_task = Arc::new(TaskControlBlock::new(elf_data));
        // 增加父子关系
        new_task.inner_exclusive_access().parent = Some(Arc::downgrade(self));
        let mut inner = self.inner_exclusive_access();
        inner.children.push(new_task.clone());
        new_task
    }
    /// 改变任务的堆大小
    ///
    /// size > 0 增大堆
    /// size < 0 减小堆
    ///
    /// 返回旧的堆底部
    pub fn change_program_brk(&self, size: i32) -> Option<usize> {
        let mut inner = self.inner_exclusive_access();
        let heap_bottom = inner.heap_bottom;
        let old_brk = inner.program_brk;
        let new_brk = inner.program_brk as isize + size as isize;

        // 不能小于堆底
        if new_brk < heap_bottom as isize {
            return None;
        }
        let result = if size < 0 {
            // 减小堆
            inner
                .memory_set
                .shrink_to(VirtAddr::from(new_brk as usize), VirtAddr::from(old_brk))
        } else {
            // 增大堆
            inner
                .memory_set
                .append_to(VirtAddr::from(old_brk), VirtAddr::from(new_brk as usize))
        };
        if result {
            // 更新堆顶
            inner.program_brk = new_brk as usize;
            Some(old_brk)
        } else {
            None
        }
    }
}

/// 可变部分
pub struct TaskControlBlockInner {
    /// 陷阱上下文页号
    pub trap_cx_ppn: PhysPageNum,
    /// 应用数据仅可能出现在低于base_size的地址空间
    pub base_size: usize,
    /// 任务上下文，用于任务切换
    pub task_cx: TaskContext,
    /// 任务状态
    pub task_status: TaskStatus,
    /// 任务地址空间
    pub memory_set: MemorySet,
    /// 父进程
    pub parent: Option<Weak<TaskControlBlock>>,
    /// 子进程
    pub children: Vec<Arc<TaskControlBlock>>,
    /// 退出码，用于父进程回收
    pub exit_code: i32,
    /// 堆底部
    heap_bottom: usize,
    program_brk: usize,
    /// stride
    pub stride: TaskStride,
}

impl TaskControlBlockInner {
    /// 获取陷阱上下文指针
    #[inline(always)]
    pub fn get_trap_cx(&self) -> &'static mut TrapContext {
        self.trap_cx_ppn.get_mut()
    }
    /// 获取任务地址空间token
    #[inline(always)]
    pub fn get_user_token(&self) -> usize {
        self.memory_set.token()
    }
    /// 获取任务上下文指针
    #[inline(always)]
    pub fn get_task_cx_ptr(&self) -> *mut TaskContext {
        &self.task_cx as *const TaskContext as *mut TaskContext
    }
    /// 是否为僵尸进程
    #[inline(always)]
    pub fn is_zombie(&self) -> bool {
        self.task_status == TaskStatus::Zombie
    }
    /// 设置优先级
    pub fn set_priority(&mut self, priority: u32) {
        self.stride.priority = priority;
    }
}

#[derive(Copy, Clone, PartialEq)]
/// 任务状态
/// 1. 未初始化
/// 2. 就绪
/// 3. 运行
/// 4. 退出
/// 5. 僵尸
pub enum TaskStatus {
    /// uninitialized
    UnInit,
    /// ready to run
    Ready,
    /// running
    Running,
    /// exited
    Exited,
    /// 僵尸进程
    Zombie,
}

#[derive(Copy, Clone, PartialEq)]
/// stride
pub struct TaskStride {
    /// 优先级
    pub priority: u32,
    /// 步长
    pub pass: u32,
}

impl Default for TaskStride {
    fn default() -> Self {
        Self {
            priority: 16,
            pass: 0,
        }
    }
}
