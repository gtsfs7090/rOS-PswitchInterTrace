// os/src/common/event_log.rs

use alloc::vec::Vec;
use crate::common::event_counter::{UnifiedEventRecord, EventType};
use crate::task::switch_log::{SwitchRecord, SWITCH_HISTORY};
use crate::trap::trap_log::{TrapRecord, TRAP_HISTORY};
use crate::sync::UPSafeCell;
use lazy_static::lazy_static;
use alloc::format;  //新增代码行
use crate::alloc::string::ToString;  //新增代码行

/// 统一的事件记录（包含具体数据）
#[derive(Clone)]  //新增代码行
pub enum UnifiedRecord {
    Switch(SwitchRecord),
    Trap(TrapRecord),
}

impl UnifiedRecord {
    pub fn event_id(&self) -> usize {
        match self {
            UnifiedRecord::Switch(r) => r.global_event_id,
            UnifiedRecord::Trap(r) => r.global_event_id,
        }
    }
    
    pub fn event_type(&self) -> EventType {
        match self {
            UnifiedRecord::Switch(_) => EventType::ProcessSwitch,
            UnifiedRecord::Trap(_) => EventType::Trap,
        }
    }
    
    pub fn display(&self) {
        match self {
            UnifiedRecord::Switch(r) => r.display(),
            UnifiedRecord::Trap(r) => r.display(),
        }
    }
    //按进程号显示进程切换信息和中断信息
    pub fn display_by_pid(&self, pid: usize) {
        match self {
            UnifiedRecord::Switch(r) => r.display_by_pid(pid),
            UnifiedRecord::Trap(r) => r.display_by_pid(pid),
        }    
    }
}

/// 统一事件历史管理器
pub struct UnifiedEventHistory {
    events: Vec<UnifiedRecord>,
}

impl UnifiedEventHistory {
    pub const fn new() -> Self {
        UnifiedEventHistory {
            events: Vec::new(),
        }
    }
    
    /// 添加切换记录
    pub fn add_switch_record(&mut self, record: SwitchRecord) {
        self.events.push(UnifiedRecord::Switch(record));
    }
    
    /// 添加trap记录
    pub fn add_trap_record(&mut self, record: TrapRecord) {
        self.events.push(UnifiedRecord::Trap(record));
    }
    
    /// 按事件顺序输出所有记录
    pub fn print_chronological(&self) {
        let mut sorted_events = self.events.clone();
        sorted_events.sort_by_key(|e| e.event_id());
        
        println!("\n|---------------------------------------------------------------------------|");
        println!("|                  UNIFIED EVENT HISTORY (Chronological Order)             |");
        println!("|---------------------------------------------------------------------------|");
        println!("|  Total Events: {}                                                         |", sorted_events.len());
        println!("|---------------------------------------------------------------------------|");
        
        for event in sorted_events {
            event.display();
        }
    }

    //按进程号输出进程切换信息和中断信息
    pub fn display_by_pid(&self, pid: usize) {
    	println!("\n=== Process {} Switch TRAP/EXCEPTION History by pid===Start", pid);
    	let mut sorted_events = self.events.clone();
        for event in sorted_events {
            event.display_by_pid(pid);
        }
        println!("\n=== Process {} Switch TRAP/EXCEPTION History by pid===End", pid);
    }    
    /// 输出事件摘要（按顺序）
    pub fn print_summary(&self) {
        let mut sorted_events = self.events.clone();
        sorted_events.sort_by_key(|e| e.event_id());
        
        println!("\n|---------------------------------------------------------------------------|");
        println!("|                       EVENT SUMMARY (Chronological Order)                 |");
        println!("|---------------------------------------------------------------------------|");
        println!("|  #   │ Type              │ PID  │ Description                             |");
        println!("|---------------------------------------------------------------------------|");
        
        for event in sorted_events {
            let (type_str, pid, desc) = match &event {
                UnifiedRecord::Switch(r) => {
                    let desc = format!("Switch: {} → {}", r.old_pid, r.new_pid);
                    ("Switch", r.old_pid, desc)
                }
                UnifiedRecord::Trap(r) => {
                    let desc = match r.trap_branch {
                        crate::trap::trap_log::TrapBranch::TimerInterrupt => 
                            format!("Timer Interrupt (Task {})", r.interrupted_pid),
                        crate::trap::trap_log::TrapBranch::UserEnvCall => {
                            if let Some(syscall) = r.syscall_info {
                                format!("Syscall: id={}", syscall.syscall_id)
                            } else {
                                "System Call".to_string()  //需添加use crate::alloc::string::ToString;
                            }
                        }
                        _ => format!("{:?}", r.trap_branch),
                    };
                    ("Trap", r.interrupted_pid, desc)
                }
            };
            
            println!("|  {:<3} │ {:<16} │ {:<4} │ {:<50} |",
                     event.event_id(), type_str, pid, &desc[..desc.len().min(50)]);
        }
        
        println!("|---------------------------------------------------------------------------|\n");
    }
    
    /// 按PID过滤输出
    pub fn print_by_pid(&self, target_pid: usize) {
        let mut events: Vec<&UnifiedRecord> = self.events
            .iter()
            .filter(|e| match e {
                UnifiedRecord::Switch(r) => r.old_pid == target_pid || r.new_pid == target_pid,
                UnifiedRecord::Trap(r) => r.interrupted_pid == target_pid,
            })
            .collect();
        events.sort_by_key(|e| e.event_id());
        
        println!("\n|---------------------------------------------------------------------------|");
        println!("|               EVENTS FOR PROCESS {} (Chronological Order)                 |", target_pid);
        println!("|---------------------------------------------------------------------------|");
        
        for event in events {
            event.display();
        }
        
        println!("|---------------------------------------------------------------------------|\n");
    }
}

lazy_static! {
    pub static ref UNIFIED_HISTORY: UPSafeCell<UnifiedEventHistory> = 
        unsafe { UPSafeCell::new(UnifiedEventHistory::new()) };
}

// 辅助函数
pub fn add_switch_record_to_unified(record: SwitchRecord) {
    UNIFIED_HISTORY.exclusive_access().add_switch_record(record);
}

pub fn add_trap_record_to_unified(record: TrapRecord) {
    UNIFIED_HISTORY.exclusive_access().add_trap_record(record);
}

pub fn print_unified_chronological() {
    UNIFIED_HISTORY.exclusive_access().print_chronological();
}

pub fn print_unified_summary() {
    UNIFIED_HISTORY.exclusive_access().print_summary();
}

pub fn print_unified_by_pid(pid: usize) {
    UNIFIED_HISTORY.exclusive_access().print_by_pid(pid);
}

pub fn display_unified_by_pid(pid: usize) {
    UNIFIED_HISTORY.exclusive_access().display_by_pid(pid);
}
