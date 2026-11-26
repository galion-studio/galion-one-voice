#!/usr/bin/env bash
# Galion Voice — Perpetual Open MIT License
# Self-test for G2 and the no-VPS skip. No cargo, no network, no microphone.
# Proves a forbidden-word list is not a G2 hit, and a real load still exits 1,
# including a load planted under tests/. Proves SKIP is not a hard failure
# and that an unset GALION_VPS_HOST is "skip", not a remote run.
# Usage: galion-gates/selftest.sh     Exit 0 when every case holds.
set -uo pipefail
HERE="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
# shellcheck source=gates_lib.sh
source "$HERE/gates_lib.sh"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
FAILS=0

expect_rc() {
  local name="$1" want="$2"
  shift 2
  local out="$WORK/case.out"
  set +e
  "$@" >"$out" 2>&1
  local got=$?
  set +e
  if [ "$got" -eq "$want" ]; then
    echo "PASS $name"
  else
    echo "FAIL $name (exit $got want $want)" >&2
    cat "$out" >&2
    FAILS=$((FAILS+1))
  fi
}

expect_ok() {
  local name="$1"
  shift
  if "$@"; then
    echo "PASS $name"
  else
    echo "FAIL $name" >&2
    FAILS=$((FAILS+1))
  fi
}

expect_fail() {
  local name="$1"
  shift
  if "$@"; then
    echo "FAIL $name (unexpected success)" >&2
    FAILS=$((FAILS+1))
  else
    echo "PASS $name"
  fi
}

write_rel() {
  local dir="$1" rel="$2"
  mkdir -p "$dir/$(dirname "$rel")"
  cat > "$dir/$rel"
}

# --- shared decisions -------------------------------------------------------
expect_ok "rustc 1.85 is new enough" rustc_at_least_185 1.85.0
expect_ok "rustc 1.85 bare minor is new enough" rustc_at_least_185 1.85
expect_ok "rustc 1.100 is new enough" rustc_at_least_185 1.100.0
expect_ok "rustc 2 is new enough" rustc_at_least_185 2.0.0
expect_fail "rustc 1.83 is below the render MSRV" rustc_at_least_185 1.83.0
expect_fail "rustc 1.84 is below the render MSRV" rustc_at_least_185 1.84.9
expect_fail "empty version is not 1.85" rustc_at_least_185 ""

saved_host="${GALION_VPS_HOST-__unset__}"
unset GALION_VPS_HOST
expect_ok "unset VPS host is a skip" test "$(vps_mode)" = skip
GALION_VPS_HOST=""
expect_ok "empty VPS host is a skip" test "$(vps_mode)" = skip
GALION_VPS_HOST="user@example.invalid"
expect_ok "set VPS host is remote" test "$(vps_mode)" = remote
if [ "$saved_host" = __unset__ ]; then unset GALION_VPS_HOST; else GALION_VPS_HOST="$saved_host"; fi

expect_fail "SKIP is not a hard failure" is_hard_fail SKIP
expect_fail "SOFTFAIL is not a hard failure" is_hard_fail SOFTFAIL
expect_fail "PENDING is not a hard failure" is_hard_fail PENDING
expect_fail "PASS is not a hard failure" is_hard_fail PASS
expect_ok "FAIL is a hard failure" is_hard_fail FAIL
expect_ok "run-gates records skipped (no VPS)" grep -q 'mark R$g SKIP "skipped (no VPS)"' "$HERE/run-gates.sh"
expect_ok "unreachable VPS stays PENDING" grep -q 'mark R$g PENDING "VPS $GALION_VPS_HOST unreachable (ssh BatchMode)"' "$HERE/run-gates.sh"
expect_ok "run-gates uses vps_mode" grep -q 'vps_mode' "$HERE/run-gates.sh"

# --- G2 on this repo --------------------------------------------------------
expect_rc "this repo has no G2 load" 0 bash "$HERE/run-gates.sh" --g2-scan "$ROOT"

# --- fixtures ---------------------------------------------------------------
ban="$WORK/ban"
write_rel "$ban" crates/horbm-ear/tests/frames.rs <<'EOF'
#[test]
fn sources_do_not_open_a_store_or_a_socket() {
    let banned = [
        "std::fs",
        "ggml",
        "gguf",
    ];
    for needle in banned {
        assert!(!text.contains(needle), "contains {needle}");
    }
}
EOF
expect_rc "forbidden-word list in a test is not a load" 0 bash "$HERE/run-gates.sh" --g2-scan "$ban"

marked="$WORK/marked"
write_rel "$marked" src/lib.rs <<'EOF'
fn words() {
    let words = [
        // galion-gates:allow-forbidden-word
        "gguf",
    ];
    let _ = words;
}
EOF
expect_rc "allow-marker clears only a bare token line" 0 bash "$HERE/run-gates.sh" --g2-scan "$marked"

bare="$WORK/bare"
write_rel "$bare" src/lib.rs <<'EOF'
fn words() {
    let words = [
        "gguf",
    ];
}
EOF
expect_rc "bare token without a denylist or marker is a hit" 1 bash "$HERE/run-gates.sh" --g2-scan "$bare"

