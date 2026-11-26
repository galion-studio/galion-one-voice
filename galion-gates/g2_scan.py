#!/usr/bin/env python3
# Galion Voice — Perpetual Open MIT License
"""G2 scanner: flag a GGUF container or a source line that loads it.

Exit 0 when the tree is clean, 1 when it prints at least one hit, 2 on usage
or an unreadable file. Hits are relative paths, or path:line:text.

A file whose name contains "gguf" is a container (except under target/ or
.git/). Docs, galion-gates/, results, and .build are not line-scanned. The
same exclusions as the previous grep remain: comments, refuse(), absence
asserts, gguf=false, and the other ban phrases in EXCLUDES.

A forbidden-word list element is not a load. The line's only "gguf" spellings
are string literals whose entire contents are that token, and the line has no
other code. It is ignored when either:

  * the previous 80 lines bind a denylist (banned, forbidden, denylist, ...)
    and contain '[', and the next 80 lines assert the token is absent, or
  * this line or the previous non-empty line carries the marker
    galion-gates:allow-forbidden-word.

The marker does not apply to any other shape. A one-line
`let banned = ["gguf"];` is the same kind of element when an absence assert
follows; without that assert it stays a hit.

A load shape is never cleared by those exclusions. A load shape is a string
whose entire contents are a relative filename ending in `.gguf` (not the bare
extension `.gguf`, and not a longer regex), an unquoted `name.gguf` that is
not the shell glob `*.gguf`, `include_bytes!`, `include_str!`, or `std::fs` /
`File::open` on the same line as the token. Test files are scanned.
format!("model.{}", "gguf") is a hit. A later use of a binding that no longer
spells the token is outside this text scan.
"""

from __future__ import annotations

import os
import re
import sys

WINDOW = 80
MARKER = "galion-gates:allow-forbidden-word"
WALK_PRUNE = {"target", ".git"}
CONTENT_SKIP = {"results", ".build", "galion-gates", "docs"}
SOURCE_EXTS = {".rs", ".py", ".toml", ".sh", ".c", ".cpp", ".h"}
SKIP_FILES = {"check-no-forbidden.sh", "LOCKS.toml"}

# Substring filters kept from run-gates.sh so ban lines that already passed
# stay passing. Matched against ":lineno:text", case-insensitive.
EXCLUDES = re.compile(
    "|".join(
        [
            r":[0-9]+:[ \t]*(//|///|//!|#|\*)",
            r"gguf=false",
            r"no gguf",
            r"do not import",
            r"refuse\(",
            r'contains\("\.?gguf',
            r'b"gguf"',
            r'"gguf" in',
            r"'gguf' in",
            r'gguf" not in',
            r"without a refusal",
            r"import \(gguf",
            r"llama_cpp",
            r"landing checks",
        ]
    ),
    re.IGNORECASE,
)

DENY_NAME = re.compile(
    r"\b(banned|forbidden|forbidden_words|denylist|deny_list|blocklist|ban_list|prohibited)\b",
    re.IGNORECASE,
)
ABSENCE = re.compile(
    r"![\w:.]*contains\s*\(|contains\s*\(\s*(needle|banned|word|token|phrase)\b|\bnot\s+in\b",
    re.IGNORECASE,
)


def split_code(line: str) -> tuple[list[str], str]:
    """String-literal contents, and the line with those literals and a // comment removed."""
    strings: list[str] = []
    rest: list[str] = []
    i = 0
    n = len(line)
    while i < n:
        c = line[i]
        if c == "/" and i + 1 < n and line[i + 1] == "/":
            break
        if c in ("'", '"'):
            quote = c
            i += 1
            buf: list[str] = []
            while i < n:
                if line[i] == "\\" and i + 1 < n:
                    buf.append(line[i + 1])
                    i += 2
                    continue
                if line[i] == quote:
                    i += 1
                    break
                buf.append(line[i])
                i += 1
            strings.append("".join(buf))
            rest.append(" ")
            continue
        rest.append(c)
        i += 1
    return strings, "".join(rest)


def mentions_in_code(line: str) -> bool:
    strings, rest = split_code(line)
    if any("gguf" in s.lower() for s in strings):
        return True
    return "gguf" in rest.lower()


def has_load_shape(line: str) -> bool:
    """True when this line itself opens, includes, or names a container file.

    The bare extension ".gguf" and the shell glob "*.gguf" are not container
    names. An absence assert can quote those. A filename cannot hide behind
    refuse() or contains() on the same line.
    """
    strings, rest = split_code(line)
    # The whole literal is a relative filename. A regex snippet such as
    # `\.gguf` inside a longer pattern is not, and neither is the bare extension.
    filename = re.compile(r"(?i)^[\w./-]+\.gguf$")
    for item in strings:
        if filename.fullmatch(item.strip()):
            return True
    if re.search(r"(?i)(?<![*\w])[\w./-]+\.gguf\b", rest):
        return True
    low_rest = rest.lower()
    io_tokens = (
        "include_bytes!",
        "include_str!",
        "std::fs",
        "file::open",
        "file::create",
        "openoptions",
        "read_to_string",
    )
    return any(tok in low_rest for tok in io_tokens) and "gguf" in line.lower()


