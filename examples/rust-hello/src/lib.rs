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
    let greeting = format!("Hello, {}! 👋 This message is from Rust and WebAssembly.", name);
    console_log!("Rust says: {}", greeting);
    greeting
}

// Export a `fibonacci` function
#[wasm_bindgen]
pub fn fibonacci(n: u32) -> u32 {
    console_log!("Calculating fibonacci({}) in Rust...", n);

    if n <= 1 {
        n
    } else {
        fibonacci(n - 1) + fibonacci(n - 2)
    }
}

// Export a `sum_array` function that works with JavaScript arrays
#[wasm_bindgen]
pub fn sum_array(numbers: &[i32]) -> i32 {
    console_log!("Summing array of {} numbers in Rust...", numbers.len());
    numbers.iter().sum()
}

// Export a simple addition function
#[wasm_bindgen]
pub fn add(a: i32, b: i32) -> i32 {
    console_log!("Adding {} + {} in Rust", a, b);
    a + b
}

// Export a function that demonstrates memory usage
#[wasm_bindgen]
pub fn get_memory_size() -> usize {
    console_log!("Getting WebAssembly memory information...");
    // This is a simple demonstration - actual memory introspection would be more complex
    std::mem::size_of::<usize>()
}

// Called when the WebAssembly module is instantiated
#[wasm_bindgen(start)]
pub fn main() {
    console_log!("🦀 Rust and WebAssembly module loaded successfully!");
}