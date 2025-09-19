use leptos::*;
use leptos_meta::*;
use leptos_router::*;
use wasm_bindgen::prelude::*;
use web_sys::console;

#[wasm_bindgen]
extern "C" {
    fn alert(s: &str);
}

#[wasm_bindgen]
pub fn greet(name: &str) {
    console::log_1(&"[LEPTOS-WEB] greet() function called (running from web-leptos)".into());
    console::log_1(&format!("[LEPTOS-WEB] greet() called with name: {}", name).into());
    alert(&format!("Hello, {}!", name));
    console::log_1(&"[LEPTOS-WEB] greet() function completed".into());
}

#[component]
pub fn App() -> impl IntoView {
    console::log_1(&"[LEPTOS-WEB] App() component initializing (running from web-leptos)".into());
    provide_meta_context();

    view! {
        <Stylesheet id="leptos" href="/pkg/web-leptos.css"/>
        <Title text="Leptos WebAssembly App"/>
        <Router>
            <main>
                <Routes>
                    <Route path="" view=HomePage/>
                    <Route path="/counter" view=Counter/>
                    <Route path="/todo" view=TodoApp/>
                    <Route path="/*any" view=NotFound/>
                </Routes>
            </main>
        </Router>
    }
}

#[component]
fn HomePage() -> impl IntoView {
    console::log_1(&"[LEPTOS-WEB] HomePage() component initializing (running from web-leptos)".into());
    let (name, set_name) = create_signal(String::new());

    view! {
        <div class="container">
            <h1>"🦀 Leptos WebAssembly Application"</h1>
            
            <div class="hero-section">
                <p>"A reactive web application built with Rust and Leptos, compiled to WebAssembly"</p>
                
                <div class="greeting-section">
                    <h3>"👋 Interactive Greeting"</h3>
                    <input
                        type="text"
                        placeholder="Enter your name"
                        prop:value=name
                        on:input=move |ev| {
                            set_name(event_target_value(&ev));
                        }
                    />
                    <button on:click=move |_| {
                        let name_val = name.get();
                        if !name_val.is_empty() {
                            greet(&name_val);
                        } else {
                            greet("Anonymous");
                        }
                    }>
                        "Greet Me!"
                    </button>
                </div>
            </div>

            <nav class="navigation">
                <h3>"🚀 Explore Features"</h3>
                <div class="nav-buttons">
                    <A href="/counter" class="nav-button">
                        "🧮 Counter Demo"
                    </A>
                    <A href="/todo" class="nav-button">
                        "📋 Todo List"
                    </A>
                </div>
            </nav>

            <div class="info-section">
                <h3>"✨ Features Demonstrated"</h3>
                <ul>
                    <li>"Reactive state management with signals"</li>
                    <li>"Client-side routing with leptos_router"</li>
                    <li>"Interactive forms and user input"</li>
                    <li>"Component-based architecture"</li>
                    <li>"WebAssembly compilation and optimization"</li>
                    <li>"Browser API integration"</li>
                </ul>
            </div>
        </div>
    }
}

#[component]
fn Counter() -> impl IntoView {
    console::log_1(&"[LEPTOS-WEB] Counter() component initializing (running from web-leptos)".into());
    let (count, set_count) = create_signal(0);
    let (step, set_step) = create_signal(1);

    view! {
        <div class="container">
            <h1>"🧮 Counter Demo"</h1>
            <A href="/" class="back-link">"← Back to Home"</A>
            
            <div class="counter-section">
                <div class="counter-display">
                    <h2>"Current Count: " {move || count.get()}</h2>
                </div>
                
                <div class="counter-controls">
                    <label>
                        "Step Size: "
                        <input
                            type="number"
                            prop:value=step
                            on:input=move |ev| {
                                if let Ok(new_step) = event_target_value(&ev).parse::<i32>() {
                                    set_step(new_step);
                                }
                            }
                        />
                    </label>
                </div>

                <div class="button-group">
                    <button
                        class="decrement"
                        on:click=move |_| set_count.update(|n| *n -= step.get())
                    >
                        {move || format!("-{}", step.get())}
                    </button>
                    
                    <button
                        class="reset"
                        on:click=move |_| set_count(0)
                    >
                        "Reset"
                    </button>
                    
                    <button
                        class="increment"
                        on:click=move |_| set_count.update(|n| *n += step.get())
                    >
                        {move || format!("+{}", step.get())}
                    </button>
                </div>

                <div class="counter-info">
                    <p>"Count is " {move || if count.get() % 2 == 0 { "even" } else { "odd" }}</p>
                    <p>"Fibonacci-like: " {move || fibonacci_like(count.get())}</p>
                </div>
            </div>
        </div>
    }
}

#[derive(Clone, Debug)]
struct TodoItem {
    id: usize,
    text: String,
    completed: bool,
}

