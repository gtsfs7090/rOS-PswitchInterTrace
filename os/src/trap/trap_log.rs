// os/src/trap/trap_log.rs

use crate::sbi::console_putchar;
use crate::alloc::string::ToString;
use alloc::format;
use lazy_static::lazy_static;
use crate::sync::UPSafeCell;
use core::mem::MaybeUninit;
use crate::common::event_counter::{GLOBAL_EVENT_COUNTER, EventType, UnifiedEventRecord};  //新增代码行
use riscv::register::{
    scause::{self},
    stval,
};  //新增代码行

/// 最大中断记录数
const MAX_TRAP_RECORDS: usize = 8192;

/// 中断处理分支类型
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TrapBranch {
    TimerInterrupt,           // 时钟中断
    UserEnvCall,              // 用户态环境调用（系统调用）
    SupervisorEnvCall,        // 内核态环境调用
    InstructionPageFault,     // 指令缺页
    LoadPageFault,           // 加载缺页
    StorePageFault,          // 存储缺页
    IllegalInstruction,      // 非法指令
    Breakpoint,              // 断点
    OtherException,          // 其他异常
    OtherInterrupt,          // 其他中断
    Unknown,                 // 未知
}

/// 系统调用信息
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct SyscallInfo {
    pub syscall_id: usize,      // 系统调用号
    pub args: [usize; 3],       // 参数
    pub result: isize,          // 返回值
}

pub fn direct_print(s: &str) {  //新增pub
    for c in s.chars() {
        console_putchar(c as usize);
    }
}

pub fn direct_println(s: &str) {  //新增pub
    direct_print(s);
    direct_print("\n");
}

impl SyscallInfo {
    pub fn new(syscall_id: usize, args: [usize; 3], result: isize) -> Self {
        SyscallInfo {
            syscall_id,
            args,
            result,
        }
    }

pub fn display(&self) {
        // 使用直接输出，不要用 println!
        direct_print("    Syscall ID: ");
        direct_print(&self.syscall_id.to_string());
        direct_print(match self.syscall_id {
            64 => " (write)\n",
            57 => " (close)\n",
            80 => " (exit)\n",
            _ => "\n",
        });
        
        direct_print("    Args: [");
        direct_print(&format_args!("{:#x}", self.args[0]).to_string());
        direct_print(", ");
        direct_print(&format_args!("{:#x}", self.args[1]).to_string());
        direct_print(", ");
        direct_print(&format_args!("{:#x}", self.args[2]).to_string());
        direct_print("]\n");
        
        direct_print("    Result: ");
        direct_print(&self.result.to_string());
        direct_print("\n");
    }
}

/// 中断类型
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(usize)]
pub enum TrapType {
    InstructionMisaligned = 0,
    InstructionFault = 1,
    IllegalInstruction = 2,
    Breakpoint = 3,
    LoadMisaligned = 4,
    LoadFault = 5,
    StoreMisaligned = 6,
    StoreFault = 7,
    UserEnvCall = 8,
    SupervisorEnvCall = 9,
    InstructionPageFault = 12,
    LoadPageFault = 13,
    StorePageFault = 15,
    Interrupt = 0x8000000000000000,
    TimerInterrupt = 0x8000000000000005,
    ExternalInterrupt = 0x8000000000000009,
    Unknown = 0xffffffffffffffff,
}

impl From<usize> for TrapType {
    fn from(scause: usize) -> Self {
        use TrapType::*;
        match scause {
            0 => InstructionMisaligned,
            1 => InstructionFault,
            2 => IllegalInstruction,
            3 => Breakpoint,
            4 => LoadMisaligned,
            5 => LoadFault,
            6 => StoreMisaligned,
            7 => StoreFault,
            8 => UserEnvCall,
            9 => SupervisorEnvCall,
            12 => InstructionPageFault,
            13 => LoadPageFault,
            15 => StorePageFault,
            _ if scause & 0x8000000000000000 != 0 => {
                match scause & 0x7fffffffffffffff {
                    5 => TimerInterrupt,
                    9 => ExternalInterrupt,
                    _ => Interrupt,
                }
            }
            _ => Unknown,
        }
    }
}

