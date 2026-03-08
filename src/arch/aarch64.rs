use crate::hal;
use core::arch::asm;

pub mod registers {
    // HCR_EL2
    // 下位レベルでの挙動を操作するレジスタ
    pub const HCR_EL2_API: u64 = 1 << 41;
    pub const HCR_EL2_RW: u64 = 1 << 31;
    pub const HCR_EL2_VM: u64 = 1 << 0;

    // SPSR_EL2
    // eretの呼び出し元の権限情報を保持する
    pub const SPSR_EL2_M_EL1H: u64 = 0b0101; // 戻り先のレベルとスタックポインタの分離を示す

    // VTTBR_EL2
    pub const VTTBR_BADDR: u64 = ((1 << 47) - 1) & !1;

    // VTCR_EL2
    pub const VTCR_EL2_RES1: u64 = 1 << 31;
    pub const VTCR_EL2_PS_BITS_OFFSET: u64 = 16;
    pub const VTCR_EL2_TG0_BITS_OFFSET: u64 = 14;
    pub const VTCR_EL2_SH0_BITS_OFFSET: u64 = 12;
    pub const VTCR_EL2_ORGN0_BITS_OFFSET: u64 = 10;
    pub const VTCR_EL2_IRGN0_BITS_OFFSET: u64 = 8;
    pub const VTCR_EL2_SL0_BITS_OFFSET: u64 = 6;
    pub const VTCR_EL2_SL0: u64 = 0b11 << VTCR_EL2_SL0_BITS_OFFSET;
    pub const VTCR_EL2_T0SZ_BITS_OFFSET: u64 = 0;
    pub const VTCR_EL2_T0SZ: u64 = 0b111111 << VTCR_EL2_T0SZ_BITS_OFFSET;

    // ID_AA64MMFR0_EL1
    pub const ID_AA64MMFR0_EL1_PARANGE: u64 = 0b1111;

    // ESR_EL2
    pub const ESR_EL2_EC_BITS_OFFSET: u64 = 26;
    pub const ESR_EL2_EC: u64 = 0b111111 << ESR_EL2_EC_BITS_OFFSET;
    pub const ESR_EL2_EC_DATA_ABORT: u64 = 0b100100 << 26;
    pub const ESR_EL2_ISS_ISV: u64 = 1 << 24;
    pub const ESR_EL2_ISS_SAS_BITS_OFFSET: u64 = 22;
    pub const ESR_EL2_ISS_SAS: u64 = 0b11 << ESR_EL2_ISS_SAS_BITS_OFFSET;
    pub const ESR_EL2_ISS_SRT_BITS_OFFSET: u64 = 16;
    pub const ESR_EL2_ISS_SRT: u64 = 0b11111 << ESR_EL2_ISS_SRT_BITS_OFFSET;
    pub const ESR_EL2_ISS_SF: u64 = 1 << 15;
    pub const ESR_EL2_ISS_WNR: u64 = 1 << 6;

    // HPFAR_EL2
    pub const HPFAR_EL2_FIPA_BITS_OFFSET: u64 = 4;
    pub const HPFAR_EL2_FIPA: u64 = ((1 << 44) - 1) & !((1 << 4) - 1);
}

use self::registers::*;

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
        let hcr_el2 = HCR_EL2_RW | HCR_EL2_API | HCR_EL2_VM;
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

pub fn get_id_aa64mmfr0_el1() -> u64 {
    let id_aa64mmfr0_el1: u64;
    unsafe { asm!("mrs {}, id_aa64mmfr0_el1", out(reg) id_aa64mmfr0_el1) };
    id_aa64mmfr0_el1
}

pub fn get_vtcr_el2() -> u64 {
    let vtcr_el2: u64;
    unsafe { asm!("mrs {}, vtcr_el2", out(reg) vtcr_el2) };
    vtcr_el2
}

pub unsafe fn set_vtcr_el2(vtcr_el2: u64) {
    unsafe { asm!("msr vtcr_el2, {}", in(reg) vtcr_el2) };
}

pub fn get_vttbr_el2() -> u64 {
    let vttbr_el2: u64;
    unsafe { asm!("mrs {}, vttbr_el2", out(reg) vttbr_el2) };
    vttbr_el2
}

pub unsafe fn set_vttbr_el2(vttbr_el2: u64) {
    unsafe { asm!("msr vttbr_el2, {}", in(reg) vttbr_el2) };
}

pub fn flush_tlb_el1() {
    unsafe {
        asm!(
            "
            dsb ishst
            tlbi alle1is
            "
        );
    }
}

pub unsafe fn set_vbar_el2(vbar_el2: u64) {
    unsafe { asm!("msr vbar_el2, {}", in(reg) vbar_el2) };
}

pub fn get_elr_el2() -> u64 {
    let elr_el2: u64;
    unsafe { asm!("mrs {}, elr_el2", out(reg) elr_el2) };
    elr_el2
}

pub unsafe fn advance_elr_el2() {
    unsafe { set_elr_el2(get_elr_el2() + 4) }
}

pub fn get_esr_el2() -> u64 {
    let esr_el2: u64;
    unsafe { asm!("mrs {}, esr_el2", out(reg) esr_el2) };
    esr_el2
}

pub fn get_far_el2() -> u64 {
    let far_el2: u64;
    unsafe { asm!("mrs {}, far_el2", out(reg) far_el2) };
    far_el2
}

pub fn get_hpfar_el2() -> u64 {
    let hpfar_el2: u64;
    unsafe { asm!("mrs {}, hpfar_el2", out(reg) hpfar_el2) };
    hpfar_el2
}

pub unsafe fn set_sp_el1(sp_el1: u64) {
    unsafe { asm!("msr sp_el1, {}", in(reg) sp_el1) };
}

