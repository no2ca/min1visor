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
}

use crate::{log::LogLevel, mutex::Mutex};
#[allow(unused_imports)]
use core::panic::PanicInfo;
use core::sync::atomic::AtomicU8;

static PL011_DEVICE: Mutex<drivers::pl011::Pl011> = Mutex::new(drivers::pl011::Pl011::invalid());
static LOG_LEVEL: AtomicU8 = AtomicU8::new(LogLevel::Info as u8);

fn main() -> ! {
    log_info!("main", "Hello from main!");

    #[cfg(test)]
    test_main();

    loop {
        core::hint::spin_loop();
    }
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {
        core::hint::spin_loop();
    }
}
