//! File and filesystem-related syscalls

use crate::fs::{open_file, OpenFlags, Stat, ROOT_INODE};
use crate::mm::{translate_str, translated_byte_buffer, translated_refmut, UserBuffer};
use crate::task::{current_task, current_user_token};

/// write buf of length `len`  to a file with `fd`
pub fn sys_write(fd: usize, buf: *const u8, len: usize) -> isize {
    trace!("kernel:pid[{}] sys_write", current_task().unwrap().pid.0);
    let token = current_user_token();
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if let Some(file) = &inner.fd_table[fd] {
        let file = file.clone();
        drop(inner);
        file.write(UserBuffer::new(translated_byte_buffer(token, buf, len))) as isize
    } else {
        -1
    }
}

/// 从fd读取len字节到buf
///
/// 目前len仅支持1
pub fn sys_read(fd: usize, buf: *const u8, len: usize) -> isize {
    trace!("kernel:pid[{}] sys_read", current_task().unwrap().pid.0);
    let token = current_user_token();
    let task = current_task().unwrap();
    let inner = task.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if let Some(file) = &inner.fd_table[fd] {
        let file = file.clone();
        drop(inner);
        file.read(UserBuffer::new(translated_byte_buffer(token, buf, len))) as isize
    } else {
        -1
    }
}

pub fn sys_open(path: *const u8, flags: u32) -> isize {
    trace!("kernel:pid[{}] sys_open", current_task().unwrap().pid.0);
    let current_task = current_task().unwrap();
    let name = translate_str(current_user_token(), path);
    if let Some(inode) = open_file(name.as_str(), OpenFlags::from_bits(flags).unwrap()) {
        let mut inner = current_task.inner_exclusive_access();
        let fd = inner.alloc_fd();
        // println!(
        //     "name: {:?}, fd: {:?}, inode stat: {:?}",
        //     name,
        //     fd,
        //     inode.fstat()
        // );
        inner.fd_table[fd] = Some(inode);
        fd as isize
    } else {
        -1
    }
}

pub fn sys_close(fd: usize) -> isize {
    trace!("kernel:pid[{}] sys_close", current_task().unwrap().pid.0);
    let current_task = current_task().unwrap();
    let mut inner = current_task.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if inner.fd_table[fd].is_none() {
        return -1;
    }
    inner.fd_table[fd].take();
    0
}

pub fn sys_fstat(fd: usize, st: *mut Stat) -> isize {
    trace!("kernel:pid[{}] sys_fstat", current_task().unwrap().pid.0);
    // println!("sys_fstat {}", fd);
    let current_task = current_task().unwrap();
    let token = current_user_token();
    let inner = current_task.inner_exclusive_access();
    if fd >= inner.fd_table.len() {
        return -1;
    }
    if inner.fd_table[fd].is_none() {
        return -1;
    }
    let inode = inner.fd_table[fd].clone().unwrap();
    let stat = inode.fstat();

    // println!("{:?}", stat);

    *translated_refmut(token, st) = stat;
    0
}

pub fn sys_linkat(old_name: *const u8, new_name: *const u8) -> isize {
    trace!("kernel:pid[{}] sys_linkat", current_task().unwrap().pid.0);
    let token = current_user_token();
    let old_name = translate_str(token, old_name);
    let new_name = translate_str(token, new_name);
    if old_name == new_name {
        return -1;
    }
    let inode = ROOT_INODE.find(&old_name);
    if inode.is_none() {
        return -1;
    }
    let inode = inode.unwrap();
    ROOT_INODE.linkat(&new_name, inode);
    0
}

pub fn sys_unlinkat(name: *const u8) -> isize {
    trace!("kernel:pid[{}] sys_unlinkat", current_task().unwrap().pid.0);
    // println!("sys_unlinkat");
    let token = current_user_token();
    let name = translate_str(token, name);
    let inode = ROOT_INODE.find(&name);
    if inode.is_none() {
        return -1;
    }
    let inode = inode.unwrap();
    // println!("{}", inode.get_inode_id());
    // println!("{}", inode.get_nlink());
    ROOT_INODE.unlinkat(&name, inode);
    0
}
