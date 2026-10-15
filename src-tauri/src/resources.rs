//! Live memory/CPU usage of the whole app: this process plus its children (the WebKit web and
//! network processes that render the UI), read from /proc.

use serde::Serialize;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Instant;

/// Linux reports CPU time in clock ticks; USER_HZ is 100 on all mainstream architectures.
const CLOCK_TICKS_PER_SEC: f64 = 100.0;

#[derive(Debug, Clone, Serialize)]
pub struct ResourceUsage {
    /// Proportional set size of the whole process tree (shared pages split fairly), in MB.
    pub memory_mb: f64,
    /// PSS of the search core alone (this process), in MB.
    pub core_memory_mb: f64,
    /// CPU used by the process tree since the previous sample, as % of one core.
    pub cpu_percent: f64,
    pub cpu_cores: usize,
    pub processes: usize,
    pub model_loaded: bool,
}

#[derive(Default)]
pub struct CpuSampler {
    last: Mutex<Option<(Instant, u64)>>,
}

impl CpuSampler {
    pub fn sample(&self, model_loaded: bool) -> ResourceUsage {
        let root = std::process::id();
        let pids = process_tree(root);
        let core_memory_kb = pss_kb(root).unwrap_or(0);
        let memory_kb: u64 = pids.iter().filter_map(|p| pss_kb(*p)).sum();
        let ticks: u64 = pids.iter().filter_map(|p| cpu_ticks(*p)).sum();

        let now = Instant::now();
        let mut last = self.last.lock().unwrap();
        let cpu_percent = match *last {
            Some((t, prev)) if now > t => {
                let secs = now.duration_since(t).as_secs_f64();
                (ticks.saturating_sub(prev) as f64 / CLOCK_TICKS_PER_SEC) / secs * 100.0
            }
            _ => 0.0,
        };
        *last = Some((now, ticks));

        ResourceUsage {
            memory_mb: memory_kb as f64 / 1024.0,
            core_memory_mb: core_memory_kb as f64 / 1024.0,
            cpu_percent,
            cpu_cores: std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(1),
            processes: pids.len(),
            model_loaded,
        }
    }
}

/// `root` and all of its descendants.
fn process_tree(root: u32) -> Vec<u32> {
    let mut children: HashMap<u32, Vec<u32>> = HashMap::new();
    if let Ok(entries) = std::fs::read_dir("/proc") {
        for entry in entries.flatten() {
            let Some(pid) = entry.file_name().to_str().and_then(|s| s.parse::<u32>().ok()) else {
                continue;
            };
            if let Some(ppid) = stat_fields(pid).and_then(|f| f.get(1).and_then(|v| v.parse().ok())) {
                children.entry(ppid).or_default().push(pid);
            }
        }
    }
    let mut tree = vec![root];
    let mut i = 0;
    while i < tree.len() {
        if let Some(kids) = children.get(&tree[i]) {
            tree.extend(kids);
        }
        i += 1;
    }
    tree
}

/// Fields of /proc/<pid>/stat after the `(comm)` field, which may itself contain spaces.
/// Index 0 is the state, 1 the parent pid, 11/12 utime/stime.
fn stat_fields(pid: u32) -> Option<Vec<String>> {
    let stat = std::fs::read_to_string(format!("/proc/{}/stat", pid)).ok()?;
    let rest = &stat[stat.rfind(')')? + 1..];
    Some(rest.split_whitespace().map(str::to_string).collect())
}

fn cpu_ticks(pid: u32) -> Option<u64> {
    let f = stat_fields(pid)?;
    Some(f.get(11)?.parse::<u64>().ok()? + f.get(12)?.parse::<u64>().ok()?)
}

fn pss_kb(pid: u32) -> Option<u64> {
    let rollup = std::fs::read_to_string(format!("/proc/{}/smaps_rollup", pid)).ok()?;
    rollup
        .lines()
        .find(|l| l.starts_with("Pss:"))?
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()
}
