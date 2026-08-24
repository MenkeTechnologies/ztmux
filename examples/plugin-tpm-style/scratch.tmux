#!/usr/bin/env bash
# plugin-tpm-style — a script plugin in the shape every published tmux plugin
# already ships in, installed by `znative` with no changes of any kind.
#
#   znative load path:examples/plugin-tpm-style
#
# There is no manifest: a repository with a `*.tmux` file at its root IS a
# script plugin, so this is what TPM would run and it is what znative runs.
# The file is executed once, at load, with `tmux` on PATH pointing at the ztmux
# server that is loading it — which is the only thing znative adds, and the
# reason a plugin works unchanged on a `-S`/`-L` server.
set -euo pipefail

# The plugin's own directory, so a binding can reach files that ship with it.
# In the store that is $ZTMUX_HOME/pkg/store/<name>@<version>/.
here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# `@`-prefixed user options are how a tmux plugin is configured. Read one with
# a default, the idiom every plugin repeats:
opt() {
    local value
    value="$(tmux show-options -gqv "$1")"
    printf '%s' "${value:-$2}"
}

key="$(opt '@scratch-key' 'C-s')"
height="$(opt '@scratch-height' '40%')"
command="$(opt '@scratch-command' "${SHELL:-/bin/sh}")"

# One binding: prefix + key opens a throwaway popup shell over the current
# pane. `display-popup -E` closes it when the command exits.
tmux bind-key "$key" display-popup -E -h "$height" -w '60%' "$command"

# Plugins commonly leave a marker so a config can tell whether they loaded.
tmux set-option -g @scratch-loaded "$here"
