#![cfg(test)]
use crate::{
    allocator::linked_list::LinkedListAllocator,
    log::{self, LogLevel},
    mutex::Mutex,
};

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
