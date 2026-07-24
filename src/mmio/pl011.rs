//!
//! PL011 の MMIO Driver
//!

#[cfg(feature = "qemu-virt")]
use crate::mmio::gicv3::GicDistributorMmio;
use crate::print;

use crate::vm::MmioHandler;

const UART_DR: usize = 0x000;
const UART_FR: usize = 0x018;
const UART_CR: usize = 0x030;
const UART_IMSC: usize = 0x038;
const UART_RIS: usize = 0x03C;
const UART_MIS: usize = 0x040;
const UART_ICR: usize = 0x044;
const UART_PERIPH_ID0: usize = 0xFE0;
const UART_PERIPH_ID1: usize = 0xFE4;
const UART_PERIPH_ID2: usize = 0xFE8;
const UART_PERIPH_ID3: usize = 0xFEC;
const UART_PCELL_ID0: usize = 0xFF0;
const UART_PCELL_ID1: usize = 0xFF4;
const UART_PCELL_ID2: usize = 0xFF8;
const UART_PCELL_ID3: usize = 0xFFC;

/// RX FIFO が空か示すビット
const UART_FR_RXFE: u16 = 1 << 4;
/// 受信割り込みが有効か示すビット
const UART_IMSC_RXIM: u16 = 1 << 4;
/// 受信割り込みが起きた事を示すビット
const UART_RIS_RXRIS: u16 = 1 << 4;
/// PL011の仮想割り込み番号
pub const PL011_INT_ID: u32 = 33;

pub struct Pl011Mmio {
    flag: u16,
    interrupt_mask: u16,
    raw_interrupt_status: u16,
    control: u16,
    read_buffer: [u8; 4],
}

impl Pl011Mmio {
    pub fn new() -> Self {
        Self {
            flag: 0,
            interrupt_mask: 0,
            raw_interrupt_status: 0,
            control: 0,
            read_buffer: [0; 4],
        }
    }

    #[cfg(feature = "qemu-virt")]
    pub fn push(&mut self, data: u8, distributor: &mut GicDistributorMmio) {
        for c in &mut self.read_buffer {
            if *c == 0 {
                *c = data;
                break;
            }
        }
        self.flag &= !(UART_FR_RXFE);
        if (self.interrupt_mask & UART_IMSC_RXIM) != 0 {
            self.raw_interrupt_status |= UART_RIS_RXRIS;
            distributor.trigger_interrupt(PL011_INT_ID, None);
        }
    }

    #[cfg(feature = "rpi4")]
    pub fn push(&mut self, data: u8) {
        for c in &mut self.read_buffer {
            if *c == 0 {
                *c = data;
                break;
            }
        }

        // RX FIFOが空ではないことを知らせる
        self.flag &= !(UART_FR_RXFE);
        
        // 受信割り込みが有効な場合
        if (self.interrupt_mask & UART_IMSC_RXIM) != 0 {
            // 受信割り込みが起こったことを通知する
            self.raw_interrupt_status |= UART_RIS_RXRIS;
            crate::mmio::gicv2::inject_interrupt(PL011_INT_ID);
        }
    }
    
    pub fn is_rx_pending(&mut self) -> bool {
        let ris = self.read(UART_RIS, 64).unwrap_or(0);
        let imsc =  self.read(UART_IMSC, 64).unwrap_or(0);
        ris & imsc != 0
    }
}

impl MmioHandler for Pl011Mmio {
    fn read(&mut self, offset: usize, _access_width: u64) -> Result<u64, ()> {
        let value: u64;
        match offset {
            UART_DR => {
                value = self.read_buffer[0] as u64;
                for i in 1..(self.read_buffer.len()) {
                    // キューを後ろに詰める
                    self.read_buffer[i - 1] = self.read_buffer[i];
                }
                if self.read_buffer[0] == 0 {
                    // キューが空のときのフラグ
                    self.flag |= UART_FR_RXFE;
                    self.raw_interrupt_status &= !(UART_RIS_RXRIS);
                }
            }
            UART_FR => {
                value = self.flag as u64;
            }
            UART_CR => {
                value = self.control as u64;
            }
            UART_IMSC => {
                value = self.interrupt_mask as u64;
            }
            UART_RIS => {
                value = self.raw_interrupt_status as u64;
            }
            UART_MIS => {
                value = (self.raw_interrupt_status & self.interrupt_mask) as u64;
            }
            UART_PERIPH_ID0 => {
                value = 0x11;
            }
            UART_PERIPH_ID1 => {
                value = 0x01 << 4;
            }
            UART_PERIPH_ID2 => {
                value = (0x03 << 4) | 0x04;
            }
            UART_PERIPH_ID3 => {
                value = 0x00;
            }
            UART_PCELL_ID0 => {
                value = 0x0D;
            }
            UART_PCELL_ID1 => {
                value = 0xF0;
            }
            UART_PCELL_ID2 => {
                value = 0x05;
            }
            UART_PCELL_ID3 => {
                value = 0xB1;
            }
            _ => {
                value = 0x00; /* uninplemented */
            }
        }
        Ok(value)
    }

    fn write(&mut self, offset: usize, _access_width: u64, value: u64) -> Result<(), ()> {
        match offset {
            UART_DR => {
                print!("{}", value as u8 as char);
            }
            UART_CR => {
                self.control = value as u16;
            }
            UART_IMSC => {
                self.interrupt_mask = value as u16;
            }
            UART_ICR => {
                self.raw_interrupt_status &= !(value as u16);
            }
            _ => { /* unimplemented */ }
        }
        Ok(())
    }
}
