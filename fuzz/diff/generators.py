"""Input and copy-mode test-case generators for the differential fuzzer.

Each generator is seeded (`random.Random`) so a divergence is reproducible from
its seed alone.
"""

import random

# --- input (byte-stream) fuzzing -----------------------------------------

# Printable bytes weighted toward things terminals treat specially.
_PRINT = (
    bytes(range(0x41, 0x5B))
    + bytes(range(0x61, 0x7B))
    + b"0123456789().,%:-[]{}"
)

_UTF8 = ["é", "€", "→", "⇥", "字", "🚀", "̈"]  # incl. wide + combining


def _token(r):
    k = r.randrange(100)
    if k < 33:  # printable run
        return bytes(r.choice(_PRINT) for _ in range(r.randint(1, 8)))
    if k < 52:  # lone control char, biased to tab/cr/lf/bs
        return bytes([
            r.choice([0x09, 0x0D, 0x0A, 0x08, 0x00, 0x07, 0x0B, 0x0C,
                      r.randrange(0x00, 0x20)])
        ])
    if k < 68:  # CSI (SGR, cursor ops, edit ops)
        params = ";".join(str(r.randrange(0, 60)) for _ in range(r.randint(0, 3)))
        final = r.choice("mHJKABCDGdfhlrST@PXL")
        priv = b"?" if r.randrange(4) == 0 else b""
        return b"\x1b[" + priv + params.encode() + final.encode()
    if k < 78:  # OSC
        return (b"\x1b]" + str(r.randrange(0, 12)).encode() + b";x"
                + r.choice([b"\x07", b"\x1b\\"]))
    if k < 86:  # ESC + single (RIS, index, charset, etc.)
        return b"\x1b" + bytes([r.choice(b"c7 8=>DEHMcn(0)0")])
    if k < 93:  # UTF-8 (wide / combining / multibyte)
        return r.choice(_UTF8).encode()
    # DCS / APC string
    return b"\x1bP" + bytes(r.choice(_PRINT) for _ in range(r.randint(0, 4))) + b"\x1b\\"


def gen_input(r):
    """A random byte stream of interleaved tokens."""
    return b"".join(_token(r) for _ in range(r.randint(3, 25)))


# --- copy-mode command-sequence fuzzing ----------------------------------

# Rich fixed content: word/WORD/whitespace/tab/punctuation boundaries, an empty
# line, trailing spaces, indentation, and a line long enough to wrap.
COPY_CONTENT = (
    b"The quick   brown_fox\tjumps.\n"
    b"foo(bar).baz[42] = qux;\n"
    b"\tindented\ttabbed  words\n"
    b"a b  c   d\n"
    b"line-with-hyphens and.dots,commas\n"
    b"\n"
    b"trailing spaces here    \n"
    b"UPPER lower MixedCase 123abc\n"
    b"veryveryveryveryveryveryveryverylongwordthatwrapsacrossthelineboundary end\n"
)

# No-argument copy-mode commands: cursor movement and selection state. (Commands
# that open the command prompt - search-*, jump-* with an argument - are omitted
# because they can't be driven headlessly in one command chain.)
COPY_OPS = [
    "cursor-left", "cursor-right", "cursor-up", "cursor-down",
    "start-of-line", "end-of-line", "back-to-indentation",
    "next-word", "previous-word", "next-word-end",
    "next-space", "next-space-end", "previous-space",
    "top-line", "bottom-line", "middle-line", "history-top", "history-bottom",
    "begin-selection", "other-end", "rectangle-toggle",
    "select-word", "select-line", "clear-selection",
]


def gen_copy(r):
    """A random sequence of copy-mode ops."""
    return [r.choice(COPY_OPS) for _ in range(r.randint(4, 16))]
