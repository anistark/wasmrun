//! Bridge for converting between wasmrun and external plugin types

use crate::compiler::builder::{BuildConfig, BuildResult, OptimizationLevel};
use std::ffi::{CStr, CString};

/// Convert wasmrun OptimizationLevel to C representation
#[allow(dead_code)] // TODO: Future C plugin bridge
pub fn optimization_to_c(opt: &OptimizationLevel) -> u8 {
    match opt {
        OptimizationLevel::Debug => 0,
        OptimizationLevel::Release => 1,
        OptimizationLevel::Size => 2,
    }
}

/// Convert wasmrun BuildConfig to C struct for external plugins
#[allow(dead_code)] // TODO: Future C FFI bridge
pub struct BuildConfigC {
    pub project_path: *const std::ffi::c_char,
    pub output_dir: *const std::ffi::c_char,
    pub optimization_level: u8,
    pub verbose: bool,
    pub watch: bool,
}

impl BuildConfigC {
    #[allow(dead_code)] // TODO: Future C FFI bridge
    pub fn from_wasmrun_config(config: &BuildConfig) -> (Self, CString, CString) {
        let project_path_cstr = CString::new(config.project_path.clone()).unwrap();
        let output_dir_cstr = CString::new(config.output_dir.clone()).unwrap();

        let config_c = Self {
            project_path: project_path_cstr.as_ptr(),
            output_dir: output_dir_cstr.as_ptr(),
            optimization_level: optimization_to_c(&config.optimization_level),
            verbose: config.verbose,
            watch: config.watch,
        };

        (config_c, project_path_cstr, output_dir_cstr)
    }
}

/// C struct for build results from external plugins
#[repr(C)]
#[allow(dead_code)] // TODO: Future C FFI bridge
pub struct BuildResultC {
    pub wasm_path: *mut std::ffi::c_char,
    pub js_path: *mut std::ffi::c_char,
    pub is_wasm_bindgen: bool,
    pub success: bool,
    pub error_message: *mut std::ffi::c_char,
}

impl BuildResultC {
    #[allow(dead_code)] // TODO: Future C FFI bridge
    pub unsafe fn into_wasmrun_result(self) -> Result<BuildResult, String> {
        if !self.success {
            let error_msg = if !self.error_message.is_null() {
                CStr::from_ptr(self.error_message)
                    .to_string_lossy()
                    .to_string()
            } else {
                "Unknown error".to_string()
            };
            return Err(error_msg);
        }

        let wasm_path = CStr::from_ptr(self.wasm_path).to_string_lossy().to_string();
        let js_path = if !self.js_path.is_null() {
            Some(CStr::from_ptr(self.js_path).to_string_lossy().to_string())
        } else {
            None
        };

        Ok(BuildResult {
            wasm_path,
            js_path,
            additional_files: vec![],
            is_wasm_bindgen: self.is_wasm_bindgen,
        })
    }
}

/// Helper functions for working with external plugin symbols
#[allow(dead_code)] // TODO: Future C plugin bridge
pub mod symbols {
    use std::ffi::c_void;

    pub type CreateBuilderFn = unsafe extern "C" fn() -> *mut c_void;
    pub type CanHandleProjectFn =
        unsafe extern "C" fn(*const c_void, *const std::ffi::c_char) -> bool;
    pub type BuildFn =
        unsafe extern "C" fn(*const c_void, *const super::BuildConfigC) -> *mut super::BuildResultC;
    pub type CleanFn = unsafe extern "C" fn(*const c_void, *const std::ffi::c_char) -> bool;
    pub type CloneBoxFn = unsafe extern "C" fn(*const c_void) -> *mut c_void;
    pub type DropFn = unsafe extern "C" fn(*mut c_void);
    pub type FreeBuildResultFn = unsafe extern "C" fn(*mut super::BuildResultC);
}

/// Plugin symbol names for different external plugins
pub struct PluginSymbols;

impl PluginSymbols {
    pub fn get_symbol_names(plugin_name: &str) -> PluginSymbolSet {
        match plugin_name {
            "wasmrust" => PluginSymbolSet {
                create_builder: b"create_wasm_builder",
                can_handle_project: b"wasmrust_can_handle_project",
                build: b"wasmrust_build",
                clean: b"wasmrust_clean",
                clone_box: b"wasmrust_clone_box",
                drop: b"wasmrust_drop",
                free_build_result: b"wasmrust_free_build_result",
            },
            "wasmgo" => PluginSymbolSet {
                create_builder: b"create_wasm_builder",
                can_handle_project: b"wasmgo_can_handle_project",
                build: b"wasmgo_build",
                clean: b"wasmgo_clean",
                clone_box: b"wasmgo_clone_box",
                drop: b"wasmgo_drop",
                free_build_result: b"wasmgo_free_build_result",
            },
            _ => PluginSymbolSet {
                create_builder: b"create_wasm_builder",
                can_handle_project: b"can_handle_project",
                build: b"build",
                clean: b"clean",
                clone_box: b"clone_box",
                drop: b"drop",
                free_build_result: b"free_build_result",
            },
        }
    }
}

#[allow(dead_code)] // TODO: Future plugin system symbols
pub struct PluginSymbolSet {
    pub create_builder: &'static [u8],
    pub can_handle_project: &'static [u8],
    pub build: &'static [u8],
    pub clean: &'static [u8],
    pub clone_box: &'static [u8],
    pub drop: &'static [u8],
    pub free_build_result: &'static [u8],
}
