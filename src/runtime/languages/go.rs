// TODO: Implement Go runtime with TinyGo WASM

#[allow(dead_code)] // TODO: Will be used when Go runtime is fully implemented
pub struct GoRuntime;

#[allow(dead_code)]
impl GoRuntime {
    pub fn new() -> Self {
        Self
    }
}

impl Default for GoRuntime {
    fn default() -> Self {
        Self::new()
    }
}
