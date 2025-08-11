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

impl BuildConfig {
    #[allow(dead_code)]
    pub fn is_wasm_bindgen(&self) -> bool {
        match &self.target_type {
            crate::compiler::builder::TargetType::Web => true,
            _ => false,
        }
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

    pub type CreateBuilderFn = unsafe extern "C" fn() -> *mut c_void;
    pub type CanHandleProjectFn =
        unsafe extern "C" fn(*const c_void, *const std::ffi::c_char) -> bool;
    pub type BuildFn =
        unsafe extern "C" fn(*const c_void, *const super::BuildConfigC) -> *mut super::BuildResultC;
    pub type CleanFn = unsafe extern "C" fn(*const c_void, *const std::ffi::c_char) -> bool;
    #[allow(dead_code)]
    pub type CloneBoxFn = unsafe extern "C" fn(*const c_void) -> *mut c_void;
    #[allow(dead_code)]
    pub type DropFn = unsafe extern "C" fn(*mut c_void);
    #[allow(dead_code)]
    pub type FreeBuildResultFn = unsafe extern "C" fn(*mut super::BuildResultC);
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
