//! Performance overlay: CPU and memory of the whole computer and of Anvil,
//! frames per second, the time the last frame took, and, where the
//! machine will say, the load on the GPU.

use std::collections::VecDeque;
use std::sync::mpsc::{Receiver, TryRecvError};
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
    gpu: GpuProbe,
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
            gpu: GpuProbe::default(),
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
        self.gpu.poll();
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
    pub fn lines(&self) -> Vec<String> {
        let s = &self.stats;
        let gb = |b: u64| b as f64 / 1024f64.powi(3);
        vec![
            format!("CPU  {:>5.1}% total, {:>5.1}% Anvil", s.cpu_total, s.cpu_app),
            format!(
                "RAM  {:.1} / {:.1} GB, Anvil {:.0} MB",
                gb(s.mem_used),
                gb(s.mem_total),
                s.mem_app as f64 / 1024f64.powi(2)
            ),
            self.gpu.line(),
            format!("{:.0} fps, frame {:.1} ms", s.fps, s.frame_ms),
        ]
    }
}

/// What a GPU reports about itself. Every field is optional: a driver
/// that will not say is left blank rather than guessed.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GpuStats {
    /// Percent of the GPU in use.
    pub busy: Option<f32>,
    pub mem_used: Option<u64>,
    pub mem_total: Option<u64>,
}

impl GpuStats {
    fn is_empty(&self) -> bool {
        self.busy.is_none() && self.mem_used.is_none() && self.mem_total.is_none()
    }

    /// One line for the overlay.
    pub fn line(&self) -> String {
        let mut parts = Vec::new();
        if let Some(b) = self.busy {
            parts.push(format!("{b:>5.1}% busy"));
        }
        let gb = |b: u64| b as f64 / 1024f64.powi(3);
        match (self.mem_used, self.mem_total) {
            (Some(u), Some(t)) => parts.push(format!("{:.1} / {:.1} GB", gb(u), gb(t))),
            (Some(u), None) => parts.push(format!("{:.1} GB used", gb(u))),
            _ => {}
        }
        if parts.is_empty() {
            "GPU  not available".into()
        } else {
            format!("GPU  {}", parts.join(", "))
        }
    }
}

/// Reads the GPU counters on a thread of its own, at most once a second,
/// so a missing tool or a slow sysfs read never holds up a frame.
#[derive(Default)]
struct GpuProbe {
    rx: Option<Receiver<Option<GpuStats>>>,
    /// None until the first answer arrives, then the last reading.
    last: Option<GpuStats>,
}

impl GpuProbe {
    /// Take whatever the thread has sent, starting it on first use.
    fn poll(&mut self) {
        if self.rx.is_none() {
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::Builder::new()
                .name("anvil-gpu-probe".into())
                .spawn(move || loop {
                    if tx.send(probe_once()).is_err() {
                        // The overlay is gone with the app; stop quietly.
                        return;
                    }
                    std::thread::sleep(REFRESH);
                })
                .ok();
            self.rx = Some(rx);
        }
        let Some(rx) = &self.rx else { return };
        loop {
            match rx.try_recv() {
                Ok(v) => self.last = Some(v.unwrap_or_default()),
                Err(TryRecvError::Empty) => return,
                Err(TryRecvError::Disconnected) => {
                    self.rx = None;
                    return;
                }
            }
        }
    }

    fn line(&self) -> String {
        match &self.last {
            Some(s) if !s.is_empty() => s.line(),
            _ => "GPU  not available".into(),
        }
    }
}

/// One reading, from nvidia-smi if it is installed, else from sysfs.
/// Returns None when nothing is readable.
fn probe_once() -> Option<GpuStats> {
    if let Some(s) = nvidia_smi() {
        return Some(s);
    }
    drm_sysfs()
}

