//! Agent mode: per-session resource limits.
//!
//! A single source of truth for the resource ceilings applied to a sandbox
//! session: linear memory, execution fuel (instruction budget), captured
//! output, individual file size, and total disk usage. Limits are configured
//! globally via CLI flags and may be overridden per-session at creation.
//!
//! A field of `None` means "unlimited" for that dimension.

/// WASM linear memory page size (64 KiB), used to convert MB → pages.
const WASM_PAGE_SIZE_BYTES: u64 = 65536;
const BYTES_PER_MB: u64 = 1024 * 1024;

/// Resource ceilings for a single sandbox session.
///
/// Each field caps one dimension of resource use. `None` disables that cap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceLimits {
    /// Maximum linear memory in WASM pages (64 KiB each). Enforced by the
    /// executor when a module calls `memory.grow`.
    pub max_memory_pages: Option<u32>,
    /// Maximum number of WASM instructions ("fuel") a single execution may
    /// run before it is aborted. Guards against runaway / infinite loops.
    pub max_fuel: Option<u64>,
    /// Maximum combined stdout + stderr bytes captured per execution. Output
    /// beyond this is dropped and the response is flagged as truncated.
    pub max_output_bytes: Option<usize>,
    /// Maximum size in bytes of any single file written into the session.
    pub max_file_size: Option<u64>,
    /// Maximum total disk usage (bytes) of the session's working directory.
    pub max_disk_bytes: Option<u64>,
}

impl Default for ResourceLimits {
    /// Production defaults: generous enough for real workloads (including the
    /// QuickJS runtime) yet bounded for everything but raw instruction count.
    ///
    /// Fuel defaults to unlimited because language runtimes need a very large,
    /// workload-dependent instruction budget; operators opt into a fuel cap
    /// explicitly via `--max-fuel`. The wall-clock exec timeout remains the
    /// default time guard.
    fn default() -> Self {
        ResourceLimits {
            max_memory_pages: Some(mb_to_pages(256)), // 256 MB
            max_fuel: None,
            max_output_bytes: Some((10 * BYTES_PER_MB) as usize), // 10 MB
            max_file_size: Some(50 * BYTES_PER_MB),               // 50 MB
            max_disk_bytes: Some(100 * BYTES_PER_MB),             // 100 MB
        }
    }
}

impl ResourceLimits {
    /// Build limits from operator-facing units (MB and a raw fuel count).
    ///
    /// A value of `0` for any field means "unlimited" for that dimension,
    /// matching the CLI convention where `0` disables a cap.
    pub fn from_cli(
        max_memory_mb: u32,
        max_fuel: u64,
        max_output_mb: u32,
        max_file_size_mb: u32,
        max_disk_mb: u32,
    ) -> Self {
        ResourceLimits {
            max_memory_pages: zero_is_none_u32(max_memory_mb).map(mb_to_pages),
            max_fuel: zero_is_none_u64(max_fuel),
            max_output_bytes: zero_is_none_u32(max_output_mb).map(|mb| mb_to_bytes(mb) as usize),
            max_file_size: zero_is_none_u32(max_file_size_mb).map(mb_to_bytes),
            max_disk_bytes: zero_is_none_u32(max_disk_mb).map(mb_to_bytes),
        }
    }

    /// Apply a set of optional overrides (already in raw units) on top of self,
    /// returning the merged limits. `None` overrides leave the existing value.
    pub fn with_overrides(&self, overrides: &LimitsOverride) -> Self {
        let mut merged = self.clone();
        if let Some(mb) = overrides.max_memory_mb {
            merged.max_memory_pages = zero_is_none_u32(mb).map(mb_to_pages);
        }
        if let Some(fuel) = overrides.max_fuel {
            merged.max_fuel = zero_is_none_u64(fuel);
        }
        if let Some(mb) = overrides.max_output_mb {
            merged.max_output_bytes = zero_is_none_u32(mb).map(|mb| mb_to_bytes(mb) as usize);
        }
        if let Some(mb) = overrides.max_file_size_mb {
            merged.max_file_size = zero_is_none_u32(mb).map(mb_to_bytes);
        }
        if let Some(mb) = overrides.max_disk_mb {
            merged.max_disk_bytes = zero_is_none_u32(mb).map(mb_to_bytes);
        }
        merged
    }

    /// Clamp every dimension to a `ceiling`, returning the tighter of the two.
    ///
    /// Used to enforce a per-tenant limit as a hard ceiling: a per-session
    /// override may *tighten* a dimension but never exceed the tenant's cap. A
    /// `None` (unlimited) value is treated as +∞, so an unlimited `ceiling`
    /// leaves the value untouched, and an unlimited value is pulled down to a
    /// finite ceiling.
    pub fn clamp_to(&self, ceiling: &ResourceLimits) -> Self {
        ResourceLimits {
            max_memory_pages: clamp_opt(self.max_memory_pages, ceiling.max_memory_pages),
            max_fuel: clamp_opt(self.max_fuel, ceiling.max_fuel),
            max_output_bytes: clamp_opt(self.max_output_bytes, ceiling.max_output_bytes),
            max_file_size: clamp_opt(self.max_file_size, ceiling.max_file_size),
            max_disk_bytes: clamp_opt(self.max_disk_bytes, ceiling.max_disk_bytes),
        }
    }

