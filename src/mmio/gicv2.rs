//!
//! GICv2 の MMIO Driver
//!

use crate::{
    arch::aarch64::{get_hcr_el2, registers::HCR_EL2_VI, set_hcr_el2}, mmio::pl011::PL011_INT_ID, mutex::Mutex, vm::{MmioHandler, get_active_vm},
};

const GICD_CTLR: usize = 0x000;
const GICD_TYPER: usize = 0x004;
const GICD_IIDR: usize = 0x008;
const GICD_ITARGETSR_START: usize = 0x800;
const GICD_ITARGETSR_END: usize = 0x900;

const GICC_CTLR: usize = 0x000;
const GICC_PMR: usize = 0x004;
const GICC_BPR: usize = 0x008;
const GICC_IAR: usize = 0x00C;
const GICC_EOIR: usize = 0x010;
const GICC_IIDR: usize = 0x0FC;

const GICC_IAR_SPURIOUS_INTERRUPT_ID: u64 = 1023;

static PENDING: Mutex<u64> = Mutex::new(0);

pub fn inject_interrupt(int_id: u32) {
    assert!(int_id < u64::BITS, "Interrupt ID is out of range");

    let mut pending = PENDING.lock();
    *pending |= 1u64 << int_id;

    let hcr_el2 = get_hcr_el2();
    unsafe { set_hcr_el2(hcr_el2 | HCR_EL2_VI) };
}

fn acknowledge_interrupt() -> u64 {
    let mut pending = PENDING.lock();
    if *pending == 0 {
        return GICC_IAR_SPURIOUS_INTERRUPT_ID;
    }

    let int_id = pending.trailing_zeros();
    *pending &= !(1u64 << int_id);
    if *pending == 0 {
        let hcr_el2 = get_hcr_el2();
        unsafe { set_hcr_el2(hcr_el2 & !HCR_EL2_VI) };
    }

    int_id as u64
}

pub struct GicDistributorMmio {
    ctlr: u32,
}

impl GicDistributorMmio {
    pub const MMIO_SIZE: usize = 0x10000;

    pub fn new() -> Self {
        Self { ctlr: 0 }
    }
}

impl MmioHandler for GicDistributorMmio {
    fn read(&mut self, offset: usize, _access_width: u64) -> Result<u64, ()> {
        let value = match offset {
            GICD_CTLR => self.ctlr as u64,
            GICD_TYPER => 0x00000001,
            GICD_IIDR => 0x0200043B,
            GICD_ITARGETSR_START..GICD_ITARGETSR_END => 0x01010101,
            _ => 0,
        };
        Ok(value)
    }

    fn write(&mut self, offset: usize, _access_width: u64, value: u64) -> Result<(), ()> {
        if offset == GICD_CTLR {
            self.ctlr = value as u32;
        }
        Ok(())
    }
}

pub struct GicCpuInterfaceMmio {
    ctlr: u32,
    pmr: u32,
    bpr: u32,
}

impl GicCpuInterfaceMmio {
    pub const MMIO_SIZE: usize = 0x10000;

    pub fn new() -> Self {
        Self {
            ctlr: 0,
            pmr: 0,
            bpr: 0,
        }
    }
}

impl MmioHandler for GicCpuInterfaceMmio {
    fn read(&mut self, offset: usize, _access_width: u64) -> Result<u64, ()> {
        let value = match offset {
            GICC_CTLR => self.ctlr as u64,
            GICC_PMR => self.pmr as u64,
            GICC_BPR => self.bpr as u64,
            GICC_IAR => acknowledge_interrupt(),
            GICC_IIDR => 0x0202143B,
            _ => 0,
        };
        Ok(value)
    }

    fn write(&mut self, offset: usize, _access_width: u64, value: u64) -> Result<(), ()> {
        match offset {
            GICC_CTLR => self.ctlr = value as u32,
            GICC_PMR => self.pmr = value as u32,
            GICC_BPR => self.bpr = value as u32,
            #[cfg(feature = "rpi4")]
            GICC_EOIR =>
            {
                use crate::{GICC_BASE, GICD_BASE, drivers::gicv2::GicV2};
                use core::sync::atomic::Ordering::Relaxed;

                let gicd_base = GICD_BASE.load(Relaxed);
                let gicc_base = GICC_BASE.load(Relaxed);
                let gic = GicV2::new(gicd_base, gicc_base);

                if value & 0x3ff == 27 {
                    gic.enable_ppi(27);
                }
                if value & 0x3ff == PL011_INT_ID.into() {
                    let pl011_mmio = unsafe { &mut *get_active_vm().get_pl011_mmio() };
                    if pl011_mmio.is_rx_pending() {
                        crate::mmio::gicv2::inject_interrupt(PL011_INT_ID);
                    }
                }
            }
            _ => {}
        }
        Ok(())
    }
}
