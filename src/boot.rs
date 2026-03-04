use core::{ffi::CStr, slice};

#[unsafe(no_mangle)]
pub extern "C" fn _start(argc: usize, argv: *const *const u8) -> usize {
    if argc != 1 {
        return 1;
    }
    let args = unsafe { slice::from_raw_parts(argv, argc) };
    let Ok(fdt_addr_str) = unsafe { CStr::from_ptr(args[0]) }.to_str() else {
        return 2;
    };
    let Some(fdt_addr) = str_to_usize(fdt_addr_str) else {
        return 3;
    };
    crate::main(fdt_addr);
}

fn str_to_usize(s: &str) -> Option<usize> {
    let radix;
    let start;
    match s.get(0..2) {
        Some("0x") => {
            radix = 16;
            start = s.get(2..);
        }
        Some("0o") => {
            radix = 8;
            start = s.get(2..);
        }
        Some("0b") => {
            radix = 2;
            start = s.get(2..);
        }
        _ => {
            radix = 10;
            start = Some(s);
        }
    }
    usize::from_str_radix(start?, radix).ok()
}
