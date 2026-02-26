use crate::error::{Result, WasmrunError};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

const WASMHUB_BASE_URL: &str = "https://github.com/anistark/wasmhub/releases/latest/download";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmhubManifest {
    pub version: String,
    pub build_date: String,
    pub languages: HashMap<String, LanguageInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageInfo {
    pub latest: String,
    pub versions: Vec<String>,
    pub source: String,
    pub license: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageManifest {
    pub language: String,
    pub latest: String,
    pub versions: HashMap<String, VersionInfo>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionInfo {
    pub file: String,
    pub size: u64,
    pub sha256: String,
    pub released: String,
    pub wasi: String,
    pub features: Vec<String>,
}

pub struct RuntimeCache {
    cache_dir: PathBuf,
}

#[allow(dead_code)]
impl RuntimeCache {
    pub fn new() -> Result<Self> {
        let cache_dir = dirs::home_dir()
            .ok_or_else(|| WasmrunError::from("Could not determine home directory".to_string()))?
            .join(".wasmrun")
            .join("runtimes");

        fs::create_dir_all(&cache_dir).map_err(|e| {
            WasmrunError::from(format!(
                "Failed to create cache directory {}: {e}",
                cache_dir.display()
            ))
        })?;

        Ok(Self { cache_dir })
    }

    #[cfg(test)]
    fn with_cache_dir(cache_dir: PathBuf) -> Result<Self> {
        fs::create_dir_all(&cache_dir).map_err(|e| {
            WasmrunError::from(format!(
                "Failed to create cache directory {}: {e}",
                cache_dir.display()
            ))
        })?;
        Ok(Self { cache_dir })
    }

    pub fn cache_dir(&self) -> &PathBuf {
        &self.cache_dir
    }

    pub fn get_runtime(&self, language: &str) -> Result<Vec<u8>> {
        if let Some(cached) = self.load_from_cache(language)? {
            return Ok(cached);
        }

        let lang_manifest = self.fetch_language_manifest(language)?;
        let latest = &lang_manifest.latest;
        let version_info = lang_manifest.versions.get(latest.as_str()).ok_or_else(|| {
            WasmrunError::from(format!("Version {latest} not found in {language} manifest"))
        })?;

        let wasm_bytes = self.download_runtime(language, &version_info.file)?;
        self.validate_checksum(&wasm_bytes, &version_info.sha256, &version_info.file)?;
        self.save_to_cache(language, latest, &wasm_bytes, &lang_manifest)?;

        println!(
            "✅ Cached {language} runtime v{latest} ({} bytes)",
            wasm_bytes.len()
        );

        Ok(wasm_bytes)
    }

    pub fn fetch_manifest(&self) -> Result<WasmhubManifest> {
        let url = format!("{WASMHUB_BASE_URL}/manifest.json");
        let body = http_get_string(&url)?;
        serde_json::from_str(&body)
            .map_err(|e| WasmrunError::from(format!("Failed to parse wasmhub manifest: {e}")))
    }

    fn fetch_language_manifest(&self, language: &str) -> Result<LanguageManifest> {
        let url = format!("{WASMHUB_BASE_URL}/{language}-manifest.json");
        let body = http_get_string(&url)?;
        serde_json::from_str(&body)
            .map_err(|e| WasmrunError::from(format!("Failed to parse {language} manifest: {e}")))
    }

    fn download_runtime(&self, language: &str, filename: &str) -> Result<Vec<u8>> {
        let url = format!("{WASMHUB_BASE_URL}/{filename}");
        println!("📦 Downloading {language} runtime from wasmhub...");
        http_get_bytes(&url)
    }

    fn validate_checksum(&self, data: &[u8], expected_sha256: &str, filename: &str) -> Result<()> {
        let actual = sha256_hex(data);
        if actual != expected_sha256 {
            return Err(WasmrunError::from(format!(
                "Checksum mismatch for {filename}: expected {expected_sha256}, got {actual}"
            )));
        }
        Ok(())
    }

    fn load_from_cache(&self, language: &str) -> Result<Option<Vec<u8>>> {
        let meta_path = self.cache_dir.join(format!("{language}.json"));
        if !meta_path.exists() {
            return Ok(None);
        }

        let meta_content = fs::read_to_string(&meta_path)
            .map_err(|e| WasmrunError::from(format!("Failed to read cache metadata: {e}")))?;
        let meta: CacheMetadata = serde_json::from_str(&meta_content)
            .map_err(|e| WasmrunError::from(format!("Failed to parse cache metadata: {e}")))?;

        let wasm_path = self.cache_dir.join(&meta.filename);
        if !wasm_path.exists() {
            return Ok(None);
        }

        let wasm_bytes = fs::read(&wasm_path)
            .map_err(|e| WasmrunError::from(format!("Failed to read cached runtime: {e}")))?;

        let actual_sha = sha256_hex(&wasm_bytes);
        if actual_sha != meta.sha256 {
            println!("⚠️ Cache integrity check failed for {language}, re-downloading...");
            return Ok(None);
        }

        println!(
            "✅ Using cached {language} runtime v{} ({})",
            meta.version,
            format_bytes(wasm_bytes.len())
        );

        Ok(Some(wasm_bytes))
    }

    fn save_to_cache(
        &self,
        language: &str,
        version: &str,
        wasm_bytes: &[u8],
        manifest: &LanguageManifest,
    ) -> Result<()> {
        let version_info = manifest
            .versions
            .get(version)
            .ok_or_else(|| WasmrunError::from(format!("Version {version} not in manifest")))?;

        let wasm_path = self.cache_dir.join(&version_info.file);
        fs::write(&wasm_path, wasm_bytes)
            .map_err(|e| WasmrunError::from(format!("Failed to write cached runtime: {e}")))?;

        let meta = CacheMetadata {
            language: language.to_string(),
            version: version.to_string(),
            filename: version_info.file.clone(),
            sha256: version_info.sha256.clone(),
            size: wasm_bytes.len() as u64,
            wasi: version_info.wasi.clone(),
        };

        let meta_path = self.cache_dir.join(format!("{language}.json"));
        let meta_json = serde_json::to_string_pretty(&meta)
            .map_err(|e| WasmrunError::from(format!("Failed to serialize cache metadata: {e}")))?;
        fs::write(&meta_path, meta_json)
            .map_err(|e| WasmrunError::from(format!("Failed to write cache metadata: {e}")))?;

        Ok(())
    }

    pub fn is_cached(&self, language: &str) -> bool {
        let meta_path = self.cache_dir.join(format!("{language}.json"));
        if !meta_path.exists() {
            return false;
        }
        if let Ok(content) = fs::read_to_string(&meta_path) {
            if let Ok(meta) = serde_json::from_str::<CacheMetadata>(&content) {
                return self.cache_dir.join(&meta.filename).exists();
            }
        }
        false
    }

    pub fn cached_version(&self, language: &str) -> Option<String> {
        let meta_path = self.cache_dir.join(format!("{language}.json"));
        let content = fs::read_to_string(&meta_path).ok()?;
        let meta: CacheMetadata = serde_json::from_str(&content).ok()?;
        Some(meta.version)
    }

    pub fn clear_cache(&self, language: Option<&str>) -> Result<()> {
        match language {
            Some(lang) => {
                let meta_path = self.cache_dir.join(format!("{lang}.json"));
                if meta_path.exists() {
                    if let Ok(content) = fs::read_to_string(&meta_path) {
                        if let Ok(meta) = serde_json::from_str::<CacheMetadata>(&content) {
                            let wasm_path = self.cache_dir.join(&meta.filename);
                            let _ = fs::remove_file(&wasm_path);
                        }
                    }
                    fs::remove_file(&meta_path).map_err(|e| {
                        WasmrunError::from(format!("Failed to remove cache metadata: {e}"))
                    })?;
                }
            }
            None => {
                if self.cache_dir.exists() {
                    fs::remove_dir_all(&self.cache_dir)
                        .map_err(|e| WasmrunError::from(format!("Failed to clear cache: {e}")))?;
                    fs::create_dir_all(&self.cache_dir).map_err(|e| {
                        WasmrunError::from(format!("Failed to recreate cache dir: {e}"))
                    })?;
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CacheMetadata {
    language: String,
    version: String,
    filename: String,
    sha256: String,
    size: u64,
    wasi: String,
}

fn http_get_string(url: &str) -> Result<String> {
    let mut body = ureq::get(url)
        .call()
        .map_err(|e| WasmrunError::from(format!("HTTP request failed for {url}: {e}")))?
        .into_body();

    let mut buf = String::new();
    std::io::Read::read_to_string(&mut body.as_reader(), &mut buf)
        .map_err(|e| WasmrunError::from(format!("Failed to read response body: {e}")))?;
    Ok(buf)
}

fn http_get_bytes(url: &str) -> Result<Vec<u8>> {
    let body = ureq::get(url)
        .call()
        .map_err(|e| WasmrunError::from(format!("HTTP request failed for {url}: {e}")))?
        .into_body();

    let mut buf = Vec::new();
    std::io::Read::read_to_end(&mut body.into_reader(), &mut buf)
        .map_err(|e| WasmrunError::from(format!("Failed to read response body: {e}")))?;
    Ok(buf)
}

fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(data);
    hash.iter().map(|b| format!("{b:02x}")).collect()
}

fn format_bytes(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

#[allow(dead_code)]
pub fn language_for_project(project_path: &str) -> Result<String> {
    use std::path::Path;
    let path = Path::new(project_path);

    if path.join("Cargo.toml").exists() {
        return Ok("rust".to_string());
    }
    if path.join("go.mod").exists() {
        return Ok("go".to_string());
    }
    if path.join("package.json").exists() {
        return Ok("nodejs".to_string());
    }
    if path.join("requirements.txt").exists() || path.join("pyproject.toml").exists() {
        return Ok("python".to_string());
    }

    Err(WasmrunError::from(format!(
        "Could not detect project language at {project_path}"
    )))
}

/// Maps project language names to wasmhub runtime identifiers.
/// wasmhub may use different names (e.g., "nodejs" projects use "quickjs" runtime).
pub fn wasmhub_language(language: &str) -> &str {
    match language {
        "nodejs" | "javascript" | "js" => "quickjs",
        "python" => "rustpython",
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(500), "500 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(1048576), "1.0 MB");
    }

    #[test]
    fn test_sha256_hex_deterministic() {
        let data = b"hello world";
        let hash1 = sha256_hex(data);
        let hash2 = sha256_hex(data);
        assert_eq!(hash1, hash2);
        assert_eq!(hash1.len(), 64);
    }

    #[test]
    fn test_sha256_hex_different_inputs() {
        let hash1 = sha256_hex(b"hello");
        let hash2 = sha256_hex(b"world");
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_wasmhub_language_mapping() {
        assert_eq!(wasmhub_language("nodejs"), "quickjs");
        assert_eq!(wasmhub_language("javascript"), "quickjs");
        assert_eq!(wasmhub_language("js"), "quickjs");
        assert_eq!(wasmhub_language("python"), "rustpython");
        assert_eq!(wasmhub_language("rust"), "rust");
        assert_eq!(wasmhub_language("go"), "go");
    }

    #[test]
    fn test_language_detection() {
        let dir = tempfile::tempdir().unwrap();

        fs::write(dir.path().join("Cargo.toml"), "[package]").unwrap();
        assert_eq!(
            language_for_project(dir.path().to_str().unwrap()).unwrap(),
            "rust"
        );
    }

    #[test]
    fn test_language_detection_nodejs() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("package.json"), "{}").unwrap();
        assert_eq!(
            language_for_project(dir.path().to_str().unwrap()).unwrap(),
            "nodejs"
        );
    }

    #[test]
    fn test_language_detection_go() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("go.mod"), "module test").unwrap();
        assert_eq!(
            language_for_project(dir.path().to_str().unwrap()).unwrap(),
            "go"
        );
    }

    #[test]
    fn test_language_detection_python() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("requirements.txt"), "flask").unwrap();
        assert_eq!(
            language_for_project(dir.path().to_str().unwrap()).unwrap(),
            "python"
        );
    }

    #[test]
    fn test_language_detection_unknown() {
        let dir = tempfile::tempdir().unwrap();
        assert!(language_for_project(dir.path().to_str().unwrap()).is_err());
    }

    #[test]
    fn test_cache_dir_creation() {
        let dir = tempfile::tempdir().unwrap();
        let cache_dir = dir.path().join("test_cache");
        let cache = RuntimeCache::with_cache_dir(cache_dir.clone()).unwrap();
        assert!(cache.cache_dir().exists());
        assert_eq!(cache.cache_dir(), &cache_dir);
    }

    #[test]
    fn test_is_cached_empty() {
        let dir = tempfile::tempdir().unwrap();
        let cache = RuntimeCache::with_cache_dir(dir.path().join("cache")).unwrap();
        assert!(!cache.is_cached("rust"));
        assert!(!cache.is_cached("go"));
    }

    #[test]
    fn test_cached_version_empty() {
        let dir = tempfile::tempdir().unwrap();
        let cache = RuntimeCache::with_cache_dir(dir.path().join("cache")).unwrap();
        assert!(cache.cached_version("rust").is_none());
    }

    #[test]
    fn test_clear_cache_nonexistent() {
        let dir = tempfile::tempdir().unwrap();
        let cache = RuntimeCache::with_cache_dir(dir.path().join("cache")).unwrap();
        assert!(cache.clear_cache(Some("rust")).is_ok());
        assert!(cache.clear_cache(None).is_ok());
    }

    #[test]
    fn test_cache_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let cache = RuntimeCache::with_cache_dir(dir.path().join("cache")).unwrap();

        let wasm_bytes = vec![0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00];
        let sha = sha256_hex(&wasm_bytes);

        let manifest = LanguageManifest {
            language: "test".to_string(),
            latest: "1.0".to_string(),
            versions: HashMap::from([(
                "1.0".to_string(),
                VersionInfo {
                    file: "test-1.0.wasm".to_string(),
                    size: wasm_bytes.len() as u64,
                    sha256: sha,
                    released: "2026-01-01T00:00:00Z".to_string(),
                    wasi: "wasip1".to_string(),
                    features: vec![],
                },
            )]),
        };

        cache
            .save_to_cache("test", "1.0", &wasm_bytes, &manifest)
            .unwrap();

        assert!(cache.is_cached("test"));
        assert_eq!(cache.cached_version("test"), Some("1.0".to_string()));

        let loaded = cache.load_from_cache("test").unwrap().unwrap();
        assert_eq!(loaded, wasm_bytes);
    }

    #[test]
    fn test_cache_integrity_failure() {
        let dir = tempfile::tempdir().unwrap();
        let cache = RuntimeCache::with_cache_dir(dir.path().join("cache")).unwrap();

        let wasm_bytes = vec![0x00, 0x61, 0x73, 0x6d];
        let manifest = LanguageManifest {
            language: "test".to_string(),
            latest: "1.0".to_string(),
            versions: HashMap::from([(
                "1.0".to_string(),
                VersionInfo {
                    file: "test-1.0.wasm".to_string(),
                    size: wasm_bytes.len() as u64,
                    sha256: sha256_hex(&wasm_bytes),
                    released: "2026-01-01T00:00:00Z".to_string(),
                    wasi: "wasip1".to_string(),
                    features: vec![],
                },
            )]),
        };

        cache
            .save_to_cache("test", "1.0", &wasm_bytes, &manifest)
            .unwrap();

        // Corrupt the cached file
        let wasm_path = cache.cache_dir().join("test-1.0.wasm");
        fs::write(&wasm_path, b"corrupted").unwrap();

        // Should detect corruption and return None
        let result = cache.load_from_cache("test").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_clear_specific_language() {
        let dir = tempfile::tempdir().unwrap();
        let cache = RuntimeCache::with_cache_dir(dir.path().join("cache")).unwrap();

        let wasm_bytes = vec![0x00, 0x61, 0x73, 0x6d];
        let manifest = LanguageManifest {
            language: "test".to_string(),
            latest: "1.0".to_string(),
            versions: HashMap::from([(
                "1.0".to_string(),
                VersionInfo {
                    file: "test-1.0.wasm".to_string(),
                    size: wasm_bytes.len() as u64,
                    sha256: sha256_hex(&wasm_bytes),
                    released: "2026-01-01T00:00:00Z".to_string(),
                    wasi: "wasip1".to_string(),
                    features: vec![],
                },
            )]),
        };

        cache
            .save_to_cache("test", "1.0", &wasm_bytes, &manifest)
            .unwrap();
        assert!(cache.is_cached("test"));

        cache.clear_cache(Some("test")).unwrap();
        assert!(!cache.is_cached("test"));
    }
}
