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
mod hal {
    pub mod aarch64;
    pub mod traits;
}

use crate::{log::LogLevel, mutex::Mutex};
#[allow(unused_imports)]
use core::panic::PanicInfo;
use core::sync::atomic::AtomicU8;

static PL011_DEVICE: Mutex<drivers::pl011::Pl011> = Mutex::new(drivers::pl011::Pl011::invalid());
static LOG_LEVEL: AtomicU8 = AtomicU8::new(LogLevel::Info as u8);

/// start.rsの_startから呼ばれる
/// This is called from _start in start.rs
fn main() -> ! {
    log_info!("main", "Hello from main!");

    #[cfg(test)]
    test_main();

    let currentel = hal::aarch64::get_currentel() >> 2;
    log_info!("main", "CurrentEL: {}", currentel);
    assert_eq!(currentel, 2);
    
    hal::aarch64::setup_hypervisor_registers();
    hal::aarch64::boot_vm(el1_main as *const fn() as usize);
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

