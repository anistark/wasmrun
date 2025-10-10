use anyhow::Result;
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use super::microkernel::{Pid, Process, ProcessState};

pub struct ProcessScheduler {
    ready_queue: Arc<Mutex<VecDeque<Pid>>>,
    current_process: Arc<Mutex<Option<Pid>>>,
    time_slice_ms: u64,
}

impl Default for ProcessScheduler {
    fn default() -> Self {
        Self::new()
    }
}

impl ProcessScheduler {
    pub fn new() -> Self {
        Self {
            ready_queue: Arc::new(Mutex::new(VecDeque::new())),
            current_process: Arc::new(Mutex::new(None)),
            time_slice_ms: 100,
        }
    }

    pub fn add_process(&self, pid: Pid) {
        let mut queue = self.ready_queue.lock().unwrap();
        queue.push_back(pid);
    }

    pub fn remove_process(&self, pid: Pid) {
        let mut queue = self.ready_queue.lock().unwrap();
        queue.retain(|&p| p != pid);

        let mut current = self.current_process.lock().unwrap();
        if *current == Some(pid) {
            *current = None;
        }
    }

    pub fn schedule_next(&self) -> Option<Pid> {
        let mut queue = self.ready_queue.lock().unwrap();
        let mut current = self.current_process.lock().unwrap();

        if let Some(current_pid) = *current {
            queue.push_back(current_pid);
        }

        let next_pid = queue.pop_front();
        *current = next_pid;
        next_pid
    }

    pub fn get_current(&self) -> Option<Pid> {
        *self.current_process.lock().unwrap()
    }

    pub fn get_time_slice(&self) -> u64 {
        self.time_slice_ms
    }

    pub fn set_time_slice(&mut self, ms: u64) {
        self.time_slice_ms = ms;
    }

    pub fn queue_size(&self) -> usize {
        self.ready_queue.lock().unwrap().len()
    }

    pub fn block_current(&self) -> Option<Pid> {
        let mut current = self.current_process.lock().unwrap();
        let blocked_pid = *current;
        *current = None;
        blocked_pid
    }

    pub fn unblock_process(&self, pid: Pid) {
        self.add_process(pid);
    }
}

pub fn update_process_state_for_schedule(
    processes: &mut std::collections::HashMap<Pid, Process>,
    old_pid: Option<Pid>,
    new_pid: Option<Pid>,
) {
    if let Some(pid) = old_pid {
        if let Some(process) = processes.get_mut(&pid) {
            if process.state == ProcessState::Running {
                process.state = ProcessState::Ready;
            }
        }
    }

    if let Some(pid) = new_pid {
        if let Some(process) = processes.get_mut(&pid) {
            process.state = ProcessState::Running;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scheduler_creation() {
        let scheduler = ProcessScheduler::new();
        assert_eq!(scheduler.queue_size(), 0);
        assert_eq!(scheduler.get_current(), None);
    }

    #[test]
    fn test_add_process() {
        let scheduler = ProcessScheduler::new();
        scheduler.add_process(1);
        scheduler.add_process(2);
        assert_eq!(scheduler.queue_size(), 2);
    }

    #[test]
    fn test_schedule_next() {
        let scheduler = ProcessScheduler::new();
        scheduler.add_process(1);
        scheduler.add_process(2);

        let next = scheduler.schedule_next();
        assert_eq!(next, Some(1));
        assert_eq!(scheduler.get_current(), Some(1));
    }

    #[test]
    fn test_round_robin() {
        let scheduler = ProcessScheduler::new();
        scheduler.add_process(1);
        scheduler.add_process(2);
        scheduler.add_process(3);

        assert_eq!(scheduler.schedule_next(), Some(1));
        assert_eq!(scheduler.schedule_next(), Some(2));
        assert_eq!(scheduler.schedule_next(), Some(3));
        assert_eq!(scheduler.schedule_next(), Some(1));
    }

    #[test]
    fn test_remove_process() {
        let scheduler = ProcessScheduler::new();
        scheduler.add_process(1);
        scheduler.add_process(2);
        scheduler.remove_process(1);

        assert_eq!(scheduler.queue_size(), 1);
        assert_eq!(scheduler.schedule_next(), Some(2));
    }

    #[test]
    fn test_block_unblock() {
        let scheduler = ProcessScheduler::new();
        scheduler.add_process(1);
        scheduler.schedule_next();

        let blocked = scheduler.block_current();
        assert_eq!(blocked, Some(1));
        assert_eq!(scheduler.get_current(), None);

        scheduler.unblock_process(1);
        assert_eq!(scheduler.queue_size(), 1);
    }
}
