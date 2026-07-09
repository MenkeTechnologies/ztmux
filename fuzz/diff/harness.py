"""Differential-fuzz harness: drive ztmux and real tmux with identical input
and diff their observable output.

The two are byte-for-byte comparable through `capture-pane` (terminal state) and
the paste buffer (copy-mode selections), so any divergence is a parity bug in
ztmux's port. This module owns process/socket lifecycle, output capture, and
test-case minimisation; the generators and CLI live alongside it.
"""

import os
import shutil
import subprocess
import tempfile
import time

# A unix socket path must fit in sun_path (~104 bytes on macOS/Linux), which the
# repo's own scratch/tmp paths can blow past - so sockets always live in /tmp
# under a short, pid-scoped prefix.
_SOCK_PREFIX = f"/tmp/ztdf-{os.getpid()}"


def find_binaries():
    """Locate (tmux, ztmux). Honour $TMUX_BIN / $ZTMUX_BIN, else search PATH and
    the usual target/ build dirs. Returns absolute paths; raises if either is
    missing."""
    tmux = os.environ.get("TMUX_BIN") or shutil.which("tmux")
    ztmux = os.environ.get("ZTMUX_BIN")
    if not ztmux:
        here = os.path.dirname(os.path.abspath(__file__))
        root = os.path.abspath(os.path.join(here, "..", ".."))
        for cand in ("target/release/ztmux", "target/debug/ztmux"):
            p = os.path.join(root, cand)
            if os.path.exists(p):
                ztmux = p
                break
        ztmux = ztmux or shutil.which("ztmux")
    if not tmux:
        raise SystemExit("tmux not found (install it or set $TMUX_BIN)")
    if not ztmux:
        raise SystemExit(
            "ztmux not found: build it (cargo build --release) or set $ZTMUX_BIN"
        )
    return os.path.abspath(tmux), os.path.abspath(ztmux)


def version(binary):
    try:
        return subprocess.run(
            [binary, "-V"], capture_output=True, text=True, timeout=10
        ).stdout.strip()
    except Exception as e:  # noqa: BLE001
        return f"<unknown: {e}>"


class Server:
    """A short-lived tmux/ztmux server running one pane that `cat`s a byte file
    then idles. Use as a context manager so the server is always torn down."""

    def __init__(self, binary, tag, width, height, settle):
        self.binary = binary
        self.sock = f"{_SOCK_PREFIX}-{tag}.sock"
        self.width = width
        self.height = height
        self.settle = settle
        self._env = dict(os.environ, TERM="xterm-256color")

    def __enter__(self):
        self._rm_sock()
        return self

    def __exit__(self, *exc):
        subprocess.run(
            [self.binary, "-S", self.sock, "kill-server"],
            stdout=subprocess.DEVNULL,
            stderr=subprocess.DEVNULL,
        )
        self._rm_sock()
        return False

    def _rm_sock(self):
        try:
            os.unlink(self.sock)
        except OSError:
            pass

    def _cmd(self, *args, capture=True):
        kw = {"capture_output": True} if capture else {
            "stdout": subprocess.DEVNULL, "stderr": subprocess.DEVNULL
        }
        return subprocess.run([self.binary, "-S", self.sock, *args], env=self._env, **kw)

    def start_with_data(self, data: bytes, binfile: str):
        """Start a session whose pane cats `data` (already written to `binfile`)
        then sleeps, and wait until the grid has stopped changing.

        Polling for a stable capture (rather than a fixed sleep) is essential:
        a fresh detached server renders asynchronously, so a fixed delay races
        the render and produces flaky empty captures - which would show up as
        phantom divergences (one side rendered, the other not)."""
        with open(binfile, "wb") as f:
            f.write(data)
        self._cmd(
            "new-session", "-d",
            "-x", str(self.width), "-y", str(self.height),
            f"cat {binfile}; sleep 30",
            capture=False,
        )
        # Poll until two consecutive captures match (grid settled), bounded by
        # `settle` seconds total.
        step = 0.05
        prev = None
        stable = 0
        for _ in range(max(1, int(self.settle / step))):
            time.sleep(step)
            cur = self.capture()
            if cur == prev:
                stable += 1
                if stable >= 2:
                    return
            else:
                stable = 0
            prev = cur

    def capture(self) -> bytes:
        return self._cmd("capture-pane", "-p").stdout

    def drive(self, argv: list) -> bytes:
        """Run a chain of commands (argv is a full `-X ...` / other command list
        with literal ';' separators) and return stdout (e.g. of save-buffer -)."""
        return self._cmd(*argv).stdout


def run_input(binary, data, binfile, width, height, settle):
    """Feed raw bytes to a pane, return capture-pane output."""
    with Server(binary, "in", width, height, settle) as s:
        s.start_with_data(data, binfile)
        return s.capture()


def run_copy(binary, ops, content, binfile, width, height, settle):
    """Drive copy-mode over `content` with a list of `send -X` op names, then
    copy the selection and return the paste buffer bytes."""
    with Server(binary, "cp", width, height, settle) as s:
        s.start_with_data(content, binfile)
        argv = [
            "copy-mode", ";",
            "send-keys", "-X", "history-top", ";",
            "send-keys", "-X", "start-of-line", ";",
        ]
        for op in ops:
            argv += ["send-keys", "-X", op, ";"]
        argv += ["send-keys", "-X", "copy-selection", ";", "save-buffer", "-"]
        return s.drive(argv)


def minimize(diverges, case):
    """Greedily drop elements from a failing case (a bytes object or a list)
    while it still diverges. Works for both input bytes and op lists."""
    is_bytes = isinstance(case, (bytes, bytearray))
    changed = True
    while changed:
        changed = False
        # Try progressively smaller chunk removals for byte inputs; single
        # elements for op lists.
        sizes = (16, 8, 4, 2, 1) if is_bytes else (4, 2, 1)
        for size in sizes:
            i = 0
            while i < len(case):
                cand = case[:i] + case[i + size:]
                if len(cand) and diverges(cand):
                    case = cand
                    changed = True
                else:
                    i += size
    return case
