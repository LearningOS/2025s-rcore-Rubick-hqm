use alloc::sync::Arc;
use lazy_static::lazy_static;

use crate::{sync::UPSafeCell, trap::TrapContext};

use super::{
    manager::fetch_task, switch::__switch, TaskContext, TaskControlBlock, TaskStatus, MAX_PRIORITY,
};

lazy_static! {
    /// 全局处理器
    pub static ref PROCESSOR: UPSafeCell<Processor> = unsafe { UPSafeCell::new(Processor::new()) };
}

/// 处理器
pub struct Processor {
    /// 当前任务
    current: Option<Arc<TaskControlBlock>>,
    /// 空闲任务流
    idle_task_cx: TaskContext,
}

impl Processor {
    /// 创建一个处理器
    pub fn new() -> Self {
        Self {
            current: None,
            idle_task_cx: TaskContext::zero_init(),
        }
    }

    /// 消费当前任务
    fn take_current(&mut self) -> Option<Arc<TaskControlBlock>> {
        self.current.take()
    }

    /// 获取当前任务引用
    fn current(&self) -> Option<Arc<TaskControlBlock>> {
        self.current.clone()
    }

    /// 获取空闲任务流
    fn get_idle_task_cx_ptr(&mut self) -> *mut TaskContext {
        &mut self.idle_task_cx as *mut TaskContext
    }
}

/// 消费当前任务
pub fn take_current_task() -> Option<Arc<TaskControlBlock>> {
    PROCESSOR.exclusive_access().take_current()
}

/// 获取当前任务引用
pub fn current_task() -> Option<Arc<TaskControlBlock>> {
    PROCESSOR.exclusive_access().current()
}

/// 获取当前任务用户token
pub fn current_user_token() -> usize {
    let task = current_task().unwrap();
    let token = task.inner_exclusive_access().get_user_token();
    token
}

/// 获取当前任务陷阱上下文
pub fn current_trap_cx() -> &'static mut TrapContext {
    current_task()
        .unwrap()
        .inner_exclusive_access()
        .get_trap_cx()
}

/// 运行任务
///
/// 1. 尝试从TaskManager中消费一个任务
/// 2. 如果获取到任务，则将任务设置为运行状态，并从空闲任务切换到该任务
///
/// 这个任务目前被两个持有 1. current 2. initproc.children
pub fn run_tasks() {
    loop {
        let mut processor = PROCESSOR.exclusive_access();
        if let Some(task) = fetch_task() {
            let idle_task_cx_ptr = processor.get_idle_task_cx_ptr();
            let next_task_cx_ptr;
            {
                let mut task_inner = task.inner_exclusive_access();
                next_task_cx_ptr = task_inner.get_task_cx_ptr();
                task_inner.task_status = TaskStatus::Running;
                task_inner.stride.pass += MAX_PRIORITY as u32 / task_inner.stride.priority;
            }
            processor.current = Some(task);
            drop(processor);
            // 切换任务流，这里会从idle_task_cx_ptr切换到next_task_cx_ptr
            // next_task_cx完成后会通过schedule函数切换回idle_task_cx_ptr
            unsafe { __switch(idle_task_cx_ptr, next_task_cx_ptr) };
        }
    }
}

/// 调度任务
///
/// 1. 从当前任务切换到空闲任务
pub fn schedule(switched_task_cx_ptr: *mut TaskContext) {
    let mut processor = PROCESSOR.exclusive_access();
    let idle_task_cx_ptr = processor.get_idle_task_cx_ptr();
    drop(processor);

    // 切换任务流，这里会从switched_task_cx_ptr切换到idle_task_cx_ptr
    // 返回到run_tasks的__switch
    unsafe { __switch(switched_task_cx_ptr, idle_task_cx_ptr) };
}
