//! Mutation fuzzing for the module parser, the executor and linear memory.
//!
//! wasmrun is a binary crate, so cargo-fuzz (which links a library) cannot
//! reach the parser without restructuring the crate. This runs inside
//! `cargo test` instead: seed modules covering the instruction groups most
//! likely to hide a bug are mutated with a seeded generator, and every result
//! goes through `Module::parse`, instantiation, and a fuel-capped call of each
//! export. Errors are expected; a panic fails the test, and the seed in the
//! failure message reproduces it.
//!
//! `WASMRUN_FUZZ_ITERS` raises the iteration count per seed module (the
//! default keeps the suite fast), and `WASMRUN_FUZZ_SEED` picks the generator
//! seed, for a longer run:
//!
//! ```sh
//! WASMRUN_FUZZ_ITERS=200000 WASMRUN_FUZZ_SEED=7 cargo test --release fuzz -- --nocapture
//! ```

use super::executor::Executor;
use super::memory::LinearMemory;
use super::module::{ExportKind, Module, ValueType};
use super::values::Value;
use crate::runtime::wasi::{create_wasi_linker, WasiEnv};
use std::sync::{Arc, Mutex};

const FUEL: u64 = 20_000;

/// xorshift64*: small, seedable, and good enough to pick mutations.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }
    fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            0
        } else {
            (self.next() % n as u64) as usize
        }
    }
}

const SEEDS: &[&str] = &[
    // Memory: loads and stores of every width, grow, bulk ops, data segments.
    r#"(module
        (memory (export "mem") 1 4)
        (data (i32.const 8) "hello world")
        (func (export "mem") (param i32 i32) (result i64)
          (i32.store8 (local.get 0) (local.get 1))
          (i32.store16 offset=2 (local.get 0) (local.get 1))
          (i64.store offset=4 align=1 (local.get 0) (i64.extend_i32_s (local.get 1)))
          (memory.copy (local.get 1) (local.get 0) (i32.const 16))
          (memory.fill (local.get 0) (i32.const 7) (local.get 1))
          (drop (memory.grow (local.get 1)))
          (i64.add (i64.load (local.get 0)) (i64.extend_i32_u (i32.load8_u offset=1 (local.get 1))))))"#,
    // Tables, call_indirect, elements, and reference types.
    r#"(module
        (type $t (func (param i32) (result i32)))
        (table 4 funcref)
        (elem (i32.const 0) $dbl $inc)
        (func $dbl (type $t) (i32.mul (local.get 0) (i32.const 2)))
        (func $inc (type $t) (i32.add (local.get 0) (i32.const 1)))
        (func (export "dispatch") (param i32 i32) (result i32)
          (call_indirect (type $t) (local.get 1) (local.get 0)))
        (func (export "refs") (param i32) (result i32)
          (ref.is_null (table.get (local.get 0)))))"#,
    // Control flow: blocks with values, br_table, loops, recursion.
    r#"(module
        (func $fib (export "fib") (param i32) (result i32)
          (if (result i32) (i32.lt_u (local.get 0) (i32.const 2))
            (then (local.get 0))
            (else (i32.add (call $fib (i32.sub (local.get 0) (i32.const 1)))
                           (call $fib (i32.sub (local.get 0) (i32.const 2)))))))
        (func (export "table") (param i32) (result i32)
          (block $c (block $b (block $a
            (br_table $a $b $c (local.get 0)))
            (return (i32.const 10)))
            (return (i32.const 20)))
          (i32.const 30))
        (func (export "multi") (param i32) (result i32 i32)
          (block (result i32 i32) (local.get 0) (i32.const 5))))"#,
    // Numerics: traps and saturating conversions, sign extension, floats.
    r#"(module
        (func (export "div") (param i32 i32) (result i32) (i32.div_s (local.get 0) (local.get 1)))
        (func (export "rem") (param i64 i64) (result i64) (i64.rem_u (local.get 0) (local.get 1)))
        (func (export "conv") (param f64) (result i32)
          (i32.add (i32.trunc_sat_f64_s (local.get 0)) (i32.trunc_f64_u (f64.abs (local.get 0)))))
        (func (export "ext") (param i32) (result i64) (i64.extend8_s (i64.extend_i32_u (local.get 0))))
        (func (export "fl") (param f32 f32) (result f32)
          (f32.copysign (f32.sqrt (local.get 0)) (f32.nearest (local.get 1)))))"#,
    // Globals, a start function, and WASI imports.
    r#"(module
        (import "wasi_snapshot_preview1" "fd_write" (func $w (param i32 i32 i32 i32) (result i32)))
        (memory (export "memory") 1)
        (global $g (mut i32) (i32.const 0))
        (data (i32.const 100) "hi\n")
        (func $s (global.set $g (i32.const 42)))
        (start $s)
        (func (export "write") (param i32) (result i32)
          (i32.store (i32.const 0) (i32.const 100))
          (i32.store (i32.const 4) (local.get 0))
          (call $w (i32.const 1) (i32.const 0) (i32.const 1) (i32.const 8)))
        (func (export "g") (result i32) (global.get $g)))"#,
];

