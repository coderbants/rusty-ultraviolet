#!/usr/bin/env bash
# POSIX-flavored so it sources cleanly from bash (3.2+) and zsh.

# Machine-wide shared Cargo cache environment for the rusty-* workspace.
#
# Every Cargo entrypoint in this repository (development launchers, test and
# parity scripts, CI jobs) must source this file and call
# `configure_shared_cargo_cache_environment` BEFORE invoking Cargo, so that
# the registry cache, final artifacts, and intermediate compiler state are
# shared with every other Rust repository on this machine instead of being
# duplicated into checkout-local directories.
#
# Canonical layout (see the machine policy `docs/build-mode/cargo-cache.md`):
#
#   CARGO_SHARED_CACHE_DIRECTORY=~/.cache/cargo
#   CARGO_HOME=~/.cache/cargo/cargo-home          (registry/Git cache)
#   CARGO_TARGET_DIR=~/.cache/cargo/cargo-target  (final artifacts)
#   CARGO_BUILD_BUILD_DIR=~/.cache/cargo/cargo-build (intermediate state)
#   CARGO_INSTALL_ROOT=~/.cargo                   (durable installed tools)
#
# Rules:
#   - Explicit standard Cargo overrides (CARGO_HOME, CARGO_TARGET_DIR,
#     CARGO_BUILD_BUILD_DIR) are preserved, but every resolved cache path
#     must be an absolute, distinct, strict descendant of
#     CARGO_SHARED_CACHE_DIRECTORY.
#   - Never run an unscoped `cargo clean` against the shared target: it
#     deletes reusable artifacts belonging to every participating project.
#   - Same-named top-level binaries from different projects share
#     `cargo-target/debug/<name>`; always build the current package before
#     launching an artifact path (see `cargo_target_dir` below).
#
# Usage (any POSIX-ish shell):
#   . scripts/cargo-env.sh && configure_shared_cargo_cache_environment

# Resolve the portable user-home spelling used by machine-local caches. HOME is
# canonical on POSIX and in the MSYS shells used by the Windows build;
# USERPROFILE is a compatibility fallback for minimal Windows workers.
resolve_shared_cache_user_home() {
  local user_home="${HOME:-}"

  if [ -z "${user_home}" ] && [ -n "${USERPROFILE:-}" ]; then
    if command -v cygpath >/dev/null 2>&1; then
      user_home="$(cygpath -u "${USERPROFILE}")" || return 1
    else
      user_home="${USERPROFILE}"
    fi
  fi
  if [ -z "${user_home}" ]; then
    printf 'HOME or USERPROFILE is required to resolve the shared cache root\n' >&2
    return 1
  fi
  canonicalize_shared_cache_path "${user_home}" "user home"
}

# Resolve the project-agnostic root shared by Cargo commands on this machine.
# CARGO_SHARED_CACHE_DIRECTORY is a workspace convention; standard CARGO_HOME,
# CARGO_TARGET_DIR, and CARGO_BUILD_BUILD_DIR remain the final Cargo controls.
resolve_shared_cargo_cache_directory() {
  local user_home
  local cache_directory

  user_home="$(resolve_shared_cache_user_home)" || return 1
  cache_directory="${CARGO_SHARED_CACHE_DIRECTORY:-${user_home}/.cache/cargo}"

  canonicalize_shared_cache_path "${cache_directory}" "CARGO_SHARED_CACHE_DIRECTORY"
}

# Configure every Cargo cache/output path through one model-free boundary.
# Downloaded registry/Git sources, final artifacts, and intermediate compiler
# state are shared across every Rust repository on this machine. Explicit
# standard Cargo variables remain highest priority, but every resolved Cargo
# path must stay inside CARGO_SHARED_CACHE_DIRECTORY.
configure_shared_cargo_cache_environment() {
  local cargo_cache_root
  local user_home
  local cargo_home
  local target_dir
  local build_dir

  cargo_cache_root="$(resolve_shared_cargo_cache_directory)" || return 1
  user_home="$(resolve_shared_cache_user_home)" || return 1
  cargo_home="$(canonicalize_shared_cache_path \
    "${CARGO_HOME:-${cargo_cache_root}/cargo-home}" \
    "CARGO_HOME")" || return 1
  target_dir="$(canonicalize_shared_cache_path \
    "${CARGO_TARGET_DIR:-${cargo_cache_root}/cargo-target}" \
    "CARGO_TARGET_DIR")" || return 1
  build_dir="$(canonicalize_shared_cache_path \
    "${CARGO_BUILD_BUILD_DIR:-${cargo_cache_root}/cargo-build}" \
    "CARGO_BUILD_BUILD_DIR")" || return 1

  validate_shared_cargo_cache_layout \
    "${cargo_cache_root}" \
    "CARGO_HOME" "${cargo_home}" \
    "CARGO_TARGET_DIR" "${target_dir}" \
    "CARGO_BUILD_BUILD_DIR" "${build_dir}" || return 1

  export CARGO_SHARED_CACHE_DIRECTORY="${cargo_cache_root}"
  export CARGO_HOME="${cargo_home}"
  export CARGO_TARGET_DIR="${target_dir}"
  export CARGO_BUILD_BUILD_DIR="${build_dir}"

  # CARGO_HOME also controls cargo-install's default destination. Installed
  # developer tools are durable executables rather than cache data, so retain
  # the conventional tool location unless the operator explicitly overrides it.
  export CARGO_INSTALL_ROOT="${CARGO_INSTALL_ROOT:-${user_home}/.cargo}"
}

