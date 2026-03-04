#![no_std]
#![no_main]

mod dtb;
mod start;
mod drivers {
    pub mod pl011;
}
mod log;
mod mutex;
mod serial;

use core::{panic::PanicInfo, sync::atomic::AtomicU8};

use crate::{log::LogLevel, mutex::Mutex};

static PL011_DEVICE: Mutex<drivers::pl011::Pl011> = Mutex::new(drivers::pl011::Pl011::invalid());
static LOG_LEVEL: AtomicU8 = AtomicU8::new(LogLevel::Info as u8);

fn main() -> ! {
    log_info!("main", "Hello from main!");
    loop {
        core::hint::spin_loop();
    }
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {
        core::hint::spin_loop();
    }
}
