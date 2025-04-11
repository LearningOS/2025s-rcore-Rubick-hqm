use alloc::vec::Vec;

pub struct TaskRecord {
    syscall: Vec<(usize, usize)>,
}

unsafe impl Sync for TaskRecord {}

impl TaskRecord {
    pub fn new() -> Self {
        Self {
            syscall: Vec::new(),
        }
    }

    pub fn record_syscall(&mut self, target_id: usize) {
        for (id, times) in self.syscall.iter_mut() {
            if *id == target_id {
                *times += 1;
                return;
            }
        }
        self.syscall.push((target_id, 1));
    }

    pub fn count_syscall(&self, target_id: usize) -> usize {
        let result = self.syscall.iter().find(|(id, _time)| *id == target_id);
        match result {
            Some((_id, times)) => *times,
            None => 0,
        }
    }
}
