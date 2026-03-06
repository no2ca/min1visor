use crate::hal;
use core::arch::asm;

// HCR_EL2
// 下位レベルでの挙動を操作するレジスタ
const HCR_EL2_API: u64 = 1 << 41;
const HCR_EL2_RW: u64 = 1 << 31;

// SPSR_EL2
// eretの呼び出し元の権限情報を保持する
const SPSR_EL2_M_EL1H: u64 = 0b0101; // 戻り先のレベルとスタックポインタの分離を示す

pub struct AArch64Interrupts;

impl hal::InterruptControl for AArch64Interrupts {
    unsafe fn disable_interrupts() -> u64 {
        unsafe { get_daif_and_disable_irq_fiq() }
    }
    unsafe fn restore_interrupts(state: u64) {
        unsafe {
            set_daif(state);
        }
    }
}

pub struct AArch64Hypervisor;

impl hal::HypervisorControl for AArch64Hypervisor {
    fn setup_hypervisor() {
        // RWはEL1でAArch64として動作させるためのメンバ
        let hcr_el2 = HCR_EL2_RW | HCR_EL2_API;
        unsafe { set_hcr_el2(hcr_el2) };
    }
    fn boot_vm(entry_point: usize) -> ! {
        unsafe {
            set_spsr_el2(SPSR_EL2_M_EL1H);
            set_elr_el2(entry_point as u64);
            eret(0, 0, 0, 0);
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

pub fn get_currentel() -> u64 {
    let currentel: u64;
    unsafe { asm!("mrs {}, currentel", out(reg) currentel) };
    currentel
}

unsafe fn set_hcr_el2(hcr_el2: u64) {
    unsafe { asm!("msr hcr_el2, {}", in(reg) hcr_el2) };
}

unsafe fn set_spsr_el2(spsr_el2: u64) {
    unsafe { asm!("msr spsr_el2, {}", in(reg) spsr_el2) };
}

unsafe fn set_elr_el2(elr_el2: u64) {
    unsafe { asm!("msr elr_el2, {}", in(reg) elr_el2) };
}

unsafe fn eret(x0: u64, x1: u64, x2: u64, x3: u64) -> ! {
    unsafe {
        asm!("eret",
             in("x0") x0,
             in("x1") x1,
             in("x2") x2,
             in("x3") x3,
             options(noreturn))
    }
}

pub fn get_stack_pointer() -> u64 {
    let sp: u64;
    unsafe { asm!("mov {}, sp", out(reg) sp) };
    sp
}
