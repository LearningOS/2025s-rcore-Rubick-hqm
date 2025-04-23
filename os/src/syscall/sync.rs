use core::cell::RefMut;

use crate::sync::{Condvar, Mutex, MutexBlocking, MutexSpin, Semaphore};
use crate::task::{
    block_current_and_run_next, current_process, current_task, ProcessControlBlockInner,
};
use crate::timer::{add_timer, get_time_ms};
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;

const MUTEX: usize = 0;
const SEMAPHORE: usize = 1;

fn prepare_dealock(
    process_inner: &RefMut<'_, ProcessControlBlockInner>,
    res_type: usize,
) -> (Vec<usize>, Vec<Vec<usize>>, Vec<Vec<usize>>) {
    let current_task_tid = current_task()
        .unwrap()
        .inner_exclusive_access()
        .res
        .as_ref()
        .unwrap()
        .tid;
    let mut available = Vec::new();
    available.push(process_inner.mutex_list.len());
    for semaphore in process_inner.semaphore_list.iter() {
        let mut v = 0;
        if let Some(semaphore) = semaphore {
            v = semaphore.inner.exclusive_access().count;
        }
        available.push(v.max(0) as usize);
    }

    let resource_len = available.len();

    let mut allocation = Vec::new();
    let mut need = Vec::new();
    for task in process_inner.tasks.iter() {
        if task.is_none() {
            continue;
        }
        let task = task.as_ref().unwrap();
        let task_inner = task.inner_exclusive_access();
        let mut v = vec![0; resource_len];
        if task_inner.res.is_some() && task_inner.res.as_ref().unwrap().tid == current_task_tid {
            v[res_type] = 1;
        }
        need.push(v);

        let mut cur_task_alloc = vec![0; resource_len];
        cur_task_alloc[MUTEX] = task_inner.mutex_cnt;
        available[MUTEX] -= task_inner.mutex_cnt;
        for (sem_id, count) in task_inner.semaphore_cnt.iter() {
            let count = (*count).max(0) as usize;
            cur_task_alloc[SEMAPHORE + sem_id] = count;
            available[SEMAPHORE + sem_id] -= count;
        }
        allocation.push(cur_task_alloc);
    }
    println!("avaliable: {:?}", available);
    println!("allocation: {:?}", allocation);
    println!("need: {:?}", need);
    (available, allocation, need)
}

/// 检查是否死锁
///
/// true为死锁
fn check_deadlock(
    available: Vec<usize>,
    allocation: Vec<Vec<usize>>,
    need: Vec<Vec<usize>>,
) -> bool {
    println!("start deadlock check");
    println!("available: {:?}", available);
    println!("allocation: {:?}", allocation);
    println!("need: {:?}", need);

    let mut work = available;
    let mut finish = vec![false; allocation.len()];
    loop {
        let index = finish
            .iter()
            .enumerate()
            .find(|(i, item)| {
                **item == false && !work.iter().enumerate().any(|(j, item)| *item < need[*i][j])
            })
            .map(|(i, _)| i);
        match index {
            Some(i) => {
                for j in 0..work.len() {
                    work[j] = work[j] + allocation[i][j];
                }
                finish[i] = true;
            }
            None => break,
        }
        println!("work: {:?}", work);
        println!("finish: {:?}", finish);
        println!("------");
    }
    println!("finish: {:?}", finish);
    println!("work: {:?}", work);
    // 只要有一个为false就是死锁
    finish.iter().any(|v| *v == false)
}

