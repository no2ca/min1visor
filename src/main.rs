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
mod elf;
mod paging;
mod exeption;
mod mmio {
    pub mod pl011;
}

use crate::{
    allocator::linked_list::LinkedListAllocator, drivers::pl011, hal::HypervisorControl, log::LogLevel, mutex::Mutex
};
#[allow(unused_imports)]
use core::panic::PanicInfo;
use core::{arch::asm, sync::atomic::AtomicU8};

static LOG_LEVEL: AtomicU8 = AtomicU8::new(LogLevel::Debug as u8);
static PL011_DEVICE: Mutex<drivers::pl011::Pl011> = Mutex::new(drivers::pl011::Pl011::invalid());
static ALLOCATOR: Mutex<LinkedListAllocator> = Mutex::new(LinkedListAllocator::new());

/// start.rsの_startから呼ばれる
/// This is called from _start in start.rs
fn main() -> ! {
    log_info!("Hello from main!");

    #[cfg(test)]
    test_main();

    #[cfg(target_arch = "aarch64")]
    {
        let currentel = arch::aarch64::get_currentel() >> 2;
        log_info!("CurrentEL: {}", currentel);
        assert_eq!(currentel, 2);
    }

    exeption::setup_exception();
    hal::HypervisorLevel::setup_hypervisor();
    hal::HypervisorLevel::boot_vm(el1_main as *const fn() as usize);
}

extern "C" fn el1_main() {
    use crate::serial::SerialDevice;
    let pl011 = drivers::pl011::Pl011::new(0x9000000, 0x1000).unwrap();
    for c in b"Hello from EL1" {
        let _ = pl011.putc(*c);
    }
    loop { unsafe { asm!("wfi") } }
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {
        core::hint::spin_loop();
    }
}
