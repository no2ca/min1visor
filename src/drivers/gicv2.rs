pub const GICD_CTLR: u32 = 0x000;
pub const GICD_TYPER: u32 = 0x004;
pub const GICD_ISENABLER: u32 = 0x100;
pub const GICD_IPRIORITYR: u32 = 0x400;
pub const GICC_CTLR: u32 = 0x0000;

pub struct GicV2Info {
    pub gicd_base: usize,
    pub gicd_size: usize,
    pub gicc_base: usize,
    pub gicc_size: usize,
}
