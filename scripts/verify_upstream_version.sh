#!/usr/bin/env bash

# Enforces the Charming version policy:
#
#   THE CRATE VERSION AND EVERY RELEASE TAG MUST EQUAL THE TRACKED UPSTREAM
#   VERSION — NEVER AHEAD, NEVER BEHIND.
#
# The tracked upstream version is the `Upstream Target Tag / Version` header
# in `src/lib.rs` (mandated by AGENTS.md). The crate version in Cargo.toml —
# and, on the release path, the pushed tag — must equal it exactly. For
# upstreams without tagged releases the upstream pseudo-version (including
# its commit suffix) is mirrored exactly.
#
# Usage:
#   scripts/verify_upstream_version.sh            # check the crate version
#   scripts/verify_upstream_version.sh v2.0.8     # also verify a v* tag

set -u

cd "$(dirname "$0")/.."

crate_version="$(grep -m1 '^version' Cargo.toml | sed 's/.*"\(.*\)".*/\1/')"
upstream="$(grep -m1 'Upstream Target Tag / Version:' src/lib.rs | sed -n 's/.*`\([^`]*\)`.*/\1/p' | tr -d ' ')"

if [ -z "${upstream}" ]; then
  if [ ! -d upstream-go ]; then
    # No tracked upstream at all (e.g. the original test harness): the
    # upstream-mirror policy does not apply.
    echo "OK: no upstream tracked (original crate); version policy not applicable"
    exit 0
  fi
  echo "ERROR: could not read the tracked upstream version from src/lib.rs" >&2
  exit 1
fi

upstream_version="${upstream#v}"

fail=0

if [ "${crate_version}" != "${upstream_version}" ]; then
  echo "ERROR: crate version '${crate_version}' does not match the tracked upstream version '${upstream_version}' (src/lib.rs header)." >&2
  echo "       Crate versions and release tags must mirror upstream Go exactly: never ahead, never behind." >&2
  echo "       Wait for upstream to overtake before bumping, then set the crate version to the upstream version." >&2
  fail=1
fi

if [ "$#" -ge 1 ]; then
  tag="$1"
  case "${tag}" in
    v*)
      if [ "${tag}" != "v${upstream_version}" ]; then
        echo "ERROR: release tag '${tag}' does not match the tracked upstream version 'v${upstream_version}' (src/lib.rs header)." >&2
        echo "       THE RELEASE TAG MUST MATCH UPSTREAM ONLY AND ALWAYS." >&2
        fail=1
      fi
      ;;
    *)
      echo "WARN: '${tag}' is not a v* tag; skipping the tag check (crate version was still verified)." >&2
      ;;
  esac
fi

if [ "${fail}" -ne 0 ]; then
  exit 1
fi

echo "OK: crate version '${crate_version}' matches upstream '${upstream_version}'"
