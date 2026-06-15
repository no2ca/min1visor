//!
//! Arm PL011のデバイスドライバ
//!
use crate::serial;

use core::fmt::Error;
use core::ptr;

pub struct Pl011 {
    base_address: usize,
    pub interrupt_number: u32,
}

#[cfg(feature = "qemu-virt")]
const UART_SIZE: usize = 0x1000;
#[cfg(feature = "rpi4")]
const UART_SIZE: usize = 0x200;

const UART_DR: usize = 0x000;
const UART_FR: usize = 0x018;
const UART_CR: usize = 0x030; // pl011の機能を設定するレジスタ
const _UART_IMSC: usize = 0x038; // pl011の割り込みに関する操作をするレジスタ

// 送信中フラグ
const _UART_FR_BUSY: u32 = 1 << 3; 
/// TX FIFO が一杯か示すビット
const UART_FR_TXFF: u32 = 1 << 5;
/// RX FIFO が空か示すビット
const UART_FR_RXFE: u32 = 1 << 4;
/// 受信が有効か示すビット
const UART_CR_RXE: u32 = 1 << 9;
/// 送信が有効か表すビット
const UART_CR_TXE: u32 = 1 << 8;
/// UARTが有効か示すビット
const UART_CR_UARTEN: u32 = 1;
/// 受信割り込みが有効か示すビット
const _UART_IMSC_RXIM: u32 = 1 << 4;

// RPi4 (BCM2711) GPIO レジスタ（物理ベース: 0xFE200000）
const GPIO_BASE: u32 = 0xFE200000;
pub const GPFSEL1: u32 = GPIO_BASE + 0x04;

#[inline(always)]
fn memory_barrier() {
    unsafe {
        core::arch::asm!("dsb sy", options(nostack));
        core::arch::asm!("isb", options(nostack));
    }
}

impl Pl011 {
    // Mutexの初期化前に使用
    pub const fn invalid() -> Self {
        Self {
            base_address: 0,
            interrupt_number: 0x0,
        }
    }
    pub fn new(base_address: usize, range: usize, interrupt_number: u32) -> Result<Self, ()> {
        if range < UART_SIZE {
            return Err(());
        }
        Ok(Self {
            base_address,
            interrupt_number,
        })
    }

    fn is_tx_fifo_full(&self) -> bool {
        (unsafe { ptr::read_volatile((self.base_address + UART_FR) as *const u32) } & UART_FR_TXFF)
            != 0
    }

    fn is_rx_fifo_empty(&self) -> bool {
        (unsafe { ptr::read_volatile((self.base_address + UART_FR) as *const u32) } & UART_FR_RXFE)
            != 0
    }

    pub fn disable_uart(&self) {
        unsafe {
            // 1. まず送信が完全に終わる（BUSYが0になる）まで安全に待機する
            while (ptr::read_volatile((self.base_address + UART_FR) as *const u32) & _UART_FR_BUSY) != 0 {
                // busy loop
            }
            // 2. その後UARTを無効化
            ptr::write_volatile((self.base_address + UART_CR) as *mut u32, 0);
        }
        memory_barrier();
    }

    pub fn enable_uart(&self) {
        unsafe {
            ptr::write_volatile(
                (self.base_address + UART_CR) as *mut u32,
                UART_CR_RXE | UART_CR_TXE | UART_CR_UARTEN,
            );
        }
        memory_barrier();
    }

    pub fn enable_interrupt(&self) {
        self.enable_uart();
        unsafe {
            ptr::write_volatile(
                (self.base_address + _UART_IMSC) as *mut u32,
                ptr::read_volatile((self.base_address + _UART_IMSC) as *const u32) | _UART_IMSC_RXIM,
            );
        }
    }

    /// RPi4 (BCM2711) 向けの GPIO14, 15 初期化処理
    #[cfg(feature = "rpi4")]
    pub fn init_gpio(&self) {
        unsafe {
            // U-BootがデフォルトでGPIOをどのように設定しているかは分からないため、初期化が必要
            // GPIO 14 & 15 を ALT0 (PL011 RXD0/TXD0) に設定
            // GPFSEL1 は GPIO 10〜19 を制御 (1ピンあたり3ビット)
            // GPIO 14: bits 12-14, GPIO 15: bits 15-17
            // ALT0 の設定値は 0b100 (4)
            let mut gpfsel1 = ptr::read_volatile(GPFSEL1 as *const u32);
            gpfsel1 &= !((7 << 12) | (7 << 15)); // 一度マスク
            gpfsel1 |= (4 << 12) | (4 << 15);    // ALT0をセット
            ptr::write_volatile(GPFSEL1 as *mut u32, gpfsel1);

            // 内部プルアップ／プルダウンを無効化 (No Pull)
            // BCM2711では GPIO_PUP_PDN_CNTRL_REG0 (0xE4) を使用 (1ピンあたり2ビット)
            // GPIO 14: bits 28-29, GPIO 15: bits 30-31
            // 0b00 (0) = No Pull / Float
            // let mut pupd0 = ptr::read_volatile(GPIO_PUP_PDN_CNTRL_REG0 as *const u32);
            // pupd0 &= !((3 << 28) | (3 << 30)); // 00にクリア
            // ptr::write_volatile(GPIO_PUP_PDN_CNTRL_REG0 as *mut u32, pupd0);
        }
        memory_barrier();
    }

}

/// Serial構造体で使うために必要な実装
impl serial::SerialDevice for Pl011 {
    fn putc(&self, c: u8) -> Result<(), Error> {
        while self.is_tx_fifo_full() {
            core::hint::spin_loop();
        }
        unsafe { ptr::write_volatile((self.base_address + UART_DR) as *mut u32, c as u32) };
        Ok(())
    }

    fn getc(&self) -> Result<Option<u8>, Error> {
        if self.is_rx_fifo_empty() {
            return Ok(None);
        }
        let c = unsafe { ptr::read_volatile((self.base_address + UART_DR) as *const u32) };
        Ok(Some(c as u8))
    }
}
