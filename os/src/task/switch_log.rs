// os/src/task/switch_log.rs

extern crate alloc;    //新增代码行
use crate::sync::UPSafeCell;
use crate::task::TaskControlBlock;
use crate::task::TaskContext;
use alloc::vec::Vec;

use lazy_static::lazy_static;
use crate::task::TaskStatus;  //新增代码行
use crate::trap::trap_log::{direct_print, direct_println};   //新增代码行
use alloc::format;  //新增代码行
use crate::common::event_counter::{GLOBAL_EVENT_COUNTER, EventType, UnifiedEventRecord};  //新增代码行
/// 进程切换记录信息
#[derive(Debug, Clone)]
pub struct SwitchRecord {
    /// 全局事件序号
    pub global_event_id: usize,
    /// 本地切换序号
    pub local_switch_id: usize,
    
    /// === 被中断进程（旧进程）信息 ===
    /// 进程标识符
    pub old_pid: usize,
    /// 进程控制块地址
    pub old_tcb_addr: usize,
    /// 进程控制块内容（关键字段）
    pub old_tcb_content: TcbContent,
    /// 上下文结构地址
    pub old_context_addr: usize,
    /// 上下文内容（寄存器值）
    pub old_registers: RegistersSnapshot,
    
    /// === 将要运行的进程（新进程）信息 ===
    /// 进程标识符
    pub new_pid: usize,
    /// 进程控制块地址
    pub new_tcb_addr: usize,
    /// 进程控制块内容（关键字段）
    pub new_tcb_content: TcbContent,
    /// 上下文结构地址
    pub new_context_addr: usize,
    /// 上下文内容（寄存器值）
    pub new_registers: RegistersSnapshot,
}

/// 进程控制块关键内容
#[derive(Debug, Clone)]
pub struct TcbContent {
    /// 任务状态
    pub task_status: TaskStatus,
    /// 进程ID
    pub pid: usize,
    /// 内核栈地址（如果有）
    pub kernel_stack_top: usize,
    /// 用户栈地址（如果有）
    pub user_stack_top: usize,
    /// 程序入口点
    pub entry_point: usize,
}

impl TcbContent {
    /// 从TaskControlBlock提取内容
    pub fn from_tcb(tcb: &TaskControlBlock) -> Self {
        TcbContent {
            task_status: tcb.task_status,
            pid: tcb.pid,
            kernel_stack_top: tcb.task_cx.sp,  // task_cx.sp指向内核栈顶
            user_stack_top: 0,  // 可根据实际情况获取
            entry_point: tcb.task_cx.ra,  // ra通常指向返回地址/入口点
        }
    }
    
    /// 格式化输出TCB内容
    pub fn display(&self, prefix: &str) {
        let mut fmtstr = format!("{}  |-- Task Control Block Content:", prefix);
        direct_println(&fmtstr);
        fmtstr = format!("{}  |-- PID: {}", prefix, self.pid);
        direct_println(&fmtstr);
        fmtstr = format!("{}  |-- Task Status: {:?}", prefix, self.task_status);
        direct_println(&fmtstr);
        fmtstr = format!("{}  |-- Kernel Stack Top: {:#x}", prefix, self.kernel_stack_top);
        direct_println(&fmtstr);
      //  fmtstr = format!("{}  |-- User Stack Top: {:#x}", prefix, self.user_stack_top);
      // direct_println(&fmtstr);
        fmtstr = format!("{}  |-- Entry Point: {:#x}", prefix, self.entry_point);
        direct_println(&fmtstr);
    }
}

/// 寄存器快照
#[derive(Debug, Default, Clone)]
pub struct RegistersSnapshot {
    /// ra (返回地址寄存器)
    pub ra: usize,
    /// sp (栈指针)
    pub sp: usize,
    /// s0-s11 (被调用者保存寄存器)
    pub s: [usize; 12],
}

impl RegistersSnapshot {
    /// 从TaskContext中捕获寄存器值
    pub fn from_task_context(task_cx: &TaskContext) -> Self {
        RegistersSnapshot {
            ra: task_cx.ra,
            sp: task_cx.sp,
            s: task_cx.s,
        }
    }
    
