//!
//! `_start` は起動引数と DTB(Device Tree Blob) を検証し、PL011の初期化まで行う
//! 失敗時は以下の戻り値で原因を示す
//!
//! - `1`: 引数個数が不正 (`argc != 1`)
//! - `2`: 引数文字列の UTF-8 変換に失敗
//! - `3`: 引数文字列を `usize` アドレスに変換できない
//! - `4`: DTB の生成 (`dtb::Dtb::new`) に失敗
//! - `5`: 利用可能な `arm,pl011` ノードが見つからない
//! - `6`: PL011 ノードの `reg` プロパティを読めない
//! - `7`: PL011 ドライバ (`drivers::pl011::Pl011::new`) の初期化に失敗
//!
use core::{ffi::CStr, slice};

use crate::{PL011_DEVICE, drivers, dtb, serial};

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
    let Ok(dtb) = dtb::Dtb::new(fdt_addr) else {
        return 4;
    };
    if let Err(e) = init_serial_port(&dtb) {
        return e;
    };
    crate::main();
}

fn init_serial_port(dtb: &dtb::Dtb) -> Result<(), usize> {
    let mut pl011 = None;
    loop {
        pl011 = dtb.search_node_by_compatible(b"arm,pl011", pl011.as_ref());
        match &pl011 {
            Some(d) => {
                if !dtb.is_node_operational(d) {
                    continue;
                } else {
                    break;
                }
            }
            None => {
                return Err(5);
            }
        }
    }
    let pl011 = pl011.unwrap();
    let Some((pl011_base, pl011_range)) = dtb.read_reg_property(&pl011, 0) else {
        return Err(6);
    };
    let Ok(pl011) = drivers::pl011::Pl011::new(pl011_base, pl011_range) else {
        return Err(7);
    };
    *PL011_DEVICE.lock() = pl011;
    serial::init_default_serial_port(&PL011_DEVICE);
    Ok(())
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
