use crate::error::{Result, WasmrunError};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

const DEFAULT_IGNORE_DIRS: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "__pycache__",
    ".venv",
    "venv",
    "dist",
    ".DS_Store",
    ".idea",
    ".vscode",
    ".cache",
    ".tox",
    ".mypy_cache",
    ".pytest_cache",
    ".eggs",
    "*.egg-info",
    ".cargo",
];

const DEFAULT_IGNORE_EXTENSIONS: &[&str] = &[
    "pyc", "pyo", "o", "so", "dylib", "dll", "exe", "class", "jar", "wasm",
];

const MAX_FILE_SIZE: u64 = 10 * 1024 * 1024;
const MAX_TOTAL_SIZE: u64 = 50 * 1024 * 1024;
const MAX_FILE_COUNT: usize = 5000;

#[derive(Debug, Serialize)]
pub struct ProjectFilesBundle {
    pub files: HashMap<String, String>,
    pub file_count: usize,
    pub total_size: u64,
    pub project_path: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub skipped: Vec<SkippedFile>,
}

#[derive(Debug, Serialize)]
pub struct SkippedFile {
    pub path: String,
    pub reason: String,
}

pub struct ProjectFilesCollector {
    root: PathBuf,
    ignore_patterns: Vec<IgnorePattern>,
}

#[derive(Debug, Clone)]
struct IgnorePattern {
    pattern: String,
    is_dir_only: bool,
    is_negated: bool,
}

impl ProjectFilesCollector {
    pub fn new(project_path: &str) -> Result<Self> {
        let root = PathBuf::from(project_path).canonicalize().map_err(|e| {
            WasmrunError::from(format!("Invalid project path '{project_path}': {e}"))
        })?;

        if !root.is_dir() {
            return Err(WasmrunError::from(format!(
                "Project path is not a directory: {}",
                root.display()
            )));
        }

        let mut ignore_patterns = default_patterns();
        if let Ok(parsed) = parse_gitignore(&root) {
            ignore_patterns.extend(parsed);
        }

        Ok(Self {
            root,
            ignore_patterns,
        })
    }

    pub fn collect(&self) -> Result<ProjectFilesBundle> {
        let mut files = HashMap::new();
        let mut total_size: u64 = 0;
        let mut skipped = Vec::new();

        self.walk_dir(&self.root, &mut files, &mut total_size, &mut skipped)?;

        let file_count = files.len();
        Ok(ProjectFilesBundle {
            files,
            file_count,
            total_size,
            project_path: self.root.display().to_string(),
            skipped,
        })
    }

    fn walk_dir(
        &self,
        dir: &Path,
        files: &mut HashMap<String, String>,
        total_size: &mut u64,
        skipped: &mut Vec<SkippedFile>,
    ) -> Result<()> {
        let entries = fs::read_dir(dir).map_err(|e| {
            WasmrunError::from(format!("Failed to read directory {}: {e}", dir.display()))
        })?;

        for entry in entries {
            let entry = entry.map_err(|e| WasmrunError::from(e.to_string()))?;
            let path = entry.path();

            if path.is_symlink() {
                continue;
            }

            let relative = path.strip_prefix(&self.root).unwrap_or(&path);
            let relative_str = relative.to_string_lossy().to_string();

            if self.is_ignored(relative, path.is_dir()) {
                continue;
            }

            if path.is_dir() {
                self.walk_dir(&path, files, total_size, skipped)?;
            } else if path.is_file() {
                if files.len() >= MAX_FILE_COUNT {
                    skipped.push(SkippedFile {
                        path: relative_str,
                        reason: "max file count reached".to_string(),
                    });
                    continue;
                }

                let metadata = fs::metadata(&path).map_err(|e| {
                    WasmrunError::from(format!("Failed to stat {}: {e}", path.display()))
                })?;
                let file_size = metadata.len();

                if file_size > MAX_FILE_SIZE {
                    skipped.push(SkippedFile {
                        path: relative_str,
                        reason: format!("exceeds {MAX_FILE_SIZE} byte limit"),
                    });
                    continue;
                }

                if *total_size + file_size > MAX_TOTAL_SIZE {
                    skipped.push(SkippedFile {
                        path: relative_str,
                        reason: "total size limit reached".to_string(),
                    });
                    continue;
                }

                match fs::read(&path) {
                    Ok(content) => {
                        *total_size += file_size;
                        let encoded = BASE64.encode(&content);
                        files.insert(relative_str, encoded);
                    }
                    Err(e) => {
                        skipped.push(SkippedFile {
                            path: relative_str,
                            reason: format!("read error: {e}"),
                        });
                    }
                }
            }
        }

        Ok(())
    }

