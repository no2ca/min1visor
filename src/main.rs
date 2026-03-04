#![no_std]
#![no_main]

mod dtb;
mod start;
mod drivers {
    pub mod pl011;
}
mod serial;
mod log;

use core::{mem::MaybeUninit, panic::PanicInfo, sync::atomic::AtomicU8};

use crate::log::LogLevel;

static mut PL011_DEVICE: MaybeUninit<drivers::pl011::Pl011> = MaybeUninit::uninit();
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
