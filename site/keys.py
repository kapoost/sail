"""The key list, read out of the program that answers to it.

`crates/sail-app/src/help.rs` is the single place where what the keys do is
written down, and a test in that file checks it against every module that reads
the keyboard. Printing the same list again by hand — in a README, on a web page,
in a PDF — is how three of them came to be wrong once already. So the page and
the manual are generated from the source rather than written beside it, and this
module is the seam.

It parses Rust string literals rather than importing anything, because `sail-app`
is a binary crate and an example cannot see inside one. The parse is narrow on
purpose: it reads string literals in order out of `const KEYS`, and it checks its
own work against `DOCUMENTED` — the array the test already keeps honest. If the
shape of that constant ever changes, this raises rather than quietly emitting
half a list.
"""

from __future__ import annotations

import pathlib
import re

HELP = pathlib.Path(__file__).resolve().parent.parent / "crates/sail-app/src/help.rs"


def _literals(text: str) -> list[tuple[int, str]]:
    """Every Rust string literal in `text`, with the index it started at.

    Handles the one escape that matters here: a backslash at the end of a line,
    which swallows the newline and the indentation after it, and is how the long
    lines in that file are wrapped.
    """
    out: list[tuple[int, str]] = []
    i = 0
    while i < len(text):
        if text[i] != '"':
            i += 1
            continue
        start = i
        i += 1
        buf: list[str] = []
        while i < len(text) and text[i] != '"':
            if text[i] == "\\":
                nxt = text[i + 1]
                if nxt == "\n":
                    i += 2
                    while i < len(text) and text[i] in " \t":
                        i += 1
                    continue
                buf.append({"n": "\n", "t": "\t", "\\": "\\", '"': '"'}.get(nxt, nxt))
                i += 2
                continue
            buf.append(text[i])
            i += 1
        i += 1
        out.append((start, "".join(buf)))
    return out


def sections() -> list[tuple[str, list[tuple[str, str]]]]:
    """The help as it is grouped on the screen: headings, then key and meaning."""
    source = HELP.read_text(encoding="utf-8")
    try:
        body = source.split("const KEYS:", 1)[1].split("= [", 1)[1].split("\n];", 1)[0]
    except IndexError as exc:  # the constant was renamed or restructured
        raise SystemExit(f"{HELP}: cannot find `const KEYS` — the parse is stale") from exc

    groups: list[tuple[str, list[tuple[str, str]]]] = []
    # Each section is a heading, then `&[`, then pairs until the matching `]`.
    for chunk in body.split("&[")[1:]:
        head_end = chunk.rfind("\n        ],")
        rows_text = chunk if head_end < 0 else chunk[:head_end]
        pairs = [v for _, v in _literals(rows_text)]
        if len(pairs) % 2:
            raise SystemExit(f"{HELP}: a key without a meaning in section {len(groups)+1}")
        groups.append(("", list(zip(pairs[0::2], pairs[1::2]))))

    # The headings are the literals that sit before each `&[`.
    heads = re.findall(r'\(\s*\n?\s*"([^"]+)",\s*\n?\s*&\[', body)
    if len(heads) != len(groups):
        raise SystemExit(f"{HELP}: {len(heads)} headings for {len(groups)} sections")
    return [(h, rows) for h, (_, rows) in zip(heads, groups)]


def documented() -> list[str]:
    """`DOCUMENTED`, which the crate's own test keeps equal to what the game reads."""
    source = HELP.read_text(encoding="utf-8")
    body = source.split("pub const DOCUMENTED", 1)[1].split("= [", 1)[1].split("\n];", 1)[0]
    return re.findall(r"KeyCode::(\w+)", body)


def check() -> None:
    """Shout if the parse has drifted from the file it reads.

    Not a substitute for the test in `help.rs` — that one asks the real question,
    which is whether the list matches the program. This one only asks whether
    *this* parse still understands the file, which is the failure a generated
    page would otherwise ship silently.
    """
    groups = sections()
    rows = sum(len(r) for _, r in groups)
    keys = documented()
    if not groups or rows < 30 or len(keys) < 30:
        raise SystemExit(f"keys.py: parsed {len(groups)} sections, {rows} rows, {len(keys)} keys — too few to be right")
    for heading, items in groups:
        if not heading.isupper():
            raise SystemExit(f"keys.py: {heading!r} does not look like a heading")
        if not items:
            raise SystemExit(f"keys.py: section {heading!r} came out empty")


if __name__ == "__main__":
    check()
    for heading, rows in sections():
        print(f"\n{heading}")
        for key, what in rows:
            print(f"  {key:<10} {what}")
    print(f"\n{len(documented())} keys in DOCUMENTED")