    /// 格式化输出寄存器内容
    pub fn display(&self, prefix: &str) {
        let mut fmtstr = format!("{}  |-- Task Context Registers:", prefix);
        direct_println(&fmtstr);
        fmtstr = format!("{}  |-- ra (Return Address): {:#x}", prefix, self.ra);
        direct_println(&fmtstr);
        fmtstr = format!("{}  |-- sp (Stack Pointer): {:#x}", prefix, self.sp);
        direct_println(&fmtstr);
        fmtstr = format!("{}  |-- s0 (Frame Pointer): {:#x}", prefix, self.s[0]);
        direct_println(&fmtstr);
        fmtstr = format!("{}  |-- s1: {:#x}", prefix, self.s[1]);
        direct_println(&fmtstr);
        fmtstr = format!("{}  |-- s2: {:#x}", prefix, self.s[2]);
        direct_println(&fmtstr);
        fmtstr = format!("{}  |-- s3: {:#x}", prefix, self.s[3]);
        direct_println(&fmtstr);
        fmtstr = format!("{}  |-- s4: {:#x}", prefix, self.s[4]);
        direct_println(&fmtstr);
        fmtstr = format!("{}  |-- s5: {:#x}", prefix, self.s[5]);
        direct_println(&fmtstr);
        fmtstr = format!("{}  |-- s6: {:#x}", prefix, self.s[6]);
        direct_println(&fmtstr);
        fmtstr = format!("{}  |-- s7: {:#x}", prefix, self.s[7]);
        direct_println(&fmtstr);
        fmtstr = format!("{}  |-- s8: {:#x}", prefix, self.s[8]);
        direct_println(&fmtstr);
        fmtstr = format!("{}  |-- s9: {:#x}", prefix, self.s[9]);
        direct_println(&fmtstr);
        fmtstr = format!("{}  |-- s10: {:#x}", prefix, self.s[10]);
        direct_println(&fmtstr);
        fmtstr = format!("{}  |-- s11: {:#x}", prefix, self.s[11]);
        direct_println(&fmtstr);
    }
    
    /// 获取寄存器值的简要显示
    pub fn display_compact(&self, prefix: &str) {
        print!("{}  Registers: ra={:#x}, sp={:#x}, s0={:#x}, s1={:#x}, ...",
               prefix, self.ra, self.sp, self.s[0], self.s[1]);
    }
}

impl SwitchRecord {
    /// 创建新的切换记录（用于普通切换）
    pub fn new(
        old_pid: usize,
        old_tcb_addr: usize,
        old_tcb_content: TcbContent,
        old_context_addr: usize,
        old_regs: RegistersSnapshot,
        new_pid: usize,
        new_tcb_addr: usize,
        new_tcb_content: TcbContent,
        new_context_addr: usize,
        new_regs: RegistersSnapshot,
    ) -> Self {
        static mut SWITCH_COUNTER: usize = 0;
        let timestamp = unsafe {
            SWITCH_COUNTER += 1;
            SWITCH_COUNTER
        };
        // 获取全局事件序号
        let global_event_id = GLOBAL_EVENT_COUNTER.next_event_id();        
        SwitchRecord {
            global_event_id,
            local_switch_id: timestamp,
            old_pid,
            old_tcb_addr,
            old_tcb_content,
            old_context_addr,
            old_registers: old_regs,
            new_pid,
            new_tcb_addr,
            new_tcb_content,
            new_context_addr,
            new_registers: new_regs,
        }
    }
    
    pub fn to_unified_event(&self) -> UnifiedEventRecord {
        UnifiedEventRecord {
            event_id: self.global_event_id,
            event_type: EventType::ProcessSwitch,
            timestamp: 0, // 可添加时间戳
            pid: self.old_pid,
            subtype_desc: match (self.old_pid, self.new_pid) {
                (0, new) => "Boot -> Task",
                (old, new) => "Task -> Task",
            },
        }
    }
        
