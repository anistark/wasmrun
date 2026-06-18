//! Agent mode: API-key authentication and tenant mapping.
//!
//! Auth is **opt-in**: it is wired into the server only when the operator passes
//! `--auth <path>`. Without it the server stays fully open (back-compat).
//!
//! Keys are never stored in plaintext. The config file holds the **SHA-256 hash**
//! (hex) of each tenant's key; at request time the presented key is hashed and
//! matched against the stored hashes via a map lookup. Because the lookup key is a
//! one-way hash of the secret, this sidesteps secret-timing attacks without a
//! manual constant-time compare. Keys must be high-entropy random strings.
//!
//! Use `wasmrun agent --hash-key <KEY>` to print the hash for a key.
//!
//! ## Config schema
//!
//! ```toml
//! [[tenants]]
//! id = "copilot"
//! key_sha256 = "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08"
//!
//! [[tenants]]
//! id = "ci"
//! key_sha256 = "60303ae22b998861bce3b28f33eec1be758a213c86c93c076dbe9f558c11c752"
//! ```

use crate::error::{ConfigError, WasmrunError};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;

/// Compute the hex-encoded SHA-256 hash of an API key.
///
/// This is the value stored in the auth config's `key_sha256` field. The
/// `--hash-key` CLI helper prints exactly this.
pub fn hash_key(key: &str) -> String {
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(key.as_bytes());
    hash.iter().map(|b| format!("{b:02x}")).collect()
}

/// Raw TOML shape for `[[tenants]]` entries, before validation/inversion.
#[derive(Debug, Deserialize)]
struct RawAuth {
    #[serde(default)]
    tenants: Vec<RawTenant>,
}

#[derive(Debug, Deserialize)]
struct RawTenant {
    id: String,
    key_sha256: String,
    /// Optional `[tenants.rate]` sub-table; absent = inherit all defaults.
    #[serde(default)]
    rate: Option<TenantRate>,
}

/// Per-tenant rate ceilings, from the optional `[tenants.rate]` sub-table.
///
/// Each field caps one dimension of a tenant's load. A value of `0` (or an
/// omitted field) means "inherit the server-wide default" for that dimension,
/// matching the `0 = default/unlimited` convention used by the CLI limit flags.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
pub struct TenantRate {
    /// Max concurrent (non-expired) sessions this tenant may hold.
    #[serde(default)]
    pub max_sessions: u32,
    /// Max exec workers this tenant may run concurrently.
    #[serde(default)]
    pub max_concurrent_exec: u32,
    /// Max requests this tenant may issue within a rolling one-minute window.
    #[serde(default)]
    pub max_requests_per_min: u32,
}

/// Resolved auth configuration: a map from key hash → tenant id.
///
/// Built once at startup and shared (behind an `Arc`) across all requests.
#[derive(Debug, Clone)]
pub struct AuthConfig {
    /// SHA-256 hex of the key → tenant id.
    keys: HashMap<String, String>,
    /// Tenant id → per-tenant rate ceilings (default = all-inherit).
    rates: HashMap<String, TenantRate>,
}

impl AuthConfig {
    /// Load and validate an auth config from a TOML file.
    ///
    /// Validates that every tenant has a non-empty id and a 64-char lowercase
    /// hex `key_sha256`, and that there are no duplicate ids or duplicate
    /// hashes. Returns a clear error otherwise — startup should abort rather
    /// than silently run open when auth was requested.
    pub fn load(path: &Path) -> Result<Self, WasmrunError> {
        if !path.exists() {
            return Err(WasmrunError::Config(ConfigError::FileNotFound {
                path: path.display().to_string(),
            }));
        }
        if path.is_dir() {
            return Err(WasmrunError::Config(ConfigError::InvalidValue {
                message: format!(
                    "Auth config path is a directory, not a file: {}",
                    path.display()
                ),
            }));
        }

        let content = std::fs::read_to_string(path).map_err(|e| {
            WasmrunError::Config(ConfigError::ParseError {
                message: format!("Failed to read auth config file: {e}"),
            })
        })?;

        let raw: RawAuth = toml::from_str(&content).map_err(|e| {
            WasmrunError::Config(ConfigError::ParseError {
                message: format!("Failed to parse auth config TOML: {e}"),
            })
        })?;

        Self::from_raw(raw)
    }

