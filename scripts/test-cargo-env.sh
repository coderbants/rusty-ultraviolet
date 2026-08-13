#!/usr/bin/env bash
# Focused tests for scripts/cargo-env.sh (shared machine-wide Cargo cache).
#
# Covers: default layout, override precedence, absolute-path enforcement,
# containment within CARGO_SHARED_CACHE_DIRECTORY, alias rejection, and the
# absence of project-local Cargo output. Safe to run in any checkout and in
# CI; never touches the real shared cache (isolated temp roots).
#
# Usage: scripts/test-cargo-env.sh

set -u

cd "$(dirname "$0")/.."

failures=0

fail() {
  printf 'FAIL: %s\n' "$1" >&2
  failures=1
}

pass() {
  printf 'ok: %s\n' "$1"
}

TMP="$(mktemp -d)"
# macOS returns /var/... for mktemp while `pwd -P` canonicalizes to
# /private/var/...; keep the canonical spelling so path comparisons hold.
TMP="$(cd "$TMP" && pwd -P)"
trap 'rm -rf "$TMP"' EXIT

# Run configure_shared_cargo_cache_environment in a clean subshell with the
# given environment overrides (VAR=VALUE ...).
run_configure() {
  ( 
    unset CARGO_SHARED_CACHE_DIRECTORY CARGO_HOME CARGO_TARGET_DIR \
      CARGO_BUILD_BUILD_DIR CARGO_INSTALL_ROOT
    while [ "$#" -gt 0 ]; do
      export "${1%%=*}=${1#*=}"
      shift
    done
    . scripts/cargo-env.sh || return 2
    configure_shared_cargo_cache_environment
  )
}

# --- 1. Defaults -----------------------------------------------------------
(
  unset CARGO_SHARED_CACHE_DIRECTORY CARGO_HOME CARGO_TARGET_DIR \
    CARGO_BUILD_BUILD_DIR CARGO_INSTALL_ROOT
  . scripts/cargo-env.sh && configure_shared_cargo_cache_environment >/dev/null 2>&1
  printf '%s\n' "${CARGO_SHARED_CACHE_DIRECTORY}" "${CARGO_HOME}" \
    "${CARGO_TARGET_DIR}" "${CARGO_BUILD_BUILD_DIR}" "${CARGO_INSTALL_ROOT}"
) >"$TMP/defaults"
d_root="$(sed -n '1p' "$TMP/defaults")"
d_home_path="$(sed -n '2p' "$TMP/defaults")"
d_target="$(sed -n '3p' "$TMP/defaults")"
d_build="$(sed -n '4p' "$TMP/defaults")"
d_install="$(sed -n '5p' "$TMP/defaults")"
if [ "${d_root}" = "$HOME/.cache/cargo" ] \
   && [ "${d_home_path}" = "$HOME/.cache/cargo/cargo-home" ] \
   && [ "${d_target}" = "$HOME/.cache/cargo/cargo-target" ] \
   && [ "${d_build}" = "$HOME/.cache/cargo/cargo-build" ] \
   && [ "${d_install}" = "$HOME/.cargo" ]; then
  pass "defaults resolve to the canonical ~/.cache/cargo layout"
else
  fail "defaults: got $(cat "$TMP/defaults" | tr '\n' ' ')"
fi

# --- 2. Override precedence -------------------------------------------------
if run_configure "CARGO_SHARED_CACHE_DIRECTORY=$TMP/root" \
  "CARGO_TARGET_DIR=$TMP/root/custom-target" >"$TMP/out2" 2>&1; then
  ( unset CARGO_SHARED_CACHE_DIRECTORY CARGO_HOME CARGO_TARGET_DIR \
      CARGO_BUILD_BUILD_DIR CARGO_INSTALL_ROOT
    CARGO_SHARED_CACHE_DIRECTORY="$TMP/root"
    CARGO_TARGET_DIR="$TMP/root/custom-target"
    export CARGO_SHARED_CACHE_DIRECTORY CARGO_TARGET_DIR
    . scripts/cargo-env.sh >/dev/null 2>&1
    configure_shared_cargo_cache_environment >/dev/null 2>&1
    printf '%s\n' "${CARGO_TARGET_DIR}" "${CARGO_HOME}" "${CARGO_BUILD_BUILD_DIR}"
  ) >"$TMP/override"
  read -r got_target <"$TMP/override"
  read -r got_home <"$TMP/override"
  read -r got_build <"$TMP/override"
  got_target="$(sed -n '1p' "$TMP/override")"
  got_home="$(sed -n '2p' "$TMP/override")"
  got_build="$(sed -n '3p' "$TMP/override")"
  if [ "${got_target}" = "$TMP/root/custom-target" ] \
     && [ "${got_home}" = "$TMP/root/cargo-home" ] \
     && [ "${got_build}" = "$TMP/root/cargo-build" ]; then
    pass "explicit CARGO_TARGET_DIR override wins; others default under the root"
  else
    fail "override precedence: got $(cat "$TMP/override" | tr '\n' ' ')"
  fi
