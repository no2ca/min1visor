#[cfg(test)]
use crate::{
    log::{self, LogLevel},
    mutex::Mutex,
};

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