/// sleep syscall
pub fn sys_sleep(ms: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_sleep",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let expire_ms = get_time_ms() + ms;
    let task = current_task().unwrap();
    add_timer(expire_ms, task);
    block_current_and_run_next();
    0
}
/// mutex create syscall
pub fn sys_mutex_create(blocking: bool) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mutex: Option<Arc<dyn Mutex>> = if !blocking {
        Some(Arc::new(MutexSpin::new()))
    } else {
        Some(Arc::new(MutexBlocking::new()))
    };
    let mut process_inner = process.inner_exclusive_access();
    if let Some(id) = process_inner
        .mutex_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.mutex_list[id] = mutex;
        id as isize
    } else {
        process_inner.mutex_list.push(mutex);
        process_inner.mutex_list.len() as isize - 1
    }
}
/// mutex lock syscall
pub fn sys_mutex_lock(mutex_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_lock",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    if process_inner.enable_deadlock {
        let (available, allocation, need) = prepare_dealock(&process_inner, MUTEX);
        if check_deadlock(available, allocation, need) {
            return -0xDEAD;
        }
    }
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    drop(process_inner);
    drop(process);
    mutex.lock();
    let task = current_task().unwrap();
    let mut task_inner = task.inner_exclusive_access();
    task_inner.mutex_cnt += 1;
    0
}
/// mutex unlock syscall
pub fn sys_mutex_unlock(mutex_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_unlock",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    drop(process_inner);
    drop(process);
    mutex.unlock();
    let task = current_task().unwrap();
    let mut task_inner = task.inner_exclusive_access();
    task_inner.mutex_cnt -= 1;
    0
}
/// semaphore create syscall
pub fn sys_semaphore_create(res_count: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_semaphore_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let id = if let Some(id) = process_inner
        .semaphore_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.semaphore_list[id] = Some(Arc::new(Semaphore::new(res_count)));
        id
    } else {
        process_inner
            .semaphore_list
            .push(Some(Arc::new(Semaphore::new(res_count))));
        process_inner.semaphore_list.len() - 1
    };
    id as isize
}
/// semaphore up syscall
pub fn sys_semaphore_up(sem_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_semaphore_up",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let sem = Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap());
    drop(process_inner);
    sem.up();
    let task = current_task().unwrap();
    let mut task_inner = task.inner_exclusive_access();
    task_inner
        .semaphore_cnt
        .entry(SEMAPHORE + sem_id)
        .and_modify(|v| *v -= 1);
    0
}
/// semaphore down syscall
pub fn sys_semaphore_down(sem_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_semaphore_down",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    if process_inner.enable_deadlock {
        let (available, allocation, need) = prepare_dealock(&process_inner, SEMAPHORE + sem_id);
        if check_deadlock(available, allocation, need) {
            return -0xDEAD;
        }
    }

    let sem = Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap());
    drop(process_inner);
    sem.down();
    let task = current_task().unwrap();
    let mut task_inner = task.inner_exclusive_access();
    task_inner
        .semaphore_cnt
        .entry(sem_id)
        .and_modify(|v| *v += 1)
        .or_insert(1);
    0
}
/// condvar create syscall
pub fn sys_condvar_create() -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let id = if let Some(id) = process_inner
        .condvar_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.condvar_list[id] = Some(Arc::new(Condvar::new()));
        id
    } else {
        process_inner
            .condvar_list
            .push(Some(Arc::new(Condvar::new())));
        process_inner.condvar_list.len() - 1
    };
    id as isize
}
/// condvar signal syscall
pub fn sys_condvar_signal(condvar_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_signal",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let condvar = Arc::clone(process_inner.condvar_list[condvar_id].as_ref().unwrap());
    drop(process_inner);
    condvar.signal();
    0
}
/// condvar wait syscall
pub fn sys_condvar_wait(condvar_id: usize, mutex_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_wait",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let condvar = Arc::clone(process_inner.condvar_list[condvar_id].as_ref().unwrap());
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    drop(process_inner);
    condvar.wait(mutex);
    0
}
/// enable deadlock detection syscall
pub fn sys_enable_deadlock_detect(enabled: usize) -> isize {
    trace!("kernel: sys_enable_deadlock_detect NOT IMPLEMENTED");
    let is_enable = match enabled {
        0 => false,
        1 => true,
        _ => return -1,
    };

    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    process_inner.enable_deadlock = is_enable;
    0
}
