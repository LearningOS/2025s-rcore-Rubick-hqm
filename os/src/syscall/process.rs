//! Process management syscalls
use alloc::sync::Arc;

use crate::{
    fs::{open_file, OpenFlags},
    mm::{translate_str, translated_byte_buffer, translated_refmut, MapPermission, VirtAddr},
    task::{
        add_task, current_task, current_user_token, exit_current_and_run_next,
        suspend_current_and_run_next,
    },
    timer::get_time_us,
};

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// task exits and submit an exit code
pub fn sys_exit(exit_code: i32) -> ! {
    trace!("kernel: sys_exit");
    exit_current_and_run_next(exit_code);
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

/// 获取当前时间
///
/// ts: 指向TimeVal结构体的指针
/// _tz: 时区，暂时不用
///
/// 返回0
pub fn sys_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    let us = get_time_us();
    let timeval = TimeVal {
        sec: us / 1_000_000,
        usec: us % 1_000_000,
    };
    let src = &timeval as *const TimeVal as *const u8;
    // ts是虚拟地址，需要拿到他的物理地址
    let dst = ts as *const u8;
    let buffers =
        translated_byte_buffer(current_user_token(), dst, core::mem::size_of_val(&timeval));
    for buffer in buffers {
        unsafe {
            buffer.copy_from_slice(core::slice::from_raw_parts(src, buffer.len()));
        }
    }
    0
}

/// 映射一段逻辑段
///
/// start: 逻辑段起始地址
/// len: 逻辑段长度
/// prot: 保护属性
///
/// 返回：成功返回0，失败返回-1
pub fn sys_mmap(start: usize, len: usize, prot: usize) -> isize {
    trace!("kernel: sys_mmap NOT IMPLEMENTED YET!");

    // 校验参数

    // 检查prot是否合法，仅允许R,W,X
    if prot & !0x7 != 0 || prot & 0x7 == 0 {
        return -1;
    }
    // 将prot转换为MapPermission
    let mut perm = MapPermission::U;
    if prot & 0x1 != 0 {
        perm |= MapPermission::R;
    }
    if prot & 0x2 != 0 {
        perm |= MapPermission::W;
    }
    if prot & 0x4 != 0 {
        perm |= MapPermission::X;
    }
    // 检查start是否4k对齐
    if start & 0xfff != 0 {
        return -1;
    }

    let current_task = current_task().unwrap();
    let mut inner = current_task.inner_exclusive_access();
    let memory_set = &mut inner.memory_set;
    // 检查是否存在重叠区域
    if memory_set.overlaps(VirtAddr::from(start), VirtAddr::from(start + len)) {
        return -1;
    }
    memory_set.insert_framed_area(VirtAddr::from(start), VirtAddr::from(start + len), perm);
    0
}

/// 取消映射一段逻辑段
///
/// start: 逻辑段起始地址
/// len: 逻辑段长度
///
/// 返回：成功返回0，失败返回-1
pub fn sys_munmap(start: usize, len: usize) -> isize {
    trace!("kernel: sys_munmap NOT IMPLEMENTED YET!");

    // 检查start是否4k对齐
    if start & 0xfff != 0 {
        return -1;
    }

    let current_task = current_task().unwrap();
    let mut inner = current_task.inner_exclusive_access();
    let memory_set = &mut inner.memory_set;

    memory_set.remove_area(VirtAddr::from(start), VirtAddr::from(start + len))
}

/// 改变堆大小
///
/// size: 堆大小，正数增大堆，负数减小堆
///
/// 返回：成功返回旧的堆顶，失败返回-1
pub fn sys_sbrk(size: i32) -> isize {
    trace!("kernel: sys_sbrk");
    let current_task = current_task().unwrap();
    if let Some(old_brk) = current_task.change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}

/// 创建一个新进程
///
/// 返回：新进程的PID
pub fn sys_fork() -> isize {
    trace!("kernel: sys_fork");
    let current_task = current_task().unwrap();
    let new_task = current_task.fork();
    let new_pid = new_task.get_pid();

    // 设置子进程返回值为0，通过设置寄存器值x10
    let trap_cx = new_task.inner_exclusive_access().get_trap_cx();
    trap_cx.x[10] = 0;
    add_task(new_task);

    new_pid as isize
}

/// 执行一个新程序
///
/// path: 程序路径
///
/// 返回：成功返回0，失败返回-1
pub fn sys_exec(path: *const u8) -> isize {
    trace!("kernel:pid[{}] sys_exec", current_task().unwrap().pid.0);
    let path = translate_str(current_user_token(), path);
    if let Some(app_inode) = open_file(path.as_str(), OpenFlags::RDONLY) {
        let all_data = app_inode.read_all();
        let current_task = current_task().unwrap();
        current_task.exec(all_data.as_slice());
        0
    } else {
        -1
    }
}

pub fn sys_spawn(path: *const u8) -> isize {
    trace!("kernel:pid[{}] sys_exec", current_task().unwrap().pid.0);
    let path = translate_str(current_user_token(), path);
    if let Some(app_inode) = open_file(path.as_str(), OpenFlags::RDONLY) {
        let all_data = app_inode.read_all();
        let current_task = current_task().unwrap();
        let new_task = current_task.spawn(all_data.as_slice());
        let new_pid = new_task.get_pid();
        add_task(new_task);
        new_pid as isize
    } else {
        -1
    }
}

/// 等待子进程结束
///
/// pid: 子进程的PID
/// exit_code_ptr: 指向exit_code的指针
///
/// 返回：成功返回子进程的PID，失败返回-1
pub fn sys_waitpid(pid: isize, exit_code_ptr: *mut i32) -> isize {
    trace!("kernel: sys_waitpid");

    let current_task = current_task().unwrap();

    let mut inner = current_task.inner_exclusive_access();
    if !inner
        .children
        .iter()
        .any(|p| pid == -1 || pid as usize == p.get_pid())
    {
        return -1;
    }

    let pair = inner.children.iter().enumerate().find(|(_, p)| {
        p.inner_exclusive_access().is_zombie() && (pid == -1 || pid as usize == p.get_pid())
    });

    if let Some((index, _)) = pair {
        let child = inner.children.remove(index);
        assert_eq!(Arc::strong_count(&child), 1);
        let found_pid = child.get_pid();
        let exit_code = child.inner_exclusive_access().exit_code;
        *translated_refmut(inner.memory_set.token(), exit_code_ptr) = exit_code;
        found_pid as isize
    } else {
        -2
    }
}

/// 获取当前进程的PID
pub fn sys_getpid() -> isize {
    trace!("kernel: sys_getpid");
    current_task().unwrap().get_pid() as isize
}

/// 设置当前进程的优先级
pub fn sys_set_priority(priority: isize) -> isize {
    trace!("kernel: sys_set_priority");
    if priority <= 1 {
        return -1;
    }
    let current_task = current_task().unwrap();
    let mut inner = current_task.inner_exclusive_access();
    inner.set_priority(priority as u32);
    0
}
