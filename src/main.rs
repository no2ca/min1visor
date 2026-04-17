//!
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
#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(crate::tests::runner::test_runner)]
#![reexport_test_harness_main = "test_main"]

mod dtb;
mod drivers {
    pub mod gicv3;
    pub mod pl011;
    pub mod virtio;
    pub mod virtio_blk;
}
mod log;
mod mutex;
mod serial;
mod tests {
    pub mod runner;
    pub mod test;
}
mod arch {
    pub mod aarch64;
}
mod hal;
mod allocator {
    pub mod linked_list;
}
mod elf;
mod exeption;
mod paging;
mod mmio {
    pub mod pl011;
}
mod fat32;

use crate::drivers::{gicv3, virtio_blk};
use crate::{
    allocator::linked_list::LinkedListAllocator, hal::HypervisorControl, log::LogLevel,
    mutex::Mutex,
};
#[allow(unused_imports)]
use core::panic::PanicInfo;
use core::{arch::asm, sync::atomic::AtomicU8};
use core::{ffi::CStr, slice};

static LOG_LEVEL: AtomicU8 = AtomicU8::new(LogLevel::Debug as u8);
static PL011_DEVICE: Mutex<drivers::pl011::Pl011> = Mutex::new(drivers::pl011::Pl011::invalid());
static ALLOCATOR: Mutex<LinkedListAllocator> = Mutex::new(LinkedListAllocator::new());

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    println!("{}", info);
    loop {
        core::hint::spin_loop();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn main(argc: usize, argv: *const *const u8) -> usize {
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

    // 例外ハンドラのセットアップ
    crate::exeption::setup_exception();

    // 割り込みコントローラのセットアップ
    let distributor = init_gic_distributor(&dtb);
    let redistributor = init_gic_redistributor(&dtb);

    // PL011の割り込みのセットアップ
    enable_serial_port_interrupt(&*PL011_DEVICE.lock(), &distributor);

    // virtio_blk (legacy) のセットアップ
    let mut virtioblk = init_virtio_blk(&dtb).unwrap();
    // let mut buffer: [u8; 512] = [0; 512];
    // virtioblk
    //     .read(&mut buffer as *mut _ as usize, 0, 512)
    //     .expect("Failed to read first 512bytes");
    // crate::println!("{:#X?}", buffer);
    // let boot_signature = [buffer[510], buffer[511]];
    // assert_eq!(u16::from_le_bytes(boot_signature), 0xAA55); /* BOOT Signature */

    // fat32のセットアップ
    init_fat32(&mut virtioblk);

    // 現在のELを表示
    let currentel = crate::arch::aarch64::get_currentel() >> 2;
    crate::log_info!("CurrentEL: {}", currentel);
    assert_eq!(currentel, 2);

    // hypervisorモードのセットアップ
    crate::hal::HypervisorLevel::setup_hypervisor();

    log_info!("Hello from main!");

    #[cfg(test)]
    test_main();

    hal::HypervisorLevel::boot_vm(el1_main as *const fn() as usize);
}

extern "C" fn el1_main() {
    use crate::serial::SerialDevice;
    let pl011 = drivers::pl011::Pl011::new(0x9000000, 0x1000, 0).unwrap();
    for c in b"Hello from EL1\n" {
        let _ = pl011.putc(*c);
    }
    loop {
        unsafe { asm!("wfi") }
    }
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

    let interrupts =
        dtb.read_property_as_u32_array(&dtb.get_property(&pl011, b"interrupts").unwrap());
    let mut interrupt_number = 0;
    // 割り込みのタイプとトリガがあっているか検証
    if u32::from_be(interrupts[0]) == gicv3::DTB_GIC_SPI
        && u32::from_be(interrupts[2]) == gicv3::DTB_GIC_LEVEL
    {
        interrupt_number = gicv3::GIC_SPI_BASE + u32::from_be(interrupts[1]);
    }

    let Ok(pl011) = drivers::pl011::Pl011::new(pl011_base, pl011_range, interrupt_number) else {
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

fn enable_serial_port_interrupt(
    pl011: &drivers::pl011::Pl011,
    distributor: &gicv3::GicDistributor,
) {
    let int_id = pl011.interrupt_number;
    if int_id == 0 {
        crate::println!("PL011 does not support interrupt.");
        return;
    }
    distributor.set_group(int_id, gicv3::GicGroup::NonSecureGroup1);
    distributor.set_priority(int_id, 0x00);
    distributor.set_routing(int_id, false, crate::arch::aarch64::get_mpidr_el1());
    distributor.set_trigger_mode(int_id, true);
    distributor.set_pending(int_id, false);
    distributor.set_enable(int_id, true);
    pl011.enable_interrupt();
}

fn init_virtio_blk(dtb: &dtb::Dtb) -> Option<virtio_blk::VirtioBlk> {
    let mut virtio = None;
    loop {
        virtio = dtb.search_node_by_compatible(b"virtio,mmio", virtio.as_ref());
        match &virtio {
            Some(virtio) => {
                if dtb.is_node_operational(virtio) {
                    let (base_address, _) = dtb.read_reg_property(virtio, 0).unwrap();
                    if let Ok(blk) = virtio_blk::VirtioBlk::new(base_address) {
                        return Some(blk);
                    }
                }
            }
            None => {
                return None;
            }
        }
    }
}

pub fn init_fat32(blk: &mut virtio_blk::VirtioBlk) {
    #[derive(Debug)]
    #[repr(C)]
    struct PartitionTableEntry {
        boot_flag: u8,
        first_sector: [u8; 3],
        partition_type: u8,
        last_sector: [u8; 3],
        first_sector_lba: u32,
        number_of_sectors: u32,
    }
    const PARTITION_TABLE_BASE: usize = 0x1BE;
    // Boot Signatureの確認
    let mut mbr: [u8; 512] = [0; 512];
    blk.read(&mut mbr as *mut _ as usize, 0, 512)
        .expect("Failed to read first 512 bytes");
    assert_eq!(u16::from_le_bytes([mbr[510], mbr[511]]), 0xAA55);

    // パーティションテーブルの解析
    let ptr = mbr[PARTITION_TABLE_BASE..].as_ptr() as *const [PartitionTableEntry; 4];
    let partition_table = unsafe {
        core::ptr::read_unaligned(ptr)
        // &*(&mbr[PARTITION_TABLE_BASE] as *const _ as usize as *const [PartitionTableEntry; 4])
    };
    let mut fat32 = Err(());
    for entry in partition_table {
        log_debug!("{:?}", entry);
        if entry.partition_type == 0x0C {
            log_debug!("ok");
            fat32 = fat32::Fat32::new(blk, entry.first_sector_lba as usize, 512);
            break;
        }
    }

    // ファイルのリストアップとmin1visor.elfの読み込み
    let fat32 = fat32.expect("The FAT32 Partition is not found!");
    // fat32.list_files();
    // let file_info = fat32.search_file("MIN1VISOR.ELF").unwrap();
    // let elf_data: [u8; 512] = [0u8; 512];
    // fat32
    //     .read(&file_info, blk, &elf_data as *const _ as usize, 0, 512)
    //     .expect("Failed to read");
    // println!("{:#X?}", elf_data);
}
