#!/usr/bin/env bash
set -euo pipefail

# Regression guard for downstream Cargo source identity. A dependency crate's
# [patch.crates-io] table is ignored by its consumers, so this test must resolve
# Ultraviolet from a separate root package rather than inspect Ultraviolet's own
# workspace graph.

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
repo_root_cargo="$(cygpath -m "$repo_root" 2>/dev/null || printf '%s' "$repo_root")"
consumer_root="$(mktemp -d)"
trap 'rm -rf "$consumer_root"' EXIT

mkdir -p "$consumer_root/src"
printf '%s\n' \
  '[package]' \
  'name = "ultraviolet-dependency-source-probe"' \
  'version = "0.0.0"' \
  'edition = "2021"' \
  '' \
  '[dependencies]' \
  "rusty-ultraviolet = { path = \"$repo_root_cargo\" }" \
  > "$consumer_root/Cargo.toml"
printf '%s\n' 'fn main() {}' > "$consumer_root/src/main.rs"

cargo metadata \
  --manifest-path "$consumer_root/Cargo.toml" \
  --format-version 1 \
  > "$consumer_root/metadata.json"

metadata_path="$(cygpath -m "$consumer_root/metadata.json" 2>/dev/null || printf '%s' "$consumer_root/metadata.json")"
node_executable="node"
if ! command -v "$node_executable" >/dev/null 2>&1; then
  node_executable="/c/Program Files/nodejs/node.exe"
fi

"$node_executable" - "$metadata_path" <<'JS'
const fs = require("node:fs");

const metadata = JSON.parse(fs.readFileSync(process.argv[2], "utf8"));
const ultraviolet = metadata.packages.find((entry) => entry.name === "rusty-ultraviolet");
if (ultraviolet === undefined) {
  throw new Error("downstream metadata contains no rusty-ultraviolet package");
}
const expected = new Map([
  ["rusty-colorprofile", "/rusty-colorprofile/cargo.toml"],
  ["rusty-x-ansi", "/rusty-x-ansi/cargo.toml"],
]);

for (const [name, expectedSuffix] of expected) {
  const matches = metadata.packages.filter((entry) => entry.name === name);
  if (matches.length !== 1) {
    throw new Error(`expected exactly one ${name} package, found ${matches.length}`);
  }
  const manifestPath = matches[0].manifest_path.replaceAll("\\", "/").toLowerCase();
  if (matches[0].source !== null || !manifestPath.endsWith(expectedSuffix)) {
    throw new Error(`${name} resolved from ${matches[0].manifest_path}, expected sibling path source`);
  }
}

const testkit = ultraviolet.dependencies.filter((entry) => entry.name === "rusty-testkit");
if (testkit.length !== 1 || testkit[0].kind !== "dev" || testkit[0].target !== "cfg(unix)") {
  throw new Error("rusty-testkit must remain a Unix-only development dependency");
}

const lipgloss = ultraviolet.dependencies.filter((entry) => entry.name === "rusty-lipgloss");
if (lipgloss.length !== 1 || lipgloss[0].kind !== "dev" || lipgloss[0].target !== null) {
  throw new Error("rusty-lipgloss must remain available to cross-platform examples");
}

console.log("downstream runtime dependency sources are unified");
JS
