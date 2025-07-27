//! Debug logging system for Wasmrun

use std::sync::atomic::{AtomicBool, Ordering};

/// Global debug flag that can be set via CLI
static DEBUG_ENABLED: AtomicBool = AtomicBool::new(false);

/// Enable debug logging
pub fn enable_debug() {
    DEBUG_ENABLED.store(true, Ordering::Relaxed);
}

/// Check if debug logging is enabled
#[allow(dead_code)]
pub fn is_debug_enabled() -> bool {
    DEBUG_ENABLED.load(Ordering::Relaxed)
}

/// Debug print macro - only prints if debug is enabled
#[macro_export]
macro_rules! debug_println {
    ($($arg:tt)*) => {
        if $crate::debug::is_debug_enabled() {
            println!("🔍 Debug: {}", format_args!($($arg)*));
        }
    };
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
