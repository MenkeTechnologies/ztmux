//! Original ztmux extensions — features with no upstream tmux counterpart.
//!
//! Code under `src/extensions/` is deliberately NOT a port of tmux and is
//! therefore exempt from the anti-drift gate (see
//! `tests/ported_fn_names_match_c.rs`), which only holds `src/ported`-style
//! ported code to C-name fidelity.
pub(crate) mod dashboard;
pub(crate) mod doctor;
pub(crate) mod events;
pub(crate) mod find;
pub(crate) mod graph;
pub(crate) mod layout;
pub(crate) mod procstat;
pub(crate) mod prune;
pub(crate) mod ps;
pub(crate) mod recent;
pub(crate) mod snapshot;
pub(crate) mod stats;
pub(crate) mod structured;
pub(crate) mod switch;
pub(crate) mod tmux_query;
pub(crate) mod tree;
pub(crate) mod usage;
pub(crate) mod watch;