    /// Check that writing a file of `new_len` bytes is allowed. `existing_len`
    /// is the size of the file being replaced (0 for a new file), so that
    /// overwriting an existing file is measured by its net change on disk.
    ///
    /// Returns an explanatory error string if the per-file or total-disk cap
    /// would be exceeded.
    pub fn check_write(
        &self,
        new_len: u64,
        existing_len: u64,
        current_disk_usage: u64,
    ) -> Result<(), String> {
        if let Some(max) = self.max_file_size {
            if new_len > max {
                return Err(format!(
                    "File size limit exceeded: {new_len} bytes > {max} byte limit"
                ));
            }
        }
        if let Some(max) = self.max_disk_bytes {
            // Net disk after replacing `existing_len` with `new_len`.
            let projected = current_disk_usage.saturating_sub(existing_len) + new_len;
            if projected > max {
                return Err(format!(
                    "Disk usage limit exceeded: {projected} bytes > {max} byte limit"
                ));
            }
        }
        Ok(())
    }
}

/// Optional per-session limit overrides, in operator-facing units.
///
/// Deserialized from the optional body of `POST /sessions`. Every field is
/// optional; absent fields fall back to the server defaults. A value of `0`
/// disables that cap (matching the CLI convention).
#[derive(Debug, Clone, Default, serde::Deserialize)]
pub struct LimitsOverride {
    pub max_memory_mb: Option<u32>,
    pub max_fuel: Option<u64>,
    pub max_output_mb: Option<u32>,
    pub max_file_size_mb: Option<u32>,
    pub max_disk_mb: Option<u32>,
}

/// Recursively sum the size of all regular files under `dir`.
///
/// Returns 0 if the directory does not exist or cannot be read; symlinks are
/// not followed. Used to measure a session's current disk footprint.
pub fn dir_size(dir: &std::path::Path) -> u64 {
    let mut total = 0u64;
    let Ok(entries) = std::fs::read_dir(dir) else {
        return 0;
    };
    for entry in entries.flatten() {
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        if meta.is_dir() {
            total += dir_size(&entry.path());
        } else if meta.is_file() {
            total += meta.len();
        }
    }
    total
}

/// Clamp an optional ceiling onto an optional value, treating `None` as +∞
/// (unlimited): no ceiling leaves the value as-is; an unlimited value is pulled
/// down to a finite ceiling; two finite values take the smaller.
fn clamp_opt<T: Ord + Copy>(val: Option<T>, ceiling: Option<T>) -> Option<T> {
    match (val, ceiling) {
        (_, None) => val,
        (None, Some(c)) => Some(c),
        (Some(v), Some(c)) => Some(v.min(c)),
    }
}

fn mb_to_pages(mb: u32) -> u32 {
    ((mb as u64 * BYTES_PER_MB) / WASM_PAGE_SIZE_BYTES) as u32
}

fn mb_to_bytes(mb: u32) -> u64 {
    mb as u64 * BYTES_PER_MB
}

fn zero_is_none_u32(v: u32) -> Option<u32> {
    if v == 0 {
        None
    } else {
        Some(v)
    }
}