impl TrapType {
    pub fn description(&self) -> &'static str {
        use TrapType::*;
        match self {
            InstructionMisaligned => "Instruction Misaligned",
            InstructionFault => "Instruction Fault",
            IllegalInstruction => "Illegal Instruction",
            Breakpoint => "Breakpoint",
            LoadMisaligned => "Load Misaligned",
            LoadFault => "Load Fault",
            StoreMisaligned => "Store Misaligned",
            StoreFault => "Store Fault",
            UserEnvCall => "User Environment Call",
            SupervisorEnvCall => "Supervisor Environment Call",
            InstructionPageFault => "Instruction Page Fault",
            LoadPageFault => "Load Page Fault",
            StorePageFault => "Store Page Fault",
            Interrupt => "Interrupt",
            TimerInterrupt => "Timer Interrupt",
            ExternalInterrupt => "External Interrupt",
            Unknown => "Unknown Trap",
        }
    }
}

/// 寄存器快照
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct TrapRegisters {
    // 通用寄存器 x0-x31
    pub x: [usize; 32],
    // 特权寄存器
    pub sepc: usize,      // 异常返回地址
    pub sstatus: usize,  //Sstatus, //usize,   // 状态寄存器
    pub stval: usize,     // 异常值寄存器
    pub scause: usize,    // 异常原因寄存器
}

impl TrapRegisters {
    /// 从 TrapContext 中捕获寄存器值
    pub fn from_trap_context(trap_cx: &crate::trap::TrapContext) -> Self {
        let mut regs = TrapRegisters::default();
        // 复制通用寄存器
        regs.x[0] = 0; // x0 总是0
        for i in 1..32 {
            regs.x[i] = trap_cx.x[i];
        }
        regs.sepc = trap_cx.sepc;
        regs.sstatus = trap_cx.sstatus.bits();
        regs.scause = scause::read().bits();
        regs.stval = stval::read();
        regs
    }
    
    pub fn display(&self, prefix: &str) {
        let mut fmtstr = format!("{}|-- Trap Context Registers:", prefix);
        direct_println(&fmtstr);
        fmtstr = format!("{}|-- sepc (Return Address): {:#x}", prefix, self.sepc);
        direct_println(&fmtstr);
        fmtstr = format!("{}|-- sstatus: {:#x}", prefix, self.sstatus);
        direct_println(&fmtstr);
        fmtstr = format!("{}|-- scause: {:#x} ({})", prefix, self.scause, 
                 TrapType::from(self.scause).description());
        direct_println(&fmtstr);
        fmtstr = format!("{}|-- stval: {:#x}", prefix, self.stval);
        direct_println(&fmtstr);
        fmtstr = format!("{}|-- ra (x1): {:#x}", prefix, self.x[1]);
        direct_println(&fmtstr);
        fmtstr = format!("{}|-- sp (x2): {:#x}", prefix, self.x[2]);
        direct_println(&fmtstr);
        fmtstr = format!("{}|-- tp (x4): {:#x}", prefix, self.x[4]);
        direct_println(&fmtstr);
        fmtstr = format!("{}|-- a0-a7: {:#x}, {:#x}, {:#x}, {:#x}, {:#x}, {:#x}, {:#x}, {:#x}",
                 prefix, self.x[10], self.x[11], self.x[12], self.x[13],
                 self.x[14], self.x[15], self.x[16], self.x[17]);
        direct_println(&fmtstr);
        fmtstr = format!("{}|-- ...", prefix);
        direct_println(&fmtstr);
    }
}

/// 增强的中断记录
/// 中断/异常记录
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct TrapRecord {
    /// 全局事件序号
    pub global_event_id: usize,  //新增代码行
    /// 本地trap序号
    pub local_trap_id: usize,  //新增代码行
    pub trap_type: TrapType,
    pub trap_branch: TrapBranch,  // 新增：记录执行的分支

    // 被中断进程信息
    pub interrupted_pid: usize,
    pub interrupted_tcb_addr: usize,
    pub interrupted_context_addr: usize,
    pub interrupted_registers: TrapRegisters,
    
    // 栈信息
    pub kernel_stack_top: usize,
    pub user_stack_top: usize,

    pub scause: usize,
    pub stval: usize,
     // 系统调用信息（仅当是系统调用时有效）
    pub syscall_info: Option<SyscallInfo>,
}

impl TrapRecord {
    pub fn new(
        trap_cx: &crate::trap::TrapContext,
        pid: usize,
        tcb_addr: usize,
        context_addr: usize,
        kernel_sp: usize,
        scause: usize,
        stval: usize,
        trap_branch: TrapBranch,
        syscall_info: Option<SyscallInfo>,
    ) -> Self {
        static mut TRAP_COUNTER: usize = 0;
        let timestamp = unsafe {
            TRAP_COUNTER += 1;
            TRAP_COUNTER
        };
        // 获取全局事件序号
        let global_event_id = GLOBAL_EVENT_COUNTER.next_event_id();  //新增代码行
        TrapRecord {
            global_event_id,  //新增代码行
            local_trap_id: timestamp,
            trap_type: TrapType::from(scause),
            trap_branch,
            interrupted_pid: pid,
            interrupted_tcb_addr: tcb_addr,
            interrupted_context_addr: context_addr,
            interrupted_registers: TrapRegisters::from_trap_context(trap_cx),
            kernel_stack_top: kernel_sp,
            user_stack_top: 0,
            scause: scause,
            stval: stval,
            syscall_info,
        }
    }
    
