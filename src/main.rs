#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(crate::tests::runner::test_runner)]
#![reexport_test_harness_main = "test_main"]

mod dtb;
mod start;
mod drivers {
    pub mod pl011;
}
mod log;
mod mutex;
mod serial;
mod tests {
    pub mod runner;
    pub mod test;
}
mod arch {
    pub mod aarch64;
}
mod hal;
mod allocator {
    pub mod linked_list;
}

use crate::{allocator::linked_list::LinkedListAllocator, hal::HypervisorControl, log::LogLevel, mutex::Mutex};
#[allow(unused_imports)]
use core::panic::PanicInfo;
use core::sync::atomic::AtomicU8;

static LOG_LEVEL: AtomicU8 = AtomicU8::new(LogLevel::Info as u8);
static PL011_DEVICE: Mutex<drivers::pl011::Pl011> = Mutex::new(drivers::pl011::Pl011::invalid());
static ALLOCATOR: Mutex<LinkedListAllocator> = Mutex::new(LinkedListAllocator::new());

/// start.rsの_startから呼ばれる
/// This is called from _start in start.rs
fn main() -> ! {
    log::set_log_level(LogLevel::Debug);
    log_info!("Hello from main!");

    #[cfg(test)]
    test_main();

    #[cfg(target_arch = "aarch64")]
    {
        let currentel = arch::aarch64::get_currentel() >> 2;
        log_info!("CurrentEL: {}", currentel);
        assert_eq!(currentel, 2);
    }

    hal::HypervisorLevel::setup_hypervisor();
    hal::HypervisorLevel::boot_vm(el1_main as *const fn() as usize);
}

extern "C" fn el1_main() {
    core::hint::spin_loop();
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {
        core::hint::spin_loop();
    }
}
