use crate::hal;
use core::arch::asm;

pub struct Interrupt;

impl hal::traits::InterruptControl for Interrupt {
    unsafe fn disable_interrupts() -> u64 {
        unsafe { get_daif_and_disable_irq_fiq() }
    }
    unsafe fn restore_interrupts(state: u64) {
        unsafe {
            set_daif(state);
        }
    }
}

unsafe fn get_daif_and_disable_irq_fiq() -> u64 {
    let daif: u64;
    unsafe {
        asm!("
            mrs {t}, daif
            mov {r}, {t}
            orr {t}, {t}, (1 << 7 /* IRQ */) | (1 << 6 /* FIQ */)
            msr daif, {t}
            isb",
            t = out(reg) _,
            r = out(reg) daif,
        )
    }
    daif
}

unsafe fn set_daif(state: u64) {
    unsafe {
        asm!("
            isb
            msr daif, {r}",
            r = in(reg) state,
        )
    }
}
