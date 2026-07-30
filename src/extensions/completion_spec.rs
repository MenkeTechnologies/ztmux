//! The completion tables harvested at build time from `completions/_ztmux`.
//!
//! `build.rs` reads the shipped zsh completion and emits `VERB_DESCRIPTIONS`
//! (a one-line description per verb) and `EXTENSION_SPEC` (each extension
//! verb's options, their fixed value sets, and its positional vocabulary).
//! Keeping them in one module lets both consumers — [`super::verbs`] for the
//! listing and [`super::repl`] for Tab completion — read the same tables, so
//! what `ztmux <Tab>` offers in the shell is what the console offers too.
//! See the `completions` module in build.rs for the harvesting rules.

include!(concat!(env!("OUT_DIR"), "/completion_spec.rs"));
