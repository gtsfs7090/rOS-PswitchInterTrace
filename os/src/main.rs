//! The main module and entrypoint
//!
//! Various facilities of the kernels are implemented as submodules. The most
//! important ones are:
//!
//! - [`trap`]: Handles all cases of switching from userspace to the kernel
//! - [`task`]: Task management
//! - [`syscall`]: System call handling and implementation
//!
//! The operating system also starts in this module. Kernel code starts
//! executing from `entry.asm`, after which [`rust_main()`] is called to
//! initialize various pieces of functionality. (See its source code for
//! details.)
//!
//! We then call [`task::run_first_task()`] and for the first time go to
//! userspace.

//#![deny(missing_docs)]
//------#![deny(warnings)]    //暂时注释该行
#![no_std]
#![no_main]
#![feature(panic_info_message)]

use core::arch::global_asm;

#[path = "boards/qemu.rs"]
mod board;

#[macro_use]
mod console;
mod config;
mod lang_items;
mod loader;
mod sbi;
mod sync;
pub mod syscall;
pub mod task;
mod timer;
pub mod trap;
// 声明 common 目录下的 event_counter 模块和event_log模块
mod common {
    pub mod event_counter;
    pub mod event_log;
}

// 导入分配器
extern crate alloc;
use buddy_system_allocator::LockedHeap;

// 声明全局分配器
#[global_allocator]
static HEAP_ALLOCATOR: LockedHeap<32> = LockedHeap::<32>::empty(); //新增代码行

global_asm!(include_str!("entry.asm"));
global_asm!(include_str!("link_app.S"));

fn init_heap() {
    // 定义堆的起始地址和大小
    const HEAP_SIZE: usize = 0xF00000;   //0x400000; // 4MB
    static mut HEAP_SPACE: [u8; HEAP_SIZE] = [0; HEAP_SIZE];
    
    unsafe {
        HEAP_ALLOCATOR
            .lock()
            .init(HEAP_SPACE.as_ptr() as usize, HEAP_SIZE);
    }
}

/// clear BSS segment
fn clear_bss() {
    extern "C" {
        fn sbss();
        fn ebss();
    }
    unsafe {
        core::slice::from_raw_parts_mut(sbss as usize as *mut u8, ebss as usize - sbss as usize)
            .fill(0);
    }
}

/// the rust entry-point of os
#[no_mangle]
pub fn rust_main() -> ! {
    clear_bss();
    // 初始化堆空间
    init_heap();
    println!("[kernel] Hello, world!");
    trap::init();
    loader::load_apps();
    trap::enable_timer_interrupt();
    timer::set_next_trigger();
    task::run_first_task();
    panic!("Unreachable in rust_main!");
}