        pub fn to_unified_event(&self) -> UnifiedEventRecord {
        UnifiedEventRecord {
            event_id: self.global_event_id,
            event_type: EventType::Trap,
            timestamp: 0,
            pid: self.interrupted_pid,
            subtype_desc: match self.trap_branch {
                TrapBranch::TimerInterrupt => "Timer Interrupt",
                TrapBranch::UserEnvCall => "System Call",
                TrapBranch::InstructionPageFault => "Instruction Page Fault",
                TrapBranch::LoadPageFault => "Load Page Fault",
                TrapBranch::StorePageFault => "Store Page Fault",
                _ => "Other Trap",
            },
        }
    }

    pub fn display(&self) {
        let mut fmtstr = format!("\n|------------------------------------------------------------------------------------------|");
        direct_println(&fmtstr);
        direct_print("                           TRAP/EXCEPTION OCCURRED                                          \n");
        fmtstr = format!("                           TRAP/EXCEPTION (Global Event #{}),Local Trap #{}                      ", self.global_event_id, self.local_trap_id);
        direct_println(&fmtstr);
        direct_print("|------------------------------------------------------------------------------------------|\n");
        direct_print(" [Trap Information]\n");
        fmtstr = format!("   Type: {} ({:?})", self.trap_type.description(), self.trap_type);
        direct_println(&fmtstr);
        fmtstr = format!("   Branch: {:?}", self.trap_branch);
        direct_println(&fmtstr);
        // 显示系统调用详情
        if let Some(syscall) = &self.syscall_info {
            direct_print(" [System Call Information]\n");
            syscall.display();
        }
        fmtstr = format!(" [Interrupted Process]");
        direct_println(&fmtstr);
        fmtstr = format!("   PID: {}", self.interrupted_pid);
        direct_println(&fmtstr);
     //   fmtstr = format!("   TCB Address: {:#x}", self.interrupted_tcb_addr);
     //   direct_println(&fmtstr);
        fmtstr = format!("   Task Context Address: {:#x}", self.interrupted_context_addr);
        direct_println(&fmtstr);
        fmtstr = format!("   Kernel Stack Top: {:#x}", self.kernel_stack_top);
        direct_println(&fmtstr);
        self.interrupted_registers.display("|   ");
        direct_print("|-------------------------------------------------------------------------------|\n");
    }
    //按进程号显示中断信息
    pub fn display_by_pid(&self, pid: usize) {
    	if self.interrupted_pid == pid {
        	let mut fmtstr = format!("\n=== Process {} TRAP/EXCEPTION History by pid===", pid);
        	direct_println(&fmtstr);
        	direct_print("                           TRAP/EXCEPTION OCCURRED                                          \n");
        	fmtstr = format!("                           TRAP/EXCEPTION (Global Event #{}),Local Trap #{}                      ", self.global_event_id, self.local_trap_id);
        	direct_println(&fmtstr);
        	direct_print("|------------------------------------------------------------------------------------------|\n");
        	direct_print(" [Trap Information]\n");
        	fmtstr = format!("   Type: {} ({:?})", self.trap_type.description(), self.trap_type);
        	direct_println(&fmtstr);
        	fmtstr = format!("   Branch: {:?}", self.trap_branch);
        	direct_println(&fmtstr);
        	// 显示系统调用详情
        	if let Some(syscall) = &self.syscall_info {
            		direct_print(" [System Call Information]\n");
            		syscall.display();
        	}
        	fmtstr = format!(" [Interrupted Process]");
        	direct_println(&fmtstr);
        	fmtstr = format!("   PID: {}", self.interrupted_pid);
        	direct_println(&fmtstr);
        	fmtstr = format!("   Task Context Address: {:#x}", self.interrupted_context_addr);
        	direct_println(&fmtstr);
        	fmtstr = format!("   Kernel Stack Top: {:#x}", self.kernel_stack_top);
        	direct_println(&fmtstr);
        	self.interrupted_registers.display("|   ");
        	direct_print("|-------------------------------------------------------------------------------|\n");
    	}
    }
}

