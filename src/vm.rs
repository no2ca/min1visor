use crate::allocator::linked_list::allocate_pages;
use crate::arch::aarch64::registers::*;
use crate::drivers::virtio_blk::VirtioBlk;
use crate::fat32::Fat32;
use crate::mmio::pl011::Pl011Mmio;
use crate::mmio::virtio_blk::VirtioBlkMmio;
use crate::mutex::Mutex;
use crate::serial::SerialDevice;
use crate::{PL011_DEVICE, log_debug, log_warn, paging::*};
#[cfg(feature = "qemu-virt")]
use crate::{
    mmio::gicv3::{GicDistributorMmio, GicRedistributorMmio},
    vgicv3,
    drivers::{gicv3::GicRedistributor, generic_timer_gicv3}
};

use alloc::boxed::Box;
use alloc::collections::linked_list::LinkedList;

pub trait MmioHandler {
    fn read(&mut self, offset: usize, access_width: u64) -> Result<u64, ()>;
    fn write(&mut self, offset: usize, access_width: u64, value: u64) -> Result<(), ()>;
}

pub struct MmioEntry {
    base_address: usize,
    length: usize,
    handler: Box<dyn MmioHandler>,
}

pub struct VM {
    _vm_id: usize,
    ram_virtual_base_address: usize,
    ram_physical_base_address: usize,
    ram_size: usize,
    mmio_handlers: LinkedList<MmioEntry>,
    #[cfg(feature = "qemu-virt")]
    gic_distributor_mmio: *mut GicDistributorMmio,
    #[cfg(feature = "qemu-virt")]
    gic_redistributor_mmio: *mut GicRedistributorMmio,
    pl011_mmio: *mut Pl011Mmio,
}

#[repr(C)]
struct KernelHeader {
    code0: u32,
    code1: u32,
    text_offset: u64,
    image_size: u64,
    flags: u64,
    res2: u64,
    res3: u64,
    res4: u64,
    magic: u32,
    res5: u32,
}

pub struct VmManager {
    next_vm_id: usize,
    vm_list: LinkedList<VM>,
    active_vm_id: Option<usize>,
}

static VM_MANAGER: Mutex<VmManager> = Mutex::new(VmManager::new());

unsafe impl Send for VmManager {}

impl VmManager {
    const fn new() -> Self {
        Self {
            next_vm_id: 0,
            vm_list: LinkedList::new(),
            active_vm_id: None,
        }
    }

    fn allocate_vm_id(&mut self) -> usize {
        let vm_id = self.next_vm_id;
        self.next_vm_id += 1;
        vm_id
    }

    fn push_vm(&mut self, vm: VM) {
        if self.active_vm_id.is_none() {
            self.active_vm_id = Some(vm._vm_id);
        }
        self.vm_list.push_back(vm);
    }

    fn current_vm_mut(&mut self) -> Option<&mut VM> {
        match self.active_vm_id {
            Some(active_vm_id) => self.vm_list.iter_mut().find(|vm| vm._vm_id == active_vm_id),
            None => self.vm_list.front_mut(),
        }
    }

    fn active_vm_mut(&mut self) -> Option<&mut VM> {
        self.current_vm_mut()
    }
}

impl VM {
    #[allow(clippy::too_many_arguments)]
    pub const fn new(
        _vm_id: usize,
        ram_virtual_base_address: usize,
        ram_physical_base_address: usize,
        ram_size: usize,
        mmio_handlers: LinkedList<MmioEntry>,
        #[cfg(feature = "qemu-virt")]
        gic_distributor_mmio: *mut GicDistributorMmio,
        #[cfg(feature = "qemu-virt")]
        gic_redistributor_mmio: *mut GicRedistributorMmio,
        pl011_mmio: *mut Pl011Mmio,
    ) -> Self {
        Self {
            _vm_id,
            ram_virtual_base_address,
            ram_physical_base_address,
            ram_size,
            mmio_handlers,
            #[cfg(feature = "qemu-virt")]
            gic_distributor_mmio,
            #[cfg(feature = "qemu-virt")]
            gic_redistributor_mmio,
            pl011_mmio,
        }
    }

    pub fn handle_mmio_read(&mut self, address: usize, access_width: u64) -> Result<u64, ()> {
        for e in &mut self.mmio_handlers {
            if e.base_address <= address && address < (e.base_address + e.length) {
                return e.handler.read(address - e.base_address, access_width);
            }
        }
        Err(())
    }

    pub fn handle_mmio_write(
        &mut self,
        address: usize,
        access_width: u64,
        value: u64,
    ) -> Result<(), ()> {
        for e in &mut self.mmio_handlers {
            if e.base_address <= address && address < (e.base_address + e.length) {
                return e
                    .handler
                    .write(address - e.base_address, access_width, value);
            }
        }
        Err(())
    }

