use notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_mini::{new_debouncer, DebouncedEvent, DebouncedEventKind};
use std::path::Path;
use std::sync::mpsc::{channel, Receiver};
use std::time::Duration;

pub struct ProjectWatcher {
    debounced_receiver: Option<Receiver<Result<Vec<DebouncedEvent>, Vec<notify::Error>>>>,
    #[allow(dead_code)]
    watcher: Option<notify_debouncer_mini::Debouncer<RecommendedWatcher>>,
}

impl ProjectWatcher {
    pub fn new(project_path: &str) -> Result<Self, String> {
        let path = Path::new(project_path);

        if !path.exists() {
            return Err(format!("Path does not exist: {}", project_path));
        }

        if !path.is_dir() {
            return Err(format!("Path is not a directory: {}", project_path));
        }

        // Channel for events
        let (tx, rx) = channel();

        let mut debouncer = new_debouncer(Duration::from_millis(500), None, tx)
            .map_err(|e| format!("Failed to create file watcher: {}", e))?;

        debouncer
            .watcher()
            .watch(path, RecursiveMode::Recursive)
            .map_err(|e| format!("Failed to watch directory: {}", e))?;

        println!("🔍 Watching directory: {}", project_path);

        Ok(Self {
            debounced_receiver: Some(rx),
            watcher: Some(debouncer),
        })
    }

    pub fn wait_for_change(&self) -> Option<Result<Vec<DebouncedEvent>, Vec<notify::Error>>> {
        if let Some(rx) = &self.debounced_receiver {
            match rx.recv() {
                Ok(event) => Some(event),
                Err(_) => None,
            }
        } else {
            None
        }
    }

    pub fn should_recompile(&self, events: &[DebouncedEvent]) -> bool {
        for event in events {
            if event.kind == DebouncedEventKind::Any {
                let path = &event.path;

                if path.components().any(|c| {
                    let s = c.as_os_str().to_string_lossy();
                    s == "target" || s.starts_with(".")
                }) {
                    continue;
                }

                if let Some(ext) = path.extension() {
                    let ext = ext.to_string_lossy().to_lowercase();

                    if [
                        "rs", "go", "c", "cpp", "h", "hpp", "ts", "js", "toml", "py", "mod",
                    ]
                    .contains(&ext.as_str())
                    {
                        return true;
                    }
                } else if path.file_name().map_or(false, |f| {
                    let name = f.to_string_lossy().to_lowercase();
                    ["cargo.toml", "makefile", "go.mod", "package.json"].contains(&name.as_str())
                }) {
                    return true;
                }
            }
        }

        false
    }
}