    /// 创建第一次任务切换的记录（从空到第一个任务）
    pub fn new_first_task(
        new_pid: usize,
        new_tcb_addr: usize,
        new_tcb_content: TcbContent,
        new_context_addr: usize,
        new_regs: RegistersSnapshot,
    ) -> Self {
        static mut SWITCH_COUNTER: usize = 0;
        let timestamp = unsafe {
            SWITCH_COUNTER += 1;
            SWITCH_COUNTER
        };
        // 获取全局事件序号
        let global_event_id = GLOBAL_EVENT_COUNTER.next_event_id();         
        // 创建虚拟的空TCB内容
        let dummy_tcb_content = TcbContent {
            task_status: TaskStatus::UnInit,
            pid: 999,   //0,
            kernel_stack_top: 0,
            user_stack_top: 0,
            entry_point: 0,
        };
        
        SwitchRecord {
            global_event_id,
            local_switch_id: timestamp,
            old_pid: 999,  //0,
            old_tcb_addr: 0,
            old_tcb_content: dummy_tcb_content,
            old_context_addr: 0,
            old_registers: RegistersSnapshot::default(),
            new_pid,
            new_tcb_addr,
            new_tcb_content,
            new_context_addr,
            new_registers: new_regs,
        }
    }
    
    /// 完整格式化输出切换信息
    pub fn display(&self) {
        // 标题
        let mut fmtstr = format!("\n|-------------------------------------------------------------------------------|");
        direct_println(&fmtstr);
        if self.old_pid == 999 && self.old_context_addr == 0 {
            fmtstr = format!("|                      PROCESS CONTEXT SWITCH (Global Event #{}),Local_id #{}                     |", self.global_event_id, self.local_switch_id);
            direct_println(&fmtstr);
        } else {
            fmtstr = format!("|                      PROCESS CONTEXT SWITCH (Global Event #{}),Local_id #{}                     |", self.global_event_id, self.local_switch_id);
            direct_println(&fmtstr);
        }
        fmtstr = format!("|-----------------------------------------------------------------------------|");
        direct_println(&fmtstr);
        // 被中断进程信息
        if self.old_pid == 999 && self.old_context_addr == 0 {
            fmtstr = format!("[OLD PROCESS] - No Previous Process (Boot State)");
            direct_println(&fmtstr);
        } else {
            fmtstr = format!("[OLD PROCESS] - Process Being Interrupted/Switched Out");
            direct_println(&fmtstr);
            fmtstr = format!("Process ID (PID): {}", self.old_pid);
            direct_println(&fmtstr);
            fmtstr = format!("TCB Address: {:#x}", self.old_tcb_addr);
            direct_println(&fmtstr);
         //   fmtstr = format!("Task Context Address: {:#x}", self.old_context_addr);
         //   direct_println(&fmtstr);
            
            // TCB内容
            self.old_tcb_content.display("|");
            
            // 寄存器内容
            self.old_registers.display("|");
        }
        
        // 将要运行的进程信息
        fmtstr = format!("[NEW PROCESS] - Process Being Switched In                                                   |");
        direct_println(&fmtstr);
        fmtstr = format!("|------------------------------------------------------------------------------------|");
        direct_println(&fmtstr);
        fmtstr = format!("Process ID (PID): {}", self.new_pid);
        direct_println(&fmtstr);
        fmtstr = format!("TCB Address: {:#x}", self.new_tcb_addr);
        direct_println(&fmtstr);
      //  fmtstr = format!("Task Context Address: {:#x}", self.new_context_addr);
      //  direct_println(&fmtstr);
        fmtstr = format!("|------------------------------------------------------------------------------------|");
        direct_println(&fmtstr);        
        // TCB内容
        self.new_tcb_content.display("|");
        
        // 寄存器内容（将要恢复的）
        self.new_registers.display("|");
        
        fmtstr = format!("|------------------------------------------------------------------------------------|\n");
        direct_println(&fmtstr);        
    }

///只输出指定被切换或切换到的进程pid上下文
    pub fn display_by_pid(&self, pid: usize) {
        // 标题
        //println!("SwitchRecord.display_by_pid:{}",pid);
        let mut fmtstr = format!("\n=== Process {} Switch History by pid===", pid);
        direct_println(&fmtstr);
        fmtstr = format!("\n|-------------------------------------------------------------------------------|");
        direct_println(&fmtstr);        
      // for record in &self.records {
      //      if record.old_pid == pid {
        if self.old_pid == pid {
              	fmtstr = format!("  Switch Global #{},Local #{}: Switched out --> Task{}", self.global_event_id, self.local_switch_id, self.old_pid);
                direct_println(&fmtstr);        
            	fmtstr = format!("[OLD PROCESS] - Process Being Interrupted/Switched Out");
            	direct_println(&fmtstr);
            	fmtstr = format!("Process ID (PID): {}", self.old_pid);
            	direct_println(&fmtstr);
            	fmtstr = format!("TCB Address: {:#x}", self.old_tcb_addr);
            	direct_println(&fmtstr);
            	// TCB内容
            	self.old_tcb_content.display("|");
            	// 寄存器内容
            	self.old_registers.display("|");
        	fmtstr = format!("|------------------------------------------------------------------------------------|\n");
        	direct_println(&fmtstr);
            }
           // if record.new_pid == pid {
            if self.new_pid == pid {
                fmtstr = format!("  Switch Global #{},Local #{}: Switched in --> Task{}", self.global_event_id, self.local_switch_id, self.new_pid);
                direct_println(&fmtstr);
                // 将要运行的进程信息
        	fmtstr = format!("[NEW PROCESS] - Process Being Switched In                                                   |");
        	direct_println(&fmtstr);
        	fmtstr = format!("|------------------------------------------------------------------------------------|");
        	direct_println(&fmtstr);
        	fmtstr = format!("Process ID (PID): {}", self.new_pid);
        	direct_println(&fmtstr);
        	fmtstr = format!("TCB Address: {:#x}", self.new_tcb_addr);
        	direct_println(&fmtstr);
        	fmtstr = format!("|------------------------------------------------------------------------------------|");
        	direct_println(&fmtstr);        
        	// TCB内容
        	self.new_tcb_content.display("|");
        	// 寄存器内容（将要恢复的）
        	self.new_registers.display("|");
        	fmtstr = format!("|------------------------------------------------------------------------------------|\n");
        	direct_println(&fmtstr);
            }
        }   
}

