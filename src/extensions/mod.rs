//! Original ztmux extensions — features with no upstream tmux counterpart.
//!
//! Code under `src/extensions/` is deliberately NOT a port of tmux and is
//! therefore exempt from the anti-drift gate (see
//! `tests/ported_fn_names_match_c.rs`), which only holds `src/ported`-style
//! ported code to C-name fidelity.
pub(crate) mod active;
pub(crate) mod alerts;
pub(crate) mod bcast;
pub(crate) mod dashboard;
pub(crate) mod dedup;
pub(crate) mod disk;
pub(crate) mod doctor;
pub(crate) mod env;
pub(crate) mod events;
pub(crate) mod find;
pub(crate) mod git;
pub(crate) mod graph;
pub(crate) mod grep;
pub(crate) mod groups;
pub(crate) mod help;
pub(crate) mod history;
pub(crate) mod info;
pub(crate) mod layout;
pub(crate) mod marks;
pub(crate) mod mode;
pub(crate) mod net;
pub(crate) mod peek;
pub(crate) mod ports;
pub(crate) mod procstat;
pub(crate) mod proctree;
pub(crate) mod prune;
pub(crate) mod ps;
pub(crate) mod pstree;
pub(crate) mod recent;
pub(crate) mod size;
pub(crate) mod snapshot;
pub(crate) mod ssh;
pub(crate) mod stats;
pub(crate) mod structured;
pub(crate) mod switch;
pub(crate) mod titles;
pub(crate) mod tmux_query;
pub(crate) mod tree;
pub(crate) mod tty;
pub(crate) mod usage;
pub(crate) mod watch;
pub(crate) mod zoom;
