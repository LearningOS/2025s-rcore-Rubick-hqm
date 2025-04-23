# Lab4实验报告

## 简述功能
process下增加bool为是否启动死锁检测，
task下增加mutex_cnt和semaphore_cnt，表示持有的互斥锁和信号量资源
每次mutex_lock和semaphore_down时，都构造Available，Allocation，Need
在真正调用上锁前调用算法即可，上锁之后，增加对应线程资源持有数量
unlock和up后，减少线程资源持有数量

## 问答题
1. 在我们的多线程实现中，当主线程 (即 0 号线程) 退出时，视为整个进程退出， 此时需要结束该进程管理的所有线程并回收其资源。 
    - 需要回收的资源有哪些？ 
    - 答：线程用户栈，线程控制块TCB，锁资源，文件描述符，内核栈空间等

    - 其他线程的 TaskControlBlock 可能在哪些位置被引用，分别是否需要回收，为什么？
    - 答：可能在TaskManager.ready_queue, PCB的metux_list, semaphore_list, condvar_list中
        ready_queue需要手动回收，因为不受process管理，其他的均由PCB持有，最后销毁PCB时自动回收


2. 对比以下两种 Mutex 中的实现，二者有什么区别？这些区别可能会导致什么问题？
    ```rust
    impl Mutex for Mutex1 {
        fn lock(&self) {
            loop {
                let mut mutex_inner = self.inner.exclusive_access();
                if mutex_inner.locked {
                    mutex_inner.wait_queue.push_back(current_task().unwrap());
                    drop(mutex_inner);
                    block_current_and_run_next();
                } else {
                    mutex_inner.locked = true;
                    break;
                }
            }
        }

        fn unlock(&self) {
            let mut mutex_inner = self.inner.exclusive_access();
            assert!(mutex_inner.locked);
            mutex_inner.locked = false;
            if let Some(waking_task) = mutex_inner.wait_queue.pop_front() {
                add_task(waking_task);
            }
        }
    }

    impl Mutex for Mutex2 {
        fn lock(&self) {
            let mut mutex_inner = self.inner.exclusive_access();
            if mutex_inner.locked {
                mutex_inner.wait_queue.push_back(current_task().unwrap());
                drop(mutex_inner);
                block_current_and_run_next();
            } else {
                mutex_inner.locked = true;
            }
        }

        fn unlock(&self) {
            let mut mutex_inner = self.inner.exclusive_access();
            assert!(mutex_inner.locked);
            if let Some(waking_task) = mutex_inner.wait_queue.pop_front() {
                add_task(waking_task);
            } else {
                mutex_inner.locked = false;
            }
        }
    }
    ```
    第一种实现lock用了循环，unlock不等排队的任务取完后就取消锁占用。
    设想一个情况：
    线程1，先lock了  
    线程2，再lock，这时就会阻塞线程2，放到wait_queue中  
    线程1，unlock了，这时锁就没人占用了  
    此时线程3先获得了cpu调度，这时线程3发现锁没占用，就给线程3了  
    此时线程获得调度权，发现又被锁了，又回到等待队列  
    就会导致线程2饥饿，调度不公平  
    第二种方法，在上述情况中，线程3会继续阻塞，加到线程2后等待调度

## 荣誉准则
1. 在完成本次实验的过程（含此前学习的过程）中，我曾分别与 以下各位 就（与本次实验相关的）以下方面做过交流，还在代码中对应的位置以注释形式记录了具体的交流对象及内容：  
无
2. 此外，我也参考了 以下资料 ，还在代码中对应的位置以注释形式记录了具体的参考来源及内容：  
无
3. 我独立完成了本次实验除以上方面之外的所有工作，包括代码与文档。 我清楚地知道，从以上方面获得的信息在一定程度上降低了实验难度，可能会影响起评分。
4. 我从未使用过他人的代码，不管是原封不动地复制，还是经过了某些等价转换。 我未曾也不会向他人（含此后各届同学）复制或公开我的实验代码，我有义务妥善保管好它们。 我提交至本实验的评测系统的代码，均无意于破坏或妨碍任何计算机系统的正常运转。 我清楚地知道，以上情况均为本课程纪律所禁止，若违反，对应的实验成绩将按“-100”分计。

## 建议
无