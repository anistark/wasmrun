# 🚀 Feature Request: OS Mode - Multi-Language Micro-Kernel

## 📋 Current Reality vs Target Vision (Last Updated: 2026-02-16)

### 🎯 **TARGET VISION: WebContainer-Style Browser Execution**
```
Browser
└─> wasmrun Kernel (WASM) ❌ NOT IMPLEMENTED
    └─> Load Node.js Runtime (WASM) ❌ NOT IMPLEMENTED
        └─> Execute User Project (in WASM VM) ❌ NOT IMPLEMENTED
            └─> Bore Tunnel (in WASM) ❌ NOT IMPLEMENTED
                └─> Public Access ❌ NOT IMPLEMENTED
```

### ⚠️ **CURRENT REALITY: 100% Server-Side Simulation**
```
Local Machine (Everything runs here)
├─> wasmrun OS Server (Native Rust) ✅ WORKING
│   ├─> MultiLanguageKernel (Native Rust) ✅ WORKING (orchestration only)
│   │   ├─> WASI Filesystem (wasi_fs.rs) ✅ Mounts host dirs
│   │   ├─> In-Memory VFS (microkernel.rs) ⚠️ DISCONNECTED from WASI FS
│   │   ├─> Process Manager ✅ BOOKKEEPING ONLY (no execution)
│   │   ├─> Scheduler ⚠️ DATA STRUCTURE ONLY (never invoked)
│   │   ├─> Placeholder WASM (Empty) ⚠️ FAKE
│   ├─> Dev Server Manager (Native Rust Threads) ⚠️ BROKEN (reads wrong FS)
│   ├─> BoreClient (Native std::net::TcpStream) ✅ WORKING ON HOST
│   ├─> Socket Syscalls ⚠️ UNSANDBOXED (real host sockets)
│   ├─> Network Namespace ⚠️ BOOKKEEPING ONLY (no real isolation)
│   └─> HTTP API ⚠️ NO AUTH, CORS: * on everything
└─> Browser
    ├─> React UI (Dashboard only) ✅ WORKING
    └─> NO WASM EXECUTION ❌ NOT IMPLEMENTED
```

**CRITICAL:** Everything currently runs on the **local machine in native Rust**. The browser is just a pretty dashboard showing server stats via REST API. **NO code runs in the browser's WASM VM yet.**

**ALSO CRITICAL:** Even the server-side implementation is largely a simulation — processes are created but never executed, the scheduler runs but is never invoked, and there are two disconnected filesystems that don't talk to each other.

---

## 📊 Detailed Status Breakdown

### ✅ What's Actually Working (Server-Side Only)

**All these components run on your LOCAL MACHINE, not in the browser:**