/// 全局切换记录存储
pub struct SwitchHistory {
    records: Vec<SwitchRecord>,
}

impl SwitchHistory {
    pub const fn new() -> Self {
        SwitchHistory {
            records: Vec::new(),
        }
    }
    
    pub fn add_record(&mut self, record: SwitchRecord) {
        self.records.push(record);
    }
    
    pub fn get_records(&self) -> &Vec<SwitchRecord> {
        &self.records
    }
    ///批量输出每次进程切换记录信息
    pub fn print_each_switch_detail(&self) {
    	println!("self.records.len():{}",self.records.len());
    	return;
        if self.records.is_empty() {
            println!("No switch records found.");

            return;
        }
        
        let mut fmtstr = format!("\n|---------------------------------------------------------------------------|");
        direct_println(&fmtstr);
        fmtstr = format!("|                         PROCESS SWITCH DETAILS ({} records)                           |", self.records.len());
        direct_println(&fmtstr);
        fmtstr = format!("|---------------------------------------------------------------------------|");
        direct_println(&fmtstr);
        
        for (index, record) in self.records.iter().enumerate() {
            fmtstr = format!("\n|---------------------------------------------------------------------------|");
            direct_println(&fmtstr);
            fmtstr = format!("|  Record #{} of {}                                                     |", index + 1, self.records.len());
            direct_println(&fmtstr);
            fmtstr = format!("|---------------------------------------------------------------------------|");
            direct_println(&fmtstr);
            
            // 调用 SwitchRecord 的 display 方法
            record.display();
            
            if index < self.records.len() - 1 {
                fmtstr = format!("|---------------------------------------------------------------------------|");
                direct_println(&fmtstr);
            }
        }
        fmtstr = format!("|---------------------------------------------------------------------------|");
        direct_println(&fmtstr);
        fmtstr = format!("|                              END OF SWITCH DETAILS                        |");
        direct_println(&fmtstr);
        fmtstr = format!("|---------------------------------------------------------------------------|\n");
        direct_println(&fmtstr);
    }
    /// 输出所有切换记录的汇总
    pub fn print_summary(&self) {
        let mut fmtstr = format!("\n|---------------------------------------------------------------------------|");
        direct_println(&fmtstr);
        fmtstr = format!("|                       PROCESS SWITCH HISTORY SUMMARY                      |");
        direct_println(&fmtstr);
        fmtstr = format!("| Total Switches: {}                                                        |", self.records.len());
        direct_println(&fmtstr);
        for record in &self.records {
            if record.old_pid == 999 && record.old_context_addr == 0 {
                fmtstr = format!("| Global #{:<4} │ Local #{:<3} | Boot --> Task{:<3} │ Old Ctx:{:#x} │ New Ctx:{:#x} │",
                         record.global_event_id, record.local_switch_id, record.new_pid, record.old_context_addr, record.new_context_addr);
                direct_println(&fmtstr);
            } else {
                fmtstr = format!("| Global #{:<4} │ Local #{:<3} | Task{} --> Task{:<3} │ Old Ctx:{:#x} │ New Ctx:{:#x} │",
                         record.global_event_id, record.local_switch_id, record.old_pid, record.new_pid, 
                         record.old_context_addr, record.new_context_addr);
                direct_println(&fmtstr);
            }
        }
        fmtstr = format!("|---------------------------------------------------------------------------|\n");
        direct_println(&fmtstr);
    }
    
