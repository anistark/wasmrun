# Leptos WebAssembly Web Application

This example demonstrates a complete reactive web application built with Rust Leptos framework and compiled to WebAssembly, featuring modern UI components and client-side routing.

## Features

- **Reactive Components**: Counter, todo list, and interactive forms
- **Client-Side Routing**: Multi-page navigation with leptos_router
- **State Management**: Reactive signals and derived state
- **Interactive UI**: Real-time updates and form handling
- **Modern Design**: Responsive glassmorphism design
- **WebAssembly Integration**: Full Rust-to-WASM compilation
- **Browser APIs**: Direct interaction with web APIs

## Build and Run

From the wasmrun project root:

```sh
# Run with wasmrun
wasmrun run examples/web-leptos

# Or compile manually
wasmrun compile examples/web-leptos
```

### Manual Setup (Optional)

If you want to build manually with wasm-pack:

```sh
cd examples/web-leptos
wasm-pack build --target web --out-dir pkg
# Then serve the directory with any static file server
```

## Application Features

### Homepage
- Interactive greeting with WebAssembly function calls
- Navigation to different application sections
- Feature overview and documentation

### Counter Demo (`/counter`)
- Reactive counter with customizable step size
- Mathematical calculations (fibonacci-like sequences)
- Real-time state updates and derived values

### Todo List (`/todo`)
- Add, toggle, and delete todo items
- Real-time statistics (total/completed counts)
- Local state management with Leptos signals

## Functions Available

The application exports these WebAssembly functions:

- `greet(name: &str)` - Display browser alert with greeting
- `main()` - Initialize the Leptos application

## Usage Patterns

```rust
// Reactive signals
let (count, set_count) = create_signal(0);

// Event handlers
on:click=move |_| set_count.update(|n| *n + 1)

// Derived state
let is_even = move || count.get() % 2 == 0;

// Component composition
<Counter initial_value=0 />
```

## Requirements

- Rust with WebAssembly target
- Leptos framework and dependencies
- Modern web browser with WebAssembly support

## Notes

This example showcases Leptos's fine-grained reactivity system, demonstrating how Rust can be used to build complex, interactive web applications that compile to efficient WebAssembly code. The application provides excellent performance while maintaining type safety and modern development patterns.