else
  fail "override precedence: configure failed: $(cat "$TMP/out2")"
fi

# --- 3. Absolute-path enforcement ------------------------------------------
if run_configure "CARGO_SHARED_CACHE_DIRECTORY=$TMP/root3" \
  "CARGO_HOME=relative/cargo-home" >"$TMP/out3" 2>&1; then
  fail "absolute-path enforcement: relative CARGO_HOME was accepted"
else
  if grep -q "must be an absolute path" "$TMP/out3"; then
    pass "relative CARGO_HOME is rejected"
  else
    fail "absolute-path enforcement: unexpected error: $(cat "$TMP/out3")"
  fi
fi

# --- 4. Containment ---------------------------------------------------------
if run_configure "CARGO_SHARED_CACHE_DIRECTORY=$TMP/root4" \
  "CARGO_TARGET_DIR=$TMP/elsewhere/target" >"$TMP/out4" 2>&1; then
  fail "containment: CARGO_TARGET_DIR outside the root was accepted"
else
  if grep -q "strict descendant" "$TMP/out4"; then
    pass "CARGO_TARGET_DIR outside CARGO_SHARED_CACHE_DIRECTORY is rejected"
  else
    fail "containment: unexpected error: $(cat "$TMP/out4")"
  fi
fi

# --- 5. Alias rejection ------------------------------------------------------
if run_configure "CARGO_SHARED_CACHE_DIRECTORY=$TMP/root5" \
  "CARGO_HOME=$TMP/root5/same" \
  "CARGO_TARGET_DIR=$TMP/root5/same" >"$TMP/out5" 2>&1; then
  fail "alias rejection: identical CARGO_HOME/CARGO_TARGET_DIR was accepted"
else
  if grep -q "same canonical cache identity" "$TMP/out5"; then
    pass "identical cache paths are rejected"
  else
    fail "alias rejection: unexpected error: $(cat "$TMP/out5")"
  fi
fi

# Alias via `..` normalization must also be rejected.
if run_configure "CARGO_SHARED_CACHE_DIRECTORY=$TMP/root5b" \
  "CARGO_HOME=$TMP/root5b/a" \
  "CARGO_TARGET_DIR=$TMP/root5b/a/../a" >"$TMP/out5b" 2>&1; then
  fail "alias rejection: '..'-spelled alias was accepted"
else
  if grep -q "same canonical cache identity" "$TMP/out5b"; then
    pass "'..'-spelled alias is rejected after normalization"
  else
    fail "alias rejection (..): unexpected error: $(cat "$TMP/out5b")"
  fi
fi

# --- 6. No project-local Cargo output ----------------------------------------
if run_configure "CARGO_SHARED_CACHE_DIRECTORY=$TMP/root6" >"$TMP/out6" 2>&1; then
  (
    unset CARGO_SHARED_CACHE_DIRECTORY CARGO_HOME CARGO_TARGET_DIR \
      CARGO_BUILD_BUILD_DIR CARGO_INSTALL_ROOT
    CARGO_SHARED_CACHE_DIRECTORY="$TMP/root6"
    export CARGO_SHARED_CACHE_DIRECTORY
    . scripts/cargo-env.sh >/dev/null 2>&1
    configure_shared_cargo_cache_environment >/dev/null 2>&1
    cargo check --quiet 2>/dev/null
    printf 'target-dir=%s\n' "${CARGO_TARGET_DIR}"
    if [ -d "target" ]; then
      printf 'project-local-target=present\n'
    else
      printf 'project-local-target=absent\n'
    fi
  ) >"$TMP/check"
  if grep -q "target-dir=$TMP/root6/cargo-target" "$TMP/check" \
     && grep -q "project-local-target=absent" "$TMP/check"; then
    pass "cargo check writes to the shared target and leaves no project-local target"
  else
    fail "no project-local output: $(cat "$TMP/check" | tr '\n' ' ')"
  fi
else
  fail "no project-local output: configure failed: $(cat "$TMP/out6")"
fi

if [ "${failures}" -ne 0 ]; then
  printf 'cargo-env tests FAILED\n' >&2
  exit 1
fi
printf 'cargo-env tests OK\n'
