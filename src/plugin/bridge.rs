use crate::compiler::builder::{BuildConfig, BuildResult};
use std::ffi::{c_char, CString};

#[repr(C)]
pub struct BuildConfigC {
    pub project_path: *const c_char,
    pub output_dir: *const c_char,
    pub watch: bool,
}

#[repr(C)]
pub struct BuildResultC {
    pub wasm_path: *const c_char,
    pub js_path: *const c_char,
    pub is_wasm_bindgen: bool,
    pub success: bool,
    pub error_message: *const c_char,
}

#[repr(C)]
pub struct StringArrayC {
    pub data: *const *const c_char,
    pub len: usize,
}

#[repr(C)]
#[allow(dead_code)]
pub struct PluginInfoC {
    pub name: *const c_char,
    pub version: *const c_char,
    pub description: *const c_char,
    pub author: *const c_char,
    pub extensions: StringArrayC,
    pub entry_files: StringArrayC,
}

#[repr(C)]
pub struct WaspyCompileResult {
    pub success: bool,
    pub wasm_data: *mut u8,
    pub wasm_len: usize,
    pub error_message: *mut c_char,
}

impl BuildConfig {
    #[allow(dead_code)]
    pub fn is_wasm_bindgen(&self) -> bool {
        matches!(&self.target_type, crate::compiler::builder::TargetType::Web)
    }
}

impl BuildResult {
    #[allow(dead_code)]
    pub fn get_output_path(&self) -> &str {
        &self.wasm_path
    }

    #[allow(dead_code)]
    pub fn get_artifacts(&self) -> Vec<&str> {
        let mut artifacts = vec![self.wasm_path.as_str()];
        if let Some(ref js_path) = self.js_path {
            artifacts.push(js_path);
        }
        for file in &self.additional_files {
            artifacts.push(file);
        }
        artifacts
    }

    #[allow(dead_code)]
    pub fn get_entry_point(&self) -> Option<&str> {
        Some("main")
    }
}

pub mod symbols {
    use std::ffi::c_void;
    use std::os::raw::{c_char, c_int};

    // Old API (deprecated)
    pub type CreateBuilderFn = unsafe extern "C" fn() -> *mut c_void;
    pub type CanHandleProjectFn = unsafe extern "C" fn(*const c_void, *const c_char) -> bool;
    pub type BuildFn =
        unsafe extern "C" fn(*const c_void, *const super::BuildConfigC) -> *mut super::BuildResultC;
    pub type CleanFn = unsafe extern "C" fn(*const c_void, *const c_char) -> bool;
    #[allow(dead_code)]
    pub type CloneBoxFn = unsafe extern "C" fn(*const c_void) -> *mut c_void;
    #[allow(dead_code)]
    pub type DropFn = unsafe extern "C" fn(*mut c_void);
    #[allow(dead_code)]
    pub type FreeBuildResultFn = unsafe extern "C" fn(*mut super::BuildResultC);

    // New API - Plugin object methods
    pub type PluginCreateFn = unsafe extern "C" fn() -> *mut c_void;
    #[allow(dead_code)]
    pub type PluginInfoFn = unsafe extern "C" fn(*const c_void) -> *const super::PluginInfoC;
    #[allow(dead_code)]
    pub type PluginCanHandleFn = unsafe extern "C" fn(*const c_void, *const c_char) -> bool;
    #[allow(dead_code)]
    pub type PluginGetBuilderFn = unsafe extern "C" fn(*const c_void) -> *mut c_void;
    #[allow(dead_code)]
    pub type PluginDropFn = unsafe extern "C" fn(*mut c_void);

    // WasmBuilder methods (for new API)
    #[allow(dead_code)]
    pub type BuilderBuildFn =
        unsafe extern "C" fn(*const c_void, *const super::BuildConfigC) -> *mut super::BuildResultC;
    #[allow(dead_code)]
    pub type BuilderCanHandleFn = unsafe extern "C" fn(*const c_void, *const c_char) -> bool;
    #[allow(dead_code)]
    pub type BuilderCheckDepsFn = unsafe extern "C" fn(*const c_void) -> *mut super::StringArrayC;
    #[allow(dead_code)]
    pub type BuilderCleanFn = unsafe extern "C" fn(*const c_void, *const c_char) -> bool;
    #[allow(dead_code)]
    pub type BuilderDropFn = unsafe extern "C" fn(*mut c_void);

    // Waspy FFI - Direct compilation functions
    pub type WaspyCompilePythonFn =
        unsafe extern "C" fn(*const c_char, c_int) -> super::WaspyCompileResult;
    pub type WaspyCompileProjectFn =
        unsafe extern "C" fn(*const c_char, c_int) -> super::WaspyCompileResult;
    pub type WaspyFreeWasmDataFn = unsafe extern "C" fn(*mut u8, usize);
    pub type WaspyFreeErrorMessageFn = unsafe extern "C" fn(*mut c_char);
}

