#![no_std]
#![no_main]

mod dtb;
mod start;
mod drivers {
    pub mod pl011;
}
mod serial;

use core::{mem::MaybeUninit, panic::PanicInfo};

static mut PL011_DEVICE: MaybeUninit<drivers::pl011::Pl011> = MaybeUninit::uninit();

fn main() -> ! {
    println!("Hello from main!");
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
