//! [Server Mode] File watching for `--watch`

use notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_mini::{new_debouncer, DebouncedEvent, DebouncedEventKind};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;

/// Directories a build writes into. Watching them would rebuild on every build
const IGNORED_DIRS: &[&str] = &["target", "node_modules", "pkg", "build", "dist"];

const SOURCE_EXTENSIONS: &[&str] = &[
    "rs", "go", "c", "cc", "cpp", "h", "hpp", "ts", "js", "mjs", "toml", "py", "mod",
];
const SOURCE_FILES: &[&str] = &["makefile", "go.mod", "package.json", "asconfig.json"];
const STATIC_EXTENSIONS: &[&str] = &[
    "html", "css", "svg", "png", "jpg", "jpeg", "gif", "webp", "ico", "json", "woff", "woff2",
];

/// What a batch of file events asks for
#[derive(Debug, PartialEq, Eq)]
pub enum Change {
    /// Source changed: rebuild
    Source(PathBuf),
    /// A page or asset changed: reload without building
    Static(PathBuf),
    None,
}

pub struct ProjectWatcher {
    root: PathBuf,
    receiver: Receiver<Result<Vec<DebouncedEvent>, notify::Error>>,
    _debouncer: notify_debouncer_mini::Debouncer<RecommendedWatcher>,
}

impl ProjectWatcher {
    pub fn new(project_path: &str) -> Result<Self, String> {
        let path = Path::new(project_path);

        if !path.exists() {
            return Err(format!("Path does not exist: {project_path}"));
        }

        if !path.is_dir() {
            return Err(format!("Path is not a directory: {project_path}"));
        }

        let (tx, rx) = channel();

        let mut debouncer = new_debouncer(Duration::from_millis(500), tx)
            .map_err(|e| format!("Failed to create file watcher: {e}"))?;

        debouncer
            .watcher()
            .watch(path, RecursiveMode::Recursive)
            .map_err(|e| format!("Failed to watch directory: {e}"))?;

        Ok(Self {
            root: path.canonicalize().unwrap_or_else(|_| path.to_path_buf()),
            receiver: rx,
            _debouncer: debouncer,
        })
    }

    pub fn wait_for_change(&self) -> Option<Result<Vec<DebouncedEvent>, notify::Error>> {
        self.receiver.recv().ok()
    }

    pub fn classify(&self, events: &[DebouncedEvent]) -> Change {
        classify_paths(
            &self.root,
            events
                .iter()
                .filter(|e| e.kind == DebouncedEventKind::Any)
                .map(|e| e.path.as_path()),
        )
    }
}

fn classify_paths<'a>(root: &Path, paths: impl Iterator<Item = &'a Path>) -> Change {
    let mut change = Change::None;
    for path in paths {
        let relative = path.strip_prefix(root).unwrap_or(path);
        if relative.components().any(|c| {
            let s = c.as_os_str().to_string_lossy();
            IGNORED_DIRS.contains(&s.as_ref()) || s.starts_with('.')
        }) {
            continue;
        }

        let name = path
            .file_name()
            .map(|f| f.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let ext = path
            .extension()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();

        if SOURCE_FILES.contains(&name.as_str()) || SOURCE_EXTENSIONS.contains(&ext.as_str()) {
            return Change::Source(path.to_path_buf());
        }
        if change == Change::None && STATIC_EXTENSIONS.contains(&ext.as_str()) {
            change = Change::Static(path.to_path_buf());
        }
    }
    change
}

#[cfg(test)]
mod tests {
    use super::*;

    fn classify(paths: &[&str]) -> Change {
        classify_paths(Path::new("/home/.me/build/p"), paths.iter().map(Path::new))
    }

    #[test]
    fn test_source_changes_rebuild() {
        assert_eq!(
            classify(&["/home/.me/build/p/src/lib.rs"]),
            Change::Source("/home/.me/build/p/src/lib.rs".into())
        );
        assert_eq!(
            classify(&["/home/.me/build/p/Makefile"]),
            Change::Source("/home/.me/build/p/Makefile".into())
        );
        assert_eq!(
            classify(&["/home/.me/build/p/assembly/index.ts"]),
            Change::Source("/home/.me/build/p/assembly/index.ts".into())
        );
    }

    #[test]
    fn test_page_changes_reload() {
        assert_eq!(
            classify(&["/home/.me/build/p/index.html"]),
            Change::Static("/home/.me/build/p/index.html".into())
        );
        assert_eq!(
            classify(&[
                "/home/.me/build/p/styles/app.css",
                "/home/.me/build/p/src/main.go"
            ]),
            Change::Source("/home/.me/build/p/src/main.go".into())
        );
    }

    #[test]
    fn test_build_output_and_hidden_files_are_ignored() {
        assert_eq!(
            classify(&[
                "/home/.me/build/p/target/wasm32-unknown-unknown/release/app.wasm",
                "/home/.me/build/p/build/optimized.js",
                "/home/.me/build/p/pkg/app.js",
                "/home/.me/build/p/node_modules/x/index.js",
                "/home/.me/build/p/.git/index",
                "/home/.me/build/p/src/.lib.rs.swp",
                "/home/.me/build/p/4913",
            ]),
            Change::None
        );
    }
}