    pub fn get_physical_address(&self, virtual_address: usize) -> Option<usize> {
        if (self.ram_virtual_base_address..(self.ram_virtual_base_address + self.ram_size))
            .contains(&virtual_address)
        {
            Some(virtual_address - self.ram_virtual_base_address + self.ram_physical_base_address)
        } else {
            None
        }
    }

    #[cfg(feature = "qemu-virt")]
    pub fn get_gic_distributor_mmio(&self) -> *mut GicDistributorMmio {
        self.gic_distributor_mmio
    }

    #[cfg(feature = "qemu-virt")]
    pub fn get_gic_redistributor_mmio(&self) -> *mut GicRedistributorMmio {
        self.gic_redistributor_mmio
    }

    pub fn get_pl011_mmio(&self) -> *mut Pl011Mmio {
        self.pl011_mmio
    }
}

impl MmioEntry {
    pub fn new(base_address: usize, length: usize, handler: Box<dyn MmioHandler>) -> Self {
        Self {
            base_address,
            length,
            handler,
        }
    }
}

#[cfg(feature = "qemu-virt")]
pub fn create_vm(
    fat32: &Fat32,
    blk: &mut VirtioBlk,
    gic_redistributor: &GicRedistributor,
) -> (usize, usize) {
    const RAM_VIRTUAL_BASE: usize = 0x40000000;
    // BASE + 128MiB
    const INITRAMFS_VIRTUAL_BASE: usize = 0x48000000;
    /// RAM SIZE: 256MiB
    const RAM_SIZE: usize = 0x10000000;
    const ALIGN_SIZE: usize = 0x200000;

    // 仮想マシンの基本要素の設定
    let ram_physical_address = allocate_pages(RAM_SIZE >> PAGE_SHIFT, PAGE_SHIFT)
        .expect("Failed to allocate memory for VM.");
    let vm_id = VM_MANAGER.lock().allocate_vm_id();
    let cpu_mpidr = crate::arch::aarch64::get_mpidr_el1();

    // 仮想化に関するハードウェアの設定
    // レジスタのセットアップ
    setup_hypervisor_registers();

    // Stage 2 Translation の初期化
    init_stage2_translation_table();
    map_address_stage2(ram_physical_address, RAM_VIRTUAL_BASE, RAM_SIZE, true, true)
        .expect("Failed to map memory");

    vgicv3::init_vgic(gic_redistributor);

    // MMIO ハンドラの初期化
    let mut mmio_handlers = LinkedList::new();

    // Generic Timerの初期化
    generic_timer_gicv3::init_generic_timer_local(gic_redistributor);

    // PL011
    let mut pl011_mmio = Box::new(Pl011Mmio::new());
    let pl011_mmio_ptr = pl011_mmio.as_mut() as *mut _;
    mmio_handlers.push_back(MmioEntry::new(0x9000000, 0x1000, pl011_mmio));

    // virtio-blk
    let file_name = [b'D', b'I', b'S', b'K', b'0' + vm_id as u8];
    let disk_file = fat32
        .search_file(core::str::from_utf8(&file_name).unwrap())
        .expect("Failed to find Disk");
    mmio_handlers.push_back(MmioEntry::new(
        0xa000000,
        0x0200,
        Box::new(VirtioBlkMmio::new(disk_file)),
    ));

    // GIC Distributor
    let mut gic_distributor_mmio = Box::new(GicDistributorMmio::new());
    let gic_distributor_mmio_ptr = gic_distributor_mmio.as_mut() as *mut _;
    mmio_handlers.push_back(MmioEntry::new(
        0x8000000,
        GicDistributorMmio::MMIO_SIZE,
        gic_distributor_mmio,
    ));

    // GIC Redistributor
    let mut gic_redistributor_mmio = Box::new(GicRedistributorMmio::new(cpu_mpidr));
    let gic_redistributor_mmio_ptr = gic_redistributor_mmio.as_mut() as *mut _;
    mmio_handlers.push_back(MmioEntry::new(
        0x80a0000,
        GicRedistributorMmio::MMIO_SIZE,
        gic_redistributor_mmio,
    ));

    // VM構造体の作成
    let vm = VM::new(
        vm_id,
        RAM_VIRTUAL_BASE,
        ram_physical_address,
        RAM_SIZE,
        mmio_handlers,
        gic_distributor_mmio_ptr,
        gic_redistributor_mmio_ptr,
        pl011_mmio_ptr,
    );

    // 仮想マシンのバイナリの読み込み
    let kernel = fat32.search_file("IMAGE").unwrap();
    let dtb = fat32.search_file("DTB").unwrap();
    // let initramfs = fat32.search_file("RAMFS.GZ").unwrap();
    let initramfs = fat32.search_file("ROOTFS").unwrap();
    let kernel_size = kernel.get_file_size();
    let dtb_size = dtb.get_file_size();
    let initramfs_size = initramfs.get_file_size();
    let kernel_virtual_address =
        ((RAM_VIRTUAL_BASE + dtb_size - 1) & !(ALIGN_SIZE - 1)) + ALIGN_SIZE;
    let initramfs_virtual_address = INITRAMFS_VIRTUAL_BASE;
    let ram_end = RAM_VIRTUAL_BASE
        .checked_add(RAM_SIZE)
        .expect("RAM address range overflow");
    let dtb_end = RAM_VIRTUAL_BASE
        .checked_add(dtb_size)
        .expect("DTB address range overflow");
    let kernel_end = kernel_virtual_address
        .checked_add(kernel_size)
        .expect("Kernel address range overflow");
    let initramfs_end = initramfs_virtual_address
        .checked_add(initramfs_size)
        .expect("Initramfs address range overflow");

    if dtb_end > ram_end {
        panic!("DTB exceeds RAM size");
    }
    if kernel_end > ram_end {
        panic!("Kernel exceeds RAM size");
    }
    if initramfs_end > ram_end {
        panic!("Initramfs exceeds RAM size");
    }
    if RAM_VIRTUAL_BASE < kernel_end && kernel_virtual_address < dtb_end {
        panic!("DTB overlaps Kernel");
    }
    if RAM_VIRTUAL_BASE < initramfs_end && initramfs_virtual_address < dtb_end {
        panic!("DTB overlaps Initramfs");
    }
    if kernel_virtual_address < initramfs_end && initramfs_virtual_address < kernel_end {
        panic!("Kernel overlaps Initramfs");
    }

    let kernel_physical_address = vm.get_physical_address(kernel_virtual_address).unwrap();
    let initramfs_physical_address = vm.get_physical_address(initramfs_virtual_address).unwrap();

    log_debug!(
        "DTB\tsize=0x{:X} placed at virt=0x{:X}, phys=0x{:X}",
        dtb_size,
        RAM_VIRTUAL_BASE,
        ram_physical_address
    );
    log_debug!(
        "Kernel\tsize=0x{:X} placed at virt=0x{:X}, phys=0x{:X}",
        kernel_size,
        kernel_virtual_address,
        kernel_physical_address
    );
    log_debug!(
        "Initramfs\tsize=0x{:X} placed at virt=0x{:X}, phys=0x{:X}",
        initramfs_size,
        initramfs_virtual_address,
        initramfs_physical_address
    );

    fat32
        .read(&dtb, blk, ram_physical_address, 0, dtb_size)
        .expect("Failed to read DTB");
    fat32
        .read(&kernel, blk, kernel_physical_address, 0, kernel_size)
        .expect("Failed to read Kernel");
    fat32
        .read(
            &initramfs,
            blk,
            initramfs_physical_address,
            0,
            initramfs_size,
        )
        .expect("Failed to read Initramfs");

    // Linux Kernel Headerの解析
    let header = unsafe { &*(kernel_physical_address as *const KernelHeader) };
    if header.magic != 0x644D5241 {
        panic!("Invalid Kernel Magic: {:#X}", header.magic);
    }
    let mut text_offset = header.text_offset;
    let image_size = header.image_size;
    if image_size == 0 {
        text_offset = 0x80000;
    }

    // VM構造体のリストへの追加
    VM_MANAGER.lock().push_vm(vm);

    (
        kernel_virtual_address + text_offset as usize,
        RAM_VIRTUAL_BASE,
    )
}

