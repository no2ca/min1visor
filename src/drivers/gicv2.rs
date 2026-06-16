use crate::log_info;

/// bits[4:0] ITLinesNumber: 最大割り込みID数 = (ITLinesNumber+1)*32
/// bits[7:5] CPUNumber: 実装されているCPUインターフェース数 - 1
pub const GICD_TYPER: usize = 0x004;

const GICD_CTLR: usize = 0x000;
const GICD_ISENABLER: usize = 0x100;
const GICD_IPRIORITYR: usize = 0x400;
const GICD_SGIR: usize = 0xF00;

const GICC_CTLR: usize = 0x0000;
const GICC_PMR: usize = 0x004;
pub const GICC_IAR: usize = 0x00C;
pub const GICC_EOIR: usize = 0x010;

pub struct GicV2 {
    pub gicd_base: usize,
    pub gicc_base: usize,
}

impl GicV2 {
    #[inline(always)]
    pub fn distributor_read32(&self, offset: usize) -> u32 {
        unsafe { core::ptr::read_volatile((self.gicd_base + offset) as *const u32) }
    }

    #[inline(always)]
    pub fn interface_read32(&self, offset: usize) -> u32 {
        unsafe { core::ptr::read_volatile((self.gicc_base + offset) as *const u32) }
    }

    #[inline(always)]
    pub unsafe fn distributor_write32(&self, offset: usize, value: u32) {
        unsafe { core::ptr::write_volatile((self.gicd_base + offset) as *mut u32, value) }
    }

    #[inline(always)]
    pub unsafe fn interface_write32(&self, offset: usize, value: u32) {
        unsafe { core::ptr::write_volatile((self.gicc_base + offset) as *mut u32, value) }
    }
    
    pub fn new(gicd_base: usize, gicc_base: usize) -> Self {
        Self { gicd_base, gicc_base }
    }

    pub fn dump_gicd_info(&self) {
        let typer = self.distributor_read32(GICD_TYPER);
        let it_lines_number = typer & 0x1F;
        let max_interrupts = (it_lines_number + 1) * 32;
        let cpu_number = (typer >> 5) & 0x7;
        
        log_info!(
            "GICD_TYPER: {:#010X} (max interrupts: {}, CPU interfaces: {})",
            typer,
            max_interrupts,
            cpu_number + 1
        );
    }

    pub fn enable_interrupt(&self) {
        unsafe {
            self.distributor_write32(GICD_CTLR, 1);
            self.interface_write32(GICC_CTLR, 1);
            // その CPU が受け取ってよい優先度の下限を決めるマスク
            self.interface_write32(GICC_PMR, 0xff);
        }
    }
    
    /// Software Generated Interruptを発生させる
    pub fn send_sgi_to_self(&self) {
        const SGI_ID: u32 = 1;
        const SGIR_TARGET_SELF: u32 = 0b10 << 24;
        unsafe {
            self.distributor_write32(GICD_SGIR, SGIR_TARGET_SELF | SGI_ID);
        }
    }
}
