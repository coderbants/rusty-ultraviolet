#!/usr/bin/env bash
set -euo pipefail

# Regression guard for both supported downstream Cargo source topologies. A
# dependency crate's [patch.crates-io] table is ignored by its consumers, so the
# consuming root must choose either the registry graph or one coherent sibling
# path graph.

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
repo_root_cargo="$(cygpath -m "$repo_root" 2>/dev/null || printf '%s' "$repo_root")"
probe_root="$(mktemp -d)"
trap 'rm -rf "$probe_root"' EXIT
cp "$repo_root/rust-toolchain.toml" "$probe_root/rust-toolchain.toml"

isolated_ultraviolet="$probe_root/rusty-ultraviolet"
bare_consumer="$probe_root/bare-consumer"
mkdir -p "$isolated_ultraviolet" "$bare_consumer/src"
cp "$repo_root/Cargo.toml" "$isolated_ultraviolet/Cargo.toml"
cp -R "$repo_root/src" "$isolated_ultraviolet/src"

isolated_ultraviolet_cargo="$(cygpath -m "$isolated_ultraviolet" 2>/dev/null || printf '%s' "$isolated_ultraviolet")"
printf '%s\n' \
  '[package]' \
  'name = "ultraviolet-bare-checkout-probe"' \
  'version = "0.0.0"' \
  'edition = "2021"' \
  '' \
  '[dependencies]' \
  "rusty-ultraviolet = { path = \"$isolated_ultraviolet_cargo\" }" \
  'rusty-x-ansi = "0.11.7"' \
  > "$bare_consumer/Cargo.toml"
printf '%s\n' \
  'fn main() {' \
  '    let method = rusty_x_ansi::method::WidthMethod::WcWidth;' \
  '    let _window = rusty_ultraviolet::new_window(1, 1, Some(method));' \
  '}' \
  > "$bare_consumer/src/main.rs"

cargo +1.98.0 check --manifest-path "$bare_consumer/Cargo.toml"

consumer_root="$probe_root/family-consumer"
mkdir -p "$consumer_root/src"
sibling_root="$(dirname "$repo_root")"
sibling_root_cargo="$(cygpath -m "$sibling_root" 2>/dev/null || printf '%s' "$sibling_root")"

mkdir -p "$consumer_root/src"
printf '%s\n' \
  '[package]' \
  'name = "ultraviolet-dependency-source-probe"' \
  'version = "0.0.0"' \
  'edition = "2021"' \
  '' \
  '[dependencies]' \
  "rusty-ultraviolet = { path = \"$repo_root_cargo\" }" \
  "rusty-lipgloss = { path = \"$sibling_root_cargo/rusty-lipgloss\" }" \
  'rusty-x-ansi = "0.11.7"' \
  '' \
  '[patch.crates-io]' \
  "rusty-colorprofile = { path = \"$sibling_root_cargo/rusty-colorprofile\" }" \
  "rusty-x-ansi = { path = \"$sibling_root_cargo/rusty-x-ansi\" }" \
  > "$consumer_root/Cargo.toml"
printf '%s\n' \
  'fn main() {' \
  '    let method = rusty_x_ansi::method::WidthMethod::WcWidth;' \
  '    let _window = rusty_ultraviolet::new_window(1, 1, Some(method));' \
  '    let _style = rusty_lipgloss::Style::new();' \
  '}' \
  > "$consumer_root/src/main.rs"

cargo +1.98.0 check --manifest-path "$consumer_root/Cargo.toml"

cargo +1.98.0 metadata \
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

console.log("bare-checkout and sibling-family dependency sources are coherent");
JS
