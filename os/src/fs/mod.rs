//! File trait & inode(dir, file, pipe, stdin, stdout)

mod inode;
mod stdio;

use crate::mm::UserBuffer;

/// 所有文件类型trait
pub trait File: Send + Sync {
    /// 是否可读
    fn readable(&self) -> bool;
    /// 是否可写
    fn writable(&self) -> bool;
    /// 读数据到buf，返回已读多少字节
    fn read(&self, buf: UserBuffer) -> usize;
    /// 写数据到buf，返回已写多少字节
    fn write(&self, buf: UserBuffer) -> usize;
    /// 返回文件fstat
    fn fstat(&self) -> Stat {
        panic!("this file can't fstat")
    }
}

/// The stat of a inode
#[repr(C)]
#[derive(Debug)]
pub struct Stat {
    /// ID of device containing file
    pub dev: u64,
    /// inode number
    pub ino: u64,
    /// file type and mode
    pub mode: StatMode,
    /// number of hard links
    pub nlink: u32,
    /// unused pad
    pad: [u64; 7],
}

bitflags! {
    /// The mode of a inode
    /// whether a directory or a file
    pub struct StatMode: u32 {
        /// null
        const NULL  = 0;
        /// directory
        const DIR   = 0o040000;
        /// ordinary regular file
        const FILE  = 0o100000;
    }
}

pub use inode::{list_apps, open_file, OSInode, OpenFlags, ROOT_INODE};
pub use stdio::{Stdin, Stdout};
