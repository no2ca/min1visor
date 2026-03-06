#[cfg(target_arch = "aarch64")]
pub type Interrupts = crate::arch::aarch64::AArch64Interrupts;

pub trait InterruptControl {
    unsafe fn disable_interrupts() -> u64;
    unsafe fn restore_interrupts(state: u64);
}

#[cfg(target_arch = "aarch64")]
pub type HypervisorLevel = crate::arch::aarch64::AArch64Hypervisor;

pub trait HypervisorControl {
    fn setup_hypervisor();
    fn boot_vm(entry_point: usize) -> !;
}
