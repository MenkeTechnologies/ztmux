# Fuzzing ztmux

Two complementary approaches:

- **`cargo fuzz`** (this directory) — in-process libFuzzer targets that fuzz
  individual Rust functions for panics / assertion failures.
- **Differential fuzzing** ([`diff/`](diff/README.md)) — drives ztmux *and* real
  tmux with identical input and diffs their output, catching parity bugs in the
  whole terminal (parser, copy-mode, …). Requires tmux installed.

## cargo fuzz

Commands should be run from the root of the ztmux repo.

List available fuzz targets:

    cargo fuzz list

Run a specific target:

    cargo fuzz run colour_find_rgb

Run with more cores:

    cargo fuzz run colour_find_rgb -- -jobs=8

Run for a specific duration:

    cargo fuzz run colour_find_rgb -- -max_total_time=60

