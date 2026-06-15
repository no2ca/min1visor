//!
//! Mailbox Property Interface for BCM2711 (RPi4)
//!
//! VPU ファームウェアにプロパティ問い合わせを行うためのインタフェース。
//! クロックレートの取得・設定などに使用する。
//!

use core::ptr;

const MAILBOX_BASE: usize = 0xFE00B880;
const MAILBOX_READ: usize = MAILBOX_BASE + 0x00;
const MAILBOX_STATUS: usize = MAILBOX_BASE + 0x18;
const MAILBOX_WRITE: usize = MAILBOX_BASE + 0x20;

const STATUS_FULL: u32 = 1 << 31;
const STATUS_EMPTY: u32 = 1 << 30;

const CHANNEL_PROPERTY: u32 = 8;

const TAG_GET_CLOCK_RATE: u32 = 0x00030002;
const TAG_SET_CLOCK_RATE: u32 = 0x00038002;
const TAG_END: u32 = 0;

const REQUEST_CODE: u32 = 0x00000000;
const RESPONSE_SUCCESS: u32 = 1 << 31;

pub const CLOCK_UART: u32 = 2;

#[repr(C, align(16))]
struct PropertyBuffer {
    size: u32,
    code: u32,
    tag_id: u32,
    tag_size: u32,
    tag_code: u32,
    clock_id: u32,
    clock_rate: u32,
    end_tag: u32,
}

#[repr(C, align(16))]
struct SetClockRateBuffer {
    size: u32,
    code: u32,
    tag_id: u32,
    tag_size: u32,
    tag_code: u32,
    clock_id: u32,
    rate: u32,
    skip_turbo: u32,
    end_tag: u32,
}

pub fn set_clock_rate(clock_id: u32, rate_hz: u32) {
    let mut buf = SetClockRateBuffer {
        size: core::mem::size_of::<SetClockRateBuffer>() as u32,
        code: REQUEST_CODE,
        tag_id: TAG_SET_CLOCK_RATE,
        tag_size: 12,
        tag_code: 0,
        clock_id,
        rate: rate_hz,
        skip_turbo: 0,
        end_tag: TAG_END,
    };

    let buf_addr = &mut buf as *mut SetClockRateBuffer as u32;
    mbox_write((buf_addr & !0x0F) | CHANNEL_PROPERTY);

    // Fire-and-forget: drain response to clear channel
    loop {
        let resp = mbox_read();
        if resp & 0x0F == CHANNEL_PROPERTY {
            break;
        }
    }
}

pub fn get_clock_rate(clock_id: u32) -> Option<u32> {
    let mut buf = PropertyBuffer {
        size: core::mem::size_of::<PropertyBuffer>() as u32,
        code: REQUEST_CODE,
        tag_id: TAG_GET_CLOCK_RATE,
        tag_size: 8,
        tag_code: 0,
        clock_id,
        clock_rate: 0,
        end_tag: TAG_END,
    };

    let buf_addr = &mut buf as *mut PropertyBuffer as u32;
    // RPi4 Low Peripheral mode: VC bus address == ARM physical address for DRAM
    // If mailbox does not respond, try buf_addr | 0x40000000
    mbox_write((buf_addr & !0x0F) | CHANNEL_PROPERTY);

    loop {
        let resp = mbox_read();
        if resp & 0x0F == CHANNEL_PROPERTY {
            break;
        }
    }

    if buf.code & RESPONSE_SUCCESS == 0 {
        return None;
    }

    Some(buf.clock_rate)
}

fn mbox_read() -> u32 {
    while (unsafe { ptr::read_volatile(MAILBOX_STATUS as *const u32) } & STATUS_EMPTY) != 0 {
        core::hint::spin_loop();
    }
    unsafe { ptr::read_volatile(MAILBOX_READ as *const u32) }
}

fn mbox_write(data: u32) {
    while (unsafe { ptr::read_volatile(MAILBOX_STATUS as *const u32) } & STATUS_FULL) != 0 {
        core::hint::spin_loop();
    }
    unsafe { ptr::write_volatile(MAILBOX_WRITE as *mut u32, data) };
}
