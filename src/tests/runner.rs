#![cfg(test)]
use crate::{print, println};
use core::panic::PanicInfo;

pub fn test_runner(tests: &[&dyn Testable]) {
    println!("Running {} tests", tests.len());
    for test in tests {
        test.run();
    }
    use qemu_exit::QEMUExit;

    let qemu_exit_handle = qemu_exit::AArch64::new();
    qemu_exit_handle.exit_success();
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("[failed]\n");
    println!("Error: {}\n", info);
    use qemu_exit::QEMUExit;
    let qemu_exit_handle = qemu_exit::AArch64::new();
    qemu_exit_handle.exit_failure();
}

pub trait Testable {
    fn run(&self) -> ();
}

#[cfg(test)]
impl<T> Testable for T
where
    T: Fn(),
{
    fn run(&self) {
        print!("{}...\t", core::any::type_name::<T>());
        self();
        println!("[ok]");
    }
}

#[test_case]
fn super_simple_assertion() {
    assert_eq!(1, 1);
}
