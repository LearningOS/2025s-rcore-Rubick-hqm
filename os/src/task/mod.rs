//! Task management implementation
//!
//! Everything about task management, like starting and switching tasks is
//! implemented here.
//!
//! A single global instance of [`TaskManager`] called `TASK_MANAGER` controls
//! all the tasks in the operating system.
//!
//! Be careful when you see `__switch` ASM function in `switch.S`. Control flow around this function
//! might not be what you expect.

mod context;
mod manager;
mod pid;
mod processor;
mod switch;
#[allow(clippy::module_inception)]
mod task;

use crate::{loader::get_app_data_by_name, sbi::shutdown};
use alloc::sync::Arc;
use lazy_static::lazy_static;

pub use context::TaskContext;
pub use manager::*;
pub use pid::*;
pub use processor::*;
pub use task::*;

lazy_static! {
    /// initproc的task control block
    pub static ref INITPROC: Arc<TaskControlBlock> = Arc::new(TaskControlBlock::new(
        get_app_data_by_name("ch5b_initproc").unwrap(),
    ));
}

//     /// 记录syscall_id 次数
//     pub fn record_syscall(&self, syscall_id: usize) {
//         let mut inner = self.inner.exclusive_access();
//         let current = inner.current_task;
//         inner.records[current]
//             .entry(syscall_id)
//             .and_modify(|v| *v += 1)
//             .or_insert(1);
//     }

//     /// 返回syscall_id次数
//     pub fn count_syscall(&self, syscall_id: usize) -> isize {
//         let inner = self.inner.exclusive_access();
//         let current = inner.current_task;
//         inner.records[current]
//             .get(&syscall_id)
//             .cloned()
//             .unwrap_or(0)
//     }

// }

/// 暂停当前任务并运行下一个任务
///
/// 1. 将当前任务设置为就绪状态
/// 2. 将当前任务添加到TaskManager中
/// 3. 运行下一个任务
///
/// task目前仍是两部分引用 1. TaskManger.ready_queue 2. initproc.children
pub fn suspend_current_and_run_next() {
    let task = take_current_task().unwrap();
    let task_cx_ptr;
    // 释放task，让task直接移动到add_task中
    {
        let mut task_inner = task.inner_exclusive_access();
        task_inner.task_status = TaskStatus::Ready;
        task_cx_ptr = task_inner.get_task_cx_ptr();
    }
    add_task(task);
    schedule(task_cx_ptr);
}

/// 退出当前任务并运行下一个任务
///
/// 1. 设置当前任务为僵尸状态
/// 2. 将当前任务的所有儿子都托管给initproc
/// 3. 回收当前任务的数据页
/// 4. 运行下一个任务
///
/// task现在仅一个引用 1. initproc.children
pub fn exit_current_and_run_next(exit_code: i32) {
    let task = take_current_task().unwrap();
    // 当前为INITPROC，直接退出
    if task.get_pid() == 0 {
        shutdown();
    }

    let mut inner = task.inner_exclusive_access();
    inner.task_status = TaskStatus::Zombie;
    inner.exit_code = exit_code;

    // 将此任务的所有儿子都托管给initproc
    {
        let mut initproc_inner = INITPROC.inner_exclusive_access();
        inner.children.iter().for_each(|child| {
            child.inner_exclusive_access().parent = Some(Arc::downgrade(&INITPROC));
            initproc_inner.children.push(Arc::clone(child));
        });
    }

    inner.children.clear();
    // 回收当前任务的数据页
    inner.memory_set.recycle_data_pages();
    drop(inner);

    drop(task);

    // 运行下一个任务
    let mut _unused = TaskContext::zero_init();
    schedule(&mut _unused as *mut _);
}

/// 添加initproc到TaskManager中
pub fn add_initproc() {
    add_task(INITPROC.clone());
}
