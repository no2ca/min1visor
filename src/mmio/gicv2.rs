//!
//! GICv2 Distributor の MMIO Driver
//!

use crate::vm::MmioHandler;

const GICD_CTLR: usize = 0x000;
const GICD_TYPER: usize = 0x004;
const GICD_IIDR: usize = 0x008;
const GICD_ITARGETSR_START: usize = 0x800;
const GICD_ITARGETSR_END: usize = 0x900;

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
