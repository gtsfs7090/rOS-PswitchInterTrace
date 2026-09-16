//! Task management implementation
//!
//! Everything about task management, like starting and switching tasks is
//! implemented here.
//!
//! A single global instance of [`TaskManager`] called `TASK_MANAGER` controls
//! all the tasks in the operating system.
//!
//! Be careful when you see `__switch` ASM function in `switch.S`. Control flow around this function
//! might not be what you expect.

mod context;
mod switch;

#[allow(clippy::module_inception)]
mod task;
pub mod switch_log;  //新增代码行,添加pub

use crate::config::MAX_APP_NUM;
use crate::loader::{get_num_app, init_app_cx,print_stack_addresses};
use crate::sbi::shutdown;
use crate::sync::UPSafeCell;
use lazy_static::*;
use switch::__switch;
pub use task::{TaskControlBlock, TaskStatus};  //要加上pub

pub use context::TaskContext;

pub use switch_log::*;  //新增代码行
pub use crate::trap::{print_trap_summary,print_branch_statistics};   //新增代码行
pub use crate::common::event_log::*;   //新增代码行
/// The task manager, where all the tasks are managed.
///
/// Functions implemented on `TaskManager` deals with all task state transitions
/// and task context switching. For convenience, you can find wrappers around it
/// in the module level.
///
/// Most of `TaskManager` are hidden behind the field `inner`, to defer
/// borrowing checks to runtime. You can see examples on how to use `inner` in
/// existing functions on `TaskManager`.
pub struct TaskManager {
    /// total number of tasks
    num_app: usize,
    /// use inner value to get mutable access
    inner: UPSafeCell<TaskManagerInner>,
}

/// Inner of Task Manager
pub struct TaskManagerInner {
    /// task list
    tasks: [TaskControlBlock; MAX_APP_NUM],
    /// id of current `Running` task
    current_task: usize,
}

lazy_static! {
    /// Global variable: TASK_MANAGER
    pub static ref TASK_MANAGER: TaskManager = {
        let num_app = get_num_app();
        let mut tasks = [TaskControlBlock {
            task_cx: TaskContext::zero_init(),
            task_status: TaskStatus::UnInit,
            pid:0,  //新增代码行
        }; MAX_APP_NUM];
        for (i, task) in tasks.iter_mut().enumerate() {
            task.task_cx = TaskContext::goto_restore(init_app_cx(i));
            task.task_status = TaskStatus::Ready;
            task.pid=i;  //新增代码行
        }
        TaskManager {
            num_app,
            inner: unsafe {
                UPSafeCell::new(TaskManagerInner {
                    tasks,
                    current_task: 0,
                })
            },
        }
    };
}

impl TaskManager {
    /// Run the first task in task list.
    ///
    /// Generally, the first task in task list is an idle task (we call it zero process later).
    /// But in ch3, we load apps statically, so the first task is a real app.
    fn run_first_task(&self) -> ! {
        let mut inner = self.inner.exclusive_access();
        let task0 = &mut inner.tasks[0];
        task0.task_status = TaskStatus::Running;
        let next_task_cx_ptr = &task0.task_cx as *const TaskContext;

        // 收集新进程的详细信息
        let new_pid = task0.pid;
        let new_tcb_addr = task0 as *const _ as usize;
        let new_context_addr = &task0.task_cx as *const TaskContext as usize;
        let new_tcb_content = TcbContent::from_tcb(task0);
        let new_regs = RegistersSnapshot::from_task_context(&task0.task_cx);
        
        drop(inner);
        
        // 创建第一次切换的记录
        let record = SwitchRecord::new_first_task(
            new_pid, new_tcb_addr, new_tcb_content, new_context_addr, new_regs,  //new_regs,  //
        );
        add_switch_record_to_unified(record); //新增代码行
        let mut _unused = TaskContext::zero_init();
        // before this, we should drop local variables that must be dropped manually
        // 执行第一次上下文切换（从虚拟上下文切换到第一个任务）
        unsafe {
            __switch(&mut _unused as *mut TaskContext, next_task_cx_ptr);
        }
        panic!("unreachable in run_first_task!");
    }

    /// Change the status of current `Running` task into `Ready`.
    fn mark_current_suspended(&self) {
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task;
        inner.tasks[current].task_status = TaskStatus::Ready;
    }

    /// Change the status of current `Running` task into `Exited`.
    fn mark_current_exited(&self) {
        let mut inner = self.inner.exclusive_access();
        let current = inner.current_task;
        inner.tasks[current].task_status = TaskStatus::Exited;
    }