    fn is_ignored(&self, relative_path: &Path, is_dir: bool) -> bool {
        for component in relative_path.components() {
            let name = component.as_os_str().to_string_lossy();
            if is_default_ignored_dir(&name) {
                return true;
            }
        }

        if !is_dir {
            if let Some(ext) = relative_path.extension() {
                let ext_str = ext.to_string_lossy();
                if DEFAULT_IGNORE_EXTENSIONS.contains(&ext_str.as_ref()) {
                    return true;
                }
            }
        }

        let path_str = relative_path.to_string_lossy();
        let mut ignored = false;
        for pattern in &self.ignore_patterns {
            if pattern.is_dir_only && !is_dir {
                continue;
            }
            if pattern_matches(&pattern.pattern, &path_str, is_dir) {
                ignored = !pattern.is_negated;
            }
        }

        ignored
    }
}

fn is_default_ignored_dir(name: &str) -> bool {
    for pattern in DEFAULT_IGNORE_DIRS {
        if let Some(suffix) = pattern.strip_prefix('*') {
            if name.ends_with(suffix) {
                return true;
            }
        } else if *pattern == name {
            return true;
        }
    }
    false
}

fn default_patterns() -> Vec<IgnorePattern> {
    Vec::new()
}

fn parse_gitignore(root: &Path) -> Result<Vec<IgnorePattern>> {
    let gitignore_path = root.join(".gitignore");
    if !gitignore_path.exists() {
        return Ok(Vec::new());
    }

    let content = fs::read_to_string(&gitignore_path)
        .map_err(|e| WasmrunError::from(format!("Failed to read .gitignore: {e}")))?;

    Ok(parse_gitignore_content(&content))
}

fn parse_gitignore_content(content: &str) -> Vec<IgnorePattern> {
    content
        .lines()
        .filter_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                return None;
            }

            let (is_negated, pattern) = if let Some(rest) = trimmed.strip_prefix('!') {
                (true, rest.to_string())
            } else {
                (false, trimmed.to_string())
            };

            let (is_dir_only, pattern) = if let Some(stripped) = pattern.strip_suffix('/') {
                (true, stripped.to_string())
            } else {
                (false, pattern)
            };

            Some(IgnorePattern {
                pattern,
                is_dir_only,
                is_negated,
            })
        })
        .collect()
}

