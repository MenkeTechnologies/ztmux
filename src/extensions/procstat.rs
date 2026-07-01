//! Shared per-process resource stats via a single `ps` call.
//!
//! Used by both the `watch` (live TUI) and `ps` (one-shot, pipeable) extensions
//! so the `ps` invocation and parsing live in exactly one place.

use std::collections::HashMap;
use std::process::Command;

/// Per-process resource stats parsed from `ps`.
#[derive(Clone, Default)]
pub(crate) struct ProcStat {
    pub(crate) cpu: f32,
    pub(crate) mem: f32,
    pub(crate) rss_kb: u64,
    pub(crate) state: String,
}

/// One `ps` call for all pids; parse into a pid→stat map. Empty on failure.
pub(crate) fn gather(pids: &[i64]) -> HashMap<i64, ProcStat> {
    if pids.is_empty() {
        return HashMap::new();
    }
    let list = pids
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let Ok(out) = Command::new("ps")
        .args(["-o", "pid=,pcpu=,pmem=,rss=,state=", "-p", &list])
        .output()
    else {
        return HashMap::new();
    };
    let text = String::from_utf8_lossy(&out.stdout);
    let mut map = HashMap::new();
    for line in text.lines() {
        if let Some((pid, stat)) = parse_ps_line(line) {
            map.insert(pid, stat);
        }
    }
    map
}

/// Parse one `pid pcpu pmem rss state` line.
pub(crate) fn parse_ps_line(line: &str) -> Option<(i64, ProcStat)> {
    let mut it = line.split_whitespace();
    let pid: i64 = it.next()?.parse().ok()?;
    let cpu: f32 = it.next()?.parse().unwrap_or(0.0);
    let mem: f32 = it.next()?.parse().unwrap_or(0.0);
    let rss_kb: u64 = it.next()?.parse().unwrap_or(0);
    let state = it.next().unwrap_or("").to_string();
    Some((
        pid,
        ProcStat {
            cpu,
            mem,
            rss_kb,
            state,
        },
    ))
}

/// Human-readable resident-set size (K/M/G).
pub(crate) fn fmt_rss(kb: u64) -> String {
    if kb >= 1_048_576 {
        format!("{:.1}G", kb as f64 / 1_048_576.0)
    } else if kb >= 1024 {
        format!("{:.1}M", kb as f64 / 1024.0)
    } else {
        format!("{kb}K")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_ps_line() {
        let (pid, s) = parse_ps_line("  1234  12.5  0.8  40960 R").unwrap();
        assert_eq!(pid, 1234);
        assert!((s.cpu - 12.5).abs() < 0.01);
        assert!((s.mem - 0.8).abs() < 0.01);
        assert_eq!(s.rss_kb, 40960);
        assert_eq!(s.state, "R");
    }

    #[test]
    fn rejects_a_non_numeric_line() {
        assert!(parse_ps_line("PID CPU MEM").is_none());
    }

    #[test]
    fn fmt_rss_scales() {
        assert_eq!(fmt_rss(512), "512K");
        assert_eq!(fmt_rss(2048), "2.0M");
    }

    // fmt_rss scale boundaries: <1024 stays K, exactly 1024 rolls to M, just
    // under 1G stays M, exactly 1<<20 rolls to G.
    #[test]
    fn fmt_rss_boundaries_and_gigabytes() {
        assert_eq!(fmt_rss(1023), "1023K");
        assert_eq!(fmt_rss(1024), "1.0M");
        assert_eq!(fmt_rss(1_048_575), "1024.0M");
        assert_eq!(fmt_rss(1_048_576), "1.0G");
        assert_eq!(fmt_rss(3_145_728), "3.0G");
    }

    // A line with no trailing state column parses with an empty state string
    // (state is the only optional field).
    #[test]
    fn parse_ps_line_missing_state_is_empty() {
        let (pid, s) = parse_ps_line("42 0.0 0.0 100").unwrap();
        assert_eq!(pid, 42);
        assert_eq!(s.rss_kb, 100);
        assert_eq!(s.state, "");
    }

    // Only pid must parse as a number; unparseable cpu/mem/rss fall back to 0
    // (parse().unwrap_or), while state is taken verbatim.
    #[test]
    fn parse_ps_line_bad_numbers_default_to_zero() {
        let (pid, s) = parse_ps_line("7 x y z S").unwrap();
        assert_eq!(pid, 7);
        assert_eq!(s.cpu, 0.0);
        assert_eq!(s.mem, 0.0);
        assert_eq!(s.rss_kb, 0);
        assert_eq!(s.state, "S");
    }

    // pid, cpu, mem and rss are all required (each is a `?`); a line short of
    // rss yields None.
    #[test]
    fn parse_ps_line_requires_pid_cpu_mem_rss() {
        assert!(parse_ps_line("").is_none());
        assert!(parse_ps_line("123").is_none());
        assert!(parse_ps_line("123 1.0 2.0").is_none());
    }

    // gather short-circuits on an empty pid list without spawning ps.
    #[test]
    fn gather_empty_pids_is_empty() {
        assert!(gather(&[]).is_empty());
    }
}
