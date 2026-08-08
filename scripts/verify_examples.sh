#!/usr/bin/env bash
# Interactive example equivalence verification for charming-ultraviolet.
#
# Ultraviolet examples are interactive screen programs; both the upstream Go
# binary and the Rust example are driven through the SAME pseudo-terminal
# (scripts/pty_driver.py) with the same scripted keystrokes at the same
# terminal size, and the captured outputs are compared.
#
# The upstream examples are not yet ported; add pairs to PAIRS as ports land.
#
# Requirements: go (1.21+), cargo, python3. Run from the repository root.
set -u

cd "$(dirname "$0")/.."
ROOT="$PWD"
UPSTREAM="$ROOT/upstream-go/examples"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

export TERM=xterm-256color
export LANG=C
export LC_ALL=C
unset NO_COLOR COLORTERM COLORFGBG 2>/dev/null || true

# Normalize ANSI control/escape sequences and compare the set of rendered
# non-empty lines (renderer-neutral comparison; see ONBOARDING.md §10.2).
normalize() {
  python3 -c "
import sys, re
s = sys.stdin.buffer.read().decode('utf-8', 'replace')
s = re.sub(r'\\x1b\\[[0-9;?>=$]*[a-zA-Z]', '', s)
s = re.sub(r'\\x1b\\][^\\x07]*\\x07', '', s)
s = re.sub(r'\\x1b[=>]', '', s)
lines = [l.strip('\\r') for l in s.split('\\n')]
for l in sorted(set(x for x in lines if x.strip())):
    print(l)
"
}

# 1. Build the upstream examples.
if [ -d "$UPSTREAM" ]; then
  (cd "$UPSTREAM" && go mod tidy >/dev/null 2>&1)
  if ! (cd "$UPSTREAM" && go build ./... >/dev/null 2>&1); then
    echo "ERROR: upstream Go examples failed to build" >&2
    exit 1
  fi
fi

# 2. Pairs to compare: upstream example dir -> Rust example name -> keys.
# Currently empty: no ultraviolet examples are ported yet.
PAIRS=""

fails=0
for entry in $PAIRS; do
  go_dir="${entry%%:*}"
  rest="${entry#*:}"
  rs_ex="${rest%%:*}"
  rest="${rest#*:}"
  keys="${rest%%:*}"
  rest="${rest#*:}"
  delay="${rest%%:*}"
  settle="${rest#*:}"
  go_bin="$TMP/go_$(echo "$go_dir" | tr '/' '_')"
  go_out="$TMP/go_$(echo "$go_dir" | tr '/' '_').out"
  rs_out="$TMP/rs_${rs_ex}.out"

  (cd "$UPSTREAM/$go_dir" && go build -o "$go_bin" .) 2>/dev/null || {
    echo "ERROR: upstream example $go_dir failed to build" >&2
    fails=1
    continue
  }

  python3 "$ROOT/scripts/pty_driver.py" --cmd "$go_bin" \
    --keys "$keys" --delay "$delay" --settle "$settle" 2>/dev/null >"$go_out" || true
  cargo build --quiet --example "$rs_ex" 2>/dev/null
  python3 "$ROOT/scripts/pty_driver.py" --cmd "$ROOT/target/debug/examples/$rs_ex" \
    --keys "$keys" --delay "$delay" --settle "$settle" 2>/dev/null >"$rs_out" || true

  if diff <(normalize <"$go_out") <(normalize <"$rs_out") >/dev/null 2>&1; then
    echo "CONTENT-EQUIVALENT: $go_dir"
  else
    echo "DIFFERS:   $go_dir"
    fails=1
  fi
done

if [ "$fails" -ne 0 ]; then
  echo "ERROR: interactive example parity check failed" >&2
  exit 1
fi
if [ -z "$PAIRS" ]; then
  echo "NOTE: no example pairs ported yet; parity check is a no-op"
fi
echo "OK: example parity verified (upstream Go vs Rust)"
