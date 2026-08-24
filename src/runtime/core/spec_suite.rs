//! Harness for the official WebAssembly spec test suite.
//!
//! The suite ships as `.wast` files: a script format that interleaves modules
//! with assertions about what calling into them should do. This module parses
//! those scripts, runs the assertions against wasmrun's own executor, and
//! reports how many of each file's assertions hold.
//!
//! **Getting the suite.** It is not vendored. `just spec-suite-fetch` clones it
//! at a pinned commit into `_tests/testsuite`, which is gitignored, and
//! `WASM_TESTSUITE_DIR` overrides that location. Every test here skips when the
//! checkout is missing, so an ordinary `cargo test` on a fresh clone still
//! passes without it.
//!
//! **What is deliberately not checked.** `assert_invalid`, `assert_malformed`
//! and `assert_unlinkable` all assert that a *validator* rejects something.
//! wasmrun has no validator: it assumes it is handed modules a toolchain
//! already produced, and rejecting bad ones is a separate piece of work. Those
//! directives are counted as skipped rather than quietly passed, so the numbers
//! never suggest coverage that does not exist. The same goes for directives
//! that need cross-module linking (`register`) or threads.

use super::executor::Executor;
use super::module::{ExportKind, Module};
use super::values::Value;
use std::path::{Path, PathBuf};
use wast::core::{NanPattern, WastArgCore, WastRetCore};
use wast::lexer::Lexer;
use wast::parser::{self, ParseBuffer};
use wast::{QuoteWat, WastArg, WastDirective, WastExecute, WastRet, Wat};

/// How many instructions any single spec assertion is allowed to run before the
/// harness gives up on it. The suite includes deliberate infinite recursion,
/// and a hang is a much worse failure mode in CI than a reported one.
const SPEC_FUEL: u64 = 200_000_000;

/// Host stack for the harness thread.
///
/// Sized so the interpreter's own `DEFAULT_MAX_CALL_DEPTH` is reachable under
/// `cargo test`, which is an unoptimized build: a guest frame costs about 70 KB
/// of host stack there against roughly 1.1 KB in release, so 1024 frames want
/// around 70 MB. The suite needs real depth (`call.wast` recurses 200 deep
/// through mutually recursive functions) and its runaway cases have to reach
/// the ceiling and trap rather than run the host stack out first. This is a
/// virtual reservation, not committed memory.
const SPEC_STACK_BYTES: usize = 256 * 1024 * 1024;

/// Tally for one `.wast` file.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct SpecReport {
    pub passed: usize,
    pub failed: usize,
    /// Directives the harness does not attempt: validation assertions, linking,
    /// threads. Counted separately so they never look like passes.
    pub skipped: usize,
    pub failures: Vec<String>,
}

impl SpecReport {
    fn fail(&mut self, what: String) {
        self.failed += 1;
        // A file that goes wrong early tends to go wrong in every assertion
        // after it, and a thousand identical lines helps nobody.
        if self.failures.len() < 40 {
            self.failures.push(what);
        }
    }

    pub fn total(&self) -> usize {
        self.passed + self.failed + self.skipped
    }
}

/// Run `f` on a thread with a stack large enough for the interpreter.
///
/// A test thread gets 2 MB by default, which an unoptimized interpreter frame
/// exhausts in about thirty guest calls. Giving the harness its own generous
/// stack lets the call-depth ceiling be the thing that stops runaway recursion,
/// which is the behavior under test, rather than the host stack running out
/// first.
fn on_a_big_stack<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new()
        .stack_size(SPEC_STACK_BYTES)
        .spawn(f)
        .expect("spawn spec suite thread")
        .join()
        .expect("spec suite thread panicked")
}

/// Where the spec suite checkout lives, if there is one.
pub fn testsuite_dir() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("WASM_TESTSUITE_DIR") {
        let path = PathBuf::from(dir);
        return path.is_dir().then_some(path);
    }
    let default = Path::new(env!("CARGO_MANIFEST_DIR")).join("_tests/testsuite");
    default.is_dir().then_some(default)
}