    /// Find next task to run and return task id.
    ///
    /// In this case, we only return the first `Ready` task in task list.
    fn find_next_task(&self) -> Option<usize> {
        let inner = self.inner.exclusive_access();
        let current = inner.current_task;
        (current + 1..current + self.num_app + 1)
            .map(|id| id % self.num_app)
            .find(|id| inner.tasks[*id].task_status == TaskStatus::Ready)
    }

    /// Switch current `Running` task to the task we have found,
    /// or there is no `Ready` task and we can exit with all applications completed
    fn run_next_task(&self) {
        if let Some(next) = self.find_next_task() {
            let mut inner = self.inner.exclusive_access();
            let current = inner.current_task;
            inner.tasks[next].task_status = TaskStatus::Running;
            inner.current_task = next;
            let current_task_cx_ptr = &mut inner.tasks[current].task_cx as *mut TaskContext;
            let next_task_cx_ptr = &inner.tasks[next].task_cx as *const TaskContext;
            
            // 获取当前进程和下一个进程的引用
            let old_task = &inner.tasks[current];
            let new_task = &inner.tasks[next];
            
            // === 收集旧进程的详细信息 ===
            let old_pid = old_task.pid;
            let old_tcb_addr = old_task as *const _ as usize;
            let old_context_addr = &old_task.task_cx as *const TaskContext as usize;
            let old_tcb_content = TcbContent::from_tcb(old_task);
            let old_regs = RegistersSnapshot::from_task_context(&old_task.task_cx);
            
            // === 收集新进程的详细信息 ===
            let new_pid = new_task.pid;
            let new_tcb_addr = new_task as *const _ as usize;
            let new_context_addr = &new_task.task_cx as *const TaskContext as usize;
            let new_tcb_content = TcbContent::from_tcb(new_task);
            let new_regs = RegistersSnapshot::from_task_context(&new_task.task_cx);
            
            // 创建切换记录
            let record = SwitchRecord::new(
                old_pid, old_tcb_addr, old_tcb_content, old_context_addr, old_regs,  //old_regs,   //
                new_pid, new_tcb_addr, new_tcb_content, new_context_addr, new_regs,  //new_regs,  //
            );
            add_switch_record_to_unified(record); //新增代码行
           drop(inner);
            // before this, we should drop local variables that must be dropped manually
            unsafe {
                __switch(current_task_cx_ptr, next_task_cx_ptr);
            }
            // go back to user mode
        } else {
            println!("All applications completed!");
            println!("===============================Process Switch/Trap History===============================");
            print_unified_chronological(); //新增代码行
            println!("===============================Process Switch/Trap Summary===============================");
            print_unified_summary(); //新增代码行
            println!("===============================Process 0 Switch/Trap Summary===============================");
            display_unified_by_pid(0);
            println!("===============================Process 1 Switch/Trap Summary===============================");
            display_unified_by_pid(1);
            println!("===============================Process 2 Switch/Trap Summary===============================");
            display_unified_by_pid(2);
            println!("===============================Process 3 Switch/Trap Summary===============================");
            display_unified_by_pid(3);
            println!("===============================position of kernel stack and user stack===============================");
            print_stack_addresses();
            shutdown(false);
        }
    }
}

/// run first task
pub fn run_first_task() {
    TASK_MANAGER.run_first_task();
}

/// rust next task
fn run_next_task() {
    TASK_MANAGER.run_next_task();
}

/// suspend current task
fn mark_current_suspended() {
    TASK_MANAGER.mark_current_suspended();
}

/// exit current task
fn mark_current_exited() {
    TASK_MANAGER.mark_current_exited();
}

/// suspend current task, then run next task
pub fn suspend_current_and_run_next() {
    mark_current_suspended();
    run_next_task();
}

/// exit current task,  then run next task
pub fn exit_current_and_run_next() {
    mark_current_exited();
    run_next_task();
}

// 返回引用，避免克隆
pub fn current_task() -> Option<&'static TaskControlBlock> {
    let manager = TASK_MANAGER.inner.exclusive_access();
    let current = manager.current_task;
    if current < manager.tasks.len() {
        // 返回引用，需要确保生命周期是 'static
        Some(unsafe { &*(&manager.tasks[current] as *const TaskControlBlock) })
    } else {
        None
    }
}

// 获取当前任务的基本信息
pub fn get_current_task_info() -> (usize, usize, usize, usize) {
    let manager = TASK_MANAGER.inner.exclusive_access();
    let current = manager.current_task;
    if current < manager.tasks.len() {
        let task = &manager.tasks[current];
        (
            task.pid,                                    // PID
            task as *const _ as usize,                   // TCB地址
            &task.task_cx as *const TaskContext as usize, // 上下文地址
            task.task_cx.sp,                             // 内核栈顶
        )
    } else {
        (0, 0, 0, 0)  // 没有当前任务时返回0
    }
}
