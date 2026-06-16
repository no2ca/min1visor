use crate::log_info;

/// bits[4:0] ITLinesNumber: 最大割り込みID数 = (ITLinesNumber+1)*32
/// bits[7:5] CPUNumber: 実装されているCPUインターフェース数 - 1
pub const GICD_TYPER: usize = 0x004;

pub const GICD_CTLR: u32 = 0x000;
pub const GICD_ISENABLER: u32 = 0x100;
pub const GICD_IPRIORITYR: u32 = 0x400;
pub const GICC_CTLR: u32 = 0x0000;

pub struct GicV2Info {
    pub gicd_base: usize,
    pub gicd_size: usize,
    pub gicc_base: usize,
    pub gicc_size: usize,
}

#[inline(always)]
unsafe fn mmio_read32(base: usize, offset: usize) -> u32 {
    core::ptr::read_volatile((base + offset) as *const u32)
}

pub fn dump_gicd_info(base: usize) {
    let typer = unsafe {
        mmio_read32(base, GICD_TYPER)
    };
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
