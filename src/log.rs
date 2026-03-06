use core::sync::atomic::Ordering;

use crate::LOG_LEVEL;

#[allow(unused)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum LogLevel {
    Error = 1,
    Warn = 2,
    Info = 3,
    Debug = 4,
}

#[inline]
pub fn set_log_level(level: LogLevel) {
    LOG_LEVEL.store(level as u8, Ordering::Relaxed);
}

#[inline]
pub fn log_enabled(level: LogLevel) -> bool {
    (level as u8) <= LOG_LEVEL.load(Ordering::Relaxed)
}

#[inline]
pub fn level_str(level: LogLevel) -> &'static str {
    match level {
        LogLevel::Error => "ERROR",
        LogLevel::Warn => "WARN",
        LogLevel::Info => "INFO",
        LogLevel::Debug => "DEBUG",
    }
}

#[doc(hidden)]
#[inline]
pub fn current_function_name(type_name: &'static str) -> &'static str {
    let without_probe = type_name
        .rsplit_once("::__log_fn_name_probe")
        .map(|(prefix, _)| prefix)
        .unwrap_or(type_name);
    let without_closure = without_probe.trim_end_matches("::{{closure}}");

    without_closure
        .rsplit("::")
        .next()
        .unwrap_or(without_closure)
}

#[doc(hidden)]
#[macro_export]
macro_rules! __log_current_component {
    () => {{
        fn __log_fn_name_probe() {}
        $crate::log::current_function_name(core::any::type_name_of_val(&__log_fn_name_probe))
    }};
}

#[macro_export]
macro_rules! log {
    ($level:expr, $component:expr, $($arg:tt)*) => {
        if $crate::log::log_enabled($level) {
            $crate::println!(
                "[{}] [{}] {}",
                $crate::log::level_str($level),
                $component,
                format_args!($($arg)*),
            );
        }
    };
}

#[macro_export]
macro_rules! log_error {
    ($component:expr, $fmt:literal $(, $arg:expr)* $(,)?) => {
        $crate::log!($crate::log::LogLevel::Error, $component, $fmt $(, $arg)*)
    };
    ($fmt:literal $(, $arg:expr)* $(,)?) => {
        $crate::log!(
            $crate::log::LogLevel::Error,
            $crate::__log_current_component!(),
            $fmt
            $(, $arg)*
        )
    };
}

#[macro_export]
macro_rules! log_warn {
    ($component:expr, $fmt:literal $(, $arg:expr)* $(,)?) => {
        $crate::log!($crate::log::LogLevel::Warn, $component, $fmt $(, $arg)*)
    };
    ($fmt:literal $(, $arg:expr)* $(,)?) => {
        $crate::log!(
            $crate::log::LogLevel::Warn,
            $crate::__log_current_component!(),
            $fmt
            $(, $arg)*
        )
    };
}

#[macro_export]
macro_rules! log_info {
    ($component:expr, $fmt:literal $(, $arg:expr)* $(,)?) => {
        $crate::log!($crate::log::LogLevel::Info, $component, $fmt $(, $arg)*)
    };
    ($fmt:literal $(, $arg:expr)* $(,)?) => {
        $crate::log!(
            $crate::log::LogLevel::Info,
            $crate::__log_current_component!(),
            $fmt
            $(, $arg)*
        )
    };
}

#[macro_export]
macro_rules! log_debug {
    ($component:expr, $fmt:literal $(, $arg:expr)* $(,)?) => {
        $crate::log!($crate::log::LogLevel::Debug, $component, $fmt $(, $arg)*)
    };
    ($fmt:literal $(, $arg:expr)* $(,)?) => {
        $crate::log!(
            $crate::log::LogLevel::Debug,
            $crate::__log_current_component!(),
            $fmt
            $(, $arg)*
        )
    };
}