| Component | Location | File | Status |
|-----------|----------|------|--------|
| **OS Server** | Local Rust process | `src/runtime/os_server.rs` | ✅ Functional (see security caveats below) |
| **WASI Filesystem** | Local Rust | `src/runtime/wasi_fs.rs` | ✅ Mount/file ops working (used by HTTP API) |
| **In-Memory VFS** | Local Rust | `src/runtime/microkernel.rs` | ✅ **UNIFIED** — removed HashMap, now delegates to WASI FS |
| **Multi-Language Kernel** | Local Rust | `src/runtime/multilang_kernel.rs` | ✅ Orchestration/bookkeeping working |
| **Process Scheduler** | Local Rust | `src/runtime/scheduler.rs` | ⚠️ Data structure only — `schedule_next()` never called in production |
| **Dev Server Manager** | Local Rust threads | `src/runtime/dev_server.rs` | ✅ Spawns threads, reads through WASI FS |
| **HTTP Proxying** | Local Rust | `src/runtime/os_server.rs:306` | ✅ /app/* → dev server |
| **Socket Syscalls** | Local Rust | `src/runtime/syscalls.rs` | ⚠️ Creates **real host sockets** — no sandboxing, any port bindable |
| **DNS Resolution** | Local Rust | `src/runtime/syscalls.rs` | ⚠️ Calls host DNS directly — no isolation |
| **BoreClient** | Local Rust | `src/runtime/tunnel/bore.rs` | ✅ **Uses std::net::TcpStream (HOST)** |
| **Tunnel to bore.pub** | Local machine → Internet | `src/runtime/tunnel/bore.rs:53` | ✅ **Exposes localhost:8420** |
| **Browser UI** | Browser | `templates/os/os.js` | ✅ React dashboard (no WASM) |
| **File Browser** | Browser → REST API | `templates/os/os.js` | ✅ Reads from server (via WASI FS) |
| **Network Namespace** | Local Rust | `src/runtime/network_namespace.rs` | ⚠️ Bookkeeping/metadata only — no real OS namespace isolation |

### ❌ What's NOT Working (Missing Browser WASM Execution)

**These are the KEY MISSING PIECES to reach the WebContainer vision:**

| Component | Expected Location | Current Status | Blocker |
|-----------|------------------|----------------|---------|
| **Kernel in Browser** | Browser WASM VM | ❌ NOT IMPLEMENTED | Need wasm-bindgen compilation |
| **Project Upload** | Browser | ❌ NOT IMPLEMENTED | Need File System Access API |
| **WASM Runtime Loading** | Browser | ❌ NOT IMPLEMENTED | Need actual Node.js/Python WASM binaries |
| **Project Execution in Browser** | Browser WASM VM | ❌ PLACEHOLDER ONLY | Runtime binaries are empty Vec<u8> |
| **Bore Tunnel in WASM** | Browser WASM VM | ❌ Running on HOST | Need to use WASM socket syscalls |
| **WASM-to-WASM Communication** | Browser | ❌ NOT IMPLEMENTED | Need import/export bridge |
| **Runtime Download/Cache** | Browser | ❌ NOT IMPLEMENTED | Need CacheAPI/IndexedDB |
| **Virtual Networking** | Browser | ❌ NOT IMPLEMENTED | Need ServiceWorker interception |

### ⚠️ What's Simulated/Fake

| Component | What It Does | Reality |
|-----------|-------------|---------|
| **WASM Runtime Execution** | Should execute Node.js/Python code | Returns **empty `Vec<u8>`** (src/runtime/languages/nodejs.rs:51) |
| **Project "Running"** | Creates process in kernel | Process has **no actual runtime**, just a PID. `load_wasm_module()` stores bytes but never executes them. |
| **Language Detection** | Detects project type correctly | But loads **empty placeholder WASM** |
| **Bore Tunnel "in WASM"** | Documented as WASM feature | Actually uses **native std::net::TcpStream on HOST** |
| **Process Scheduling** | Round-robin scheduler exists | `schedule_next()` is never called outside tests — no execution loop, no preemption |
| **Network Isolation** | NetworkNamespace per process | Pure metadata tracking — no OS network namespaces, no port restrictions |

---

## 🔴 Known Structural Issues (Must Fix Before Browser Migration)

These are bugs and design problems in the current server-side code that will carry forward or block the browser migration if not addressed first. See **PRIORITY 0** in the roadmap.

### ~~**Issue 1: Dual Disconnected Filesystems**~~ ✅ RESOLVED

~~There are two completely separate filesystems that don't talk to each other.~~

**Fixed:** The in-memory `HashMap<String, Vec<u8>>` VFS has been removed. `SyscallInterface` now delegates to `WasiFilesystem`. A workspace temp directory is mounted at `/` providing a unified backing store. `NodeJSRuntime::run_project()` writes through `wasi_filesystem()` so files are visible to both syscalls and the HTTP API.

### **Issue 2: Security Vulnerabilities**

| Vulnerability | Location | Risk |
|--------------|----------|------|
| `Access-Control-Allow-Origin: *` | All API responses in `os_server.rs` | Any webpage can call all APIs |
| No authentication | All `/api/*` endpoints | File write/delete fully exposed |
| No path validation | `microkernel.rs` `SyscallInterface` | Path traversal in syscall VFS (WASI FS has canonicalization, but syscall VFS does not) |
| No permission checks on `kill` | `syscalls.rs` | Any process can kill any other process |
| Unrestricted socket creation | `syscalls.rs` | "Sandboxed" processes can bind any host port, connect anywhere |

### ~~**Issue 3: Dev Server Reads Wrong Filesystem**~~ ✅ RESOLVED

~~`DevServerManager::start_server()` spawns a thread running `serve_wasi_files()`. Despite its name, this function reads from the **host filesystem** directly via `std::fs::read()`. The `project_root` parameter is a virtual path like `/projects/{pid}` which doesn't exist on the real filesystem, so dev servers **never actually serve project files**.~~

**Fixed:** `serve_wasi_files()` now receives an `Arc<WasiFilesystem>` and reads files through `wasi_fs.read_file()`. Virtual paths like `/projects/{pid}/index.html` are correctly resolved through the WASI mount table to actual host paths.

### **Issue 4: Template Loading is CWD-Dependent**

`OsServer::load_templates()` uses `Path::new("templates/os")` — a hardcoded relative path. When wasmrun is installed via `cargo install` and run from any directory, this path doesn't resolve. Templates should be embedded via `include_str!` or resolved relative to the binary.

### **Issue 5: Race Conditions in OsServer**

`handle_start_project()` reads `project_pid` via RwLock, drops the lock, then calls `start_project()` which re-acquires it. Between the check and the action, another request could start the project (TOCTOU race). `handle_restart_project()` has the same pattern.

### **Issue 6: Scheduler Never Executes**

`ProcessScheduler::schedule_next()` implements round-robin but is **never called in production code**. `start_scheduler()` sets a boolean and enqueues existing processes, but there's no execution loop, no timer, no preemption. The scheduler is dead code in practice.

### **Issue 7: sock_open Has Broken Implementation**

The `handle_sock_open` syscall for TCP streams tries to connect to `0.0.0.0:0` (which always fails), catches the error, creates two `TcpListener`s binding to `0.0.0.0:0`, and immediately drops one. The surviving listener is stored as the "socket" but it's bound to a random port that's unrelated to what the caller wants.

### **Issue 8: Port Allocation Wraparound Bug**

In `NetworkNamespace::allocate_port()`, when `next_host_port >= base_port + 1000`, it resets to `base_port` without checking if those ports are still allocated. This causes conflicts. Also `calculate_base_port` uses `pid * 1000` as `u32` which overflows for PIDs > 65.

### **Issue 9: Dead/Unimplemented Syscalls**

`Fork`, `Exec`, `Exit`, `Wait`, `Mmap`, `Munmap`, `Rmdir`, `Input` are defined in `SyscallNumber` but `handle_syscall()` has no dispatch arms for them. They fall through to the catch-all error. Extensive `#[allow(dead_code)]` annotations throughout.

---

## 🚨 Critical Understanding

### **What Users Think is Happening:**
```rust
// User uploads project to browser
browser.upload_files(project_dir);

// Browser WASM kernel executes project
let pid = wasm_kernel.run_project(files, "nodejs");

// Bore tunnel runs inside WASM
let tunnel = wasm_syscall::sock_connect("bore.pub:7835");
```

### **What's Actually Happening:**
```rust
// Server reads project from local filesystem
let project_path = "/Users/ani/Documents/my-project";

// Server's native Rust kernel creates fake process
let pid = kernel.create_process(empty_wasm_binary);

// Server's native Rust bore client connects
let stream = std::net::TcpStream::connect("bore.pub:7835"); // HOST!

// Browser just displays dashboard
browser.fetch("/api/kernel/stats") // Shows HOST stats
```

---

## 🎯 ROADMAP: Server-Side → Browser-Side (9-12 Weeks)

### **PRIORITY 0: Foundation Fixes** (Week 1-3) 🔧 PREREQUISITE

**Goal:** Fix structural issues that will block or corrupt the browser migration. Without this, Priorities 1-6 build on a broken foundation.

**Tasks:**

- [x] **0.1: Unify the Dual Filesystems** (Day 1-4) ✅ COMPLETED

  Removed the in-memory `HashMap<String, Vec<u8>>` VFS from `microkernel.rs`. All filesystem operations now route through `WasiFilesystem` as the single source of truth.

  **Changes made:**
  - `WasmMicroKernel` now creates a temp workspace directory and mounts it at `/` in the WASI FS
  - `SyscallInterface` impl delegates to `self.wasi_fs` for all operations (`read_file`, `write_file`, `list_directory`, `create_directory`, `delete_file`)
  - `init_vfs()` creates real directories (`tmp`, `home`, `usr/bin`, `projects`) on disk instead of HashMap entries
  - Added `ensure_process_workspace(pid)` to create per-process directories under `/projects/{pid}`
  - `NodeJSRuntime::run_project()` writes files through `kernel.wasi_filesystem()` so they're visible to HTTP API
  - `WasiFilesystem::resolve_path()` uses longest-prefix matching so specific mounts (e.g. `/my-project`) shadow the root `/` mount

  **Files:** `src/runtime/microkernel.rs`, `src/runtime/wasi_fs.rs`, `src/runtime/languages/nodejs.rs`, `Cargo.toml`
  **Verified:** All 375 tests pass, `just format` and `just lint` clean.

- [x] **0.2: Fix Dev Server to Use WASI FS** (Day 3-4) ✅ COMPLETED

  `serve_wasi_files()` in `dev_server.rs` was reading from host FS via `std::fs::read()` using virtual paths like `/projects/{pid}` that don't exist on the real filesystem.

  **Changes made:**
  - `DevServerManager::start_server()` now accepts `Arc<WasiFilesystem>` and passes it to `serve_wasi_files()`
  - `serve_wasi_files()` uses `wasi_fs.read_file()` instead of `std::fs::read()` and constructs proper virtual paths
  - `WasmMicroKernel::wasi_filesystem_arc()` added to expose a cloneable `Arc<WasiFilesystem>` handle
  - `MultiLanguageKernel::setup_dev_environment()` passes the WASI FS arc to the dev server manager
  - Added integration test `test_dev_server_serves_wasi_files` that mounts a temp dir, starts the server, and verifies file serving via HTTP
  - Added `ureq` dev-dependency for HTTP integration tests

  **Files:** `src/runtime/dev_server.rs`, `src/runtime/multilang_kernel.rs`, `src/runtime/microkernel.rs`, `Cargo.toml`
  **Verified:** All 376 tests pass, `just format` and `just lint` clean.

- [ ] **0.3: Embed Templates** (Day 4-5)

  Replace `Path::new("templates/os")` with `include_str!` so templates work regardless of CWD.

  ```rust
  const INDEX_HTML: &str = include_str!("../../templates/os/index.html");
  const OS_JS: &str = include_str!("../../templates/os/os.js");
  const INDEX_CSS: &str = include_str!("../../templates/os/index.css");
  ```

  **Files:** `src/runtime/os_server.rs`

- [ ] **0.4: Fix OsServer Race Conditions** (Day 5-6)

  Replace the read-then-act pattern with atomic operations. Use a single lock scope for check-and-start:

  ```rust
  fn handle_start_project(&self, request: Request) -> Result<()> {
      let mut project_pid = self.project_pid.write().unwrap();
      if project_pid.is_some() {
          // already running — respond with error
      } else {
          // start project while holding the lock
          // ...
          *project_pid = Some(pid);
      }
  }
  ```

  **Files:** `src/runtime/os_server.rs`

- [ ] **0.5: Security Hardening** (Day 6-8)

  - Replace `Access-Control-Allow-Origin: *` with configurable origin (default: `localhost` only)
  - Add path validation to `SyscallInterface` methods (reject `..`, ensure paths stay within mounts)
  - Add PID-based permission checks on `kill` syscall (only parent can kill child, or self)
  - Add `--allow-cors` flag for explicit opt-in to wildcard CORS

  **Files:** `src/runtime/os_server.rs`, `src/runtime/microkernel.rs`, `src/runtime/syscalls.rs`

- [ ] **0.6: Fix sock_open Implementation** (Day 8-9)

  Replace the broken "connect to 0.0.0.0:0 then catch error" pattern. For `SOCK_STREAM`, don't create a listener eagerly — just create a placeholder entry in the FD table and defer actual socket creation to `sock_bind` or `sock_connect`.

  **Files:** `src/runtime/syscalls.rs`

- [ ] **0.7: Fix Port Allocation Overflow** (Day 9)

  - Guard `calculate_base_port()` against overflow for large PIDs
  - Track allocated ports in a HashSet and check for conflicts on wraparound

  **Files:** `src/runtime/network_namespace.rs`

- [ ] **0.8: Clean Up Dead Code** (Day 9-10)

  - Remove or implement the unimplemented syscalls (`Fork`, `Exec`, `Exit`, `Wait`, `Mmap`, `Munmap`, `Rmdir`, `Input`)
  - Remove `#[allow(dead_code)]` annotations where possible — if code is truly unused and unplanned, delete it
  - Delete the in-memory VFS (`filesystem: Arc<Mutex<HashMap<String, Vec<u8>>>>`) from `WasmMicroKernel` after 0.1 is done

  **Files:** `src/runtime/syscalls.rs`, `src/runtime/microkernel.rs`

- [ ] **0.9: Fix Dev Server Stop Signal** (Day 10)

  The `tiny_http::Server::incoming_requests()` is a blocking iterator — the stop signal is only checked after a request arrives. Set a non-blocking timeout or use a different shutdown mechanism.

  **Files:** `src/runtime/dev_server.rs`

**Deliverable:** A clean, correct server-side implementation to build the browser migration on.

**Success Criteria:**
- ✅ Single unified filesystem — syscalls and HTTP APIs read/write the same files ✅ DONE (0.1)
- ✅ Dev server actually serves project files ✅ DONE (0.2)
- ✅ Templates work when installed via `cargo install`
- ✅ No TOCTOU races in OsServer
- ✅ CORS restricted by default
- ✅ `cargo test` passes (no regressions)

---

### **PRIORITY 1: Get Kernel Running in Browser** (Week 3-4) 🔥 CRITICAL

**Goal:** Compile wasmrun kernel to WASM and load in browser

**Tasks:**
- [ ] **1.1: Setup wasm-bindgen** (Day 1)
  ```toml
  # Cargo.toml
  [lib]
  crate-type = ["cdylib", "rlib"]

  [dependencies]
  wasm-bindgen = "0.2"
  wasm-bindgen-futures = "0.4"
  js-sys = "0.3"
  web-sys = { version = "0.3", features = [
    "Window", "Document", "Element", "HtmlElement",
    "console", "CacheStorage", "Cache", "Request", "Response"
  ]}
  ```

- [ ] **1.2: Create WASM bindings** (Day 1-2)
  ```rust
  // src/lib.rs
  use wasm_bindgen::prelude::*;

  #[wasm_bindgen]
  pub struct WasmRunOS {
      kernel: MultiLanguageKernel,
  }

  #[wasm_bindgen]
  impl WasmRunOS {
      #[wasm_bindgen(constructor)]
      pub fn new() -> Self {
          Self {
              kernel: MultiLanguageKernel::new(),
          }
      }

      pub fn start(&mut self) -> Result<JsValue, JsValue> {
          self.kernel.start()
              .map(|_| JsValue::from_str("Kernel started"))
              .map_err(|e| JsValue::from_str(&e.to_string()))
      }

      pub fn mount_project(&mut self, path: &str) -> Result<(), JsValue> {
          self.kernel.mount_project(path)
              .map_err(|e| JsValue::from_str(&e.to_string()))
      }
  }
  ```

- [ ] **1.3: Build to WASM** (Day 2)
  ```bash
  # Install wasm-pack
  cargo install wasm-pack

  # Build for web
  wasm-pack build --target web --out-dir pkg

  # Output: pkg/wasmrun_bg.wasm, pkg/wasmrun.js
  ```

- [ ] **1.4: Create browser loader** (Day 3)
  ```javascript
  // templates/os/kernel-loader.js
  import init, { WasmRunOS } from './pkg/wasmrun.js';

  export async function loadKernel() {
      // Load WASM binary
      await init();

      // Create kernel instance
      const kernel = new WasmRunOS();
      await kernel.start();

      console.log('✅ Kernel running in browser WASM VM!');
      return kernel;
  }
  ```

- [ ] **1.5: Update templates/os/index.html** (Day 3)
  ```html
  <script type="module">
      import { loadKernel } from './kernel-loader.js';

      window.addEventListener('DOMContentLoaded', async () => {
          window.wasmKernel = await loadKernel();
          console.log('Kernel loaded!', window.wasmKernel);
      });
  </script>
  ```

- [ ] **1.6: Test basic operations** (Day 4-5)
  - Process creation in browser
  - WASI filesystem in browser
  - Syscall handling

**Deliverable:** wasmrun kernel running in browser, can create processes

**Success Criteria:**
- ✅ `pkg/wasmrun_bg.wasm` exists (~2-3MB)
- ✅ Browser console shows "Kernel running in browser WASM VM!"
- ✅ Can create process: `kernel.create_process()`
- ✅ WASI filesystem works in browser

---

### **PRIORITY 2: File Upload & Project Loading** (Week 4-5) 📁

**Goal:** Upload project files from local machine to browser WASM

**Tasks:**
- [ ] **2.1: Implement File System Access API** (Day 1-2)
  ```javascript
  // templates/os/file-upload.js
  export async function uploadProject() {
      // Request directory picker
      const dirHandle = await window.showDirectoryPicker();

      // Read all files recursively
      const files = {};
      async function readDir(handle, path = '') {
          for await (const entry of handle.values()) {
              const fullPath = path + '/' + entry.name;

              if (entry.kind === 'file') {
                  const file = await entry.getFile();
                  files[fullPath] = await file.arrayBuffer();
              } else if (entry.kind === 'directory') {
                  await readDir(entry, fullPath);
              }
          }
      }

      await readDir(dirHandle);
      return files;
  }
  ```

- [ ] **2.2: Add WASM bindings for file loading** (Day 2-3)
  ```rust
  // src/lib.rs
  #[wasm_bindgen]
  impl WasmRunOS {
      pub fn load_project_files(&mut self, files: JsValue) -> Result<(), JsValue> {
          let files: HashMap<String, Vec<u8>> = serde_wasm_bindgen::from_value(files)?;

          for (path, content) in files {
              self.kernel.wasi_filesystem().write_file(&path, &content)?;
          }

          Ok(())
      }
  }
  ```

- [ ] **2.3: Integrate with UI** (Day 3-4)
  ```javascript
  // Update templates/os/os.js
  async function handleUploadProject() {
      const files = await uploadProject();
      await window.wasmKernel.load_project_files(files);
      console.log('✅ Project loaded into browser WASM!');
  }
  ```

- [ ] **2.4: Test with real projects** (Day 4-5)
  - Upload Node.js project
  - Upload Python project
  - Verify files in WASI filesystem

**Deliverable:** Can upload projects to browser WASM

**Success Criteria:**
- ✅ File picker dialog works
- ✅ All project files loaded into WASM filesystem
- ✅ Can read uploaded files via WASI syscalls

---

### **PRIORITY 3: Rust & Go Runtimes — Validate WASI Bridge** (Week 5-8) 🟢 CRITICAL

**Goal:** Get real code executing in WASM by starting with languages that compile **directly** to `wasm32-wasi`. This validates the entire WASI bridge before tackling interpreter-based runtimes.

> **🎯 Why Rust & Go first?**
>
> Rust and Go (via TinyGo) compile directly to `wasm32-wasi` targets. The resulting `.wasm`
> binaries use standard WASI Preview 1 imports (`fd_read`, `fd_write`, `path_open`, etc.).
> This means:
>
> 1. **No interpreter needed** — the user's code IS the WASM binary
> 2. **Standard WASI interface** — we build the bridge once, all WASI languages benefit
> 3. **Easy to validate** — compile a "Hello World", run it, verify output routes through our kernel
> 4. **Small binaries** — a Rust hello-world is ~60KB, TinyGo is ~260KB
> 5. **Battle-tested targets** — `wasm32-wasi` is Rust's Tier 2 target, TinyGo WASI is mature
>
> Once the WASI bridge is proven with Rust & Go, adding JS (QuickJS) and Python (RustPython)
> in Priority 6 is just "load a different .wasm file with the same bridge."
>
> **Compilation flow:**
> ```
> User's Rust/Go source code
>   → compile to .wasm (wasm32-wasi target)
>   → load into browser via our kernel
>   → kernel provides WASI imports (fd_read, fd_write, etc.)
>   → _start() executes, I/O routes through kernel
> ```

> **✅ Prebuilt WASI runtimes are already available:**
>
> Prebuilt and tested Rust & Go WASI runtime binaries are published at
> **[wasmhub v0.1.4](https://github.com/anistark/wasmhub/releases/tag/v0.1.4)**.
>
> | Runtime | File | Size | WASI | SHA256 |
> |---------|------|------|------|--------|
> | **Rust 1.84** | `rust-1.84.wasm` | 60 KB | wasip1 | `04a09a9...22f09` |
> | **Go 1.23** | `go-1.23.wasm` | 260 KB | wasip1 | `07d5c42...cd05a` |
>
> Also available: Brotli (`.wasm.br`) and Gzip (`.wasm.gz`) compressed variants, per-language
> manifests (`rust-manifest.json`, `go-manifest.json`), a combined `manifest.json`, and
> `SHA256SUMS` for integrity verification.
>
> **These binaries can be used directly — no need to build from source for tasks 3.1 and 3.2.**
> ```bash
> # Download directly:
> curl -LO https://github.com/anistark/wasmhub/releases/download/v0.1.4/rust-1.84.wasm
> curl -LO https://github.com/anistark/wasmhub/releases/download/v0.1.4/go-1.23.wasm
>
> # Verify:
> wasmtime rust-1.84.wasm   # → runs Rust WASI binary
> wasmtime go-1.23.wasm     # → runs Go WASI binary
> ```

**Tasks:**

- [ ] **3.1: Obtain Rust WASI runtime binary** (Day 1)

  Prebuilt binaries are available from [wasmhub v0.1.4](https://github.com/anistark/wasmhub/releases/tag/v0.1.4). No need to build from source.

  ```bash
  # Download prebuilt Rust WASI runtime
  curl -LO https://github.com/anistark/wasmhub/releases/download/v0.1.4/rust-1.84.wasm
  curl -LO https://github.com/anistark/wasmhub/releases/download/v0.1.4/rust-manifest.json

  # Verify
  wasmtime rust-1.84.wasm
  ```

  Also test compiling a user project to `wasm32-wasi` to validate the full pipeline:

  ```bash
  # Ensure wasm32-wasi target is installed
  rustup target add wasm32-wasi

  # Create test project
  cargo new --name hello-wasi /tmp/hello-wasi
  cd /tmp/hello-wasi

  cat > src/main.rs << 'EOF'
  use std::fs;

  fn main() {
      println!("Hello from Rust WASM!");
      eprintln!("This goes to stderr");

      // Test WASI filesystem
      fs::write("/tmp/test.txt", "written from wasm").unwrap();
      let content = fs::read_to_string("/tmp/test.txt").unwrap();
      println!("Read back: {content}");

      // Test environment
      for (key, value) in std::env::vars() {
          println!("ENV: {key}={value}");
      }

      // Test args
      for arg in std::env::args() {
          println!("ARG: {arg}");
      }
  }
  EOF

  cargo build --target wasm32-wasi --release
  # Output: target/wasm32-wasi/release/hello-wasi.wasm

  # Verify with wasmtime
  wasmtime target/wasm32-wasi/release/hello-wasi.wasm
  # Should print "Hello from Rust WASM!"
  ```

- [ ] **3.2: Obtain Go WASI runtime binary** (Day 1-2)

  Prebuilt binary available from [wasmhub v0.1.4](https://github.com/anistark/wasmhub/releases/tag/v0.1.4):

  ```bash
  # Download prebuilt Go WASI runtime
  curl -LO https://github.com/anistark/wasmhub/releases/download/v0.1.4/go-1.23.wasm
  curl -LO https://github.com/anistark/wasmhub/releases/download/v0.1.4/go-manifest.json

  # Verify
  wasmtime go-1.23.wasm
  ```

  Also test compiling a user project with TinyGo:

  ```bash
  # Install TinyGo
  brew install tinygo  # or from https://tinygo.org/getting-started/install/

  cat > /tmp/hello.go << 'EOF'
  package main

  import "fmt"

  func main() {
      fmt.Println("Hello from Go WASM!")
      fmt.Println("TinyGo WASI target works!")
  }
  EOF

  tinygo build -target=wasi -o /tmp/hello-go.wasm /tmp/hello.go

  wasmtime /tmp/hello-go.wasm
  # Should print "Hello from Go WASM!"
  ```

  > **⚠️ Go WASM note:** `GOOS=js GOARCH=wasm` (standard Go) produces a binary that requires
  > Go's `wasm_exec.js` and targets the browser JS environment — NOT WASI. It cannot use our
  > WASI bridge. **Always use TinyGo with `-target=wasi` for wasmrun OS mode.**
  >
  > **TinyGo limitations:** No `net/http`, limited `reflect`, no cgo. Sufficient for
  > compute-heavy code, CLI tools, and WASI-based I/O.

- [ ] **3.3: Host runtime binaries locally** (Day 2-3)

  Copy wasmhub binaries into the project for serving:

  ```bash
  mkdir -p static/runtimes/

  # From wasmhub v0.1.4
  curl -L -o static/runtimes/rust-1.84.wasm \
    https://github.com/anistark/wasmhub/releases/download/v0.1.4/rust-1.84.wasm
  curl -L -o static/runtimes/go-1.23.wasm \
    https://github.com/anistark/wasmhub/releases/download/v0.1.4/go-1.23.wasm
  curl -L -o static/runtimes/manifest.json \
    https://github.com/anistark/wasmhub/releases/download/v0.1.4/manifest.json
  curl -L -o static/runtimes/rust-manifest.json \
    https://github.com/anistark/wasmhub/releases/download/v0.1.4/rust-manifest.json
  curl -L -o static/runtimes/go-manifest.json \
    https://github.com/anistark/wasmhub/releases/download/v0.1.4/go-manifest.json
  curl -L -o static/runtimes/SHA256SUMS \
    https://github.com/anistark/wasmhub/releases/download/v0.1.4/SHA256SUMS

  # Optional: compressed variants for production
  curl -L -o static/runtimes/rust-1.84.wasm.br \
    https://github.com/anistark/wasmhub/releases/download/v0.1.4/rust-1.84.wasm.br
  curl -L -o static/runtimes/go-1.23.wasm.br \
    https://github.com/anistark/wasmhub/releases/download/v0.1.4/go-1.23.wasm.br
  ```

  **Future:** As wasmhub publishes new versions, update the runtimes here.
  The manifest format is already compatible with our runtime loader design.

- [ ] **3.4: Create the WASI imports bridge** (Day 4-8)

  This is the core piece — our kernel provides the WASI Preview 1 functions that `.wasm`
  binaries expect. This bridge is language-agnostic: any `wasm32-wasi` binary can use it.

  ```rust
  // src/runtime/wasm_bridge.rs
  use wasm_bindgen::prelude::*;

  /// Create WASI Preview 1 import object for instantiating .wasm modules.
  /// This is the glue between our kernel's filesystem/process model and
  /// any wasm32-wasi compiled binary (Rust, Go, C, or interpreter runtimes).
  pub fn create_wasi_imports(kernel: &MultiLanguageKernel) -> Result<js_sys::Object, JsValue> {
      let imports = js_sys::Object::new();
      let wasi = js_sys::Object::new();

      // === File descriptor operations ===
      // These route through our unified WasiFilesystem
      js_sys::Reflect::set(&wasi, &"fd_read".into(), &create_fd_read(kernel))?;
      js_sys::Reflect::set(&wasi, &"fd_write".into(), &create_fd_write(kernel))?;
      js_sys::Reflect::set(&wasi, &"fd_close".into(), &create_fd_close(kernel))?;
      js_sys::Reflect::set(&wasi, &"fd_seek".into(), &create_fd_seek(kernel))?;
      js_sys::Reflect::set(&wasi, &"fd_fdstat_get".into(), &create_fd_fdstat_get(kernel))?;
      js_sys::Reflect::set(&wasi, &"fd_prestat_get".into(), &create_fd_prestat_get(kernel))?;
      js_sys::Reflect::set(&wasi, &"fd_prestat_dir_name".into(), &create_fd_prestat_dir_name(kernel))?;

      // === Path operations ===
      js_sys::Reflect::set(&wasi, &"path_open".into(), &create_path_open(kernel))?;
      js_sys::Reflect::set(&wasi, &"path_filestat_get".into(), &create_path_filestat_get(kernel))?;
      js_sys::Reflect::set(&wasi, &"path_create_directory".into(), &create_path_create_directory(kernel))?;
      js_sys::Reflect::set(&wasi, &"path_unlink_file".into(), &create_path_unlink_file(kernel))?;
      js_sys::Reflect::set(&wasi, &"path_readdir".into(), &create_path_readdir(kernel))?;

      // === Environment ===
      js_sys::Reflect::set(&wasi, &"environ_get".into(), &create_environ_get(kernel))?;
      js_sys::Reflect::set(&wasi, &"environ_sizes_get".into(), &create_environ_sizes_get(kernel))?;
      js_sys::Reflect::set(&wasi, &"args_get".into(), &create_args_get(kernel))?;
      js_sys::Reflect::set(&wasi, &"args_sizes_get".into(), &create_args_sizes_get(kernel))?;

      // === Clock ===
      js_sys::Reflect::set(&wasi, &"clock_time_get".into(), &create_clock_time_get())?;

      // === Process lifecycle ===
      js_sys::Reflect::set(&wasi, &"proc_exit".into(), &create_proc_exit(kernel))?;

      // === Random ===
      js_sys::Reflect::set(&wasi, &"random_get".into(), &create_random_get())?;

      js_sys::Reflect::set(&imports, &"wasi_snapshot_preview1".into(), &wasi)?;
      Ok(imports)
  }
  ```

  **Key WASI functions to implement (ordered by priority):**

  | Function | Used by | Notes |
  |----------|---------|-------|
  | `fd_write` (fd=1,2) | Every program (stdout/stderr) | Route to kernel log / browser console |
  | `proc_exit` | Every program | Clean up process, update state |
  | `args_get` / `args_sizes_get` | Programs that read argv | Pass entry file path as arg |
  | `environ_get` / `environ_sizes_get` | Programs that read env vars | Configurable per-process env |
  | `fd_prestat_get` / `fd_prestat_dir_name` | Programs that use preopened dirs | Map to WASI FS mounts |
  | `path_open` | File I/O | Delegate to unified WasiFilesystem |
  | `fd_read` | File I/O | Read from opened fd |
  | `fd_close` | File I/O | Release fd |
  | `fd_seek` | File I/O | Seek within fd |
  | `clock_time_get` | Timing | Use `performance.now()` in browser |
  | `random_get` | Crypto/random | Use `crypto.getRandomValues()` in browser |

- [ ] **3.5: Implement Rust runtime in wasmrun** (Day 8-11)

  ```rust
  // src/runtime/languages/rust.rs
  use crate::runtime::registry::{LanguageRuntime, ProjectBundle, ProjectMetadata};
  use crate::runtime::microkernel::{Pid, WasmMicroKernel};
  use crate::runtime::wasm_bridge;

  pub struct RustRuntime;

  impl LanguageRuntime for RustRuntime {
      fn name(&self) -> &str { "rust" }
      fn extensions(&self) -> &[&str] { &["rs"] }
      fn entry_files(&self) -> &[&str] { &["Cargo.toml"] }

      fn detect_project(&self, project_path: &str) -> bool {
          Path::new(project_path).join("Cargo.toml").exists()
      }

      fn load_wasm_binary(&self) -> Result<Vec<u8>> {
          // Rust projects are compiled to .wasm by the user or by our compile step.
          // This returns the compiled .wasm bytes, not an interpreter.
          Err(anyhow!("Rust projects must be compiled first — use `wasmrun compile`"))
      }

      fn prepare_project(&self, project_path: &str) -> Result<ProjectBundle> {
          // Look for precompiled .wasm in target/wasm32-wasi/release/
          let wasm_path = find_rust_wasm_output(project_path)?;
          let wasm_bytes = std::fs::read(&wasm_path)?;

          let cargo_toml = std::fs::read_to_string(
              Path::new(project_path).join("Cargo.toml")
          )?;
          let name = extract_cargo_name(&cargo_toml)
              .unwrap_or_else(|| "rust-project".to_string());

          Ok(ProjectBundle {
              name,
              language: "rust".to_string(),
              entry_point: wasm_path.to_string_lossy().to_string(),
              files: HashMap::from([("main.wasm".to_string(), wasm_bytes)]),
              dependencies: vec![],
              metadata: ProjectMetadata {
                  version: "0.1.0".to_string(),
                  description: None, author: None, license: None,
              },
          })
      }

      fn run_project(&self, bundle: ProjectBundle, kernel: &mut WasmMicroKernel) -> Result<Pid> {
          let pid = kernel.create_process(bundle.name.clone(), "rust".to_string(), None)?;
          let wasm_bytes = bundle.files.get("main.wasm")
              .ok_or_else(|| anyhow!("No compiled .wasm found in bundle"))?;
          kernel.load_wasm_module(pid, wasm_bytes)?;

          // In browser mode: instantiate with WASI imports and call _start
          // In server mode: use wasmtime/wasmer to execute
          Ok(pid)
      }

      // ... remaining trait methods
  }

  fn find_rust_wasm_output(project_path: &str) -> Result<PathBuf> {
      let target_dir = Path::new(project_path).join("target/wasm32-wasi/release");
      if !target_dir.exists() {
          anyhow::bail!(
              "No wasm32-wasi build found. Run: cargo build --target wasm32-wasi --release"
          );
      }
      // Find the .wasm file
      for entry in std::fs::read_dir(&target_dir)? {
          let entry = entry?;
          if entry.path().extension().map(|e| e == "wasm").unwrap_or(false) {
              return Ok(entry.path());
          }
      }
      anyhow::bail!("No .wasm file found in {}", target_dir.display())
  }
  ```

- [ ] **3.6: Implement Go runtime in wasmrun** (Day 11-14)

  ```rust
  // src/runtime/languages/go.rs
  pub struct GoRuntime;

  impl LanguageRuntime for GoRuntime {
      fn name(&self) -> &str { "go" }
      fn extensions(&self) -> &[&str] { &["go"] }
      fn entry_files(&self) -> &[&str] { &["go.mod", "main.go"] }

      fn detect_project(&self, project_path: &str) -> bool {
          Path::new(project_path).join("go.mod").exists()
              || Path::new(project_path).join("main.go").exists()
      }

      fn prepare_project(&self, project_path: &str) -> Result<ProjectBundle> {
          // Look for precompiled .wasm or compile via TinyGo
          let wasm_path = find_go_wasm_output(project_path)?;
          let wasm_bytes = std::fs::read(&wasm_path)?;

          Ok(ProjectBundle {
              name: extract_go_module_name(project_path)
                  .unwrap_or_else(|| "go-project".to_string()),
              language: "go".to_string(),
              entry_point: "main.wasm".to_string(),
              files: HashMap::from([("main.wasm".to_string(), wasm_bytes)]),
              dependencies: vec![],
              metadata: ProjectMetadata {
                  version: "0.1.0".to_string(),
                  description: None, author: None, license: None,
              },
          })
      }

      fn run_project(&self, bundle: ProjectBundle, kernel: &mut WasmMicroKernel) -> Result<Pid> {
          let pid = kernel.create_process(bundle.name.clone(), "go".to_string(), None)?;
          let wasm_bytes = bundle.files.get("main.wasm")
              .ok_or_else(|| anyhow!("No compiled .wasm found in bundle"))?;
          kernel.load_wasm_module(pid, wasm_bytes)?;
          Ok(pid)
      }

      // ... remaining trait methods (same WASI bridge as Rust)
  }

  fn find_go_wasm_output(project_path: &str) -> Result<PathBuf> {
      // Check for precompiled .wasm
      let wasm_file = Path::new(project_path).join("main.wasm");
      if wasm_file.exists() {
          return Ok(wasm_file);
      }
      anyhow::bail!(
          "No .wasm file found. Compile with: tinygo build -target=wasi -o main.wasm ."
      )
  }
  ```

- [ ] **3.7: Register runtimes and integrate compile step** (Day 14-16)

  ```rust
  // src/runtime/registry.rs — update register_builtin_runtimes()
  pub fn register_builtin_runtimes() -> Self {
      let mut registry = Self::new();
      registry.register("rust", Box::new(RustRuntime::new()));
      registry.register("go", Box::new(GoRuntime::new()));
      registry.register("nodejs", Box::new(NodeJSRuntime::new())); // still placeholder for now
      registry
  }
  ```

  ```rust
  // src/commands/compile.rs — add wasm32-wasi compilation support
  // For Rust: cargo build --target wasm32-wasi --release
  // For Go: tinygo build -target=wasi -o main.wasm .
  ```

- [ ] **3.8: Browser-side execution via WASI bridge** (Day 16-19)

  ```rust
  // src/lib.rs (wasm-bindgen entry point)
  #[wasm_bindgen]
  impl WasmRunOS {
      /// Load and execute a precompiled .wasm binary (Rust or Go)
      pub async fn run_wasi_binary(&mut self, wasm_bytes: &[u8], args: Vec<String>) -> Result<u32, JsValue> {
          // Create WASI imports from our kernel
          let imports = wasm_bridge::create_wasi_imports(&self.kernel)?;

          // Instantiate the user's .wasm with our WASI imports
          let module = WebAssembly::compile(wasm_bytes).await?;
          let instance = WebAssembly::instantiate_module(&module, &imports).await?;

          // Call _start (WASI entry point)
          let start = instance.get_export("_start")?;
          start.call(&[])?;

          Ok(pid)
      }
  }
  ```

- [ ] **3.9: Test end-to-end** (Day 19-21)

  ```bash
  # Test Rust project
  cd examples/rust-hello
  cargo build --target wasm32-wasi --release
  # Upload target/wasm32-wasi/release/rust_hello.wasm to browser
  # → "Hello from Rust WASM!" appears in browser console

  # Test Go project
  cd examples/go-hello
  tinygo build -target=wasi -o main.wasm .
  # Upload main.wasm to browser
  # → "Hello from Go WASM!" appears in browser console
  ```

  **Test cases:**
  - stdout output (`println!` / `fmt.Println`) → routed to browser console via `fd_write`
  - stderr output → separate stream, also visible
  - File read/write via WASI → uses our unified filesystem
  - Process exit code → `proc_exit` handled cleanly
  - Command-line arguments → `args_get` provides configured args
  - Environment variables → `environ_get` provides configured env

**Deliverable:** Real compiled Rust and Go code executing in browser WASM with full WASI I/O.

**Success Criteria:**
- ✅ Rust `wasm32-wasi` binary executes in browser via our WASI bridge
- ✅ TinyGo WASI binary executes in browser via the **same** WASI bridge
- ✅ `fd_write` to stdout/stderr routes output through our kernel to browser UI
- ✅ File I/O works (read/write files in WASI filesystem)
- ✅ `proc_exit` cleans up process correctly
- ✅ The WASI bridge is language-agnostic (same code for Rust, Go, and later JS/Python)

**What this proves:**
- The WASI bridge works correctly with real-world binaries
- Our kernel's filesystem integration handles actual file I/O
- The browser WASM instantiation pipeline is solid
- Any future `wasm32-wasi` binary (interpreters included) will "just work"

---

### **PRIORITY 4: Browser Networking & Tunnel** (Week 8-10) 🌐

**Goal:** Enable network access from browser WASM, including public tunneling.

> **⚠️ CRITICAL: Browsers do NOT support raw TCP/UDP sockets.**
>
> The previous plan assumed we could call `sock_open(AF_INET, SOCK_STREAM)` from browser WASM
> and get a real TCP connection. **This is impossible.** Browsers only offer:
> - `fetch()` / `XMLHttpRequest` — HTTP(S) only
> - `WebSocket` — upgrades from HTTP, bidirectional, but not raw TCP
> - `WebTransport` — HTTP/3 based, emerging standard
>
> **Bore's protocol uses raw TCP on port 7835.** You cannot connect to `bore.pub:7835` from a
> browser because it's not an HTTP/WebSocket endpoint.
>
> **Three viable approaches:**
>
> | Approach | How It Works | Trade-off |
> |----------|-------------|-----------|
> | **A: WebSocket relay server** | Lightweight server bridges WebSocket↔TCP to bore.pub | Requires a small relay server (defeats "zero server") |
> | **B: Fork bore to add WebSocket support** | Modify bore server to accept WebSocket connections | Requires maintaining a bore fork |
> | **C: Use a WebSocket-native tunnel** | Replace bore with a tunnel service that supports WebSocket clients (e.g., Cloudflare Tunnel, custom) | Different tunnel protocol |
>
> **Recommended: Approach A (WebSocket relay) for MVP**, then explore B/C for full browser-only.
> A thin relay server (`wss://relay.wasmrun.dev` → TCP `bore.pub:7835`) is ~50 lines of code
> and can be hosted cheaply. This is honest about the browser limitation while still getting
> tunneling working.

**Current Problem:**
```rust
// src/runtime/tunnel/bore.rs:53 - RUNS ON HOST!
let stream = std::net::TcpStream::connect(server) // ❌ HOST ONLY
// AND: browsers can't do raw TCP at all
```

**Tasks:**

- [ ] **4.1: Implement ServiceWorker for virtual networking** (Day 1-4)

  Intercept `fetch()` calls from WASM runtimes and route them to in-kernel HTTP handlers or the outside world.

  ```javascript
  // templates/os/service-worker.js
  self.addEventListener('fetch', (event) => {
      const url = new URL(event.request.url);

      // Route internal requests to WASM kernel
      if (url.hostname === 'localhost' && isKernelPort(url.port)) {
          event.respondWith(kernelHandle(event.request));
          return;
      }

      // External requests pass through normally
      event.respondWith(fetch(event.request));
  });
  ```

- [ ] **4.2: Implement WebSocket-based socket layer** (Day 4-7)

  For WASM processes that need "socket-like" connections, provide a WebSocket adapter:

  ```rust
  // src/runtime/syscalls_browser.rs
  // Browser socket implementation — maps sock_* syscalls to WebSocket/fetch

  pub async fn browser_sock_connect(addr: &str, protocol: &str) -> Result<BrowserSocket> {
      match protocol {
          "ws" | "wss" => {
              let ws = WebSocket::new(&format!("wss://{}", addr))?;
              // ... setup
              Ok(BrowserSocket::WebSocket(ws))
          }
          "http" | "https" => {
              // Use fetch API for request/response patterns
              Ok(BrowserSocket::Http(addr.to_string()))
          }
          _ => Err(anyhow!("Browser cannot create raw TCP/UDP sockets"))
      }
  }
  ```

- [ ] **4.3: Build WebSocket relay for bore** (Day 7-10)

  A minimal relay server that bridges WebSocket clients to bore's TCP protocol:

  ```rust
  // relay-server/src/main.rs (separate tiny binary, ~50 lines)
  // Accepts: wss://relay.wasmrun.dev/tunnel
  // Connects: TCP bore.pub:7835
  // Bridges: WebSocket frames ↔ TCP bytes

  async fn handle_ws(ws: WebSocket, bore_addr: &str) -> Result<()> {
      let tcp = TcpStream::connect(bore_addr).await?;
      // Bidirectional copy between ws and tcp
      tokio::select! {
          _ = copy_ws_to_tcp(&ws, &tcp) => {},
          _ = copy_tcp_to_ws(&tcp, &ws) => {},
      }
      Ok(())
  }
  ```

- [ ] **4.4: Create browser-side BoreClient** (Day 10-13)

  ```rust
  // src/runtime/tunnel/bore_browser.rs
  pub struct BoreBrowserClient {
      relay_url: String,  // wss://relay.wasmrun.dev/tunnel
      local_port: u16,
  }

  impl BoreBrowserClient {
      pub async fn connect(&mut self) -> Result<String> {
          // Connect to relay via WebSocket (this works in browsers!)
          let ws = WebSocket::new(&self.relay_url)?;

          // Relay forwards bore protocol over WebSocket
          ws.send_text("HELLO 1\n")?;
          ws.send_text(&format!("{}\n", self.local_port))?;

          let response = ws.recv_text().await?;
          let public_port = parse_bore_response(&response)?;

          Ok(format!("http://bore.pub:{}", public_port))
      }
  }
  ```

- [ ] **4.5: Test end-to-end** (Day 13-16)
  - Start project in browser WASM
  - Create bore tunnel via WebSocket relay
  - Access via public URL
  - Verify traffic flows: browser → relay → bore.pub → internet

**Deliverable:** Networking from browser WASM, tunneling via WebSocket relay.

**Success Criteria:**
- ✅ ServiceWorker intercepts internal fetch requests
- ✅ WebSocket connections work from WASM
- ✅ Bore tunnel accessible via relay (public URL works)
- ✅ Honest about the relay dependency (documented, not hidden)

**Future (post-MVP):**
- Explore bore fork with native WebSocket support (eliminates relay)
- Explore WebTransport for lower-latency tunneling
- Explore Cloudflare Tunnel integration as alternative

---

### **PRIORITY 5: Runtime Caching & Version Management** (Week 10-11) 📦

**Goal:** Download runtimes once, cache forever

**Tasks:**
- [ ] **5.1: Implement CacheAPI storage** (Day 1-3)
  ```rust
  #[wasm_bindgen]
  impl RuntimeLoader {
      async fn store_in_cache(&self, key: &str, bytes: &[u8]) -> Result<(), JsValue> {
          let window = window().ok_or("No window")?;
          let caches = window.caches()?;
          let cache = JsFuture::from(caches.open(&self.cache_name)).await?;
          let cache: web_sys::Cache = cache.dyn_into()?;

          let response = Response::new_with_opt_u8_array(Some(bytes))?;
          let request = Request::new_with_str(&format!("/runtimes/{}", key))?;

          JsFuture::from(cache.put_with_request_and_response(&request, &response)).await?;
          Ok(())
      }
  }
  ```

- [ ] **5.2: Create runtime manifest** (Day 4-5)
  ```json
  // static/runtimes/manifest.json
  //
  // NOTE: Rust and Go are "compiled" languages — the user compiles their own .wasm
  // binary (via `cargo build --target wasm32-wasi` or `tinygo build -target=wasi`).
  // No interpreter runtime needs to be downloaded for them.
  //
  // JS and Python are "interpreted" — we host the interpreter .wasm binaries here.
  // The user's source code is written to WASI FS and interpreted at runtime.
  {
    "js": {
      "type": "interpreter",
      "latest": "quickjs-2024.01",
      "runtimes": {
        "quickjs": {
          "description": "QuickJS ES2023 interpreter (no Node.js APIs)",
          "versions": [
            {
              "version": "2024.01",
              "size": 1572864,
              "sha256": "abc123...",
              "url": "/runtimes/js-quickjs-2024.01.wasm",
              "wasi": "preview1"
            }
          ]
        },
        "javy": {
          "description": "Shopify Javy (QuickJS-based, optimized)",
          "versions": [
            {
              "version": "3.0.0",
              "size": 2097152,
              "sha256": "def456...",
              "url": "/runtimes/js-javy-3.0.0.wasm",
              "wasi": "preview1"
            }
          ]
        }
      }
    },
    "python": {
      "type": "interpreter",
      "latest": "rustpython-0.3",
      "runtimes": {
        "rustpython": {
          "description": "RustPython (partial Python 3.10, smaller binary)",
          "versions": [
            {
              "version": "0.3",
              "size": 5242880,
              "sha256": "ghi789...",
              "url": "/runtimes/python-rustpython-0.3.wasm",
              "wasi": "preview1"
            }
          ]
        },
        "cpython-wasi": {
          "description": "CPython WASI build (full Python 3.11, larger binary)",
          "versions": [
            {
              "version": "3.11.7",
              "size": 15728640,
              "sha256": "jkl012...",
              "url": "/runtimes/python-cpython-3.11.7.wasm",
              "wasi": "preview1"
            }
          ]
        }
      }
    },
    "rust": {
      "type": "compiled",
      "description": "User compiles via: cargo build --target wasm32-wasi --release",
      "compile_target": "wasm32-wasi",
      "wasi": "wasip1",
      "wasmhub": "https://github.com/anistark/wasmhub/releases/tag/v0.1.4",
      "prebuilt": {
        "1.84": {
          "file": "rust-1.84.wasm",
          "size": 61498,
          "sha256": "04a09a94536ddc4c6d4bb29f73d606c503796005258dd7b3425a06afaa922f09"
        }
      }
    },
    "go": {
      "type": "compiled",
      "description": "User compiles via: tinygo build -target=wasi -o main.wasm .",
      "compile_target": "wasi (tinygo)",
      "wasi": "wasip1",
      "wasmhub": "https://github.com/anistark/wasmhub/releases/tag/v0.1.4",
      "prebuilt": {
        "1.23": {
          "file": "go-1.23.wasm",
          "size": 266213,
          "sha256": "07d5c427eea461fcb5194881966702c941931b7baa2728b1aab669a9e4bcd05a"
        }
      },
      "notes": "Standard Go (GOOS=js GOARCH=wasm) is NOT compatible — must use TinyGo"
    }
  }
  ```

- [ ] **5.3: Version/runtime selection logic** (Day 6-8)
  ```rust
  impl RuntimeLoader {
      /// Select the best runtime for a language based on project config
      pub fn select_runtime(&self, language: &str, project_path: &str) -> Result<RuntimeChoice> {
          match language {
              "js" => {
                  // Default to QuickJS (smaller, faster to load)
                  // Could check for a .wasmrunrc or wasmrun.toml for preferences
                  Ok(RuntimeChoice {
                      runtime: "quickjs".to_string(),
                      version: "2024.01".to_string(),
                  })
              }
              "python" => {
                  // Check if project needs full CPython (e.g., C extensions)
                  let needs_full = Path::new(project_path).join("setup.py").exists();
                  if needs_full {
                      Ok(RuntimeChoice {
                          runtime: "cpython-wasi".to_string(),
                          version: "3.11.7".to_string(),
                      })
                  } else {
                      Ok(RuntimeChoice {
                          runtime: "rustpython".to_string(),
                          version: "0.3".to_string(),
                      })
                  }
              }
              "go" => {
                  Ok(RuntimeChoice {
                      runtime: "tinygo".to_string(),
                      version: "0.31".to_string(),
                  })
              }
              _ => Err(anyhow!("Unsupported language: {language}"))
          }
      }
  }
  ```

- [ ] **5.4: Progress tracking UI** (Day 9-11)
  ```javascript
  // Add to templates/os/os.js
  async function loadRuntime(language, version) {
      const loader = new RuntimeLoader('wasmrun-runtimes');

      loader.on_progress((percent, loaded, total) => {
          console.log(`Downloading ${language} v${version}: ${percent}%`);
          updateProgressBar(percent);
      });

      const bytes = await loader.load_runtime(language, version);
      console.log('✅ Runtime cached');
      return bytes;
  }
  ```

- [ ] **5.5: Test multi-version support** (Day 12-14)
  - Load Node.js v18 and v20 simultaneously
  - Cache both versions
  - Switch between versions for different projects

**Deliverable:** Runtime caching works, versions auto-detected

**Success Criteria:**
- ✅ First download: < 2s for QuickJS (~1-2MB), < 10s for CPython-WASI (~15MB)
- ✅ Subsequent loads instant (from CacheAPI)
- ✅ Runtime selection respects project config (wasmrun.toml, setup.py presence, etc.)
- ✅ Multiple runtimes and versions cached simultaneously
- ✅ Integrity verification via SHA-256 from manifest

---

### **PRIORITY 6: Node.js & Python Runtimes (Interpreter-Based)** (Week 11-12) 🟢 JS 🐍 PY

**Goal:** Add JavaScript and Python support using interpreter runtimes compiled to `wasm32-wasi`. These build on the WASI bridge already proven with Rust & Go in Priority 3.

> **How this differs from Rust/Go (Priority 3):**
>
> | | Rust / Go (P3) | JS / Python (P6) |
> |---|---|---|
> | **Compilation** | User's code compiles to `.wasm` directly | User's code is **interpreted** by a runtime compiled to `.wasm` |
> | **Binary** | User's project IS the `.wasm` | Runtime (QuickJS/RustPython) is the `.wasm`, user code is input |
> | **Size** | Small (300KB–2MB per project) | Larger (1–15MB for the interpreter runtime) |
> | **WASI bridge** | Same | Same (already built in P3) |
> | **Execution** | `_start()` runs user code directly | `_start()` boots interpreter, reads user code via WASI fs, executes |
>
> **The key insight:** QuickJS and RustPython are themselves compiled to `wasm32-wasi`. They
> use the exact same WASI imports we already built. We just need to:
> 1. Load the interpreter `.wasm` (instead of user-compiled `.wasm`)
> 2. Write the user's source files to WASI filesystem
> 3. Pass the entry file path as a command-line argument via `args_get`

> **Viable JS runtimes (all WASI-compatible):**
>
> | Runtime | Size | JS Compat | WASI | Notes |
> |---------|------|-----------|------|-------|
> | **[QuickJS](https://bellard.org/quickjs/)** via wasi-sdk | ~1-2 MB | ES2023 ✅ | ✅ Preview 1 | Best balance of size & compat |
> | **[Javy](https://github.com/nicmcd/nicmcd-javy)** (Shopify, QuickJS-based) | ~2 MB | ES2023 ✅ | ✅ Preview 1 | Optimized, pre-packaged |
>
> **Viable Python runtimes:**
>
> | Runtime | Size | Compat | WASI | Notes |
> |---------|------|--------|------|-------|
> | **[RustPython](https://rustpython.github.io/)** | ~5 MB | Python 3.10 (partial) | ✅ Can target WASI | Smaller, less stdlib |
> | **[CPython WASI build](https://nicmcd.github.io/nicmcd-cpython-wasi)** | ~15 MB | Full CPython 3.11 | ✅ Preview 1 | Larger, full compat |
> | **[Pyodide](https://pyodide.org/)** | ~35 MB | Full CPython 3.11 | ❌ Emscripten only | Not WASI, doesn't fit our bridge |
>
> **Previous incorrect assumptions (corrected):**
> - **nodebox** (CodeSandbox) is NOT a standalone WASM binary — it's an iframe sandbox backed by cloud infra
> - **StackBlitz WebContainers** is proprietary
> - Building full Node.js/V8 to WASM is not feasible (V8 JIT cannot target WASM)

**Tasks:**

- [ ] **6.1: Build/obtain QuickJS WASI binary** (Day 1-3)

  ```bash
  # Option A: Javy (Shopify's QuickJS-to-WASM, easiest)
  cargo install javy-cli
  # Javy compiles individual JS files to .wasm — but we want the interpreter itself
  # For a general-purpose QuickJS WASM runtime:

  # Option B: Build QuickJS to WASM via wasi-sdk
  git clone https://github.com/nicmcd/nicmcd-nicmcd-nicmcd-nicmcd-nicmcd
  # Or use a maintained QuickJS WASI build:
  # https://nicmcd.github.io/nicmcd/nicmcd-nicmcd (check latest)

  # Verify it runs
  echo 'console.log("Hello from QuickJS WASM!")' > /tmp/test.js
  wasmtime quickjs.wasm -- /tmp/test.js
  # Should print: Hello from QuickJS WASM!
  ```

  **Key requirement:** The QuickJS WASM binary must accept a JS file path as an argument
  and read it via WASI `fd_read` / `path_open`. Our kernel provides those imports.

- [ ] **6.2: Build/obtain Python WASI binary** (Day 3-5)

  ```bash
  # Option A: RustPython (smaller, Rust-native)
  git clone https://github.com/nicmcd/nicmcd-RustPython
  cd RustPython
  cargo build --target wasm32-wasi --release --features freeze-stdlib
  # Output: target/wasm32-wasi/release/rustpython.wasm (~5MB)

  # Verify
  echo 'print("Hello from Python WASM!")' > /tmp/test.py
  wasmtime target/wasm32-wasi/release/rustpython.wasm -- /tmp/test.py
  # Should print: Hello from Python WASM!

  # Option B: CPython WASI (fuller compat, larger)
  # See https://nicmcd.github.io/nicmcd-cpython-wasi for prebuilt binaries
  ```

- [ ] **6.3: Implement Node.js/JS runtime** (Day 5-8)

  ```rust
  // src/runtime/languages/nodejs.rs — REWRITE
  pub struct NodeJSRuntime;

  impl LanguageRuntime for NodeJSRuntime {
      fn name(&self) -> &str { "nodejs" }
      fn extensions(&self) -> &[&str] { &["js", "mjs"] }
      fn entry_files(&self) -> &[&str] { &["package.json", "index.js", "main.js", "app.js"] }

      fn detect_project(&self, project_path: &str) -> bool {
          let p = Path::new(project_path);
          p.join("package.json").exists() || p.join("index.js").exists()
      }

      fn load_wasm_binary(&self) -> Result<Vec<u8>> {
          // Load the QuickJS interpreter compiled to wasm32-wasi
          let loader = RuntimeLoader::new("wasmrun-runtimes");
          loader.load_runtime("js", "quickjs-latest")
      }

      fn prepare_project(&self, project_path: &str) -> Result<ProjectBundle> {
          let files = self.read_project_files(project_path)?;
          let entry = self.detect_entry_point(project_path)?;

          Ok(ProjectBundle {
              name: self.detect_project_name(project_path),
              language: "nodejs".to_string(),
              entry_point: entry,  // e.g., "index.js"
              files,               // all .js files — written to WASI FS
              dependencies: vec![],
              metadata: ProjectMetadata { version: "1.0.0".to_string(), ..Default::default() },
          })
      }

      fn run_project(&self, bundle: ProjectBundle, kernel: &mut WasmMicroKernel) -> Result<Pid> {
          let pid = kernel.create_process(bundle.name.clone(), "nodejs".to_string(), None)?;

          // 1. Write user's JS files to WASI filesystem
          for (path, content) in &bundle.files {
              let vfs_path = format!("/projects/{pid}/{path}");
              kernel.wasi_filesystem().write_file(&vfs_path, content)?;
          }

          // 2. Load QuickJS interpreter WASM
          let interpreter_wasm = self.load_wasm_binary()?;
          kernel.load_wasm_module(pid, &interpreter_wasm)?;

          // 3. Set args: quickjs <entry_file>
          //    When _start() is called, QuickJS reads args via WASI args_get,
          //    opens the file via WASI path_open, and executes it.
          kernel.set_process_args(pid, vec![
              "quickjs".to_string(),
              format!("/projects/{pid}/{}", bundle.entry_point),
          ])?;

          Ok(pid)
      }
      // ...
  }
  ```

- [ ] **6.4: Implement Python runtime** (Day 8-11)

  ```rust
  // src/runtime/languages/python.rs — REWRITE
  pub struct PythonRuntime;

  impl LanguageRuntime for PythonRuntime {
      fn name(&self) -> &str { "python" }
      fn extensions(&self) -> &[&str] { &["py"] }
      fn entry_files(&self) -> &[&str] { &["requirements.txt", "pyproject.toml", "main.py", "app.py"] }

      fn detect_project(&self, project_path: &str) -> bool {
          let p = Path::new(project_path);
          p.join("requirements.txt").exists()
              || p.join("pyproject.toml").exists()
              || p.join("main.py").exists()
      }

      fn load_wasm_binary(&self) -> Result<Vec<u8>> {
          // Load RustPython (or CPython-WASI) interpreter compiled to wasm32-wasi
          let loader = RuntimeLoader::new("wasmrun-runtimes");
          loader.load_runtime("python", "rustpython-latest")
      }

      fn run_project(&self, bundle: ProjectBundle, kernel: &mut WasmMicroKernel) -> Result<Pid> {
          let pid = kernel.create_process(bundle.name.clone(), "python".to_string(), None)?;

          // Write user's .py files to WASI filesystem
          for (path, content) in &bundle.files {
              let vfs_path = format!("/projects/{pid}/{path}");
              kernel.wasi_filesystem().write_file(&vfs_path, content)?;
          }

          // Load Python interpreter WASM
          let interpreter_wasm = self.load_wasm_binary()?;
          kernel.load_wasm_module(pid, &interpreter_wasm)?;

          // Set args: python <entry_file>
          kernel.set_process_args(pid, vec![
              "python".to_string(),
              format!("/projects/{pid}/{}", bundle.entry_point),
          ])?;

          Ok(pid)
      }
      // ...
  }
  ```

- [ ] **6.5: Host interpreter runtimes** (Day 11-12)

  ```bash
  mkdir -p static/runtimes/
  cp quickjs.wasm static/runtimes/js-quickjs-latest.wasm
  cp rustpython.wasm static/runtimes/python-rustpython-latest.wasm
  ```

- [ ] **6.6: Test all four languages** (Day 12-14)

  ```bash
  # Rust (compiled, from P3)
  wasmrun os -p examples/rust-hello     # → "Hello from Rust WASM!"

  # Go (compiled, from P3)
  wasmrun os -p examples/go-hello       # → "Hello from Go WASM!"

  # JavaScript (interpreted via QuickJS)
  wasmrun os -p examples/js-hello       # → "Hello from JS!"
  # examples/js-hello/index.js: console.log("Hello from JS!")

  # Python (interpreted via RustPython)
  wasmrun os -p examples/python-hello   # → "Hello from Python!"
  # examples/python-hello/main.py: print("Hello from Python!")
  ```

  **Multi-language integration tests:**
  - All four languages running simultaneously in browser
  - Each in its own process with isolated WASI filesystem view
  - Verify no cross-contamination between runtimes
  - Test process lifecycle (start, stop, restart) for each language
  - Verify interpreted runtimes use the same WASI bridge as compiled ones

**Deliverable:** Full four-language support (Rust, Go, JS, Python) in browser.

**Success Criteria:**
- ✅ QuickJS WASM loads and interprets JS files via WASI
- ✅ RustPython/CPython-WASI loads and interprets Python files via WASI
- ✅ Both use the **exact same WASI bridge** built in Priority 3
- ✅ User code is passed as files (written to WASI FS) + args (via `args_get`)
- ✅ All four languages can run simultaneously
- ✅ All isolated in browser VM

**Known Limitations:**
- JS: QuickJS subset only — no Node.js APIs (`require('fs')`, `http`, etc.), no npm
- Python: RustPython has partial stdlib; CPython-WASI is fuller but ~15MB
- No package manager integration (npm/pip) — dependencies must be pre-bundled
- Interpreter overhead: ~3-10x slower than native (QuickJS is an interpreter, not JIT)

---

## 📈 Migration Path: Server-Side → Browser-Side

### **Phase 0: Fix Foundation (Week 1-3)**
Fix structural issues before building on top of them.

- Unify dual filesystems
- Fix dev server, template loading, race conditions
- Security hardening
- Clean up dead code

### **Phase A: Dual Mode (Week 3-8)**
Support both server-side and browser-side execution.

```rust
// src/runtime/multilang_kernel.rs
pub enum ExecutionMode {
    ServerSide,  // Current: runs on local machine
    BrowserSide, // Target: runs in browser WASM
}

impl MultiLanguageKernel {
    pub fn run_project(&mut self, mode: ExecutionMode) -> Result<Pid> {
        match mode {
            ExecutionMode::ServerSide => self.run_server_side(),
            ExecutionMode::BrowserSide => self.run_browser_side(),
        }
    }
}
```

### **Phase B: Gradual Migration (Week 8-12)**
Move components one by one.

1. Week 3-4: Kernel runs in browser
2. Week 4-5: File upload works
3. Week 5-8: Rust & Go WASI runtimes execute (validates WASI bridge)
4. Week 8-10: Networking via ServiceWorker + WebSocket relay tunnel
5. Week 10-11: Caching, version management
6. Week 11-12: Node.js (QuickJS) & Python (RustPython) interpreter runtimes

### **Phase C: Deprecate Server-Side (Week 12)**
Browser-only execution.

```rust
// Remove server-side execution code
// OS server only serves static files + WASM binaries
// All execution happens in browser
// Relay server remains for tunneling (honest about this)
```

---

## 🎯 Success Metrics

### **Week 3 Milestone (Foundation Done):**
- ✅ Single unified filesystem — syscalls and HTTP APIs share one FS
- ✅ Dev server correctly serves files
- ✅ Templates embedded — `cargo install` + run from anywhere works
- ✅ No TOCTOU races, CORS restricted
- ✅ All existing tests pass

### **Week 5 Milestone (Kernel in Browser):**
- ✅ Kernel compiled to WASM, loads in browser
- ✅ Can create processes in browser
- ✅ WASI filesystem works in browser
- ✅ File upload via File System Access API works

### **Week 8 Milestone (Compiled Languages Execute):**
- ✅ Rust `wasm32-wasi` binaries execute in browser
- ✅ TinyGo WASI binaries execute in browser
- ✅ WASI bridge routes stdout/stderr/file I/O through our kernel
- ✅ `println!()` / `fmt.Println()` output visible in browser UI
- ✅ WASI bridge is proven and language-agnostic

### **Week 10 Milestone (Networking):**
- ✅ ServiceWorker intercepts internal requests
- ✅ WebSocket connections from WASM
- ✅ Bore tunnel working via WebSocket relay
- ✅ Public URL accessible from anywhere

### **Week 12 Milestone (Interpreted Languages + Polish):**
- ✅ QuickJS WASM interprets JavaScript via the same WASI bridge
- ✅ RustPython/CPython-WASI interprets Python via the same WASI bridge
- ✅ All four languages (Rust, Go, JS, Python) running simultaneously
- ✅ Runtime caching via CacheAPI
- ✅ Browser-only execution (server serves static files + relay only)

---

## 🔧 Current Architecture (Server-Side)

### Architecture Diagram
```
User's Local Machine
├─> wasmrun CLI (cargo run -- os -p ./project)
│   └─> OsRunConfig { project_path, port, language }
│
├─> OsServer (Native Rust HTTP Server on localhost:8420)
│   ├─> Serves static HTML/CSS/JS to browser
│   ├─> REST API endpoints (/api/*)
│   │   ├─> /api/kernel/stats
│   │   ├─> /api/kernel/start
│   │   ├─> /api/kernel/restart
│   │   ├─> /api/fs/list/<path>
│   │   ├─> /api/fs/read/<path>
│   │   └─> /api/fs/write/<path>
│   └─> HTTP Proxy (/app/* → dev server)
│
├─> MultiLanguageKernel (Native Rust)
│   ├─> WasmMicroKernel
│   │   ├─> Process Scheduler (round-robin)
│   │   ├─> WASI Filesystem (mounted from host)
│   │   └─> SyscallHandler (native socket syscalls)
│   ├─> LanguageRuntimeRegistry
│   │   ├─> NodeJSRuntime (placeholder WASM)
│   │   ├─> PythonRuntime (placeholder WASM)
│   │   └─> GoRuntime (placeholder WASM)
│   ├─> DevServerManager (Native Rust threads)
│   │   └─> Serves static files on port 8000+PID
│   └─> NetworkNamespace (per-process isolation)
│
├─> BoreClient (Native Rust - std::net::TcpStream)
│   ├─> Connects to bore.pub:7835 from HOST
│   ├─> Exposes localhost:8420 publicly
│   └─> Returns http://bore.pub:12345
│
└─> Browser (http://localhost:8420)
    ├─> React UI (templates/os/os.js)
    │   ├─> File Browser (fetches from /api/fs/*)
    │   ├─> Kernel Dashboard (fetches from /api/kernel/stats)
    │   └─> Application Panel (iframe to /app/)
    └─> NO WASM EXECUTION

Request Flow:
1. User → wasmrun os -p ./my-project
2. OsServer starts on localhost:8420
3. Browser opens http://localhost:8420
4. Browser fetches React UI from server
5. React UI fetches data via REST API
6. Data comes from SERVER (not browser WASM)
7. BoreClient (on HOST) exposes to Internet
```

### File Structure (Current)
```
src/
├─> commands/os.rs              # CLI: wasmrun os -p <path>
├─> runtime/
│   ├─> os_server.rs           # ✅ HTTP server (localhost:8420) — ⚠️ CORS:*, no auth, TOCTOU races
│   ├─> multilang_kernel.rs    # ✅ Kernel orchestration (runs on HOST)
│   ├─> microkernel.rs         # ⚠️ Process bookkeeping + DISCONNECTED in-memory VFS
│   ├─> wasi_fs.rs             # ✅ WASI Filesystem (HOST mounts) — used by HTTP API only
│   ├─> syscalls.rs            # ⚠️ Syscalls use real host sockets (unsandboxed), broken sock_open
│   ├─> scheduler.rs           # ⚠️ Data structure only — schedule_next() never called
│   ├─> dev_server.rs          # ⚠️ Spawns threads but reads wrong FS (broken)
│   ├─> network_namespace.rs   # ⚠️ Bookkeeping only — no real isolation
│   ├─> tunnel/
│   │   └─> bore.rs            # ✅ Bore client (HOST - std::net::TcpStream)
│   └─> languages/
│       ├─> nodejs.rs          # ⚠️ Placeholder (returns fake WASM bytes)
│       ├─> python.rs          # ⚠️ Empty stub — no LanguageRuntime impl
│       └─> go.rs              # ⚠️ Empty stub — no LanguageRuntime impl
└─> templates/os/
    ├─> index.html             # ✅ UI shell — ⚠️ loaded via CWD-relative path
    ├─> os.js                  # ✅ React dashboard
    ├─> index.css              # ✅ Styles
    └─> wasmrun_wasi_impl.js   # ⚠️ Not used (no WASM in browser)
```

---

## 🎯 Target Architecture (Browser-Side)

### Architecture Diagram
```
Browser (code execution here)
├─> wasmrun Kernel (Rust compiled to WASM ~2-3MB)
│   ├─> pkg/wasmrun_bg.wasm
│   └─> pkg/wasmrun.js
│
├─> WASM Kernel Instance
│   ├─> MultiLanguageKernel (in WASM)
│   │   ├─> WasmMicroKernel
│   │   │   ├─> Unified WASI Filesystem (in WASM memory)
│   │   │   └─> SyscallHandler (WASI imports)
│   │   └─> RuntimeLoader
│   │       ├─> Downloads from CDN/cache
│   │       ├─> CacheAPI storage
│   │       └─> Version management
│   │
│   ├─> Language Runtimes (all wasm32-wasi, loaded on-demand)
│   │   ├─> [Compiled] User's Rust .wasm (~60KB) - from wasmhub or cargo build --target wasm32-wasi
│   │   ├─> [Compiled] User's Go .wasm (~260KB) - from wasmhub or tinygo build -target=wasi
│   │   ├─> [Interpreter] quickjs.wasm (~1-2MB) - JS execution, reads .js files via WASI
│   │   └─> [Interpreter] rustpython.wasm (~5MB) - Python execution, reads .py files via WASI
│   │
│   ├─> User Project Files (uploaded via File System Access API)
│   │   ├─> Cargo.toml / go.mod / package.json / requirements.txt
│   │   ├─> src/main.rs / main.go / index.js / main.py
│   │   ├─> Precompiled .wasm (for Rust/Go — compiled before upload)
│   │   └─> Source files only (for JS/Python — interpreted at runtime)
│   │
│   └─> BoreClient (WebSocket to relay server)
│       ├─> wss://relay.wasmrun.dev → TCP bore.pub:7835
│       └─> Creates public tunnel from browser
│
├─> Virtual Networking (ServiceWorker)
│   ├─> Intercepts fetch() calls from WASM
│   ├─> Routes internal requests to kernel
│   └─> External requests pass through
│
└─> WebSocket connections (only network primitive available in browser)

Server Infrastructure (static files + relay)
├─> CDN / Static Server
│   ├─> /pkg/wasmrun_bg.wasm        # Kernel binary (~2-3MB)
│   ├─> /pkg/wasmrun.js             # JS bindings
│   └─> /runtimes/                   # Interpreter runtimes only (Rust/Go .wasm is user-compiled)
│       ├─> manifest.json           # Version info
│       ├─> js-quickjs-latest.wasm  # QuickJS interpreter (~1-2MB)
│       └─> python-rustpython.wasm  # RustPython interpreter (~5MB)
│
└─> WebSocket Relay Server (~50 lines of code)
    ├─> Accepts: wss://relay.wasmrun.dev/tunnel
    ├─> Connects: TCP bore.pub:7835
    └─> Bridges: WebSocket frames ↔ TCP bytes

Request Flow:
1. User opens https://wasmrun.dev
2. Browser downloads kernel WASM (~2MB)
3. User uploads project via File Picker
4a. [Rust/Go] User uploads precompiled .wasm → kernel instantiates with WASI imports → runs
4b. [JS/Python] Browser loads interpreter .wasm from cache → writes user source to WASI FS → runs
5. WASI bridge routes all I/O through kernel (same bridge for all 4 languages)
6. Bore tunnel created via WebSocket relay
7. Public URL accessible worldwide
8. Code execution in browser; relay server only bridges tunnel protocol
```

### File Structure (Target)
```
src/
├─> lib.rs                     # WASM bindings (wasm-bindgen)
├─> commands/os.rs              # CLI (minimal — serves static files + pkg/)
├─> runtime/
│   ├─> microkernel.rs         # Unified kernel (no dual FS — uses wasi_fs only)
│   ├─> wasi_fs.rs             # WASI filesystem (in-memory for browser, host-mounted for CLI)
│   ├─> multilang_kernel.rs    # Orchestration
│   ├─> runtime_loader.rs      # Download/cache runtimes via CacheAPI
│   ├─> wasm_bridge.rs         # WASI imports bridge for runtime WASM modules
│   ├─> syscalls_browser.rs    # Browser socket layer (WebSocket/fetch, NOT raw TCP)
│   ├─> tunnel/
│   │   ├─> bore.rs            # Host-side bore client (kept for CLI mode)
│   │   └─> bore_browser.rs    # Browser bore client (WebSocket via relay)
│   └─> languages/
│       ├─> rust.rs            # [P3] Compiled: loads user's wasm32-wasi binary
│       ├─> go.rs              # [P3] Compiled: loads user's TinyGo WASI binary
│       ├─> nodejs.rs          # [P6] Interpreter: loads QuickJS WASM + user's .js files
│       └─> python.rs          # [P6] Interpreter: loads RustPython WASM + user's .py files
├─> templates/os/              # Embedded via include_str!
│   ├─> index.html             # UI shell
│   ├─> kernel-loader.js       # Load WASM kernel
│   ├─> file-upload.js         # File System Access API
│   ├─> service-worker.js      # Virtual networking
│   └─> os.js                  # React UI
│
relay-server/                   # Separate tiny binary
├─> Cargo.toml
└─> src/main.rs                 # WebSocket↔TCP bridge for bore (~50 lines)
```

---

## 🚨 Critical Blockers & Solutions

### **Blocker 0: Broken Foundation**
**Problem:** Dual filesystems, broken dev server, race conditions, security holes, dead scheduler
**Solution:** Priority 0 — must fix before any browser work begins
**Risk if skipped:** Browser migration builds on broken abstractions; bugs carry forward and multiply

### **Blocker 1: No Actual WASM Runtimes**
**Problem:** `nodejs.rs` returns a fake placeholder WASM. Python/Go/Rust have no real `LanguageRuntime` impl.
**Solution:** Priority 3 (Rust & Go compiled to wasm32-wasi) validates the WASI bridge first, then Priority 6 (QuickJS for JS, RustPython for Python) builds on the proven bridge.
**Previous incorrect assumption:** nodebox was proposed but it's not a standalone WASM binary

### **Blocker 2: Kernel Not Compiled to WASM**
**Problem:** Kernel is native Rust, not WASM
**Solution:** Priority 1 - Add wasm-bindgen, compile to WASM

### **Blocker 3: No File Upload**
**Problem:** Can't load projects into browser
**Solution:** Priority 2 - File System Access API

### **Blocker 4: Browsers Cannot Do Raw TCP**
**Problem:** `bore.rs:53` uses `std::net::TcpStream`. Browsers don't support raw TCP/UDP sockets at all. bore.pub uses raw TCP on port 7835.
**Solution:** Priority 4 — WebSocket relay server as bridge, ServiceWorker for internal networking
**Previous incorrect assumption:** Plan showed `sock_open(AF_INET, SOCK_STREAM)` from browser WASM, which is impossible

### **Blocker 5: No Browser Testing**
**Problem:** Everything tested on server-side
**Solution:** Create `examples/browser/` with E2E tests

### **Blocker 6: Template Loading Breaks on Install**
**Problem:** `Path::new("templates/os")` is CWD-relative; fails after `cargo install`
**Solution:** Priority 0 task 0.3 — embed templates via `include_str!`

---

## 📚 Reference Documentation

### **Key Files to Modify (Phase 0 — Foundation):**
1. **src/runtime/microkernel.rs** - Remove in-memory VFS HashMap, delegate to `wasi_fs`
2. **src/runtime/dev_server.rs** - Fix to read through WASI FS, fix stop signal
3. **src/runtime/os_server.rs** - Embed templates, fix TOCTOU races, restrict CORS
4. **src/runtime/syscalls.rs** - Fix `sock_open`, add path validation, remove dead syscalls
5. **src/runtime/network_namespace.rs** - Fix port allocation overflow

### **Key Files to Modify (Phase A — Browser Migration):**
1. **src/lib.rs** - Add wasm-bindgen for WASM compilation
2. **src/runtime/wasm_bridge.rs** (new) - WASI Preview 1 import bridge (core piece, language-agnostic)
3. **src/runtime/languages/rust.rs** (new) - [P3] Rust LanguageRuntime (compiled wasm32-wasi)
4. **src/runtime/languages/go.rs** - [P3] Rewrite: Go LanguageRuntime (compiled TinyGo WASI)
5. **src/runtime/languages/nodejs.rs** - [P6] Rewrite: loads QuickJS interpreter WASM + user's .js
6. **src/runtime/languages/python.rs** - [P6] Rewrite: loads RustPython interpreter WASM + user's .py
7. **src/runtime/runtime_loader.rs** (new) - Download/cache interpreter runtimes via CacheAPI
8. **src/runtime/tunnel/bore_browser.rs** (new) - WebSocket-based bore client
9. **relay-server/** (new, separate) - WebSocket↔TCP relay for bore
10. **templates/os/service-worker.js** (new) - Virtual networking in browser
11. **Cargo.toml** - Add wasm-bindgen, wasm-bindgen-futures, js-sys, web-sys dependencies

### **Testing Checklist:**

**Phase 0 (Foundation):**
- [ ] `NodeJSRuntime::run_project()` writes files visible via `/api/fs/read/`
- [ ] Dev server serves actual project files (not 404s)
- [ ] `cargo install wasmrun && wasmrun os -p ./project` works (templates load)
- [ ] Concurrent `/api/kernel/start` requests don't race
- [ ] CORS header is restrictive by default
- [ ] All existing `cargo test` pass

**Phase A (Browser — Compiled Languages, P1-P3):**
- [ ] Browser console shows "Kernel running in browser WASM VM!"
- [ ] Can upload project files via File Picker
- [ ] Rust `wasm32-wasi` binary executes — `println!()` output visible in browser
- [ ] TinyGo WASI binary executes — `fmt.Println()` output visible in browser
- [ ] WASI file I/O works (read/write through kernel filesystem)
- [ ] WASI bridge is language-agnostic (same code serves both Rust and Go)

**Phase B (Browser — Networking + Interpreted Languages, P4-P6):**
- [ ] Bore tunnel created via WebSocket relay — public URL works
- [ ] QuickJS interprets `console.log("Hello")` — output visible (uses same WASI bridge)
- [ ] RustPython interprets `print("Hello")` — output visible (uses same WASI bridge)
- [ ] All four languages can run simultaneously in isolated processes
- [ ] No server-side code execution (verify with `ps aux | grep wasmrun`)

### **Performance Targets:**
- Kernel load: < 1s (2-3MB WASM)
- Rust/Go .wasm instantiation: < 100ms (60-260KB from wasmhub, near-instant)
- QuickJS runtime download: < 2s (~1-2MB, much smaller than previous 50MB assumption)
- Python runtime download: < 10s (~5-15MB depending on RustPython vs CPython-WASI)
- Runtime cache hit: < 100ms
- Compiled language execution (Rust/Go): Within 2x of native
- Interpreted language execution (JS/Python): Within 5-10x of native (interpreter overhead)
- Tunnel creation: < 5s (includes relay hop)

---

## 🎉 Final Vision

### **Week 12 Demo:**
```bash
# Minimal server needed (static files + WebSocket relay for tunneling)
# Open browser to https://wasmrun.dev

# In browser — Rust project:
1. Compile locally: cargo build --target wasm32-wasi --release
2. Upload .wasm via File Picker → executes instantly
3. Output streams to browser console via WASI fd_write

# In browser — Go project:
1. Compile locally: tinygo build -target=wasi -o main.wasm .
2. Upload .wasm via File Picker → executes instantly

# In browser — JS project:
1. Upload project folder (File Picker)
2. QuickJS interpreter loads from cache (~1-2MB, first-time download ~2s)
3. JS source files written to WASI FS, interpreted by QuickJS

# In browser — Python project:
1. Upload project folder (File Picker)
2. RustPython interpreter loads from cache (~5MB, first-time download ~5s)
3. Python source files written to WASI FS, interpreted by RustPython

# ALL code execution in browser
# Same WASI bridge for all 4 languages
# Server only serves static files + tunnel relay
```

### **Success Criteria:**
- ✅ Compiled language execution in browser (Rust via wasm32-wasi, Go via TinyGo WASI)
- ✅ Interpreted language execution in browser (JS via QuickJS, Python via RustPython)
- ✅ All four languages share a single WASI bridge (built once, works for all)
- ✅ No server-side code execution (server only serves static files)
- ✅ Public tunneling via WebSocket relay (honest about the relay dependency)
- ✅ Interpreter runtime caching via CacheAPI
- ✅ File upload via File System Access API
- ✅ Virtual networking via ServiceWorker
- ✅ Clean foundation (unified FS, no races, embedded templates, security hardened)

### **Honest Differences from StackBlitz WebContainers:**
- WebContainers runs full Node.js with npm — we run QuickJS (ES2023 but no Node.js APIs)
- WebContainers has proprietary V8-in-WASM tech — we use open-source lightweight runtimes
- WebContainers does full npm install — we require pre-bundled dependencies (for now)
- We support Rust, Go, and Python out of the box — WebContainers is JS-only
- Rust & Go run at near-native speed (compiled to WASM) — not possible in WebContainers
- Our tunnel requires a relay server — WebContainers has proprietary networking

---

**Last Updated:** 2026-02-16
**Status:** Server-Side Simulation → Foundation Fixes → Browser Migration
**ETA:** 9-12 weeks (3 weeks foundation + 9 weeks browser migration)
