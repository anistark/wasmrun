//! Pure WASI Rust example - no JavaScript bindings needed
//! These are simple, pure functions with no I/O
//!
//! Compile with: wasmrun compile examples/native-rust
//! Run with: wasmrun exec examples/native-rust/native_rust.wasm -c add 5 3

/// Add two numbers
#[no_mangle]
pub extern "C" fn add(a: i32, b: i32) -> i32 {
    a + b
}

/// Multiply two numbers
#[no_mangle]
pub extern "C" fn multiply(a: i32, b: i32) -> i32 {
    a * b
}

/// Calculate Fibonacci number
#[no_mangle]
pub extern "C" fn fibonacci(n: u32) -> u32 {
    match n {
        0 => 0,
        1 => 1,
        _ => fibonacci(n - 1) + fibonacci(n - 2),
    }
}

/// Power function
#[no_mangle]
pub extern "C" fn power(base: i32, exp: u32) -> i32 {
    let mut result = 1;
    for _ in 0..exp {
        result *= base;
    }
    result
}

/// Check if number is even
#[no_mangle]
pub extern "C" fn is_even(n: i32) -> i32 {
    if n % 2 == 0 { 1 } else { 0 }
}

/// Check if number is prime
#[no_mangle]
pub extern "C" fn is_prime(n: i32) -> i32 {
    if n < 2 {
        return 0;
    }
    if n == 2 {
        return 1;
    }
    if n % 2 == 0 {
        return 0;
    }

    let mut i = 3;
    while i * i <= n {
        if n % i == 0 {
            return 0;
        }
        i += 2;
    }
    1
}

fn main() {
    // Empty main - all functionality is in exported functions
}