fn seed_modules() -> Vec<Vec<u8>> {
    SEEDS
        .iter()
        .map(|text| {
            let buf = wast::parser::ParseBuffer::new(text).expect("seed lexes");
            let mut wat: wast::Wat = wast::parser::parse(&buf).expect("seed parses");
            wat.encode().expect("seed encodes")
        })
        .collect()
}

/// Bytes a mutation likes: LEB boundaries, opcodes that open blocks, and
/// values that sit at a type's edges.
const INTERESTING: &[u8] = &[
    0x00, 0x01, 0x02, 0x40, 0x7f, 0x80, 0xff, 0x0b, 0x0f, 0x41, 0x10, 0x11,
];

fn mutate(rng: &mut Rng, input: &[u8]) -> Vec<u8> {
    let mut b = input.to_vec();
    for _ in 0..1 + rng.below(4) {
        if b.is_empty() {
            b.push(rng.next() as u8);
            continue;
        }
        let at = rng.below(b.len());
        match rng.below(7) {
            0 => b[at] ^= 1 << rng.below(8),
            1 => b[at] = rng.next() as u8,
            2 => b[at] = INTERESTING[rng.below(INTERESTING.len())],
            3 => b.insert(at, rng.next() as u8),
            4 => {
                let end = (at + 1 + rng.below(8)).min(b.len());
                b.drain(at..end);
            }
            5 => b.truncate(at),
            _ => {
                let len = (1 + rng.below(16)).min(b.len() - at);
                let slice = b[at..at + len].to_vec();
                let to = rng.below(b.len());
                b.splice(to..to, slice);
            }
        }
    }
    b
}

fn arg_for(rng: &mut Rng, ty: &ValueType) -> Value {
    let pick = |rng: &mut Rng| -> i64 {
        match rng.below(6) {
            0 => 0,
            1 => -1,
            2 => i64::MAX,
            3 => i64::MIN,
            4 => (rng.next() % 64) as i64,
            _ => rng.next() as i64,
        }
    };
    match ty {
        ValueType::I32 => Value::I32(pick(rng) as i32),
        ValueType::I64 => Value::I64(pick(rng)),
        ValueType::F32 => Value::F32(f32::from_bits(rng.next() as u32)),
        ValueType::F64 => Value::F64(f64::from_bits(rng.next())),
        ValueType::FuncRef => Value::FuncRef(None),
        _ => Value::I32(0),
    }
}

