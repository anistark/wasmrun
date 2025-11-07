use super::LogEntry;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

pub const MAX_LOG_ENTRIES: usize = 1000;

pub struct LogTrailSystem {
    entries: Arc<Mutex<VecDeque<LogEntry>>>,
}

impl LogTrailSystem {
    pub fn new() -> Self {
        Self {
            entries: Arc::new(Mutex::new(VecDeque::with_capacity(MAX_LOG_ENTRIES))),
        }
    }

    pub fn log(&self, entry: LogEntry) {
        let mut entries = self.entries.lock().unwrap();
        entries.push_back(entry);

        if entries.len() > MAX_LOG_ENTRIES {
            entries.pop_front();
        }
    }

    pub fn get_all(&self) -> Vec<LogEntry> {
        let entries = self.entries.lock().unwrap();
        entries.iter().cloned().collect()
    }

    pub fn get_recent(&self, count: usize) -> Vec<LogEntry> {
        let entries = self.entries.lock().unwrap();
        entries
            .iter()
            .rev()
            .take(count)
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect()
    }

    #[allow(dead_code)]
    pub fn clear(&self) {
        self.entries.lock().unwrap().clear();
    }

    #[allow(dead_code)]
    pub fn count(&self) -> usize {
        self.entries.lock().unwrap().len()
    }
}

impl Default for LogTrailSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for LogTrailSystem {
    fn clone(&self) -> Self {
        Self {
            entries: Arc::clone(&self.entries),
        }
    }
}