#[component]
fn TodoApp() -> impl IntoView {
    console::log_1(&"[LEPTOS-WEB] TodoApp() component initializing (running from web-leptos)".into());
    let (todos, set_todos) = create_signal(Vec::<TodoItem>::new());
    let (new_todo, set_new_todo) = create_signal(String::new());
    let (next_id, set_next_id) = create_signal(1);

    let add_todo = move |_| {
        console::log_1(&"[LEPTOS-WEB] add_todo() function called (running from web-leptos)".into());
        let text = new_todo.get().trim().to_string();
        if !text.is_empty() {
            console::log_1(&format!("[LEPTOS-WEB] add_todo() adding todo: {}", text).into());
            let todo = TodoItem {
                id: next_id.get(),
                text,
                completed: false,
            };
            set_todos.update(|todos| todos.push(todo));
            set_next_id.update(|id| *id += 1);
            set_new_todo(String::new());
            console::log_1(&"[LEPTOS-WEB] add_todo() todo added successfully".into());
        } else {
            console::log_1(&"[LEPTOS-WEB] add_todo() empty text, not adding".into());
        }
    };

    let toggle_todo = move |id: usize| {
        console::log_1(&format!("[LEPTOS-WEB] toggle_todo() called for id: {} (running from web-leptos)", id).into());
        set_todos.update(|todos| {
            if let Some(todo) = todos.iter_mut().find(|t| t.id == id) {
                todo.completed = !todo.completed;
                console::log_1(&format!("[LEPTOS-WEB] toggle_todo() toggled todo {} to {}", id, todo.completed).into());
            }
        });
    };

    let delete_todo = move |id: usize| {
        console::log_1(&format!("[LEPTOS-WEB] delete_todo() called for id: {} (running from web-leptos)", id).into());
        set_todos.update(|todos| todos.retain(|t| t.id != id));
        console::log_1(&format!("[LEPTOS-WEB] delete_todo() deleted todo {}", id).into());
    };

    let completed_count = move || todos.with(|todos| todos.iter().filter(|t| t.completed).count());
    let total_count = move || todos.with(|todos| todos.len());

    view! {
        <div class="container">
            <h1>"📋 Todo List"</h1>
            <A href="/" class="back-link">"← Back to Home"</A>
            
            <div class="todo-section">
                <div class="todo-input">
                    <input
                        type="text"
                        placeholder="Add a new todo..."
                        prop:value=new_todo
                        on:input=move |ev| {
                            set_new_todo(event_target_value(&ev));
                        }
                        on:keydown=move |ev| {
                            if ev.key() == "Enter" {
                                add_todo(ev);
                            }
                        }
                    />
                    <button on:click=add_todo>"Add Todo"</button>
                </div>

                <div class="todo-stats">
                    <p>"Total: " {total_count} " | Completed: " {completed_count}</p>
                </div>

                <div class="todo-list">
                    <For
                        each=move || todos.get()
                        key=|todo| todo.id
                        children=move |todo: TodoItem| {
                            let id = todo.id;
                            view! {
                                <div class=("todo-item", move || if todo.completed { "completed" } else { "" })>
                                    <input
                                        type="checkbox"
                                        prop:checked=todo.completed
                                        on:change=move |_| toggle_todo(id)
                                    />
                                    <span class="todo-text">{&todo.text}</span>
                                    <button
                                        class="delete-btn"
                                        on:click=move |_| delete_todo(id)
                                    >
                                        "❌"
                                    </button>
                                </div>
                            }
                        }
                    />
                </div>

                {move || if todos.with(|todos| todos.is_empty()) {
                    view! {
                        <div class="empty-state">
                            <p>"No todos yet. Add one above! 👆"</p>
                        </div>
                    }.into_view()
                } else {
                    view! { <div></div> }.into_view()
                }}
            </div>
        </div>
    }
}

#[component]
fn NotFound() -> impl IntoView {
    let params = use_params_map();
    let path = move || params.with(|params| params.get("any").cloned().unwrap_or_default());

    view! {
        <div class="container">
            <h1>"404 - Page Not Found"</h1>
            <p>"The page '" {path} "' was not found."</p>
            <A href="/" class="nav-button">"Go Home"</A>
        </div>
    }
}

fn fibonacci_like(n: i32) -> i32 {
    console::log_1(&format!("[LEPTOS-WEB] fibonacci_like() called with n: {} (running from web-leptos)", n).into());
    if n <= 0 {
        console::log_1(&"[LEPTOS-WEB] fibonacci_like() n<=0, returning 0".into());
        return 0;
    }
    if n <= 2 {
        console::log_1(&"[LEPTOS-WEB] fibonacci_like() n<=2, returning 1".into());
        return 1;
    }

    console::log_1(&format!("[LEPTOS-WEB] fibonacci_like() calculating for n={}", n).into());
    let mut a = 0;
    let mut b = 1;
    for i in 2..=n {
        let temp = a + b;
        a = b;
        b = temp;
        if i <= 5 { // Only log first few iterations to avoid spam
            console::log_1(&format!("[LEPTOS-WEB] fibonacci_like() iteration {}: result={}", i, b).into());
        }
    }
    console::log_1(&format!("[LEPTOS-WEB] fibonacci_like() final result: {}", b).into());
    b
}

#[wasm_bindgen(start)]
pub fn main() {
    console_error_panic_hook::set_once();
    console::log_1(&"[LEPTOS-WEB] ===== Leptos WebAssembly Example Starting =====".into());
    console::log_1(&"[LEPTOS-WEB] Initializing Leptos-Web example module".into());
    console::log_1(&"[LEPTOS-WEB] Module: Leptos-Web Example (web-leptos/src/lib.rs)".into());
    console::log_1(&"Leptos WebAssembly app starting...".into());
    console::log_1(&"[LEPTOS-WEB] Mounting App component to body".into());
    leptos::mount_to_body(App);
    console::log_1(&"[LEPTOS-WEB] ===== Leptos WebAssembly Example Ready =====".into());
}