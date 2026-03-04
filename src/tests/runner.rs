#![cfg(test)]
use crate::{print, println};
use core::panic::PanicInfo;

const TEST_FILTER: Option<&str> = Some("linked_list");

pub fn test_runner(tests: &[&dyn Testable]) -> ! {
    let selected = tests
        .iter()
        .filter(|test| TEST_FILTER.is_none_or(|f| test.name().contains(f)))
        .count();
    println!("Running {} tests", selected);

    for test in tests {
        if TEST_FILTER.is_none_or(|f| test.name().contains(f)) {
            test.run();
        }
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
    fn name(&self) -> &'static str;
    fn run(&self) -> ();
}

impl<T> Testable for T
where
    T: Fn(),
{
    fn name(&self) -> &'static str {
        core::any::type_name::<T>()
    }

    fn run(&self) {
        print!("{}...\t", self.name());
        self();
        println!("[ok]");
    }
}
