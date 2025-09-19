use wasm_bindgen::prelude::*;

// Import the `console.log` function from the browser
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

// Define a macro for easier console logging
macro_rules! console_log {
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

// Export a `greet` function from Rust to JavaScript
#[wasm_bindgen]
pub fn greet(name: &str) -> String {
    console_log!("[RUST-HELLO] greet() function called with name: {} (running from rust-hello)", name);
    let greeting = format!("Hello, {}! 👋 This message is from Rust and WebAssembly.", name);
    console_log!("[RUST-HELLO] greet() generated greeting: {} (running from rust-hello)", greeting);
    console_log!("Rust says: {} (running from rust-hello)", greeting);
    console_log!("[RUST-HELLO] greet() function completed");
    greeting
}

// Export a `fibonacci` function
#[wasm_bindgen]
pub fn fibonacci(n: u32) -> u32 {
    console_log!("[RUST-HELLO] fibonacci() function called with n: {} (running from rust-hello)", n);
    console_log!("Calculating fibonacci({}) in Rust... (running from rust-hello)", n);

    if n <= 1 {
        console_log!("[RUST-HELLO] fibonacci() base case reached, returning {}", n);
        n
    } else {
        console_log!("[RUST-HELLO] fibonacci() calculating recursively for n={}", n);
        let result = fibonacci(n - 1) + fibonacci(n - 2);
        console_log!("[RUST-HELLO] fibonacci({}) calculated result: {}", n, result);
        result
    }
}

// Export a `sum_array` function that works with JavaScript arrays
#[wasm_bindgen]
pub fn sum_array(numbers: &[i32]) -> i32 {
    console_log!("[RUST-HELLO] sum_array() function called with {} numbers (running from rust-hello)", numbers.len());
    console_log!("Summing array of {} numbers in Rust... (running from rust-hello)", numbers.len());
    console_log!("[RUST-HELLO] sum_array() processing array: {:?}", numbers);
    let result = numbers.iter().sum();
    console_log!("[RUST-HELLO] sum_array() calculated sum: {}", result);
    console_log!("[RUST-HELLO] sum_array() function completed");
    result
}

// Export a simple addition function
#[wasm_bindgen]
pub fn add(a: i32, b: i32) -> i32 {
    console_log!("[RUST-HELLO] add() function called with a={}, b={} (running from rust-hello)", a, b);
    console_log!("Adding {} + {} in Rust (running from rust-hello)", a, b);
    let result = a + b;
    console_log!("[RUST-HELLO] add() calculated result: {}", result);
    console_log!("[RUST-HELLO] add() function completed");
    result
}

// Export a function that demonstrates memory usage
#[wasm_bindgen]
pub fn get_memory_size() -> usize {
    console_log!("[RUST-HELLO] get_memory_size() function called (running from rust-hello)");
    console_log!("Getting WebAssembly memory information... (running from rust-hello)");
    console_log!("[RUST-HELLO] get_memory_size() calculating memory size");
    // This is a simple demonstration - actual memory introspection would be more complex
    let size = std::mem::size_of::<usize>();
    console_log!("[RUST-HELLO] get_memory_size() calculated size: {} bytes", size);
    console_log!("[RUST-HELLO] get_memory_size() function completed");
    size
}

// Called when the WebAssembly module is instantiated
#[wasm_bindgen(start)]
pub fn main() {
    console_log!("[RUST-HELLO] ===== Rust WebAssembly Example Starting =====");
    console_log!("[RUST-HELLO] Initializing Rust-Hello example module");
    console_log!("[RUST-HELLO] Module: Rust-Hello Example (rust-hello/src/lib.rs)");
    console_log!("🦀 Rust and WebAssembly module loaded successfully!");
    console_log!("[RUST-HELLO] Available functions: greet, fibonacci, sum_array, add, get_memory_size");
    console_log!("[RUST-HELLO] ===== Rust WebAssembly Example Ready =====");
}