fn pattern_matches(pattern: &str, path: &str, is_dir: bool) -> bool {
    if pattern.contains('/') {
        glob_match(pattern, path)
    } else {
        let file_name = Path::new(path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        if glob_match(pattern, &file_name) {
            return true;
        }

        if is_dir {
            return false;
        }

        for component in Path::new(path).components() {
            let name = component.as_os_str().to_string_lossy();
            if glob_match(pattern, &name) {
                return true;
            }
        }

        false
    }
}

fn glob_match(pattern: &str, text: &str) -> bool {
    let pattern_chars: Vec<char> = pattern.chars().collect();
    let text_chars: Vec<char> = text.chars().collect();
    glob_match_recursive(&pattern_chars, &text_chars, 0, 0)
}

fn glob_match_recursive(pattern: &[char], text: &[char], pi: usize, ti: usize) -> bool {
    if pi == pattern.len() {
        return ti == text.len();
    }

    if pattern[pi] == '*' {
        if pi + 1 < pattern.len() && pattern[pi + 1] == '*' {
            let next_pi = if pi + 2 < pattern.len() && pattern[pi + 2] == '/' {
                pi + 3
            } else {
                pi + 2
            };
            for i in ti..=text.len() {
                if glob_match_recursive(pattern, text, next_pi, i) {
                    return true;
                }
            }
            return false;
        }

        for i in ti..=text.len() {
            if glob_match_recursive(pattern, text, pi + 1, i) {
                return true;
            }
            if i < text.len() && text[i] == '/' {
                break;
            }
        }
        return false;
    }

    if ti >= text.len() {
        return false;
    }

    if pattern[pi] == '?' || pattern[pi] == text[ti] {
        return glob_match_recursive(pattern, text, pi + 1, ti + 1);
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_glob_match_exact() {
        assert!(glob_match("hello", "hello"));
        assert!(!glob_match("hello", "world"));
    }

    #[test]
    fn test_glob_match_star() {
        assert!(glob_match("*.js", "index.js"));
        assert!(glob_match("*.js", ".js"));
        assert!(!glob_match("*.js", "index.ts"));
        assert!(glob_match("test*", "testing"));
    }

    #[test]
    fn test_glob_match_question() {
        assert!(glob_match("?.js", "a.js"));
        assert!(!glob_match("?.js", "ab.js"));
    }

    #[test]
    fn test_glob_match_double_star() {
        assert!(glob_match("**/*.js", "src/index.js"));
        assert!(glob_match("**/*.js", "a/b/c/index.js"));
        assert!(glob_match("**/test", "a/b/test"));
    }

    #[test]
    fn test_parse_gitignore_content() {
        let content = "
# comment
node_modules/
*.pyc
!important.pyc
build
";
        let patterns = parse_gitignore_content(content);
        assert_eq!(patterns.len(), 4);
        assert!(patterns[0].is_dir_only);
        assert_eq!(patterns[0].pattern, "node_modules");
        assert!(!patterns[1].is_negated);
        assert_eq!(patterns[1].pattern, "*.pyc");
        assert!(patterns[2].is_negated);
        assert_eq!(patterns[2].pattern, "important.pyc");
        assert!(!patterns[3].is_dir_only);
        assert_eq!(patterns[3].pattern, "build");
    }

    #[test]
    fn test_parse_gitignore_skips_comments_and_blanks() {
        let content = "# this is a comment\n\n  \nfoo\n";
        let patterns = parse_gitignore_content(content);
        assert_eq!(patterns.len(), 1);
        assert_eq!(patterns[0].pattern, "foo");
    }

    #[test]
    fn test_default_ignored_dirs() {
        assert!(is_default_ignored_dir("node_modules"));
        assert!(is_default_ignored_dir(".git"));
        assert!(is_default_ignored_dir("target"));
        assert!(is_default_ignored_dir("__pycache__"));
        assert!(!is_default_ignored_dir("src"));
        assert!(!is_default_ignored_dir("lib"));
    }

    #[test]
    fn test_default_ignored_dir_glob() {
        assert!(is_default_ignored_dir("mypackage.egg-info"));
        assert!(is_default_ignored_dir("another.egg-info"));
    }

    #[test]
    fn test_default_ignored_extensions() {
        assert!(DEFAULT_IGNORE_EXTENSIONS.contains(&"pyc"));
        assert!(DEFAULT_IGNORE_EXTENSIONS.contains(&"wasm"));
        assert!(!DEFAULT_IGNORE_EXTENSIONS.contains(&"js"));
        assert!(!DEFAULT_IGNORE_EXTENSIONS.contains(&"rs"));
    }

    #[test]
    fn test_collect_simple_project() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("index.js"), "console.log('hi')").unwrap();
        fs::write(dir.path().join("package.json"), "{}").unwrap();

        let collector = ProjectFilesCollector::new(dir.path().to_str().unwrap()).unwrap();
        let bundle = collector.collect().unwrap();

        assert_eq!(bundle.file_count, 2);
        assert!(bundle.files.contains_key("index.js"));
        assert!(bundle.files.contains_key("package.json"));

        let decoded = BASE64
            .decode(bundle.files.get("index.js").unwrap())
            .unwrap();
        assert_eq!(decoded, b"console.log('hi')");
    }

    #[test]
    fn test_collect_nested_directories() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("src/utils")).unwrap();
        fs::write(dir.path().join("src/main.rs"), "fn main() {}").unwrap();
        fs::write(dir.path().join("src/utils/helper.rs"), "pub fn help() {}").unwrap();

        let collector = ProjectFilesCollector::new(dir.path().to_str().unwrap()).unwrap();
        let bundle = collector.collect().unwrap();

        assert_eq!(bundle.file_count, 2);
        assert!(bundle.files.contains_key("src/main.rs"));
        assert!(bundle.files.contains_key("src/utils/helper.rs"));
    }

    #[test]
    fn test_collect_ignores_node_modules() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("index.js"), "hi").unwrap();
        fs::create_dir_all(dir.path().join("node_modules/pkg")).unwrap();
        fs::write(dir.path().join("node_modules/pkg/index.js"), "lib").unwrap();

        let collector = ProjectFilesCollector::new(dir.path().to_str().unwrap()).unwrap();
        let bundle = collector.collect().unwrap();

        assert_eq!(bundle.file_count, 1);
        assert!(bundle.files.contains_key("index.js"));
    }

    #[test]
    fn test_collect_ignores_dot_git() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("main.py"), "print('hi')").unwrap();
        fs::create_dir_all(dir.path().join(".git/objects")).unwrap();
        fs::write(dir.path().join(".git/HEAD"), "ref: refs/heads/main").unwrap();

        let collector = ProjectFilesCollector::new(dir.path().to_str().unwrap()).unwrap();
        let bundle = collector.collect().unwrap();

        assert_eq!(bundle.file_count, 1);
        assert!(bundle.files.contains_key("main.py"));
    }

    #[test]
    fn test_collect_ignores_binary_extensions() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("main.rs"), "fn main() {}").unwrap();
        fs::write(dir.path().join("output.o"), vec![0u8; 100]).unwrap();
        fs::write(dir.path().join("lib.so"), vec![0u8; 100]).unwrap();

        let collector = ProjectFilesCollector::new(dir.path().to_str().unwrap()).unwrap();
        let bundle = collector.collect().unwrap();

        assert_eq!(bundle.file_count, 1);
        assert!(bundle.files.contains_key("main.rs"));
    }

    #[test]
    fn test_collect_respects_gitignore() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(".gitignore"), "*.log\nsecrets/\n").unwrap();
        fs::write(dir.path().join("app.js"), "app").unwrap();
        fs::write(dir.path().join("debug.log"), "log data").unwrap();
        fs::create_dir_all(dir.path().join("secrets")).unwrap();
        fs::write(dir.path().join("secrets/key.pem"), "secret").unwrap();

        let collector = ProjectFilesCollector::new(dir.path().to_str().unwrap()).unwrap();
        let bundle = collector.collect().unwrap();

        assert!(bundle.files.contains_key("app.js"));
        assert!(bundle.files.contains_key(".gitignore"));
        assert!(!bundle.files.contains_key("debug.log"));
        assert!(!bundle.files.contains_key("secrets/key.pem"));
    }

    #[test]
    fn test_collect_negation_in_gitignore() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(".gitignore"), "*.log\n!important.log\n").unwrap();
        fs::write(dir.path().join("debug.log"), "debug").unwrap();
        fs::write(dir.path().join("important.log"), "keep this").unwrap();

        let collector = ProjectFilesCollector::new(dir.path().to_str().unwrap()).unwrap();
        let bundle = collector.collect().unwrap();

        assert!(!bundle.files.contains_key("debug.log"));
        assert!(bundle.files.contains_key("important.log"));
    }

    #[test]
    fn test_collect_nonexistent_path() {
        let result = ProjectFilesCollector::new("/nonexistent/path/xyz");
        assert!(result.is_err());
    }

    #[test]
    fn test_collect_file_not_directory() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("file.txt");
        fs::write(&file_path, "hello").unwrap();

        let result = ProjectFilesCollector::new(file_path.to_str().unwrap());
        assert!(result.is_err());
    }

    #[test]
    fn test_collect_empty_directory() {
        let dir = tempfile::tempdir().unwrap();
        let collector = ProjectFilesCollector::new(dir.path().to_str().unwrap()).unwrap();
        let bundle = collector.collect().unwrap();

        assert_eq!(bundle.file_count, 0);
        assert_eq!(bundle.total_size, 0);
    }

    #[test]
    fn test_collect_binary_content_base64() {
        let dir = tempfile::tempdir().unwrap();
        let binary_data: Vec<u8> = (0..=255).collect();
        fs::write(dir.path().join("data.bin"), &binary_data).unwrap();

        let collector = ProjectFilesCollector::new(dir.path().to_str().unwrap()).unwrap();
        let bundle = collector.collect().unwrap();

        let encoded = bundle.files.get("data.bin").unwrap();
        let decoded = BASE64.decode(encoded).unwrap();
        assert_eq!(decoded, binary_data);
    }

    #[test]
    fn test_pattern_matches_filename() {
        assert!(pattern_matches("*.js", "index.js", false));
        assert!(!pattern_matches("*.js", "index.ts", false));
        assert!(pattern_matches("Makefile", "Makefile", false));
    }

    #[test]
    fn test_pattern_matches_path() {
        assert!(pattern_matches("build/output", "build/output", false));
        assert!(!pattern_matches("build/output", "other/output", false));
    }

    #[test]
    fn test_bundle_serialization() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("test.txt"), "hello").unwrap();

        let collector = ProjectFilesCollector::new(dir.path().to_str().unwrap()).unwrap();
        let bundle = collector.collect().unwrap();

        let json = serde_json::to_string(&bundle).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed["file_count"], 1);
        assert!(parsed["files"]["test.txt"].is_string());
    }
}
