// os/src/common/event_counter.rs

use core::sync::atomic::{AtomicUsize, Ordering};

/// 全局事件计数器，统一记录所有内核事件
pub struct GlobalEventCounter {
    counter: AtomicUsize,
}

impl GlobalEventCounter {
    pub const fn new() -> Self {
        GlobalEventCounter {
            counter: AtomicUsize::new(0),
        }
    }
    
    /// 获取下一个事件序号
    pub fn next_event_id(&self) -> usize {
        self.counter.fetch_add(1, Ordering::SeqCst) + 1
    }
    
    /// 获取当前事件序号（不增加）
    pub fn current_event_id(&self) -> usize {
        self.counter.load(Ordering::SeqCst)
    }
    
    /// 重置计数器（用于测试）
    pub fn reset(&self) {
        self.counter.store(0, Ordering::SeqCst);
    }
}

// 全局静态事件计数器
pub static GLOBAL_EVENT_COUNTER: GlobalEventCounter = GlobalEventCounter::new();

/// 事件类型
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EventType {
    /// 进程切换事件
    ProcessSwitch,
    /// 中断/异常事件
    Trap,
}

/// 统一的事件记录
#[derive(Debug, Clone, Copy)]
pub struct UnifiedEventRecord {
    /// 全局事件序号
    pub event_id: usize,
    /// 事件类型
    pub event_type: EventType,
    /// 时间戳（可选）
    pub timestamp: usize,
    /// 关联的进程PID
    pub pid: usize,
    /// 事件子类型描述
    pub subtype_desc: &'static str,
}
