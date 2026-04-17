#![cfg(test)]
use crate::{
    allocator::linked_list::LinkedListAllocator,
    drivers::{
        gicv3::GicDistributor,
        pl011::Pl011,
        virtio::{
            NUMBER_OF_DESCRIPTORS, NUMBER_OF_PAGES_QUEUE, VIRTIO_MMIO_DEVICE_ID, VIRTIO_MMIO_MAGIC,
            VIRTIO_MMIO_MAGIC_VALUE, VIRTIO_MMIO_QUEUE_NOTIFY, VIRTIO_MMIO_STATUS,
            VIRTIO_PAGE_SIZE, VirtQueueAvail, VirtQueueDesc, VirtQueueUsed,
        },
        virtio_blk::{
            VIRTIO_BLK_S_IOERR, VIRTIO_BLK_S_OK, VIRTIO_BLK_TYPE_IN, VIRTIO_BLK_TYPE_OUT,
            VirtioBlkReq,
        },
    },
    log::{self, LogLevel},
    mutex::Mutex,
};
use core::mem::{offset_of, size_of};

const TEST_HEAP_START: usize = 0x5000_0000;
const TEST_HEAP_SIZE: usize = 0x1000_0000;

fn make_allocator_with_test_heap() -> LinkedListAllocator {
    let mut allocator = LinkedListAllocator::new();
    unsafe {
        allocator.init(TEST_HEAP_START, TEST_HEAP_SIZE);
    }
    allocator
}

#[test_case]
fn mutex_lock_mutates_and_persists_value() {
    static TEST_MUTEX: Mutex<u32> = Mutex::new(0);

    {
        let mut guard = TEST_MUTEX.lock();
        *guard = 42;
    }

    let guard = TEST_MUTEX.lock();
    assert_eq!(*guard, 42);
}

#[test_case]
fn mutex_unlocks_on_guard_drop() {
    static TEST_MUTEX: Mutex<u32> = Mutex::new(10);

    let before_drop = {
        let mut guard = TEST_MUTEX.lock();
        *guard += 5;
        *guard
    };
    assert_eq!(before_drop, 15);

    let guard = TEST_MUTEX.lock();
    assert_eq!(*guard, 15);
}

#[test_case]
fn mutex_guard_deref_handles_composite_data() {
    static TEST_MUTEX: Mutex<[u32; 3]> = Mutex::new([1, 2, 3]);

    {
        let mut guard = TEST_MUTEX.lock();
        guard[0] += 10;
        guard[2] *= 2;
    }

    let guard = TEST_MUTEX.lock();
    assert_eq!(*guard, [11, 2, 6]);
}

#[test_case]
fn log_level_filtering_works() {
    log::set_log_level(LogLevel::Warn);
    assert!(log::log_enabled(LogLevel::Error));
    assert!(log::log_enabled(LogLevel::Warn));
    assert!(!log::log_enabled(LogLevel::Info));
    assert!(!log::log_enabled(LogLevel::Debug));

    log::set_log_level(LogLevel::Debug);
    assert!(log::log_enabled(LogLevel::Error));
    assert!(log::log_enabled(LogLevel::Warn));
    assert!(log::log_enabled(LogLevel::Info));
    assert!(log::log_enabled(LogLevel::Debug));
}

#[test_case]
fn log_level_threshold_matrix_is_consistent() {
    const LEVELS: [LogLevel; 4] = [
        LogLevel::Error,
        LogLevel::Warn,
        LogLevel::Info,
        LogLevel::Debug,
    ];

    for active in LEVELS {
        log::set_log_level(active);
        for candidate in LEVELS {
            assert_eq!(
                log::log_enabled(candidate),
                (candidate as u8) <= (active as u8)
            );
        }
    }
}

#[test_case]
fn log_level_strings_are_stable() {
    assert_eq!(log::level_str(LogLevel::Error), "ERROR");
    assert_eq!(log::level_str(LogLevel::Warn), "WARN");
    assert_eq!(log::level_str(LogLevel::Info), "INFO");
    assert_eq!(log::level_str(LogLevel::Debug), "DEBUG");
}

#[test_case]
fn log_current_component_defaults_to_function_name() {
    assert_eq!(
        crate::__log_current_component!(),
        "log_current_component_defaults_to_function_name"
    );
}

