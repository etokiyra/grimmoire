#!/usr/bin/env python3
"""Paste the generated maps into `src/game/areas.rs`.

The maps are **generated, not hand-written**: `tools/generate-areas.py` draws
them and enforces the design rules (passage width, reachability, doorway width),
and this script writes the result into the area file so the repository always
holds the layout the generator produces.

    tools/generate-areas.py          # print the maps, with their coordinates
    tools/apply-areas.py             # the same, written into src/game/areas.rs

Everything *around* the grids - the cast, the props, the doorways, the spawns and
the landmarks - is authored by hand in `areas.rs`. Only the `&[...]` grid is
replaced, and the generator prints the coordinates to paste into the rest.
"""

import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
GENERATOR = ROOT / "tools" / "generate-areas.py"
TARGET = ROOT / "src" / "game" / "areas.rs"

#: The area constructors to fill, in the order the generator prints them.
AREAS = ["chapel", "cellar", "woods", "village", "shrine", "catacombs"]

# A map header, then every line up to the next comment. Map rows never begin with
# `//`, so that is what separates one block from the next.
HEADER = re.compile(r"^// (\w+): (\d+) x (\d+)\n((?:(?!//)[^\n]*\n)+)", re.M)


def generated():
    """The maps the generator draws, as `{name: rows}`."""
    result = subprocess.run(
        [sys.executable, str(GENERATOR)],
        capture_output=True,
        text=True,
        check=True,
    )
    blocks = {}
    for match in HEADER.finditer(result.stdout):
        name, width, height = match.group(1), int(match.group(2)), int(match.group(3))
        rows = match.group(4).split("\n")[:height]
        assert all(len(row) == width for row in rows), f"{name} is ragged"
        blocks[name] = rows
    missing = [name for name in AREAS if name not in blocks]
    if missing:
        raise SystemExit(f"the generator did not print: {missing}")
    return blocks


def replace(text, name, rows):
    """Replaces one constructor's grid, and only its grid.

    Finds the first `&[...]` after the constructor and swaps its contents, so the
    tool is idempotent: it works whether the grid is a placeholder or a map it
    wrote itself last time.
    """
    start = text.index(f"pub fn {name}()")
    at = text.index("&[", start)
    end = text.index("\n        ],", at)
    body = "&[\n" + "\n".join(f'            "{row}",' for row in rows) + "\n        "
    return text[:at] + body + text[end + len("\n        ") :]


def main():
    blocks = generated()
    text = TARGET.read_text()
    for name in AREAS:
        text = replace(text, name, blocks[name])
    TARGET.write_text(text)
    print(f"wrote {len(AREAS)} maps into {TARGET.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
