// ztmux has no C libraries to find or link: the event loop lives in
// src/event_loop (Rust, replacing libevent) and terminfo is read by
// terminfo-lean. All this build script does is generate the command grammar,
// so a build needs nothing but a Rust toolchain — no pkg-config probe, no
// Homebrew prefix, no -dev packages.
fn main() {
    println!("cargo::rerun-if-changed=src/cmd_parse.lalrpop");
    lalrpop::process_root().unwrap();
}