/// Parse, instantiate and run every export. Anything but a panic is fine.
fn exercise(rng: &mut Rng, bytes: &[u8]) {
    let Ok(module) = Module::parse(bytes) else {
        return;
    };
    let mut exports: Vec<(u32, Vec<ValueType>)> = Vec::new();
    for desc in module.exports.values() {
        if !matches!(desc.kind, ExportKind::Function) {
            continue;
        }
        let imported = module
            .imports
            .iter()
            .filter(|i| matches!(i.kind, super::module::ImportKind::Function(_)))
            .count() as u32;
        let params = desc
            .index
            .checked_sub(imported)
            .and_then(|i| module.functions.get(i as usize))
            .and_then(|f| module.types.get(f.type_index as usize))
            .map(|t| t.params.clone());
        if let Some(params) = params {
            exports.push((desc.index, params));
        }
    }
    let env = WasiEnv::new();
    let linker = create_wasi_linker(Arc::new(Mutex::new(env)));
    let Ok(mut executor) = Executor::new_with_linker(module, linker) else {
        return;
    };
    // The default ceiling is calibrated for release builds, where a guest
    // frame is about 1.1 KB of host stack; an unoptimized frame is about
    // 70 KB, so the test sets one that fits its 64 MB stack.
    if cfg!(debug_assertions) {
        executor.set_max_call_depth(256);
    }
    for (idx, params) in exports {
        executor.set_fuel(Some(FUEL));
        let args = params.iter().map(|t| arg_for(rng, t)).collect();
        let _ = executor.execute_with_args(idx, args);
    }
}

fn env_u64(name: &str, default: u64) -> u64 {
    std::env::var(name)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// The stack agent mode runs an execution on, so a deep but bounded
/// recursion is judged the way production would run it.
const EXEC_STACK: usize = 64 * 1024 * 1024;

#[test]
fn fuzz_parser_and_executor_never_panic() {
    std::thread::Builder::new()
        .stack_size(EXEC_STACK)
        .spawn(fuzz_parser_and_executor)
        .expect("fuzz thread")
        .join()
        .expect("no panic escaped");
}

fn fuzz_parser_and_executor() {
    let iters = env_u64("WASMRUN_FUZZ_ITERS", 400);
    let seed = env_u64("WASMRUN_FUZZ_SEED", 0x5eed);
    let corpus = seed_modules();
    // The seeds themselves must run cleanly, or the mutations prove little.
    for bytes in &corpus {
        Module::parse(bytes).expect("a seed module parses");
    }
    for (n, base) in corpus.iter().enumerate() {
        let mut rng = Rng(seed ^ (n as u64 + 1).wrapping_mul(0x9e37_79b9_7f4a_7c15));
        for i in 0..iters {
            let mutated = mutate(&mut rng, base);
            if std::env::var_os("WASMRUN_FUZZ_TRACE").is_some() {
                eprintln!("seed module {n}, iteration {i}: {mutated:02x?}");
            }
            let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                exercise(&mut Rng(rng.next() | 1), &mutated)
            }));
            assert!(
                caught.is_ok(),
                "panic on seed module {n}, iteration {i} (WASMRUN_FUZZ_SEED={seed}): {:02x?}",
                mutated
            );
        }
    }
}

#[test]
fn fuzz_linear_memory_bounds() {
    let iters = env_u64("WASMRUN_FUZZ_ITERS", 400) * 10;
    let mut rng = Rng(env_u64("WASMRUN_FUZZ_SEED", 0x5eed) | 1);
    let mut mem = LinearMemory::new(1, Some(3)).expect("memory");
    for _ in 0..iters {
        let offset = match rng.below(4) {
            0 => rng.below(65_536 * 3 + 16),
            1 => usize::MAX - rng.below(16),
            2 => mem.size_bytes().saturating_sub(rng.below(16)),
            _ => rng.next() as usize,
        };
        let len = rng.below(64);
        let caught =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match rng.below(6) {
                0 => drop(mem.read_bytes(offset, len)),
                1 => drop(mem.write_bytes(offset, &vec![0xab; len])),
                2 => drop(mem.read_i32(offset)),
                3 => drop(mem.write_i64(offset, -1)),
                4 => drop(mem.read_f64(offset)),
                _ => drop(mem.grow(rng.below(3) as u32)),
            }));
        assert!(
            caught.is_ok(),
            "memory access panicked at offset {offset}, len {len}"
        );
    }
}
