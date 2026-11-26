#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License
# Copyright (c) 2026 Galion Labs.
#
# Third-party licence check for the three workspaces (root, horbm, voice-stack).
# For each manifest, reads:
#   cargo metadata --format-version 1 --locked --offline
# and does not use the network. Path packages (source null) are this repo.
# Every other package is third-party.
#
# An SPDX expression is accepted when one OR-arm is made only of
# MIT, Apache-2.0, BSD-2-Clause, BSD-3-Clause, Zlib, ISC.
# Unicode-3.0 and Unicode-DFS-2016 are data licences. They are not in that
# set, so a package that names one is listed. It is not silently allowed.
# A `WITH` exception is not an allowed id either. When a plain MIT or
# Apache-2.0 arm is also present, that arm is the one chosen.
#
# The chosen arm is MIT if that arm exists, otherwise Apache-2.0, otherwise
# any other allowed arm. A chosen arm that is not only MIT and/or Apache-2.0
# must have a `## <name>` or `### <name>` heading in THIRD_PARTY_NOTICES.md.
# The name is the first token of that heading, or the token after `and`,
# so `### num_enum 0.7.6 and num_enum_derive 0.7.6` covers both crates.
#
# Exit 0 when nothing is listed. Exit 1 and print every offender, then one
# summary line. Cargo or python failures also exit 1.
set -euo pipefail
cd "$(dirname "$0")/.."

command -v cargo >/dev/null 2>&1 || { echo "FAIL: cargo is required" >&2; exit 1; }
command -v python3 >/dev/null 2>&1 || { echo "FAIL: python3 is required" >&2; exit 1; }
[ -f THIRD_PARTY_NOTICES.md ] || { echo "FAIL: THIRD_PARTY_NOTICES.md not found" >&2; exit 1; }

tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

collect() {
  local lock=$1
  local manifest=$2
  local out=$3
  if ! cargo metadata --format-version 1 --locked --offline --manifest-path "$manifest" >"$out" 2>"$out.err"; then
    echo "FAIL: cargo metadata --format-version 1 --locked --offline --manifest-path $manifest ($lock)" >&2
    cat "$out.err" >&2
    exit 1
  fi
}

collect Cargo.lock Cargo.toml "$tmp/root.json"
collect horbm/Cargo.lock horbm/Cargo.toml "$tmp/horbm.json"
collect voice-stack/Cargo.lock voice-stack/Cargo.toml "$tmp/voice.json"

python3 - THIRD_PARTY_NOTICES.md \
  "Cargo.lock=$tmp/root.json" \
  "horbm/Cargo.lock=$tmp/horbm.json" \
  "voice-stack/Cargo.lock=$tmp/voice.json" <<'PY'
import json
import sys

NOTICES = sys.argv[1]
SOURCES = []
for arg in sys.argv[2:]:
    lock, path = arg.split("=", 1)
    SOURCES.append((lock, path))

ALLOWED = frozenset({
    "MIT",
    "Apache-2.0",
    "BSD-2-Clause",
    "BSD-3-Clause",
    "Zlib",
    "ISC",
})
MIT_APACHE = frozenset({"MIT", "Apache-2.0"})
DATA = frozenset({"Unicode-3.0", "Unicode-DFS-2016"})
LOCK_ORDER = {lock: i for i, (lock, _) in enumerate(SOURCES)}


class ParseError(Exception):
    pass


def tokenize(expr):
    tokens = []
    i = 0
    n = len(expr)
    while i < n:
        ch = expr[i]
        if ch.isspace():
            i += 1
            continue
        if ch in "()":
            tokens.append(ch)
            i += 1
            continue
        if ch == "/":
            tokens.append("/")
            i += 1
            continue
        j = i
        while j < n and not expr[j].isspace() and expr[j] not in "()/":
            j += 1
        word = expr[i:j]
        if not word:
            raise ParseError("empty token")
        if word.upper() in ("OR", "AND", "WITH"):
            tokens.append(word.upper())
        else:
            tokens.append(word)
        i = j
    if not tokens:
        raise ParseError("empty expression")
    return tokens


def parse(tokens):
    pos = 0

    def peek():
        return tokens[pos] if pos < len(tokens) else None

    def eat(expected=None):
        nonlocal pos
        if pos >= len(tokens):
            raise ParseError("truncated expression")
        cur = tokens[pos]
        if expected is not None and cur != expected:
            raise ParseError("expected %s, found %s" % (expected, cur))
        pos += 1
        return cur

    def parse_or():
        node = parse_and()
        while peek() in ("OR", "/"):
            eat()
            node = ("OR", node, parse_and())
        return node

    def parse_and():
        node = parse_with()
        while peek() == "AND":
            eat("AND")
            node = ("AND", node, parse_with())
        return node

    def parse_with():
        node = parse_primary()
        if peek() == "WITH":
            eat("WITH")
            exc = eat()
            if exc in ("OR", "AND", "WITH", "(", ")"):
                raise ParseError("exception is %s" % exc)
            node = ("WITH", node, exc)
        return node

    def parse_primary():
        tok = peek()
        if tok is None:
            raise ParseError("truncated expression")
        if tok == "(":
            eat("(")
            node = parse_or()
            eat(")")
            return node
        if tok in ("OR", "AND", "WITH", ")"):
            raise ParseError("unexpected %s" % tok)
        return ("ID", eat())

    tree = parse_or()
    if pos != len(tokens):
        raise ParseError("trailing %s" % tokens[pos])
    return tree


