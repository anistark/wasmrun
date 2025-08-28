//! Debug logging system for Wasmrun

use std::sync::atomic::{AtomicBool, Ordering};

/// Global debug flag that can be set via CLI
static DEBUG_ENABLED: AtomicBool = AtomicBool::new(false);

/// Enable debug logging
pub fn enable_debug() {
    DEBUG_ENABLED.store(true, Ordering::Relaxed);
    if DEBUG_ENABLED.load(Ordering::Relaxed) {
        eprintln!("🔍 \x1b[36mDEBUG\x1b[0m [debug.rs:11] Debug mode enabled for Wasmrun session");
    }
}

/// Check if debug logging is enabled
pub fn is_debug_enabled() -> bool {
    DEBUG_ENABLED.load(Ordering::Relaxed)
}

/// Debug print macro - only prints if debug is enabled
#[macro_export]
macro_rules! debug_println {
    ($($arg:tt)*) => {
        if $crate::debug::is_debug_enabled() {
            eprintln!("🔍 \x1b[36mDEBUG\x1b[0m [{}:{}] {}",
                file!().split('/').last().unwrap_or("unknown"),
                line!(),
                format_args!($($arg)*));
        }
    };
}

/// Trace-level debug - for very detailed debugging
#[macro_export]
macro_rules! trace_println {
    ($($arg:tt)*) => {
        if $crate::debug::is_debug_enabled() {
            eprintln!("🔬 \x1b[90mTRACE\x1b[0m [{}:{}] {}",
                file!().split('/').last().unwrap_or("unknown"),
                line!(),
                format_args!($($arg)*));
        }
    };
}

/// Debug function entry - logs when entering a function
#[macro_export]
macro_rules! debug_enter {
    ($func_name:expr) => {
        if $crate::debug::is_debug_enabled() {
            eprintln!("🚪 \x1b[32mENTER\x1b[0m [{}:{}] {}",
                file!().split('/').last().unwrap_or("unknown"),
                line!(),
                $func_name);
        }
    };
    ($func_name:expr, $($arg:tt)*) => {
        if $crate::debug::is_debug_enabled() {
            eprintln!("🚪 \x1b[32mENTER\x1b[0m [{}:{}] {} - {}",
                file!().split('/').last().unwrap_or("unknown"),
                line!(),
                $func_name,
                format_args!($($arg)*));
        }
    };
}

/// Debug function exit - logs when exiting a function
#[macro_export]
macro_rules! debug_exit {
    ($func_name:expr) => {
        if $crate::debug::is_debug_enabled() {
            eprintln!(
                "🚶 \x1b[33mEXIT\x1b[0m  [{}:{}] {}",
                file!().split('/').last().unwrap_or("unknown"),
                line!(),
                $func_name
            );
        }
    };
    ($func_name:expr, $result:expr) => {
        if $crate::debug::is_debug_enabled() {
            eprintln!(
                "🚶 \x1b[33mEXIT\x1b[0m  [{}:{}] {} -> {:?}",
                file!().split('/').last().unwrap_or("unknown"),
                line!(),
                $func_name,
                $result
            );
        }
    };
}

/// Debug timing - measure execution time
#[macro_export]
macro_rules! debug_time {
    ($name:expr, $block:block) => {{
        let start = std::time::Instant::now();
        let result = $block;
        if $crate::debug::is_debug_enabled() {
            eprintln!(
                "⏱️  \x1b[35mTIME\x1b[0m  [{}:{}] {} took {:?}",
                file!().split('/').last().unwrap_or("unknown"),
                line!(),
                $name,
                start.elapsed()
            );
        }
        result
    }};
}

/// Info print - always shown to users
#[macro_export]
macro_rules! info_println {
    ($($arg:tt)*) => {
        println!("{}", format_args!($($arg)*));
    };
}

/// Success print - always shown to users
#[macro_export]
macro_rules! success_println {
    ($($arg:tt)*) => {
        println!("✅ {}", format_args!($($arg)*));
    };
}

/// Warning print - always shown to users
#[macro_export]
macro_rules! warn_println {
    ($($arg:tt)*) => {
        eprintln!("⚠️  {}", format_args!($($arg)*));
    };
}

/// Error print - always shown to users
#[macro_export]
macro_rules! error_println {
    ($($arg:tt)*) => {
        eprintln!("❌ {}", format_args!($($arg)*));
    };
}