fn zero_is_none_u64(v: u64) -> Option<u64> {
    if v == 0 {
        None
    } else {
        Some(v)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mb_to_pages() {
        // 1 MB = 16 pages of 64 KiB
        assert_eq!(mb_to_pages(1), 16);
        assert_eq!(mb_to_pages(256), 4096);
    }

    #[test]
    fn test_from_cli_units() {
        let l = ResourceLimits::from_cli(256, 1_000_000, 10, 50, 100);
        assert_eq!(l.max_memory_pages, Some(4096));
        assert_eq!(l.max_fuel, Some(1_000_000));
        assert_eq!(l.max_output_bytes, Some(10 * 1024 * 1024));
        assert_eq!(l.max_file_size, Some(50 * 1024 * 1024));
        assert_eq!(l.max_disk_bytes, Some(100 * 1024 * 1024));
    }

    #[test]
    fn test_from_cli_zero_means_unlimited() {
        let l = ResourceLimits::from_cli(0, 0, 0, 0, 0);
        assert_eq!(l.max_memory_pages, None);
        assert_eq!(l.max_fuel, None);
        assert_eq!(l.max_output_bytes, None);
        assert_eq!(l.max_file_size, None);
        assert_eq!(l.max_disk_bytes, None);
    }

    #[test]
    fn test_default_has_no_fuel_cap() {
        // Fuel is opt-in so heavy runtimes (QuickJS) are not broken by default.
        assert_eq!(ResourceLimits::default().max_fuel, None);
        assert!(ResourceLimits::default().max_memory_pages.is_some());
    }

    #[test]
    fn test_with_overrides_partial() {
        let base = ResourceLimits::from_cli(256, 0, 10, 50, 100);
        let ov = LimitsOverride {
            max_fuel: Some(5000),
            max_file_size_mb: Some(1),
            ..Default::default()
        };
        let merged = base.with_overrides(&ov);
        // Overridden fields change
        assert_eq!(merged.max_fuel, Some(5000));
        assert_eq!(merged.max_file_size, Some(1024 * 1024));
        // Untouched fields stay
        assert_eq!(merged.max_memory_pages, base.max_memory_pages);
        assert_eq!(merged.max_output_bytes, base.max_output_bytes);
    }

    #[test]
    fn test_with_overrides_zero_disables() {
        let base = ResourceLimits::from_cli(256, 1000, 10, 50, 100);
        let ov = LimitsOverride {
            max_fuel: Some(0),
            ..Default::default()
        };
        let merged = base.with_overrides(&ov);
        assert_eq!(merged.max_fuel, None);
    }

    #[test]
    fn test_clamp_to_tightens_only() {
        let ceiling = ResourceLimits {
            max_memory_pages: Some(100),
            max_fuel: Some(1000),
            max_output_bytes: Some(50),
            max_file_size: Some(500),
            max_disk_bytes: Some(5000),
        };
        // A request below the ceiling on every dimension is left untouched.
        let below = ResourceLimits {
            max_memory_pages: Some(10),
            max_fuel: Some(100),
            max_output_bytes: Some(5),
            max_file_size: Some(50),
            max_disk_bytes: Some(500),
        };
        assert_eq!(below.clamp_to(&ceiling), below);

        // A request above the ceiling on every dimension is pulled down to it.
        let above = ResourceLimits {
            max_memory_pages: Some(999),
            max_fuel: Some(99999),
            max_output_bytes: Some(999),
            max_file_size: Some(99999),
            max_disk_bytes: Some(99999),
        };
        assert_eq!(above.clamp_to(&ceiling), ceiling);
    }

    #[test]
    fn test_clamp_to_none_semantics() {
        // A finite ceiling pulls an "unlimited" (None) value down to it.
        let ceiling = ResourceLimits {
            max_memory_pages: Some(64),
            max_fuel: Some(10),
            max_output_bytes: Some(1),
            max_file_size: Some(2),
            max_disk_bytes: Some(3),
        };
        let unlimited = ResourceLimits {
            max_memory_pages: None,
            max_fuel: None,
            max_output_bytes: None,
            max_file_size: None,
            max_disk_bytes: None,
        };
        assert_eq!(unlimited.clamp_to(&ceiling), ceiling);

        // An unlimited ceiling leaves any value untouched.
        let val = ResourceLimits {
            max_memory_pages: Some(7),
            max_fuel: None,
            max_output_bytes: Some(9),
            max_file_size: None,
            max_disk_bytes: Some(11),
        };
        assert_eq!(val.clamp_to(&unlimited), val);
    }

    #[test]
    fn test_check_write_file_size() {
        let l = ResourceLimits {
            max_file_size: Some(100),
            max_disk_bytes: None,
            ..ResourceLimits::default()
        };
        assert!(l.check_write(100, 0, 0).is_ok());
        let err = l.check_write(101, 0, 0).unwrap_err();
        assert!(err.contains("File size limit"));
    }

    #[test]
    fn test_check_write_disk_usage() {
        let l = ResourceLimits {
            max_file_size: None,
            max_disk_bytes: Some(1000),
            ..ResourceLimits::default()
        };
        // 900 used + 100 new = 1000, ok
        assert!(l.check_write(100, 0, 900).is_ok());
        // 900 used + 101 new = 1001, over
        let err = l.check_write(101, 0, 900).unwrap_err();
        assert!(err.contains("Disk usage limit"));
    }

    #[test]
    fn test_check_write_disk_counts_replacement() {
        let l = ResourceLimits {
            max_file_size: None,
            max_disk_bytes: Some(1000),
            ..ResourceLimits::default()
        };
        // Replacing a 500-byte file with a 600-byte one when 900 total are used:
        // 900 - 500 + 600 = 1000, still within limit.
        assert!(l.check_write(600, 500, 900).is_ok());
    }

    #[test]
    fn test_check_write_unlimited() {
        let l = ResourceLimits {
            max_file_size: None,
            max_disk_bytes: None,
            ..ResourceLimits::default()
        };
        assert!(l.check_write(u64::MAX, 0, u64::MAX).is_ok());
    }

    #[test]
    fn test_dir_size_missing_is_zero() {
        let p = std::path::Path::new("/nonexistent/wasmrun/path/xyz");
        assert_eq!(dir_size(p), 0);
    }

    #[test]
    fn test_dir_size_sums_files() {
        let tmp = std::env::temp_dir().join(format!("wasmrun_dirsize_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(tmp.join("sub")).unwrap();
        std::fs::write(tmp.join("a.txt"), b"hello").unwrap(); // 5
        std::fs::write(tmp.join("sub/b.txt"), b"world!").unwrap(); // 6
        assert_eq!(dir_size(&tmp), 11);
        let _ = std::fs::remove_dir_all(&tmp);
    }
}
