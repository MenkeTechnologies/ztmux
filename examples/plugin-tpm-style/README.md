# plugin-tpm-style

A **script plugin** — the kind every published tmux plugin already is, installed
by `znative` unmodified:

```tmux
znative load path:examples/plugin-tpm-style
```

There is no `ztnative.toml` and no Rust. A repository whose root holds a
`*.tmux` file *is* a script plugin, so the detection, the store copy, and the
run are the same ones a TPM plugin from GitHub gets:

```tmux
znative load tmux-plugins/tmux-sensible
```

`scratch.tmux` binds `prefix + C-s` to a throwaway popup shell, configured the
way tmux plugins are configured — with `@`-prefixed user options:

```tmux
set -g @scratch-key     'C-s'
set -g @scratch-height  '40%'
set -g @scratch-command 'htop'
```

## What znative does differently from TPM

- The file is copied into the content-addressed store
  (`$ZTMUX_HOME/pkg/store/<name>@<version>/`) and SHA-256 pinned, rather than
  left in a clone directory.
- It is made executable on the way in, so a repo that shipped a `*.tmux`
  without the bit still loads.
- It runs with a `tmux` shim first on `PATH` that execs the ztmux server that
  is loading it. A plugin's bare `tmux …` calls therefore reach *this* server,
  including on a `-S`/`-L` socket, without the plugin knowing anything about it.

For the compiled kind — real commands, `#{…}` formats and hooks registered
through the [`ztnative`](../../ztnative/) ABI — see the sibling examples
(`plugin-hello`, `plugin-battery`, `plugin-sessionizer`, `plugin-hooklog`).