#[allow(dead_code)]
pub struct PluginSymbols;

impl PluginSymbols {
    #[allow(dead_code)]
    pub fn get_symbol_names(_plugin_name: &str) -> PluginSymbolSet {
        // Use generic naming for all plugins
        PluginSymbolSet {
            create_builder: b"create_wasm_builder",
            can_handle_project: b"can_handle_project",
            build: b"build",
            clean: b"clean",
            clone_box: b"clone_box",
            drop: b"drop",
            free_build_result: b"free_build_result",
        }
    }

    #[allow(dead_code)]
    pub fn get_generic_symbol_names(_plugin_name: &str) -> PluginSymbolSet {
        PluginSymbolSet {
            create_builder: b"create_wasm_builder",
            can_handle_project: b"can_handle_project",
            build: b"build",
            clean: b"clean",
            clone_box: b"clone_box",
            drop: b"drop",
            free_build_result: b"free_build_result",
        }
    }

    #[allow(dead_code)]
    pub fn from_metadata(_exports: &crate::plugin::metadata::MetadataExports) -> PluginSymbolSet {
        PluginSymbolSet {
            create_builder: b"create_wasm_builder",
            can_handle_project: b"can_handle_project",
            build: b"build",
            clean: b"clean",
            clone_box: b"clone_box",
            drop: b"drop",
            free_build_result: b"free_build_result",
        }
    }
}

#[allow(dead_code)]
pub struct PluginSymbolSet {
    pub create_builder: &'static [u8],
    pub can_handle_project: &'static [u8],
    pub build: &'static [u8],
    pub clean: &'static [u8],
    pub clone_box: &'static [u8],
    pub drop: &'static [u8],
    pub free_build_result: &'static [u8],
}

impl BuildConfigC {
    pub fn from_build_config(config: &BuildConfig) -> Self {
        let project_path = CString::new(config.project_path.clone()).unwrap_or_default();
        let output_dir = CString::new(config.output_dir.clone()).unwrap_or_default();

        Self {
            project_path: project_path.into_raw(),
            output_dir: output_dir.into_raw(),
            watch: config.watch,
        }
    }
}

impl StringArrayC {
    /// Convert from Rust Vec<String> to C array
    /// Caller must free the returned pointers
    #[allow(dead_code)]
    pub fn from_vec(strings: &[String]) -> Self {
        let c_strings: Vec<CString> = strings
            .iter()
            .map(|s| CString::new(s.as_str()).unwrap_or_default())
            .collect();

        let ptrs: Vec<*const c_char> = c_strings.iter().map(|s| s.as_ptr()).collect();

        // Leak the strings so they remain valid
        std::mem::forget(c_strings);

        let data = ptrs.as_ptr();
        let len = ptrs.len();
        std::mem::forget(ptrs);

        Self { data, len }
    }

    /// Convert from C array to Rust Vec<String>
    #[allow(dead_code)]
    pub unsafe fn to_vec(&self) -> Vec<String> {
        if self.data.is_null() || self.len == 0 {
            return vec![];
        }

        let slice = std::slice::from_raw_parts(self.data, self.len);
        slice
            .iter()
            .filter_map(|&ptr| {
                if ptr.is_null() {
                    None
                } else {
                    std::ffi::CStr::from_ptr(ptr)
                        .to_str()
                        .ok()
                        .map(String::from)
                }
            })
            .collect()
    }
}

impl BuildResultC {
    pub fn to_build_result(ptr: *mut BuildResultC) -> BuildResult {
        unsafe {
            if ptr.is_null() {
                return BuildResult {
                    wasm_path: String::new(),
                    js_path: None,
                    additional_files: vec![],
                    is_wasm_bindgen: false,
                };
            }

            let result = &*ptr;
            let wasm_path = if result.wasm_path.is_null() {
                String::new()
            } else {
                std::ffi::CStr::from_ptr(result.wasm_path)
                    .to_string_lossy()
                    .to_string()
            };

            let js_path = if result.js_path.is_null() {
                None
            } else {
                Some(
                    std::ffi::CStr::from_ptr(result.js_path)
                        .to_string_lossy()
                        .to_string(),
                )
            };

            BuildResult {
                wasm_path,
                js_path,
                additional_files: vec![],
                is_wasm_bindgen: result.is_wasm_bindgen,
            }
        }
    }
}

#[allow(dead_code)]
pub trait BuildConfigExt {
    fn to_c(&self) -> BuildConfigC;
}

impl BuildConfigExt for BuildConfig {
    fn to_c(&self) -> BuildConfigC {
        BuildConfigC::from_build_config(self)
    }
}

#[allow(dead_code)]
pub trait BuildResultExt {
    fn from_c_ptr(ptr: *mut BuildResultC) -> Self;
}

impl BuildResultExt for BuildResult {
    fn from_c_ptr(ptr: *mut BuildResultC) -> Self {
        BuildResultC::to_build_result(ptr)
    }
}
