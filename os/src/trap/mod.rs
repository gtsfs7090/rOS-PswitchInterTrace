//! Trap handling functionality
//!
//! For rCore, we have a single trap entry point, namely `__alltraps`. At
//! initialization in [`init()`], we set the `stvec` CSR to point to it.
//!
//! All traps go through `__alltraps`, which is defined in `trap.S`. The
//! assembly language code does just enough work restore the kernel space
//! context, ensuring that Rust code safely runs, and transfers control to
//! [`trap_handler()`].
//!
//! It then calls different functionality based on what exactly the exception
//! was. For example, timer interrupts trigger task preemption, and syscalls go
//! to [`syscall()`].

mod context;
pub mod trap_log;
use crate::syscall::syscall;
use crate::task::{exit_current_and_run_next, suspend_current_and_run_next};
use crate::timer::set_next_trigger;
pub use trap_log::*;
pub use crate::task::get_current_task_info;  // 导入函数
use core::arch::global_asm;
use riscv::register::{
    mtvec::TrapMode,
    scause::{self, Exception, Interrupt, Trap},
    sie, stval, stvec,
};

global_asm!(include_str!("trap.S"));

/// initialize CSR `stvec` as the entry of `__alltraps`
pub fn init() {
    extern "C" {
        fn __alltraps();
    }
    unsafe {
        stvec::write(__alltraps as usize, TrapMode::Direct);
    }
}

/// timer interrupt enabled
pub fn enable_timer_interrupt() {
    unsafe {
        sie::set_stimer();
    }
}

// 记录系统调用的辅助函数
fn record_syscall_info(trap_cx: &mut TrapContext, syscall_id: usize, args: [usize; 3], result: isize) {
    let (pid, tcb_addr, context_addr, kernel_sp) = crate::task::get_current_task_info();
    let scause_raw = scause::read().bits();
    let stval = stval::read();
    
    let syscall_info = SyscallInfo::new(syscall_id, args, result);
    let record = TrapRecord::new(
        trap_cx, pid, tcb_addr, 
        context_addr, kernel_sp, scause_raw, stval,
        TrapBranch::UserEnvCall,
        Some(syscall_info),
    );
    add_trap_record(record);
}

// 在现有的 trap_handler 函数开头添加记录
#[no_mangle]
/// handle an interrupt, exception, or system call from user space
pub fn trap_handler(cx: &mut TrapContext) -> &mut TrapContext {
    let scause = scause::read();
    let stval = stval::read();
    let scause_raw = scause.bits();
    // 获取当前任务信息
    let (pid, tcb_addr, context_addr, kernel_sp) = crate::task::get_current_task_info();
    match scause.cause() {
        Trap::Exception(Exception::UserEnvCall) => {// 系统调用
            // 获取系统调用参数
            let syscall_id = cx.x[17];
            let args = [cx.x[10], cx.x[11], cx.x[12]];

            cx.sepc += 4;
            cx.x[10] = syscall(cx.x[17], [cx.x[10], cx.x[11], cx.x[12]]) as usize;
            let result = cx.x[10] as isize;
            // 记录系统调用信息
            record_syscall_info(cx, syscall_id, args, result);
        }
        Trap::Exception(Exception::StoreFault) | Trap::Exception(Exception::StorePageFault) => {  // 指令缺页
            let record = TrapRecord::new(
                cx, pid, tcb_addr, 
                context_addr, kernel_sp, scause_raw, stval,
                TrapBranch::InstructionPageFault,
                None,
            );
            add_trap_record(record);
            println!("[kernel] PageFault in application, bad addr = {:#x}, bad instruction = {:#x}, kernel killed it.", stval, cx.sepc);
            exit_current_and_run_next();
        }
        Trap::Exception(Exception::IllegalInstruction) => {  // 非法指令
            let record = TrapRecord::new(
                cx, pid, tcb_addr, 
                context_addr, kernel_sp, scause_raw, stval,
                TrapBranch::IllegalInstruction,
                None,
            );
            add_trap_record(record);
            println!("[kernel] IllegalInstruction in application, kernel killed it.");
            exit_current_and_run_next();
        }
        
        Trap::Interrupt(Interrupt::SupervisorTimer) => {// 时钟中断
            let record = TrapRecord::new(
                cx, pid, tcb_addr, 
                context_addr, kernel_sp, scause_raw, stval,
                TrapBranch::TimerInterrupt,
                None,
            );
            add_trap_record(record);

            set_next_trigger();
            suspend_current_and_run_next();
        }
        
        // 其他异常
        
        Trap::Exception(e) => {
            let record = TrapRecord::new(
                cx, pid, tcb_addr, 
                context_addr, kernel_sp, scause_raw, stval,
                TrapBranch::OtherException,
                None,
            );
            add_trap_record(record);
            
            println!("[kernel] Unhandled Exception {:?} at {:#x}", e, cx.sepc);
            panic!("Unhandled Exception");
        }
        
        _ => {
            panic!(
                "Unsupported trap {:?}, stval = {:#x}!",
                scause.cause(),
                stval
            );
        }
    }
    cx
}

pub use context::TrapContext;
