# Lab4实验报告

## 简述功能
1. sys_linkat
    扩展DiskInode，增减nlink字段，减少direct，保持128字节
    先通过old_name找到对饮Inode，nlink++，
    再ROOT_INODE增加目录项，将其inode_id设为上面的Inode
2. sys_unlinkat
    通过name找到对应Inode，nlink--
    当nlink=0，Inode::Clear()清空data，
    再将ROOT_INODE减少目录项
3. sys_fstat
    先在fd_table中找对应Inode，为File拓展fstat方法，
    然后通过efs来根据Inode的block_id和offset转成inode_id,
    mode就根据Inode的type_
    nlink就找Inode对应DiskInode的nlink字段

## 问答题
1. 在我们的easy-fs中，root inode起着什么作用？如果root inode中的内容损坏了，会发生什么？
    - root_inode作为根目录存放所有文件，所有文件相关操作都通过查找root inode的目录项找到对应文件Inode
    - 如果root inode损坏，那么可能所有文件相关操作都会失败，比如sys_exec时会去打开文件，但是open_file会失败从而无法切换进程执行

## 荣誉准则
1. 在完成本次实验的过程（含此前学习的过程）中，我曾分别与 以下各位 就（与本次实验相关的）以下方面做过交流，还在代码中对应的位置以注释形式记录了具体的交流对象及内容：  
无
2. 此外，我也参考了 以下资料 ，还在代码中对应的位置以注释形式记录了具体的参考来源及内容：  
无
3. 我独立完成了本次实验除以上方面之外的所有工作，包括代码与文档。 我清楚地知道，从以上方面获得的信息在一定程度上降低了实验难度，可能会影响起评分。
4. 我从未使用过他人的代码，不管是原封不动地复制，还是经过了某些等价转换。 我未曾也不会向他人（含此后各届同学）复制或公开我的实验代码，我有义务妥善保管好它们。 我提交至本实验的评测系统的代码，均无意于破坏或妨碍任何计算机系统的正常运转。 我清楚地知道，以上情况均为本课程纪律所禁止，若违反，对应的实验成绩将按“-100”分计。

## 建议
无