fn nvidia_smi() -> Option<GpuStats> {
    // A missing nvidia-smi gives an Err here, and prints nothing.
    let out = std::process::Command::new("nvidia-smi")
        .args(["--query-gpu=utilization.gpu,memory.used,memory.total", "--format=csv,noheader,nounits"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    parse_nvidia_smi(&String::from_utf8_lossy(&out.stdout))
}

/// Parse the first card of `nvidia-smi --format=csv,noheader,nounits`
/// output: "utilization, memory used, memory total", the memory in MiB.
pub fn parse_nvidia_smi(text: &str) -> Option<GpuStats> {
    let line = text.lines().map(str::trim).find(|l| !l.is_empty())?;
    let f: Vec<&str> = line.split(',').map(str::trim).collect();
    let mib = |v: Option<&&str>| v.and_then(|s| s.parse::<u64>().ok()).map(|m| m * 1024 * 1024);
    let s = GpuStats {
        busy: f.first().and_then(|s| s.parse::<f32>().ok()),
        mem_used: mib(f.get(1)),
        mem_total: mib(f.get(2)),
    };
    if s.is_empty() {
        None
    } else {
        Some(s)
    }
}

/// AMD, and the Intel drivers that expose the same files, through
/// /sys/class/drm/card*/device. Anything missing stays blank.
fn drm_sysfs() -> Option<GpuStats> {
    let dir = std::fs::read_dir("/sys/class/drm").ok()?;
    let mut cards: Vec<std::path::PathBuf> = dir
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("card") && !n.contains('-')))
        .collect();
    cards.sort();
    for card in cards {
        let read = |name: &str| std::fs::read_to_string(card.join("device").join(name)).ok();
        let s = GpuStats {
            busy: read("gpu_busy_percent").as_deref().and_then(parse_sysfs_percent),
            mem_used: read("mem_info_vram_used").as_deref().and_then(parse_sysfs_u64),
            mem_total: read("mem_info_vram_total").as_deref().and_then(parse_sysfs_u64),
        };
        if !s.is_empty() {
            return Some(s);
        }
    }
    None
}

/// A sysfs file holding a single unsigned number, with a newline.
pub fn parse_sysfs_u64(text: &str) -> Option<u64> {
    text.trim().parse::<u64>().ok()
}

/// A sysfs file holding a percentage, clamped to a sane range.
pub fn parse_sysfs_percent(text: &str) -> Option<f32> {
    let v = text.trim().parse::<f32>().ok()?;
    if (0.0..=100.0).contains(&v) {
        Some(v)
    } else {
        None
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
        assert!(m.lines().last().unwrap().contains("fps"));
    }

    #[test]
    fn parses_nvidia_smi_output() {
        let s = parse_nvidia_smi("23, 1234, 8192\n").unwrap();
        assert_eq!(s.busy, Some(23.0));
        assert_eq!(s.mem_used, Some(1234 * 1024 * 1024));
        assert_eq!(s.mem_total, Some(8192 * 1024 * 1024));
        assert!(s.line().starts_with("GPU  "));
        assert!(s.line().contains("23.0% busy"));
    }

    #[test]
    fn parses_the_first_card_only() {
        let s = parse_nvidia_smi("5, 100, 4096\n80, 3000, 4096\n").unwrap();
        assert_eq!(s.busy, Some(5.0));
        assert_eq!(s.mem_used, Some(100 * 1024 * 1024));
    }

    #[test]
    fn rejects_nvidia_smi_noise() {
        assert!(parse_nvidia_smi("").is_none());
        assert!(parse_nvidia_smi("[N/A], [N/A], [N/A]").is_none());
        // A card that reports memory but no utilization still counts.
        let s = parse_nvidia_smi("[N/A], 512, 2048").unwrap();
        assert_eq!(s.busy, None);
        assert_eq!(s.mem_used, Some(512 * 1024 * 1024));
    }

    #[test]
    fn parses_sysfs_values() {
        assert_eq!(parse_sysfs_percent("45\n"), Some(45.0));
        assert_eq!(parse_sysfs_percent(" 0 "), Some(0.0));
        assert_eq!(parse_sysfs_percent("nonsense"), None);
        assert_eq!(parse_sysfs_percent("101"), None);
        assert_eq!(parse_sysfs_u64("8573157376\n"), Some(8573157376));
        assert_eq!(parse_sysfs_u64(""), None);
    }

    #[test]
    fn says_so_when_nothing_is_readable() {
        assert_eq!(GpuStats::default().line(), "GPU  not available");
        assert_eq!(GpuProbe::default().line(), "GPU  not available");
    }
}
