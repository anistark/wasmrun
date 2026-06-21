/// WASM value types and operations

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Value {
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
    /// A `funcref`. `Some(func_idx)` is a reference to a function in the
    /// module's function index space; `None` is the null reference.
    FuncRef(Option<u32>),
    /// An `externref`. The interpreter has no host object table yet, so an
    /// external reference is modeled as an opaque handle: `Some(handle)` is a
    /// non-null reference, `None` is null. Handles round-trip through locals,
    /// globals, tables, and params unchanged.
    ExternRef(Option<u32>),
}

impl Value {
    pub fn from_i32(v: i32) -> Self {
        Value::I32(v)
    }

    pub fn from_i64(v: i64) -> Self {
        Value::I64(v)
    }

    pub fn from_f32(v: f32) -> Self {
        Value::F32(v)
    }

    pub fn from_f64(v: f64) -> Self {
        Value::F64(v)
    }

    /// The null `funcref`.
    pub fn null_funcref() -> Self {
        Value::FuncRef(None)
    }

    /// The null `externref`.
    pub fn null_externref() -> Self {
        Value::ExternRef(None)
    }

    /// True for a reference value (`funcref`/`externref`), false for a number.
    pub fn is_ref(&self) -> bool {
        matches!(self, Value::FuncRef(_) | Value::ExternRef(_))
    }

    /// True when this is a null reference. Returns `false` for numbers.
    pub fn is_null_ref(&self) -> bool {
        matches!(self, Value::FuncRef(None) | Value::ExternRef(None))
    }

    /// The function index of a `funcref`, if this is a non-null `funcref`.
    pub fn as_func_idx(&self) -> Option<u32> {
        match self {
            Value::FuncRef(f) => *f,
            _ => None,
        }
    }
}