/// The current module under test, plus the executor holding its state.
struct Instance {
    executor: Executor,
}

impl Instance {
    fn new(bytes: &[u8]) -> Result<Self, String> {
        let module = Module::parse(bytes).map_err(|e| e.to_string())?;
        let mut executor = Executor::new(module)?;
        executor.set_fuel(Some(SPEC_FUEL));
        Ok(Instance { executor })
    }

    fn invoke(&mut self, name: &str, args: Vec<Value>) -> Result<Vec<Value>, String> {
        let idx = self
            .executor
            .module()
            .exports
            .iter()
            .find(|(n, d)| n.as_str() == name && matches!(d.kind, ExportKind::Function))
            .map(|(_, d)| d.index)
            .ok_or_else(|| format!("no exported function named {name:?}"))?;
        // Each assertion starts from a clean fuel budget; the cap is there to
        // stop one runaway call, not to bound the file as a whole.
        self.executor.set_fuel(Some(SPEC_FUEL));
        self.executor.execute_with_args(idx, args)
    }
}

/// Run every directive in one `.wast` file.
pub fn run_wast_file(path: &Path) -> Result<SpecReport, String> {
    let contents =
        std::fs::read_to_string(path).map_err(|e| format!("reading {}: {e}", path.display()))?;
    let mut lexer = Lexer::new(&contents);
    lexer.allow_confusing_unicode(true);
    let buf = ParseBuffer::new_with_lexer(lexer).map_err(|e| e.to_string())?;
    let wast: wast::Wast = parser::parse(&buf).map_err(|e| e.to_string())?;

    let mut report = SpecReport::default();
    let mut instance: Option<Instance> = None;

    for directive in wast.directives {
        match directive {
            WastDirective::Module(module) => match instantiate(module) {
                Ok(inst) => {
                    instance = Some(inst);
                    report.passed += 1;
                }
                Err(e) => {
                    instance = None;
                    report.fail(format!("module instantiation: {e}"));
                }
            },

            WastDirective::AssertReturn {
                exec,
                results,
                span,
            } => {
                let line = line_of(&contents, span.offset());
                match run_execute(&mut instance, exec) {
                    Ok(actual) => match check_results(&actual, &results) {
                        Ok(()) => report.passed += 1,
                        Err(e) => report.fail(format!("line {line}: {e}")),
                    },
                    Err(e) if needs_linking(&e) => report.skipped += 1,
                    Err(e) => report.fail(format!("line {line}: expected a value, trapped: {e}")),
                }
            }

            WastDirective::AssertTrap { exec, span, .. } => {
                let line = line_of(&contents, span.offset());
                match run_execute(&mut instance, exec) {
                    // The message is not compared: wasmrun's trap text is its
                    // own, and pinning it here would make every reworded error
                    // a spec failure.
                    Err(_) => report.passed += 1,
                    Ok(v) => report.fail(format!("line {line}: expected a trap, got {v:?}")),
                }
            }

            WastDirective::AssertExhaustion { call, span, .. } => {
                let line = line_of(&contents, span.offset());
                match run_invoke(&mut instance, call) {
                    Err(_) => report.passed += 1,
                    Ok(v) => report.fail(format!("line {line}: expected exhaustion, got {v:?}")),
                }
            }

            WastDirective::Invoke(invoke) => {
                let line = line_of(&contents, invoke.span.offset());
                match run_invoke(&mut instance, invoke) {
                    Ok(_) => report.passed += 1,
                    Err(e) if needs_linking(&e) => report.skipped += 1,
                    Err(e) => report.fail(format!("line {line}: invoke failed: {e}")),
                }
            }

            // Everything below asserts something wasmrun does not do.
            _ => report.skipped += 1,
        }
    }

    Ok(report)
}