fmt="$WORK/fmt"
write_rel "$fmt" src/lib.rs <<'EOF'
fn main() {
    let fmt = "gguf";
    let _ = fmt;
}
EOF
expect_rc "a format binding is a hit" 1 bash "$HERE/run-gates.sh" --g2-scan "$fmt"

interp="$WORK/interp"
write_rel "$interp" src/lib.rs <<'EOF'
fn main() {
    let _ = format!("model.{}", "gguf");
}
EOF
expect_rc "format string that still spells the token is a hit" 1 bash "$HERE/run-gates.sh" --g2-scan "$interp"

load="$WORK/load"
write_rel "$load" crates/example/tests/evil.rs <<'EOF'
#[test]
fn loads() {
    let _ = std::fs::read("model.gguf");
}
EOF
expect_rc "read of model.gguf inside tests/ is a hit" 1 bash "$HERE/run-gates.sh" --g2-scan "$load"

marked_load="$WORK/marked-load"
write_rel "$marked_load" crates/example/tests/evil.rs <<'EOF'
#[test]
fn loads() {
    let _ = std::fs::read("model.gguf"); // galion-gates:allow-forbidden-word
}
EOF
expect_rc "allow-marker does not clear a path load" 1 bash "$HERE/run-gates.sh" --g2-scan "$marked_load"

inc="$WORK/inc"
write_rel "$inc" crates/example/src/lib.rs <<'EOF'
fn baked() {
    let _ = include_bytes!("fixture.gguf");
}
EOF
expect_rc "include_bytes of a container is a hit" 1 bash "$HERE/run-gates.sh" --g2-scan "$inc"

namehit="$WORK/name"
mkdir -p "$namehit/models"
: > "$namehit/models/weights.gguf"
expect_rc "a .gguf file is a hit" 1 bash "$HERE/run-gates.sh" --g2-scan "$namehit"

hidden="$WORK/hidden"
mkdir -p "$hidden/target"
write_rel "$hidden" target/leak.rs <<'EOF'
fn main() { let _ = std::fs::read("model.gguf"); }
EOF
: > "$hidden/target/hidden.gguf"
expect_rc "target/ is not scanned" 0 bash "$HERE/run-gates.sh" --g2-scan "$hidden"

comment="$WORK/comment"
write_rel "$comment" src/lib.rs <<'EOF'
// The format name gguf is named here so it stays unloaded.
fn main() {}
EOF
expect_rc "a comment that names the ban is not a load" 0 bash "$HERE/run-gates.sh" --g2-scan "$comment"

absence="$WORK/absence"
write_rel "$absence" src/lib.rs <<'EOF'
fn t() {
    assert!(!low.contains("gguf"));
}
EOF
expect_rc "an absence assert is not a load" 0 bash "$HERE/run-gates.sh" --g2-scan "$absence"

magic="$WORK/magic"
write_rel "$magic" src/lib.rs <<'EOF'
fn t() {
    assert!(!on.windows(4).any(|w| w == b"GGUF"));
}
EOF
expect_rc "a magic-byte absence check is not a load" 0 bash "$HERE/run-gates.sh" --g2-scan "$magic"

glob="$WORK/glob"
write_rel "$glob" refuse.sh <<'EOF'
refuse_name() {
  case "$name" in
    *.wav|*.gguf)
      exit 2
      ;;
  esac
}
EOF
expect_rc "a shell refuse glob is not a load" 0 bash "$HERE/run-gates.sh" --g2-scan "$glob"

fname="$WORK/fname"
write_rel "$fname" src/lib.rs <<'EOF'
fn t() {
    let banned = [
        "model.gguf",
    ];
    assert!(!text.contains(needle));
}
EOF
expect_rc "a filename inside a denylist is still a hit" 1 bash "$HERE/run-gates.sh" --g2-scan "$fname"

oneline="$WORK/oneline"
write_rel "$oneline" crates/example/tests/ban.rs <<'EOF'
#[test]
fn list_only() {
    let banned = ["ggml", "gguf"];
    for needle in banned {
        assert!(!text.contains(needle));
    }
}
EOF
expect_rc "one-line denylist plus an absence assert is not a load" 0 bash "$HERE/run-gates.sh" --g2-scan "$oneline"

alone="$WORK/alone"
write_rel "$alone" crates/example/tests/ban.rs <<'EOF'
#[test]
fn list_only() {
    let banned = ["gguf"];
}
EOF
expect_rc "one-line denylist without an absence assert is a hit" 1 bash "$HERE/run-gates.sh" --g2-scan "$alone"

same="$WORK/same"
write_rel "$same" crates/example/tests/evil.rs <<'EOF'
#[test]
fn loads() {
    refuse("policy"); let _ = std::fs::read("model.gguf");
}
EOF
expect_rc "refuse() on the same line does not clear a path load" 1 bash "$HERE/run-gates.sh" --g2-scan "$same"

both="$WORK/both"
write_rel "$both" src/lib.rs <<'EOF'
fn loads() {
    let _ = std::fs::read("model.gguf"); assert!(!path.contains(".gguf"));
}
EOF
expect_rc "an absence assert on the same line does not clear a path load" 1 bash "$HERE/run-gates.sh" --g2-scan "$both"

if [ "$FAILS" -eq 0 ]; then
  echo "selftest: PASS"
  exit 0
fi
echo "selftest: FAIL $FAILS" >&2
exit 1
