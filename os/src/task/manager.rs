use alloc::{collections::vec_deque::VecDeque, sync::Arc};
use lazy_static::lazy_static;

use crate::sync::UPSafeCell;

use super::TaskControlBlock;

lazy_static! {
    static ref TASK_MANAGER: UPSafeCell<TaskManager> =
        unsafe { UPSafeCell::new(TaskManager::new()) };
}

struct TaskManager {
    ready_queue: VecDeque<Arc<TaskControlBlock>>,
}

impl TaskManager {
    fn new() -> Self {
        Self {
            ready_queue: VecDeque::new(),
        }
    }
    fn add(&mut self, task: Arc<TaskControlBlock>) {
        let inner = task.inner_exclusive_access();
        let add_pass = inner.stride.pass;
        drop(inner);

        for (i, t) in self.ready_queue.iter().enumerate() {
            let pass = t.inner_exclusive_access().stride.pass;
            if pass > add_pass {
                self.ready_queue.insert(i, task);
                return;
            }
        }

        self.ready_queue.push_back(task);
    }
    fn fetch(&mut self) -> Option<Arc<TaskControlBlock>> {
        self.ready_queue.pop_front()
    }
}

/// 添加一个任务到就绪队列
pub fn add_task(task: Arc<TaskControlBlock>) {
    TASK_MANAGER.exclusive_access().add(task);
}

/// 从就绪队列中获取一个任务
pub fn fetch_task() -> Option<Arc<TaskControlBlock>> {
    TASK_MANAGER.exclusive_access().fetch()
}