    /// 输出特定进程的所有切换记录
    pub fn print_process_history(&self, pid: usize) {
        println!("\n=== Process {} Switch History ===", pid);
        for record in &self.records {
            if record.old_pid == pid {
                println!("  Switch Global #{},Local #{}: Switched out --> Task{}", record.global_event_id, record.local_switch_id, record.old_pid);
            }
            if record.new_pid == pid {
                if record.old_pid == 0 {
                    println!("  Switch Global #{},Local #{}: Switched out --> Task{}", record.global_event_id, record.local_switch_id, record.new_pid);
                } else {
                    println!("  Switch Global #{},Local #{}: Switched out --> Task{}", record.global_event_id, record.local_switch_id, record.new_pid);
                }
            }
        }
    }
    
    pub fn print_process_by_id(&self, pid: usize) {
    	//println!("SwitchHistory.print_process_by_id:{}",pid);
    	let mut records = self.records.clone();
    	println!("self.records.len():{}",self.records.len());
    	println!("records.len():{}",records.len());
    	//for record in &self.records {
    	for record in records {
    	//for (index, record) in self.records.iter().enumerate() {
    		println!("record.old_pid:{}",record.old_pid);
    		println!("record.new_pid:{}",record.new_pid);
    		if record.old_pid == pid {
    			record.display_by_pid(pid);
    			println!("record.old_pid:{}---",record.old_pid);
    		}
    		if record.new_pid == pid {
    			record.display_by_pid(pid);
    			println!("record.new_pid:{}---",record.new_pid);
    		}    		
    	}
    }
}

lazy_static! {
    pub static ref SWITCH_HISTORY: UPSafeCell<SwitchHistory> = unsafe{
        UPSafeCell::new(SwitchHistory::new())};
}

pub fn print_all_switch_details() {
    // 如果使用 UPSafeCell
    SWITCH_HISTORY.exclusive_access().print_each_switch_detail();
    //println!("print_all_switch_details:SWITCH_HISTORY.records.len():{}",SWITCH_HISTORY.exclusive_access().records.len());
}

// 修改 add_switch_record 函数
pub fn add_switch_record(record: SwitchRecord) {
    // 同时添加到本地历史和统一历史
    SWITCH_HISTORY.exclusive_access().add_record(record.clone());
    crate::common::event_log::add_switch_record_to_unified(record);
}

pub fn print_process_by_id(pid: usize) {
    //println!("SWITCH_HISTORY.print_process_by_id:{}",pid);
    //println!("SWITCH_HISTORY.records.len():{}",SWITCH_HISTORY.exclusive_access().records.len());
    SWITCH_HISTORY.exclusive_access().print_process_by_id(pid);
}
