//! Process management syscalls
use crate::{
    task::{
        count_current_syscall, exit_current_and_run_next, read_current_data,
        suspend_current_and_run_next, write_current_data,
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
    trace!("[kernel] Application exited with code {}", exit_code);
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

/// get time with second and microsecond
pub fn sys_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    let us = get_time_us();
    unsafe {
        *ts = TimeVal {
            sec: us / 1_000_000,
            usec: us % 1_000_000,
        };
    }
    0
}

// TODO: implement the syscall
//  read trace id
const TRACE_READ: usize = 0;
//  write trace id
const TRACE_WRITE: usize = 1;
//  count trace id
const TRACE_COUNT: usize = 2;

pub fn sys_trace(trace_request: usize, id: usize, data: usize) -> isize {
    trace!("kernel: sys_trace");
    match trace_request {
        TRACE_READ => read_current_data(id as *const u8) as isize,
        TRACE_WRITE => write_current_data(id as *const u8, data as u8) as isize,
        TRACE_COUNT => count_current_syscall(id) as isize,
        _ => -1,
    }
}