def is_exact_token_list_line(line: str) -> bool:
    strings, rest = split_code(line)
    hits = [s for s in strings if "gguf" in s.lower()]
    if not hits:
        return False
    if any(s.lower() != "gguf" for s in hits):
        return False
    if "gguf" in rest.lower():
        return False
    return re.fullmatch(r"[\s,]*", rest) is not None


def marker_near(lines: list[str], idx: int) -> bool:
    if MARKER in lines[idx]:
        return True
    j = idx - 1
    while j >= 0 and j >= idx - 5:
        if lines[j].strip() == "":
            j -= 1
            continue
        return MARKER in lines[j]
    return False


def denylist_context(lines: list[str], idx: int) -> bool:
    start = max(0, idx - WINDOW)
    before = "\n".join(lines[start : idx + 1])
    if "[" not in before or DENY_NAME.search(before) is None:
        return False
    end = min(len(lines), idx + WINDOW + 1)
    after = "\n".join(lines[idx:end])
    return ABSENCE.search(after) is not None


def is_denylist_declaration(line: str) -> bool:
    """`let banned = ["gguf"];` on one line. The name must be a denylist binding."""
    strings, rest = split_code(line)
    hits = [item for item in strings if "gguf" in item.lower()]
    if not hits or any(item.lower() != "gguf" for item in hits):
        return False
    if "gguf" in rest.lower():
        return False
    if DENY_NAME.search(rest) is None:
        return False
    return (
        re.fullmatch(
            r"\s*(?:pub(?:\([^)]*\))?\s+)?(?:let|const|static)\s+\w+\s*(?::[^=]+)?=\s*\[[\s,]*\]\s*;?\s*",
            rest,
        )
        is not None
    )


def is_forbidden_word_list_element(lines: list[str], idx: int) -> bool:
    line = lines[idx]
    if has_load_shape(line):
        return False
    if not is_exact_token_list_line(line) and not is_denylist_declaration(line):
        return False
    if marker_near(lines, idx):
        return True
    return denylist_context(lines, idx)


def is_load_line(lines: list[str], idx: int) -> bool:
    line = lines[idx]
    if "gguf" not in line.lower():
        return False
    # A container name or an open/include on this line is a load even when the
    # line also refuses the format or asserts some other absence.
    if has_load_shape(line):
        return True
    record = f":{idx + 1}:{line}"
    if EXCLUDES.search(record) is not None:
        return False
    if not mentions_in_code(line):
        return False
    if is_forbidden_word_list_element(lines, idx):
        return False
    return True


def scan_file(path: str, rel: str) -> list[str]:
    try:
        with open(path, "rb") as fh:
            data = fh.read()
    except OSError as exc:
        raise OSError(f"{rel}: {exc}") from exc
    if b"\0" in data:
        return []
    lines = data.decode("utf-8", "replace").splitlines()
    hits = []
    for i, line in enumerate(lines):
        if is_load_line(lines, i):
            hits.append(f"{rel}:{i + 1}:{line}")
    return hits


def scan(root: str) -> list[str]:
    root = os.path.abspath(root)
    hits: list[str] = []
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [d for d in dirnames if d not in WALK_PRUNE]
        rel_dir = os.path.relpath(dirpath, root)
        parts = [] if rel_dir == "." else rel_dir.split(os.sep)
        skip_content = any(part in CONTENT_SKIP for part in parts)
        for name in filenames:
            rel = "/".join(parts + [name]) if parts else name
            path = os.path.join(dirpath, name)
            if "gguf" in name.lower():
                hits.append(rel)
            if skip_content or name in SKIP_FILES or name.endswith(".off"):
                continue
            ext = os.path.splitext(name)[1]
            if ext not in SOURCE_EXTS:
                continue
            hits.extend(scan_file(path, rel))
    hits.sort()
    return hits


def main(argv: list[str]) -> int:
    if len(argv) != 2:
        print("usage: g2_scan.py ROOT", file=sys.stderr)
        return 2
    root = argv[1]
    if not os.path.isdir(root):
        print(f"not a directory: {root}", file=sys.stderr)
        return 2
    try:
        hits = scan(root)
    except OSError as exc:
        print(str(exc), file=sys.stderr)
        return 2
    for hit in hits:
        print(hit)
    return 1 if hits else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
