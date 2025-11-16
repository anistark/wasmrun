/// WASM value types and operations

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Value {
    I32(i32),
    I64(i64),
    F32(f32),
    F64(f64),
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
}