# Print the resolved Cargo target directory for the current package. Callers
# must always build the current package before launching an artifact from this
# path: with a shared machine-wide target, a same-named binary may belong to
# another project. Falls back to the checkout-local `target` when the shared
# environment has not been configured.
cargo_target_dir() {
  if [ -n "${CARGO_TARGET_DIR:-}" ]; then
    printf '%s\n' "${CARGO_TARGET_DIR}"
    return 0
  fi
  printf '%s\n' "${PWD}/target"
}

normalize_absolute_shared_cache_path() {
  local candidate="$1"
  local rest="${candidate}"
  local out=""
  local seg

  while [ -n "${rest}" ]; do
    case "${rest}" in
      */*) seg="${rest%%/*}"; rest="${rest#*/}" ;;
      *) seg="${rest}"; rest="" ;;
    esac
    case "${seg}" in
      ""|".") : ;;
      "..") out="${out%/*}" ;;
      *) out="${out}/${seg}" ;;
    esac
  done
  printf '%s\n' "${out:-/}"
}

canonicalize_shared_cache_path() {
  local candidate="$1"
  local label="${2:-cache path}"
  local existing_ancestor
  local missing_suffix=""
  local leaf
  local parent
  local canonical_ancestor

  case "${candidate}" in
    /*) ;;
    *) printf '%s must be an absolute path: %s\n' "${label}" "${candidate}" >&2; return 1 ;;
  esac

  candidate="$(normalize_absolute_shared_cache_path "${candidate}")"
  while [ "${candidate}" != "/" ] && [ "${candidate}" != "${candidate%/}" ]; do
    candidate="${candidate%/}"
  done
  existing_ancestor="${candidate}"
  while [ ! -e "${existing_ancestor}" ] && [ ! -L "${existing_ancestor}" ]; do
    leaf="${existing_ancestor##*/}"
    parent="${existing_ancestor%/*}"
    if [ -z "${parent}" ]; then
      parent="/"
    fi
    if [ "${parent}" = "${existing_ancestor}" ]; then
      printf '%s has no resolvable filesystem ancestor: %s\n' "${label}" "${candidate}" >&2
      return 1
    fi
    missing_suffix="/${leaf}${missing_suffix}"
    existing_ancestor="${parent}"
  done
  if [ ! -d "${existing_ancestor}" ]; then
    printf '%s must resolve from a directory ancestor: %s\n' "${label}" "${candidate}" >&2
    return 1
  fi
  if ! canonical_ancestor="$(cd "${existing_ancestor}" && pwd -P)"; then
    printf '%s could not be canonicalized: %s\n' "${label}" "${candidate}" >&2
    return 1
  fi
  printf '%s%s\n' "${canonical_ancestor}" "${missing_suffix}"
}

shared_cache_identity() {
  case "${OSTYPE:-}" in
    msys*|cygwin*|win32*)
      printf '%s\n' "$1" | tr '[:upper:]' '[:lower:]'
      ;;
    *)
      printf '%s\n' "$1"
      ;;
  esac
}

assert_shared_cargo_cache_descendant() {
  local cache_root_identity
  local candidate_identity
  local label="$3"
  cache_root_identity="$(shared_cache_identity "$1")"
  candidate_identity="$(shared_cache_identity "$2")"
  case "${candidate_identity}" in
    "${cache_root_identity}/"*)
      return 0
      ;;
    *)
      printf '%s must be a strict descendant of CARGO_SHARED_CACHE_DIRECTORY: %s\n' "${label}" "$2" >&2
      return 1
      ;;
  esac
}

# validate_shared_cargo_cache_layout ROOT LABEL PATH [LABEL PATH ...]
# Every PATH must be an absolute strict descendant of ROOT, and no two PATHs
# may resolve to the same canonical identity.
validate_shared_cargo_cache_layout() {
  local cache_root="$1"
  shift
  local seen=""
  local identity
  local previous_label

  while [ "$#" -ge 2 ]; do
    assert_shared_cargo_cache_descendant "${cache_root}" "$2" "$1" || return 1
    identity="$(shared_cache_identity "$2")" || return 1
    case " ${seen} " in
      *"|${identity} "*)
        previous_label="${seen%%|${identity} *}"
        printf '%s and %s resolve to the same canonical cache identity: %s\n' \
          "${previous_label##*|}" "$1" "$2" >&2
        return 1
        ;;
      *)
        seen="${seen} ${1}|${identity}"
        ;;
    esac
    shift 2
  done
}

if [ -n "${BASH_SOURCE:-}" ] && [ "${BASH_SOURCE[0]}" = "$0" ]; then
  printf '%s\n' "Source this file and call configure_shared_cargo_cache_environment:" >&2
  printf '%s\n' "  . scripts/cargo-env.sh && configure_shared_cargo_cache_environment" >&2
  exit 1
fi
