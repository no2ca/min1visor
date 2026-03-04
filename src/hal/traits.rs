pub trait InterruptControl {
    unsafe fn disable_interrupts() -> u64;
    unsafe fn restore_interrupts(state: u64);
}