#[test_case]
fn log_current_component_ignores_closure_suffix() {
    let current = (|| crate::__log_current_component!())();
    assert_eq!(current, "log_current_component_ignores_closure_suffix");
}

#[test_case]
fn linked_list_alloc_returns_heap_start_for_first_fit() {
    let mut allocator = make_allocator_with_test_heap();

    let ptr = unsafe { allocator.alloc(0x10, 0x10) };
    assert_eq!(ptr as usize, TEST_HEAP_START);
}

#[test_case]
fn linked_list_alloc_preserves_alignment() {
    let mut allocator = make_allocator_with_test_heap();

    let ptr = unsafe { allocator.alloc(0x20, 0x1000) };
    assert_eq!((ptr as usize) % 0x1000, 0);
    assert_eq!(ptr as usize, TEST_HEAP_START);
}

#[test_case]
fn linked_list_alloc_exhausts_range_and_then_fails() {
    let mut allocator = make_allocator_with_test_heap();

    let whole = unsafe { allocator.alloc(TEST_HEAP_SIZE, 0x10) };
    assert_eq!(whole as usize, TEST_HEAP_START);

    let next = unsafe { allocator.alloc(0x10, 0x10) };
    assert!(next.is_null());
}

#[test_case]
fn linked_list_dealloc_makes_region_reusable() {
    let mut allocator = make_allocator_with_test_heap();

    let ptr = unsafe { allocator.alloc(0x1000, 0x1000) };
    assert_eq!(ptr as usize, TEST_HEAP_START);

    unsafe {
        allocator.dealloc(ptr, 0x1000);
    }

    let reused = unsafe { allocator.alloc(0x1000, 0x1000) };
    assert_eq!(reused as usize, TEST_HEAP_START);
}

#[test_case]
fn virtio_mmio_register_offsets_match_expected_values() {
    assert_eq!(VIRTIO_MMIO_MAGIC, 0x000);
    assert_eq!(VIRTIO_MMIO_DEVICE_ID, 0x008);
    assert_eq!(VIRTIO_MMIO_STATUS, 0x070);
    assert_eq!(VIRTIO_MMIO_QUEUE_NOTIFY, 0x050);
    assert_eq!(VIRTIO_MMIO_MAGIC_VALUE, 0x7472_6976);
}

#[test_case]
fn virtio_queue_layout_fits_in_reserved_pages() {
    let descriptor_table = 0usize;
    let available_ring = descriptor_table + size_of::<VirtQueueDesc>() * NUMBER_OF_DESCRIPTORS;
    let used_ring = ((available_ring + size_of::<VirtQueueAvail>() - 1) & !(VIRTIO_PAGE_SIZE - 1))
        + VIRTIO_PAGE_SIZE;
    let queue_bytes = NUMBER_OF_PAGES_QUEUE * VIRTIO_PAGE_SIZE;

    assert_eq!(used_ring % VIRTIO_PAGE_SIZE, 0);
    assert!(used_ring > available_ring);
    assert!(used_ring + size_of::<VirtQueueUsed>() <= queue_bytes);
}

#[test_case]
fn virtio_blk_request_layout_is_stable() {
    assert_eq!(size_of::<VirtioBlkReq>(), 16);
    assert_eq!(offset_of!(VirtioBlkReq, req_type), 0);
    assert_eq!(offset_of!(VirtioBlkReq, reserved), 4);
    assert_eq!(offset_of!(VirtioBlkReq, sector), 8);
}

#[test_case]
fn virtio_blk_constants_follow_expected_protocol_values() {
    assert_eq!(VIRTIO_BLK_TYPE_IN, 0);
    assert_eq!(VIRTIO_BLK_TYPE_OUT, 1);
    assert_eq!(VIRTIO_BLK_S_OK, 0);
    assert_eq!(VIRTIO_BLK_S_IOERR, 1);
}

#[test_case]
fn pl011_new_rejects_too_small_range_and_accepts_valid_range() {
    assert!(Pl011::new(0x0900_0000, 0x0FFF, 33).is_err());

    let pl011 = Pl011::new(0x0900_0000, 0x1000, 33).expect("pl011 init should succeed");
    assert_eq!(pl011.interrupt_number, 33);
}

#[test_case]
fn gic_distributor_new_requires_exact_mmio_size() {
    assert!(GicDistributor::new(0x0800_0000, 0xFFFF).is_err());
    assert!(GicDistributor::new(0x0800_0000, 0x10000).is_ok());
}
