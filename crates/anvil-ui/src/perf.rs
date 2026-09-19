//! Performance overlay: CPU and memory of the whole computer and of Anvil,
//! frames per second, and the time the last frame took.

use std::collections::VecDeque;
use std::time::{Duration, Instant};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

/// How often the CPU and memory numbers are read.
const REFRESH: Duration = Duration::from_millis(1000);

#[derive(Clone, Copy, Debug, Default)]
pub struct Stats {
    /// Whole computer, percent of all cores.
    pub cpu_total: f32,
    /// Anvil, percent of all cores.
    pub cpu_app: f32,
    pub mem_used: u64,
    pub mem_total: u64,
    pub mem_app: u64,
    pub fps: f32,
    /// Time spent building the last frame, in milliseconds.
    pub frame_ms: f32,
}

pub struct PerfMonitor {
    sys: System,
    pid: Option<Pid>,
    cores: f32,
    last_refresh: Option<Instant>,
    frames: VecDeque<Instant>,
    pub stats: Stats,
}

impl Default for PerfMonitor {
    fn default() -> Self {
        let mut sys = System::new();
        sys.refresh_cpu_usage();
        let cores = sys.cpus().len().max(1) as f32;
        PerfMonitor {
            sys,
            pid: sysinfo::get_current_pid().ok(),
            cores,
            last_refresh: None,
            frames: VecDeque::new(),
            stats: Stats::default(),
        }
    }
}

impl PerfMonitor {
    /// Count a frame that took `frame` to build, and read the system
    /// numbers when they are due.
    pub fn tick(&mut self, frame: Duration) {
        let now = Instant::now();
        self.frames.push_back(now);
        while self.frames.front().is_some_and(|t| now.duration_since(*t) > Duration::from_secs(1)) {
            self.frames.pop_front();
        }
        self.stats.fps = self.frames.len() as f32;
        self.stats.frame_ms = frame.as_secs_f32() * 1000.0;
        if self.last_refresh.is_some_and(|t| now.duration_since(t) < REFRESH) {
            return;
        }
        self.last_refresh = Some(now);
        self.sys.refresh_cpu_usage();
        self.sys.refresh_memory();
        self.stats.cpu_total = self.sys.global_cpu_usage();
        self.stats.mem_used = self.sys.used_memory();
        self.stats.mem_total = self.sys.total_memory();
        if let Some(pid) = self.pid {
            self.sys.refresh_processes_specifics(
                ProcessesToUpdate::Some(&[pid]),
                true,
                ProcessRefreshKind::nothing().with_cpu().with_memory(),
            );
            if let Some(p) = self.sys.process(pid) {
                // A process reports percent of one core; show percent of all.
                self.stats.cpu_app = p.cpu_usage() / self.cores;
                self.stats.mem_app = p.memory();
            }
        }
    }

    /// Lines of text for the overlay.
    pub fn lines(&self) -> [String; 3] {
        let s = &self.stats;
        let gb = |b: u64| b as f64 / 1024f64.powi(3);
        [
            format!("CPU  {:>5.1}% total, {:>5.1}% Anvil", s.cpu_total, s.cpu_app),
            format!(
                "RAM  {:.1} / {:.1} GB, Anvil {:.0} MB",
                gb(s.mem_used),
                gb(s.mem_total),
                s.mem_app as f64 / 1024f64.powi(2)
            ),
            format!("{:.0} fps, frame {:.1} ms", s.fps, s.frame_ms),
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_memory_and_counts_frames() {
        let mut m = PerfMonitor::default();
        m.tick(Duration::from_millis(4));
        m.tick(Duration::from_millis(4));
        assert_eq!(m.stats.fps, 2.0);
        assert!(m.stats.mem_total > 0);
        assert!(m.stats.mem_app > 0, "own process memory should be readable");
        assert!(m.lines()[2].contains("fps"));
    }
}