/// Whether a failure is the harness reaching a `register`ed module rather than
/// wasmrun getting an answer wrong.
///
/// A `.wast` script can name a previously registered instance in an import.
/// wasmrun's executor instantiates one module at a time with no registry to
/// resolve those against, so an assertion that depends on one is not a result
/// this harness can judge. Counting it as skipped keeps it out of the pass
/// column as well as the fail column.
fn needs_linking(err: &str) -> bool {
    err.contains("No linker: cannot call import")
}

fn instantiate(mut module: QuoteWat<'_>) -> Result<Instance, String> {
    let bytes = module.encode().map_err(|e| e.to_string())?;
    Instance::new(&bytes)
}

fn run_execute(
    instance: &mut Option<Instance>,
    exec: WastExecute<'_>,
) -> Result<Vec<Value>, String> {
    match exec {
        WastExecute::Invoke(invoke) => run_invoke(instance, invoke),
        WastExecute::Wat(Wat::Module(module)) => {
            let mut quoted = QuoteWat::Wat(Wat::Module(module));
            instantiate(quoted_mut(&mut quoted)).map(|_| Vec::new())
        }
        WastExecute::Wat(_) => Err("component modules are not supported".to_string()),
        WastExecute::Get { global, .. } => Err(format!("global.get of {global:?} not supported")),
    }
}

/// `QuoteWat::encode` needs `&mut`, and the borrow checker is happier with the
/// indirection than with reborrowing through the match arm above.
fn quoted_mut<'a, 'b>(q: &'b mut QuoteWat<'a>) -> QuoteWat<'a>
where
    'a: 'b,
{
    std::mem::replace(
        q,
        QuoteWat::QuoteModule(wast::token::Span::from_offset(0), Vec::new()),
    )
}

fn run_invoke(
    instance: &mut Option<Instance>,
    invoke: wast::WastInvoke<'_>,
) -> Result<Vec<Value>, String> {
    let inst = instance
        .as_mut()
        .ok_or_else(|| "no instantiated module".to_string())?;
    let mut args = Vec::with_capacity(invoke.args.len());
    for arg in invoke.args {
        args.push(convert_arg(arg)?);
    }
    inst.invoke(invoke.name, args)
}

fn convert_arg(arg: WastArg<'_>) -> Result<Value, String> {
    let core = match arg {
        WastArg::Core(c) => c,
        _ => return Err("non-core argument".to_string()),
    };
    Ok(match core {
        WastArgCore::I32(v) => Value::I32(v),
        WastArgCore::I64(v) => Value::I64(v),
        WastArgCore::F32(v) => Value::F32(f32::from_bits(v.bits)),
        WastArgCore::F64(v) => Value::F64(f64::from_bits(v.bits)),
        WastArgCore::RefNull(_) => Value::FuncRef(None),
        WastArgCore::RefExtern(v) => Value::ExternRef(Some(v)),
        other => return Err(format!("unsupported argument {other:?}")),
    })
}

