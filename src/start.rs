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

use crate::{ALLOCATOR, PL011_DEVICE, drivers::{self, gicv3}, dtb, elf, paging, serial};

#[unsafe(no_mangle)]
pub extern "C" fn _start(argc: usize, argv: *const *const u8) -> usize {
    let stack_pointer = crate::arch::aarch64::get_stack_pointer() as usize;
    if argc != 2 {
        return 1;
    }
    let args = unsafe { slice::from_raw_parts(argv, argc) };
    let Ok(dtb_addr_str) = unsafe { CStr::from_ptr(args[0]) }.to_str() else {
        return 2;
    };
    let Some(dtb_address) = str_to_usize(dtb_addr_str) else {
        return 3;
    };
    let Ok(dtb) = dtb::Dtb::new(dtb_address) else {
        return 4;
    };
    if let Err(e) = init_pl011_serial_port(&dtb) {
        return e;
    };
    // これ以前はprintln!()などを使用しない

    // メモリ管理のセットアップ
    let elf_addr_str = unsafe { CStr::from_ptr(args[1]) }
        .to_str()
        .expect("Failed to get argv[1]");
    let elf_address = str_to_usize(elf_addr_str).expect("Failed to convert the address");
    setup_memory(&dtb, dtb_address, elf_address, stack_pointer);

    // ページングのセットアップ
    paging::init_stage2_translation_table();
    paging::map_address_stage2(0x40000000, 0x40000000, 0x80000000, true, true)
        .expect("Failed to map memory");

    crate::exeption::setup_exception();
    let distributor = init_gic_distributor(&dtb);
    let redistributor = init_gic_redistributor(&dtb);

    crate::main();
}

fn init_pl011_serial_port(dtb: &dtb::Dtb) -> Result<(), usize> {
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

pub fn setup_memory(dtb: &dtb::Dtb, dtb_address: usize, elf_address: usize, stack_pointer: usize) {
    let memory = dtb
        .search_node(b"memory", None)
        .expect("Expected memory node.");
    let (ram_start, ram_size) = dtb
        .read_reg_property(&memory, 0)
        .expect("Expected reg entry");
    let ram_end = ram_start + ram_size;
    crate::println!("RAM is [{:#X} ~ {:#X}]", ram_start, ram_end);

    // DTB領域を確認
    crate::println!(
        "DTB is [{:#X} ~ {:#X}]",
        dtb_address,
        dtb_address + dtb.get_total_size()
    );

    // ハイパーバイザー自身の領域を確認
    let mut elf_end: usize = 0;
    let elf_header = elf::Elf64Header::new(elf_address).expect("Invalid ELF Header");
    for p in elf_header.get_program_headers() {
        if p.get_segment_type() == elf::ELF_PROGRAM_HEADER_SEGMENT_LOAD {
            let start = p.get_physical_address() as usize;
            let size = p.get_memory_size() as usize;
            crate::println!("ELF is [{:#X} ~ {:#X}]", start, start + size);
            elf_end = elf_end.max(start + size);
        }
    }

    // Stack領域を確保
    const STACK_SIZE: usize = 0x10000;
    let stack_end = ((stack_pointer - 1) & !(paging::PAGE_SIZE - 1)) + paging::PAGE_SIZE;
    let stack_start = stack_end - STACK_SIZE;
    crate::println!("Reserve [{:#X} ~ {:#X}] for Stack", stack_start, stack_end);

    // メモリを初期化
    crate::println!("Initialize heap [{:#X} ~ {:#X}]", elf_end, stack_start);
    unsafe { ALLOCATOR.lock().init(elf_end, stack_start) };
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

fn init_gic_distributor(dtb: &dtb::Dtb) -> gicv3::GicDistributor {
    let gic_node = dtb.search_node_by_compatible(b"arm,gic-v3", None).unwrap();
    let (base_address, size) = dtb.read_reg_property(&gic_node, 0).unwrap();
    crate::println!("GIC Distributor's Base Address: {:#X}", base_address);
    let gic_distributor = gicv3::GicDistributor::new(base_address, size).unwrap();
    gic_distributor.init();
    gic_distributor
}


fn init_gic_redistributor(dtb: &dtb::Dtb) -> gicv3::GicRedistributor {
    let gic_node = dtb.search_node_by_compatible(b"arm,gic-v3", None).unwrap();
    let (base_address, size) = dtb.read_reg_property(&gic_node, 1).unwrap();
    crate::println!("GIC Redistributor's Base Address: {:#X}", base_address);
    let gic_redistributor = gicv3::get_self_redistributor(base_address, size).unwrap();
    gic_redistributor.init();
    gic_redistributor
}
