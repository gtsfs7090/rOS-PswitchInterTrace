//! Types related to task management

use super::TaskContext;

#[derive(Copy, Clone)]
pub struct TaskControlBlock {
    pub task_status: TaskStatus,
    pub task_cx: TaskContext,
    pub pid: usize,  //新增代码行
}

#[derive(Copy, Clone, PartialEq)]
#[derive(Debug)]  //新增代码行
pub enum TaskStatus {
    UnInit,
    Ready,
    Running,
    Exited,
}