    /// Validate raw tenants and invert into the hash → id map.
    fn from_raw(raw: RawAuth) -> Result<Self, WasmrunError> {
        if raw.tenants.is_empty() {
            return Err(WasmrunError::Config(ConfigError::InvalidValue {
                message: "Auth config has no [[tenants]] entries".to_string(),
            }));
        }

        let mut keys: HashMap<String, String> = HashMap::with_capacity(raw.tenants.len());
        let mut rates: HashMap<String, TenantRate> = HashMap::with_capacity(raw.tenants.len());
        let mut seen_ids: HashMap<String, ()> = HashMap::with_capacity(raw.tenants.len());

        for tenant in raw.tenants {
            let id = tenant.id.trim();
            if id.is_empty() {
                return Err(WasmrunError::Config(ConfigError::InvalidValue {
                    message: "Auth config has a tenant with an empty id".to_string(),
                }));
            }

            let hash = tenant.key_sha256.trim();
            if !is_sha256_hex(hash) {
                return Err(WasmrunError::Config(ConfigError::InvalidValue {
                    message: format!(
                        "Tenant '{id}' has an invalid key_sha256 (expected 64 lowercase hex chars)"
                    ),
                }));
            }

            if seen_ids.insert(id.to_string(), ()).is_some() {
                return Err(WasmrunError::Config(ConfigError::InvalidValue {
                    message: format!("Auth config has duplicate tenant id '{id}'"),
                }));
            }

            if keys.insert(hash.to_string(), id.to_string()).is_some() {
                return Err(WasmrunError::Config(ConfigError::InvalidValue {
                    message: format!(
                        "Auth config has duplicate key_sha256 (shared by tenant '{id}')"
                    ),
                }));
            }

            rates.insert(id.to_string(), tenant.rate.unwrap_or_default());
        }

        Ok(AuthConfig { keys, rates })
    }

    /// Number of configured tenants.
    pub fn tenant_count(&self) -> usize {
        self.keys.len()
    }

    /// Resolve a presented API key to its tenant id, or `None` if unknown.
    pub fn resolve(&self, presented_key: &str) -> Option<&str> {
        let hash = hash_key(presented_key);
        self.keys.get(&hash).map(|s| s.as_str())
    }

    /// Per-tenant rate ceilings for `id`. Returns `None` for an unknown tenant;
    /// a known tenant with no `[tenants.rate]` table yields the all-inherit
    /// default.
    pub fn rate(&self, id: &str) -> Option<&TenantRate> {
        self.rates.get(id)
    }
}

