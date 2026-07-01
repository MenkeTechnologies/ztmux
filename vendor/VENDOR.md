# Vendored references

This tree is a **read-only reference**, committed as a plain copy (not a submodule) so
`ztmux` is self-contained and its clone never depends on an upstream repo staying alive.
Do not develop in it — the living port is the crate at the repo root (`../src`).

| Path | Upstream | Pinned at | License | Role |
| --- | --- | --- | --- | --- |
| `tmux/` | https://github.com/tmux/tmux (`master`) | `3d58e04c93c17af60c6852531aeb6d85a5975d09` | ISC | **Source of truth.** The tmux C sources we are porting. When a Rust module is incomplete or wrong, this is the reference to diff against. |

Vendored on 2026-07-01.

## Refreshing a reference

To re-pin a reference to a newer upstream commit, replace the tree with a fresh
`git archive` export of the desired commit (no `.git`) and update the SHA in the table
above:

```sh
# from a scratch dir
git clone --depth 1 https://github.com/tmux/tmux
git -C tmux rev-parse HEAD           # <- record this SHA
rm -rf tmux/.git
# then rsync tmux/ over vendor/tmux/ and commit
```
