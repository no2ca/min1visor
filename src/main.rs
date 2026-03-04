#![no_std]
#![no_main]

mod boot;
use core::panic::PanicInfo;

fn main(fdt_addr: usize) -> ! {
    loop {
        core::hint::spin_loop();
    }
}

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