/// True if `s` is exactly 64 lowercase hex characters (a SHA-256 hex digest).
fn is_sha256_hex(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn test_hash_key_known_vector() {
        // sha256("foo") — standard vector.
        assert_eq!(
            hash_key("foo"),
            "2c26b46b68ffc68ff99b453c1d30413413422d706483bfa0f98a5e886266e7ae"
        );
        // sha256("") — empty string vector.
        assert_eq!(
            hash_key(""),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    fn write_toml(body: &str) -> tempfile::NamedTempFile {
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.write_all(body.as_bytes()).unwrap();
        f
    }

    #[test]
    fn test_load_valid_and_resolve() {
        let body = format!(
            "[[tenants]]\nid = \"copilot\"\nkey_sha256 = \"{}\"\n\n[[tenants]]\nid = \"ci\"\nkey_sha256 = \"{}\"\n",
            hash_key("sk_copilot"),
            hash_key("sk_ci")
        );
        let f = write_toml(&body);
        let cfg = AuthConfig::load(f.path()).unwrap();

        assert_eq!(cfg.tenant_count(), 2);
        assert_eq!(cfg.resolve("sk_copilot"), Some("copilot"));
        assert_eq!(cfg.resolve("sk_ci"), Some("ci"));
        assert_eq!(cfg.resolve("sk_unknown"), None);
        assert_eq!(cfg.resolve(""), None);
    }

    #[test]
    fn test_reject_duplicate_id() {
        let body = format!(
            "[[tenants]]\nid = \"dup\"\nkey_sha256 = \"{}\"\n\n[[tenants]]\nid = \"dup\"\nkey_sha256 = \"{}\"\n",
            hash_key("a"),
            hash_key("b")
        );
        let f = write_toml(&body);
        let err = AuthConfig::load(f.path()).unwrap_err();
        assert!(err.to_string().contains("duplicate tenant id"));
    }

    #[test]
    fn test_reject_duplicate_hash() {
        let body = format!(
            "[[tenants]]\nid = \"a\"\nkey_sha256 = \"{0}\"\n\n[[tenants]]\nid = \"b\"\nkey_sha256 = \"{0}\"\n",
            hash_key("same")
        );
        let f = write_toml(&body);
        let err = AuthConfig::load(f.path()).unwrap_err();
        assert!(err.to_string().contains("duplicate key_sha256"));
    }

    #[test]
    fn test_reject_malformed_hash() {
        let body = "[[tenants]]\nid = \"a\"\nkey_sha256 = \"not-a-real-hash\"\n";
        let f = write_toml(body);
        let err = AuthConfig::load(f.path()).unwrap_err();
        assert!(err.to_string().contains("invalid key_sha256"));
    }

    #[test]
    fn test_reject_empty_id() {
        let body = format!(
            "[[tenants]]\nid = \"\"\nkey_sha256 = \"{}\"\n",
            hash_key("x")
        );
        let f = write_toml(&body);
        let err = AuthConfig::load(f.path()).unwrap_err();
        assert!(err.to_string().contains("empty id"));
    }

    #[test]
    fn test_reject_no_tenants() {
        let f = write_toml("");
        let err = AuthConfig::load(f.path()).unwrap_err();
        assert!(err.to_string().contains("no [[tenants]]"));
    }

    #[test]
    fn test_reject_malformed_toml() {
        let f = write_toml("this is not valid toml = = =");
        let err = AuthConfig::load(f.path()).unwrap_err();
        assert!(err.to_string().contains("parse"));
    }

    #[test]
    fn test_missing_file() {
        let err = AuthConfig::load(Path::new("/nonexistent/wasmrun-auth.toml")).unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn test_parse_tenant_rate() {
        let body = format!(
            "[[tenants]]\nid = \"a\"\nkey_sha256 = \"{}\"\n[tenants.rate]\nmax_sessions = 5\nmax_concurrent_exec = 3\nmax_requests_per_min = 100\n\n[[tenants]]\nid = \"b\"\nkey_sha256 = \"{}\"\n",
            hash_key("ka"),
            hash_key("kb"),
        );
        let f = write_toml(&body);
        let cfg = AuthConfig::load(f.path()).unwrap();

        let ra = cfg.rate("a").unwrap();
        assert_eq!(ra.max_sessions, 5);
        assert_eq!(ra.max_concurrent_exec, 3);
        assert_eq!(ra.max_requests_per_min, 100);

        // A tenant without a [tenants.rate] table inherits the all-zero default.
        assert_eq!(*cfg.rate("b").unwrap(), TenantRate::default());

        // Unknown tenant id resolves to no rate.
        assert!(cfg.rate("nope").is_none());
    }

    #[test]
    fn test_is_sha256_hex() {
        assert!(is_sha256_hex(&hash_key("anything")));
        assert!(!is_sha256_hex("short"));
        assert!(!is_sha256_hex(&"A".repeat(64))); // uppercase rejected
        assert!(!is_sha256_hex(&"g".repeat(64))); // non-hex rejected
    }
}
