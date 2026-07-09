#!/usr/bin/env python3
"""Differential fuzzer: compare ztmux against real tmux on identical input.

    ./diff_fuzz.py input [--count N] [--seed S]   # fuzz the terminal parser
    ./diff_fuzz.py copy  [--count N] [--seed S]   # fuzz copy-mode sequences
    ./diff_fuzz.py both  [--count N] [--seed S]
    ./diff_fuzz.py repro <repro.json>             # replay a saved divergence

A divergence prints the minimised case and is saved under repros/ so it can be
replayed and turned into a regression test. Exit status is nonzero if any
divergence is found (usable in CI).
"""

import argparse
import json
import os
import sys
import tempfile

import harness
import generators as gen

REPRO_DIR = os.path.join(os.path.dirname(os.path.abspath(__file__)), "repros")
_BINFILE = os.path.join(tempfile.gettempdir(), f"ztdf-{os.getpid()}.bin")


class Mode:
    """Bundles the generator, the two-binary compare, and (de)serialisation for
    one fuzzing domain."""

    def __init__(self, name, tmux, ztmux, args):
        self.name = name
        self.tmux = tmux
        self.ztmux = ztmux
        self.w, self.h, self.settle = args.width, args.height, args.settle

    def generate(self, r):
        return gen.gen_input(r) if self.name == "input" else gen.gen_copy(r)

    def _run(self, binary, case):
        if self.name == "input":
            return harness.run_input(binary, case, _BINFILE, self.w, self.h, self.settle)
        return harness.run_copy(binary, case, gen.COPY_CONTENT, _BINFILE, self.w, self.h, self.settle)

    def diverges(self, case):
        return self._run(self.tmux, case) != self._run(self.ztmux, case)

    def both(self, case):
        return self._run(self.tmux, case), self._run(self.ztmux, case)

    @staticmethod
    def encode(case):
        return case.hex() if isinstance(case, (bytes, bytearray)) else list(case)

    def decode(self, blob):
        return bytes.fromhex(blob) if self.name == "input" else list(blob)


def _save_repro(mode, case):
    os.makedirs(REPRO_DIR, exist_ok=True)
    n = 1
    while os.path.exists(p := os.path.join(REPRO_DIR, f"{mode.name}-{n:03d}.json")):
        n += 1
    tmux_out, ztmux_out = mode.both(case)
    with open(p, "w") as f:
        json.dump(
            {
                "mode": mode.name,
                "case": mode.encode(case),
                "tmux": tmux_out.hex(),
                "ztmux": ztmux_out.hex(),
            },
            f,
            indent=2,
        )
    return p


def _report(mode, case):
    t, z = mode.both(case)
    print(f"  case : {mode.encode(case)!r}")
    print(f"  tmux : {t!r}")
    print(f"  ztmux: {z!r}")


def run_mode(mode, count, seed0):
    fails = 0
    for i in range(count):
        import random  # local: keep module import cheap
        case = mode.generate(random.Random(seed0 + i))
        if mode.diverges(case):
            fails += 1
            print(f"[{mode.name}] DIVERGENCE at seed {seed0 + i}")
            small = harness.minimize(mode.diverges, case)
            _report(mode, small)
            path = _save_repro(mode, small)
            print(f"  saved: {os.path.relpath(path)}")
        elif (i + 1) % 25 == 0:
            print(f"  ...{i + 1}/{count} ({mode.name})", file=sys.stderr)
    print(f"[{mode.name}] {fails}/{count} diverged")
    return fails


def cmd_repro(path):
    with open(path) as f:
        data = json.load(f)
    tmux, ztmux = harness.find_binaries()
    args = argparse.Namespace(width=40, height=12, settle=1.5)
    mode = Mode(data["mode"], tmux, ztmux, args)
    case = mode.decode(data["case"])
    t, z = mode.both(case)
    print(f"replaying {data['mode']} case: {mode.encode(case)!r}")
    print(f"  tmux : {t!r}")
    print(f"  ztmux: {z!r}")
    if t == z:
        print("  => matches now (fixed?)")
        return 0
    print("  => still diverges")
    return 1


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    sub = ap.add_subparsers(dest="cmd", required=True)
    for m in ("input", "copy", "both"):
        p = sub.add_parser(m)
        p.add_argument("--count", type=int, default=100)
        p.add_argument("--seed", type=int, default=1)
        p.add_argument("--width", type=int, default=40)
        p.add_argument("--height", type=int, default=12 if m != "input" else 10)
        p.add_argument("--settle", type=float, default=1.5,
                       help="max seconds to poll for the grid to settle (returns early once stable)")
    rp = sub.add_parser("repro")
    rp.add_argument("file")
    a = ap.parse_args()

    if a.cmd == "repro":
        return cmd_repro(a.file)

    tmux, ztmux = harness.find_binaries()
    print(f"tmux : {harness.version(tmux)}  ({tmux})")
    print(f"ztmux: {harness.version(ztmux)}  ({ztmux})")
    names = ["input", "copy"] if a.cmd == "both" else [a.cmd]
    total = 0
    try:
        for name in names:
            total += run_mode(Mode(name, tmux, ztmux, a), a.count, a.seed)
    finally:
        try:
            os.unlink(_BINFILE)
        except OSError:
            pass
    return 1 if total else 0


if __name__ == "__main__":
    sys.exit(main())