fn setup_hypervisor_registers() {
    // MIDR_EL1
    unsafe { crate::arch::aarch64::set_vpidr_el2(crate::arch::aarch64::get_midr_el1()) };

    // MPIDR_EL1
    unsafe { crate::arch::aarch64::set_vmpidr_el2(crate::arch::aarch64::get_mpidr_el1()) };

    // HCR_EL2
    let hcr_el2 = HCR_EL2_RW | HCR_EL2_API | HCR_EL2_AMO | HCR_EL2_IMO | HCR_EL2_FMO | HCR_EL2_VM;
    unsafe { crate::arch::aarch64::set_hcr_el2(hcr_el2) };
}

pub fn input_uart() {
    let c = (PL011_DEVICE.lock()).getc();
    if c.is_err() {
        log_warn!("Failed to get a character");
        return;
    }
    let c = c.unwrap().unwrap_or(0);
    if c == 0 {
        return;
    }

    let vm = get_active_vm();

    #[cfg(feature = "qemu-virt")]
    unsafe { (*vm.get_pl011_mmio()).push(c, &mut *vm.get_gic_distributor_mmio()) };
}

pub fn get_current_vm() -> &'static mut VM {
    let mut manager = VM_MANAGER.lock();
    let vm = manager.current_vm_mut().unwrap() as *mut VM;
    unsafe { &mut *vm }
}

pub fn get_active_vm() -> &'static mut VM {
    let mut manager = VM_MANAGER.lock();
    let vm = manager.active_vm_mut().unwrap() as *mut VM;
    unsafe { &mut *vm }
}