def arms_of(tree):
    kind = tree[0]
    if kind == "ID":
        return [frozenset([tree[1]])]
    if kind == "WITH":
        # The exception is part of the arm. It is not an allowed id.
        return [arm | frozenset(["WITH:" + tree[2]]) for arm in arms_of(tree[1])]
    if kind == "OR":
        return arms_of(tree[1]) + arms_of(tree[2])
    if kind == "AND":
        return [left | right for left in arms_of(tree[1]) for right in arms_of(tree[2])]
    raise ParseError("unknown node %s" % kind)


def choose(arms):
    """Prefer MIT, then Apache-2.0, then any other arm inside the allowed set."""
    plain = []
    for arm in arms:
        if any(x.startswith("WITH:") for x in arm):
            continue
        if arm and arm <= ALLOWED:
            plain.append(arm)
    for want in (frozenset(["MIT"]), frozenset(["Apache-2.0"])):
        for arm in plain:
            if arm == want:
                return arm
    mit_apache = [arm for arm in plain if arm <= MIT_APACHE]
    if mit_apache:
        return min(mit_apache, key=lambda arm: (len(arm), sorted(arm)))
    if plain:
        return min(plain, key=lambda arm: (len(arm), sorted(arm)))
    return None


def data_ids(arms):
    found = set()
    for arm in arms:
        for ident in arm:
            if ident in DATA:
                found.add(ident)
    return found


def heading_names(text):
    """Names a `##` or `###` heading credits.

    The first token is the crate. A token after `and` is another crate,
    as in `### num_enum 0.7.6 and num_enum_derive 0.7.6`.
    """
    names = set()
    for line in text.splitlines():
        hashes = 0
        while hashes < len(line) and line[hashes] == "#":
            hashes += 1
        if hashes not in (2, 3):
            continue
        if hashes >= len(line) or not line[hashes].isspace():
            continue
        body = line[hashes:].strip().replace("`", "")
        tokens = [part.strip(".,;:()[]{}\"'") for part in body.split()]
        tokens = [token for token in tokens if token]
        if not tokens:
            continue
        names.add(tokens[0])
        for index, token in enumerate(tokens):
            if token == "and" and index + 1 < len(tokens):
                names.add(tokens[index + 1])
    return names


def lock_list(locks):
    return ", ".join(sorted(locks, key=lambda item: LOCK_ORDER.get(item, 99)))


def arm_text(arm):
    return " AND ".join(sorted(arm))


def main():
    with open(NOTICES, encoding="utf-8") as handle:
        notices = handle.read()
    headed = heading_names(notices)

    # (name, version) -> record
    packages = {}
    for lock, path in SOURCES:
        with open(path, encoding="utf-8") as handle:
            meta = json.load(handle)
        for pkg in meta.get("packages") or []:
            if pkg.get("source") is None:
                continue
            key = (pkg.get("name") or "", pkg.get("version") or "")
            rec = packages.get(key)
            if rec is None:
                rec = {
                    "license": pkg.get("license"),
                    "locks": set(),
                }
                packages[key] = rec
            rec["locks"].add(lock)
            # Same crate version can be resolved in more than one workspace.
            # A disagreement is itself an offender: keep both strings visible.
            other = pkg.get("license")
            if rec["license"] != other:
                rec.setdefault("also", set()).add(other)

    no_arm = []
    data_only = []
    missing = []
    data_count = 0

    for (name, version) in sorted(packages):
        rec = packages[(name, version)]
        locks = lock_list(rec["locks"])
        license_expr = rec["license"]
        label = "%s %s" % (name, version)
        if "also" in rec:
            no_arm.append(
                "no-permissive-arm: %s [%s] license differs across workspaces: %r vs %r"
                % (label, locks, license_expr, sorted(rec["also"], key=lambda v: str(v)))
            )
            continue
        if not license_expr or not str(license_expr).strip():
            no_arm.append(
                "no-permissive-arm: %s [%s] <no license expression>"
                % (label, locks)
            )
            continue
        expr = str(license_expr).strip()
        try:
            arms = arms_of(parse(tokenize(expr)))
        except ParseError as exc:
            no_arm.append(
                "no-permissive-arm: %s [%s] %s (unparsed: %s)"
                % (label, locks, expr, exc)
            )
            continue
        named = data_ids(arms)
        if named:
            data_count += 1
        chosen = choose(arms)
        if chosen is None:
            note = ""
            if named:
                note = " (data-licence: %s)" % ", ".join(sorted(named))
            no_arm.append(
                "no-permissive-arm: %s [%s] %s%s" % (label, locks, expr, note)
            )
        elif named:
            data_only.append(
                "data-licence: %s [%s] %s (%s; chosen %s)"
                % (label, locks, expr, ", ".join(sorted(named)), arm_text(chosen))
            )
        if chosen is not None and not chosen <= MIT_APACHE and name not in headed:
            missing.append(
                "missing-notice: %s [%s] chosen %s; license %s"
                % (label, locks, arm_text(chosen), expr)
            )

    for line in no_arm:
        print(line)
    for line in data_only:
        print(line)
    for line in missing:
        print(line)
    print(
        "summary: third_party=%d no_permissive_arm=%d data_licences=%d missing_notices=%d"
        % (len(packages), len(no_arm), data_count, len(missing))
    )
    if no_arm or data_only or missing:
        sys.exit(1)


if __name__ == "__main__":
    main()
PY