/// 中断历史记录存储（使用固定大小数组）
pub struct TrapHistory {
    records: [MaybeUninit<TrapRecord>; MAX_TRAP_RECORDS],
    count: usize,
}

impl TrapHistory {
    pub const fn new() -> Self {
        const INIT: MaybeUninit<TrapRecord> = MaybeUninit::uninit();
        TrapHistory {
            records: [INIT; MAX_TRAP_RECORDS],
            count: 0,
        }
    }
    
    pub fn add_record(&mut self, record: TrapRecord) {
        if self.count < MAX_TRAP_RECORDS {
            self.records[self.count].write(record);
            self.count += 1;
            record.display();
        } else {
            // 缓冲区满，覆盖最早的一条
            for i in 1..MAX_TRAP_RECORDS {
                self.records[i-1] = self.records[i];
            }
            self.records[MAX_TRAP_RECORDS-1].write(record);
            record.display();
            println!("[TrapHistory] Warning: Buffer full, oldest record overwritten");
        }
    }
    
    pub fn print_summary(&self) {
        println!("\n|------------------------------------------------------------------------------------------|");
        println!("|                           TRAP/EXCEPTION HISTORY SUMMARY                                   |");
        println!("|------------------------------------------------------------------------------------------|");
        println!(" Total Traps/Exceptions: {}/{}", self.count, MAX_TRAP_RECORDS);
        
        for i in 0..self.count {
            unsafe {
                let record = self.records[i].assume_init_ref();
                println!(" │ Global #{:<4} │ Local #{:<3} │ PID {:<3} │ {:<35} │ sepc={:#x} │",
                        record.global_event_id,
                        record.local_trap_id,
                        record.interrupted_pid,
                        record.trap_type.description(),
                        record.interrupted_registers.sepc);
            }
        }
        
        println!("|_____________________________________________________________________________________|\n");
    }
    
    pub fn print_by_pid(&self, pid: usize) {
        println!("\n=== Traps for Process {} ===", pid);
        let mut found = false;
        for i in 0..self.count {
            unsafe {
                let record = self.records[i].assume_init_ref();
                if record.interrupted_pid == pid {
                    found = true;
                    println!("  Global #{:<4} │ Local #{:<3}: {} at sepc={:#x}", 
                             record.global_event_id, 
                             record.local_trap_id,
                             record.trap_type.description(),
                             record.interrupted_registers.sepc);
                }
            }
        }
        if !found {
            println!("  No traps recorded for this process");
        }
    }
    // 统计各分支执行次数
    pub fn print_branch_statistics(&self) {
        let mut timer_count = 0;
        let mut syscall_count = 0;
        let mut page_fault_count = 0;
        let mut other_count = 0;
        
        for i in 0..self.count {
            unsafe {
                let record = self.records[i].assume_init_ref();
                match record.trap_branch {
                    TrapBranch::TimerInterrupt => timer_count += 1,
                    TrapBranch::UserEnvCall => syscall_count += 1,
                    TrapBranch::InstructionPageFault | 
                    TrapBranch::LoadPageFault | 
                    TrapBranch::StorePageFault => page_fault_count += 1,
                    _ => other_count += 1,
                }
            }
        }
        
        println!("\n|----------------------------------------------------------------|");
        println!("|                    TRAP BRANCH STATISTICS                        |");
        println!("|----------------------------------------------------------------|");
        println!("|  Timer Interrupts:     {:>6}                                   |", timer_count);
        println!("|  System Calls:         {:>6}                                   |", syscall_count);
        println!("|  Page Faults:          {:>6}                                   |", page_fault_count);
        println!("|  Other:                {:>6}                                   |", other_count);
        println!("|  Total:                {:>6}                                   |", self.count);
        println!("|----------------------------------------------------------------|\n");
    }
}

// 全局静态变量
lazy_static! {
    pub static ref TRAP_HISTORY: UPSafeCell<TrapHistory> = unsafe { UPSafeCell::new(TrapHistory::new()) };
}
// 辅助函数

pub fn add_trap_record(record: TrapRecord) {
    // 同时添加到本地历史和统一历史
    TRAP_HISTORY.exclusive_access().add_record(record.clone());
    crate::common::event_log::add_trap_record_to_unified(record);
}

pub fn print_trap_summary() {
    TRAP_HISTORY.exclusive_access().print_summary();
}

pub fn print_traces_by_pid(pid: usize) {
    TRAP_HISTORY.exclusive_access().print_by_pid(pid);
}

pub fn print_branch_statistics() {
    TRAP_HISTORY.exclusive_access().print_branch_statistics();
}