fn check_results(actual: &[Value], expected: &[WastRet<'_>]) -> Result<(), String> {
    if actual.len() != expected.len() {
        return Err(format!(
            "expected {} result(s), got {}: {actual:?}",
            expected.len(),
            actual.len()
        ));
    }
    for (i, (got, want)) in actual.iter().zip(expected).enumerate() {
        let core = match want {
            WastRet::Core(c) => c,
            _ => return Err("non-core expectation".to_string()),
        };
        check_one(got, core).map_err(|e| format!("result {i}: {e}"))?;
    }
    Ok(())
}

fn check_one(got: &Value, want: &WastRetCore<'_>) -> Result<(), String> {
    match (got, want) {
        (Value::I32(a), WastRetCore::I32(b)) if a == b => Ok(()),
        (Value::I64(a), WastRetCore::I64(b)) if a == b => Ok(()),
        (Value::F32(a), WastRetCore::F32(pattern)) => check_f32(*a, pattern),
        (Value::F64(a), WastRetCore::F64(pattern)) => check_f64(*a, pattern),
        (Value::FuncRef(None), WastRetCore::RefNull(_)) => Ok(()),
        (Value::ExternRef(None), WastRetCore::RefNull(_)) => Ok(()),
        (Value::FuncRef(Some(_)), WastRetCore::RefFunc(_)) => Ok(()),
        (Value::ExternRef(Some(a)), WastRetCore::RefExtern(Some(b))) if a == b => Ok(()),
        (Value::ExternRef(Some(_)), WastRetCore::RefExtern(None)) => Ok(()),
        // `either` lists the answers a spec-conformant runtime may give; any
        // one of them is a pass.
        (_, WastRetCore::Either(options)) => {
            if options.iter().any(|o| check_one(got, o).is_ok()) {
                Ok(())
            } else {
                Err(format!("{got:?} matched none of {options:?}"))
            }
        }
        _ => Err(format!("expected {want:?}, got {got:?}")),
    }
}

fn check_f32(got: f32, want: &NanPattern<wast::token::F32>) -> Result<(), String> {
    match want {
        // A canonical NaN has the payload's top bit set and nothing else; an
        // arithmetic NaN only has to be some NaN. Comparing floats by value
        // would let a wrong NaN through, so both go through the bits.
        NanPattern::CanonicalNan => {
            if got.is_nan() && got.to_bits() & 0x003f_ffff == 0 {
                Ok(())
            } else {
                Err(format!("expected a canonical NaN, got {got}"))
            }
        }
        NanPattern::ArithmeticNan => {
            if got.is_nan() {
                Ok(())
            } else {
                Err(format!("expected an arithmetic NaN, got {got}"))
            }
        }
        NanPattern::Value(v) => {
            if got.to_bits() == v.bits {
                Ok(())
            } else {
                Err(format!(
                    "expected f32 bits {:#x}, got {:#x}",
                    v.bits,
                    got.to_bits()
                ))
            }
        }
    }
}

fn check_f64(got: f64, want: &NanPattern<wast::token::F64>) -> Result<(), String> {
    match want {
        NanPattern::CanonicalNan => {
            if got.is_nan() && got.to_bits() & 0x0007_ffff_ffff_ffff == 0 {
                Ok(())
            } else {
                Err(format!("expected a canonical NaN, got {got}"))
            }
        }
        NanPattern::ArithmeticNan => {
            if got.is_nan() {
                Ok(())
            } else {
                Err(format!("expected an arithmetic NaN, got {got}"))
            }
        }
        NanPattern::Value(v) => {
            if got.to_bits() == v.bits {
                Ok(())
            } else {
                Err(format!(
                    "expected f64 bits {:#x}, got {:#x}",
                    v.bits,
                    got.to_bits()
                ))
            }
        }
    }
}

/// Turn a byte offset into a line number, for failure messages that point at
/// the assertion in the `.wast` file rather than at a byte.
fn line_of(contents: &str, offset: usize) -> usize {
    contents[..offset.min(contents.len())]
        .bytes()
        .filter(|b| *b == b'\n')
        .count()
        + 1
}

/// The files this harness gates on. Every one of them is core MVP plus the
/// proposals wasmrun implements, which is the surface exec mode actually
/// promises. Files covering SIMD, threads, GC, exceptions, the Component Model
/// and the multi-memory or 64-bit-memory proposals are out of scope by design;
/// `EXEC.md` tracks those separately.
pub const CORE_FILES: &[&str] = &[
    "address.wast",
    "align.wast",
    "block.wast",
    "br.wast",
    "br_if.wast",
    "br_table.wast",
    "call.wast",
    "call_indirect.wast",
    "comments.wast",
    "const.wast",
    "conversions.wast",
    "custom.wast",
    "endianness.wast",
    "f32.wast",
    "f32_bitwise.wast",
    "f32_cmp.wast",
    "f64.wast",
    "f64_bitwise.wast",
    "f64_cmp.wast",
    "fac.wast",
    "float_literals.wast",
    "float_memory.wast",
    "float_misc.wast",
    "forward.wast",
    "func.wast",
    "func_ptrs.wast",
    "global.wast",
    "i32.wast",
    "i64.wast",
    "if.wast",
    "int_exprs.wast",
    "int_literals.wast",
    "labels.wast",
    "left-to-right.wast",
    "load.wast",
    "local_get.wast",
    "local_set.wast",
    "local_tee.wast",
    "loop.wast",
    "memory.wast",
    "memory_copy.wast",
    "memory_fill.wast",
    "memory_grow.wast",
    "memory_init.wast",
    "memory_redundancy.wast",
    "memory_size.wast",
    "memory_trap.wast",
    "nop.wast",
    "ref_func.wast",
    "ref_is_null.wast",
    "ref_null.wast",
    "return.wast",
    "select.wast",
    "stack.wast",
    "store.wast",
    "switch.wast",
    "table.wast",
    "table_copy.wast",
    "table_fill.wast",
    "table_get.wast",
    "table_grow.wast",
    "table_init.wast",
    "table_set.wast",
    "table_size.wast",
    "traps.wast",
    "unreachable.wast",
    "unreached-valid.wast",
    "unwind.wast",
];

/// Known failures, per file, with the reason each one is still open.
///
/// This is what the CI gate compares against: a file may not fail *more* than
/// its entry here, and a file that starts passing more than its entry says is
/// reported so the number gets tightened rather than silently drifting. Every
/// entry is a feature wasmrun has not implemented, not a bug it is ignoring.
/// Zero is the expectation everywhere else.
pub const KNOWN_FAILURES: &[(&str, usize, &str)] = &[
    // The function-references proposal: a table typed `(ref null $t)` encodes
    // its element type as 0x63, which the table parser rejects. It is the
    // module at the top of the file, so nothing in it runs.
    (
        "br_table.wast",
        162,
        "typed function references (ref null $t)",
    ),
    ("table.wast", 11, "typed function references (ref null $t)"),
    // The multi-memory proposal: the file's modules declare two memories.
    ("memory_grow.wast", 50, "multiple memories"),
    // WasmGC: `anyref` and the typed reference shorthands.
    ("ref_null.wast", 34, "anyref and typed references (WasmGC)"),
    (
        "ref_is_null.wast",
        20,
        "anyref and typed references (WasmGC)",
    ),
    ("unreached-valid.wast", 1, "typed references (WasmGC)"),
    ("table_init.wast", 2, "typed function type forms"),
    // Imported globals get their index slots but no value, because wasmrun
    // instantiates one module at a time and has nothing to resolve a host
    // global against. The reads return the type's zero instead of the
    // registered module's value.
    ("global.wast", 13, "cross-module global imports"),
];

/// The failure count this harness expects from a file.
fn expected_failures(name: &str) -> usize {
    KNOWN_FAILURES
        .iter()
        .find(|(f, _, _)| *f == name)
        .map(|(_, n, _)| *n)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn skip_message() {
        eprintln!(
            "spec testsuite not found, skipping. Run `just spec-suite-fetch`, \
             or point WASM_TESTSUITE_DIR at a checkout."
        );
    }

    /// Run the whole gated subset and report per-file counts.
    #[test]
    fn test_spec_core_subset_runs() {
        let Some(dir) = testsuite_dir() else {
            skip_message();
            return;
        };
        on_a_big_stack(move || run_subset(dir));
    }

    fn run_subset(dir: PathBuf) {
        // `WASM_TESTSUITE_ONLY=br_table.wast` narrows a run to one file, which
        // is how you read a single file's failures without the other sixty.
        let only = std::env::var("WASM_TESTSUITE_ONLY").ok();

        let mut total = SpecReport::default();
        for name in CORE_FILES {
            if let Some(filter) = &only {
                if !name.contains(filter.as_str()) {
                    continue;
                }
            }
            let path = dir.join(name);
            if !path.is_file() {
                eprintln!("{name}: missing from the checkout");
                continue;
            }
            match run_wast_file(&path) {
                Ok(r) => {
                    eprintln!(
                        "{name}: {} passed, {} failed, {} skipped",
                        r.passed, r.failed, r.skipped
                    );
                    for f in &r.failures {
                        eprintln!("    {f}");
                    }
                    total.passed += r.passed;
                    total.failed += r.failed;
                    total.skipped += r.skipped;
                }
                Err(e) => eprintln!("{name}: harness error: {e}"),
            }
        }
        eprintln!(
            "TOTAL: {} passed, {} failed, {} skipped",
            total.passed, total.failed, total.skipped
        );
        assert!(total.total() > 0, "no directives ran at all");
    }

    /// The gate. Every gated file must fail no more than its recorded
    /// baseline, and a file that improves past its baseline is reported so the
    /// number is tightened rather than left to rot.
    #[test]
    fn test_spec_core_meets_baseline() {
        let Some(dir) = testsuite_dir() else {
            skip_message();
            return;
        };
        on_a_big_stack(move || check_baseline(dir));
    }

    fn check_baseline(dir: PathBuf) {
        let mut regressions = Vec::new();
        let mut improvements = Vec::new();
        let mut missing = Vec::new();

        for name in CORE_FILES {
            let path = dir.join(name);
            if !path.is_file() {
                missing.push(*name);
                continue;
            }
            let report = match run_wast_file(&path) {
                Ok(r) => r,
                Err(e) => {
                    regressions.push(format!("{name}: harness error: {e}"));
                    continue;
                }
            };
            let expected = expected_failures(name);
            if report.failed > expected {
                regressions.push(format!(
                    "{name}: {} failures, baseline is {expected}\n        {}",
                    report.failed,
                    report.failures.join("\n        ")
                ));
            } else if report.failed < expected {
                improvements.push(format!(
                    "{name}: {} failures, baseline says {expected} — lower the baseline",
                    report.failed
                ));
            }
        }

        assert!(
            missing.len() < CORE_FILES.len(),
            "no spec files found in {}; is the checkout complete?",
            dir.display()
        );
        assert!(
            regressions.is_empty(),
            "spec suite regressed:\n    {}",
            regressions.join("\n    ")
        );
        assert!(
            improvements.is_empty(),
            "spec suite improved, update KNOWN_FAILURES:\n    {}",
            improvements.join("\n    ")
        );
    }

    #[test]
    fn test_known_failures_name_gated_files() {
        // A stale entry here would silently excuse a file that is no longer
        // even run.
        for (name, _, _) in KNOWN_FAILURES {
            assert!(
                CORE_FILES.contains(name),
                "{name} is in KNOWN_FAILURES but not in CORE_FILES"
            );
        }
    }

    #[test]
    fn test_line_of_counts_from_one() {
        assert_eq!(line_of("a\nb\nc", 0), 1);
        assert_eq!(line_of("a\nb\nc", 2), 2);
        assert_eq!(line_of("a\nb\nc", 4), 3);
        assert_eq!(line_of("a\nb", 999), 2);
    }

    #[test]
    fn test_canonical_nan_rejects_an_arithmetic_nan() {
        let arithmetic = f32::from_bits(0x7fc0_0001);
        assert!(check_f32(arithmetic, &NanPattern::CanonicalNan).is_err());
        assert!(check_f32(arithmetic, &NanPattern::ArithmeticNan).is_ok());
        assert!(check_f32(f32::NAN, &NanPattern::CanonicalNan).is_ok());
        assert!(check_f32(1.0, &NanPattern::ArithmeticNan).is_err());
    }

    #[test]
    fn test_f64_value_pattern_compares_bits_not_value() {
        let neg_zero = wast::token::F64 {
            bits: (-0.0f64).to_bits(),
        };
        assert!(check_f64(-0.0, &NanPattern::Value(neg_zero)).is_ok());
        assert!(check_f64(0.0, &NanPattern::Value(neg_zero)).is_err());
    }
}
