//! Shared OS process table for the `pstree` and `ports` extensions.
//!
//! One `ps -A -o pid=,ppid=,comm=` call yields every process's pid, parent pid,
//! and command; the parsing lives here so both extensions agree on the process
//! graph. `comm` is reduced to its basename so macOS (full path) and Linux
//! (short name) render identically.

use std::collections::HashMap;
use std::process::Command;

/// One row of the process table.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Proc {
    pub(crate) pid: i64,
    pub(crate) ppid: i64,
    pub(crate) comm: String,
}

/// The whole process table via a single `ps` call. Empty on failure.
pub(crate) fn table() -> Vec<Proc> {
    let Ok(out) = Command::new("ps")
        .args(["-A", "-o", "pid=,ppid=,comm="])
        .output()
    else {
        return Vec::new();
    };
    parse_table(&String::from_utf8_lossy(&out.stdout))
}

/// Parse `pid ppid comm` lines. `comm` (the rest of the line) is basename'd and
/// may contain spaces, so only the first two fields are split off as numbers.
pub(crate) fn parse_table(text: &str) -> Vec<Proc> {
    text.lines()
        .filter_map(|line| {
            let mut it = line.split_whitespace();
            let pid: i64 = it.next()?.parse().ok()?;
            let ppid: i64 = it.next()?.parse().ok()?;
            let comm = it.next()?; // executable path/name (no embedded spaces from comm)
            Some(Proc {
                pid,
                ppid,
                comm: basename(comm).to_string(),
            })
        })
        .collect()
}

/// Last path component of a command (`/usr/bin/zsh` → `zsh`).
pub(crate) fn basename(comm: &str) -> &str {
    comm.rsplit('/').next().unwrap_or(comm)
}

/// Map each pid to the pids of its direct children, in ascending pid order.
pub(crate) fn children_map(procs: &[Proc]) -> HashMap<i64, Vec<i64>> {
    let mut map: HashMap<i64, Vec<i64>> = HashMap::new();
    let mut sorted: Vec<&Proc> = procs.iter().collect();
    sorted.sort_by_key(|p| p.pid);
    for p in sorted {
        if p.ppid != p.pid {
            map.entry(p.ppid).or_default().push(p.pid);
        }
    }
    map
}

/// Walk from `pid` up through its ancestors (not including `pid` itself),
/// returning the first ancestor found in `roots`, or `None`. Cycle-guarded.
pub(crate) fn ancestor_in(pid: i64, parent: &HashMap<i64, i64>, roots: &[i64]) -> Option<i64> {
    let mut cur = pid;
    let mut seen = 0;
    while let Some(&pp) = parent.get(&cur) {
        if pp == cur || seen > 4096 {
            break;
        }
        if roots.contains(&pp) {
            return Some(pp);
        }
        cur = pp;
        seen += 1;
    }
    None
}

/// pid → ppid map for ancestor walks.
pub(crate) fn parent_map(procs: &[Proc]) -> HashMap<i64, i64> {
    procs.iter().map(|p| (p.pid, p.ppid)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_pid_ppid_and_basenames_comm() {
        let t = parse_table("    1     0 /sbin/launchd\n  341     1 /usr/libexec/logd\n");
        assert_eq!(t.len(), 2);
        assert_eq!(
            t[0],
            Proc {
                pid: 1,
                ppid: 0,
                comm: "launchd".into()
            }
        );
        assert_eq!(t[1].comm, "logd");
    }

    #[test]
    fn skips_unparseable_lines() {
        let t = parse_table("PID PPID COMM\n100 50 bash\n\ngarbage\n");
        assert_eq!(t.len(), 1);
        assert_eq!(t[0].pid, 100);
    }

    #[test]
    fn basename_handles_plain_and_pathed_names() {
        assert_eq!(basename("zsh"), "zsh");
        assert_eq!(basename("/usr/bin/zsh"), "zsh");
        assert_eq!(basename("/a/b/c/node"), "node");
    }

    #[test]
    fn children_map_groups_and_sorts_by_pid() {
        let procs = vec![
            Proc {
                pid: 10,
                ppid: 1,
                comm: "zsh".into(),
            },
            Proc {
                pid: 30,
                ppid: 10,
                comm: "node".into(),
            },
            Proc {
                pid: 20,
                ppid: 10,
                comm: "vim".into(),
            },
        ];
        let m = children_map(&procs);
        assert_eq!(m.get(&10), Some(&vec![20, 30]));
        assert_eq!(m.get(&1), Some(&vec![10]));
    }

    #[test]
    fn ancestor_in_finds_nearest_root() {
        // 40 → 30 → 10 → 1. Roots {10} → 10; roots {1} → 1 (skips 30,10 if not root).
        let procs = vec![
            Proc {
                pid: 1,
                ppid: 0,
                comm: "init".into(),
            },
            Proc {
                pid: 10,
                ppid: 1,
                comm: "zsh".into(),
            },
            Proc {
                pid: 30,
                ppid: 10,
                comm: "npm".into(),
            },
            Proc {
                pid: 40,
                ppid: 30,
                comm: "node".into(),
            },
        ];
        let pm = parent_map(&procs);
        assert_eq!(ancestor_in(40, &pm, &[10]), Some(10));
        assert_eq!(ancestor_in(40, &pm, &[1]), Some(1));
        assert_eq!(ancestor_in(40, &pm, &[99]), None);
    }

    #[test]
    fn ancestor_walk_is_cycle_safe() {
        // A self-parent (pid == ppid) must not loop forever.
        let procs = vec![Proc {
            pid: 5,
            ppid: 5,
            comm: "weird".into(),
        }];
        let pm = parent_map(&procs);
        assert_eq!(ancestor_in(5, &pm, &[1]), None);
    }
}
