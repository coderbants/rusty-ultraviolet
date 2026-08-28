# Contributing to `rusty-ultraviolet`

Thanks for your interest in contributing! `rusty-ultraviolet` is a cleanroom Rust port
of the upstream Go [charmbracelet/ultraviolet](https://github.com/charmbracelet/ultraviolet)
library (input, window and rendering primitives), pinned to upstream pseudo-versions
`v0.0.0-20251205161215-1948445e3318` (first pin) and
`v0.0.0-20260703014108-f5a850f9c2b7` (second pin, used by rusty-bubbletea v2.0.8).

Please read the workspace rules in [`AGENTS.md`](AGENTS.md) (and the root
[`AGENTS.md`](../AGENTS.md)) before contributing. This file summarizes the practical
workflow.

## Development setup

- Rust 1.98.0, selected automatically by the checked-in `rust-toolchain.toml`.
- Go (for the upstream parity scripts and the pinned `upstream-go/` checkout).
- No other system dependencies; there are no C build steps.

```sh
cargo build --all-targets
cargo test --all-targets
```

## Repository layout

- `src/` — the ported crate. Every public symbol has rustdoc; every module mirrors an
  upstream Go file.
- `examples/` — executable Rust ports of upstream Go examples.
- `tests/` — Rust integration tests ported from upstream `*_test.go` suites.
- `upstream-go/` — the pinned upstream Go checkouts (git-ignored, never commit them).
- `scripts/` — parity and mapping verification helpers.
- `UPSTREAM_MAPPING.md` — the authoritative 1:1 account of every upstream file.

## The cleanroom porting workflow

1. **Upstream sync (Phase A/B).** New upstream pins are fetched into `upstream-go/`
   (full history is kept so the diff-forward workflow can run). Diff the new pin against
   the previous one with `git diff <prev>..<new> -- '*.go'` inside `upstream-go/` and
   update `UPSTREAM_MAPPING.md` so every upstream file (source, tests, examples, docs,
   support files) stays accounted for.

2. **Mechanical porting (Phase C).** Port Go source to Rust modules, Go `*_test.go`
   suites to `tests/`, and Go example programs to `examples/`. Every ported file MUST
   start with the header:

   ```rust
   //! Cleanroom Rust port of upstream Go source file: `<upstream-go-filepath>`
   //! Upstream Target Tag / Version: `<pin-version>`
   ```

3. **Comment invariants.** Tag doc comments ported directly from Go with
   `<upstream-comment>...</upstream-comment>`, include `<public-docs>...</public-docs>`
   blocks on user-facing modules, and prefer borrowing (`&str`, `&[T]`) over allocation
   (`Arc`, `Rc`). Maintain 100% rustdoc coverage: `cargo doc --no-deps --all-features`
   must emit no warnings.

4. **Verification.** Before committing:

   ```sh
   cargo test --all-targets
   ./scripts/verify_mapping.sh   # upstream file accounting
   cargo doc --no-deps           # rustdoc coverage
   ```

   Input decoding behavior (key sequences, escape handling) is exercised with the pty
   driver: `python3 scripts/pty_driver.py --cmd target/debug/examples/<name> ...`.

## Releases

- Upstream ultraviolet has no tagged releases (pseudo-version pins), so no GitHub release
  is required for pins; crates.io publishes use the `v*` tag or `dev` push workflow.
- Pushing a `v*` tag runs tests and attempts the crates.io publish (non-fatal without a
  registry token); `dev` branch pushes run tests only.

## Versioning

Every release that matches an upstream version uses the upstream `MAJOR.MINOR.PATCH` plus a
fourth dot-separated iteration number that internally tracks which deployed release of this
port it is for that upstream version:

- `v0.1.0.0` — first port release of a given upstream pin
- `v0.1.0.1` — a hotfix iteration for that pin (bug fix released without an upstream
  version bump)

The iteration increments whenever we publish a new release of our port without an upstream
version bump (e.g. a bug fix that upstream has not yet released). The git tag and GitHub
release carry the full four-part version. `Cargo.toml` keeps the upstream `X.Y.Z`, since
crates.io only accepts `MAJOR.MINOR.PATCH`; iteration hotfixes publish under the same
`X.Y.Z` on crates.io, replacing the previous deployment (iterations are only used for bug
fixes, so the contents differ only in fixes).

## Contribution guidelines

- Keep the 1:1 file mapping intact — do not add or remove modules without updating
  `UPSTREAM_MAPPING.md`.
- Match the upstream file layout: a change to an upstream Go file lands in the
  corresponding Rust module.
- Commit messages should describe the upstream behaviour being ported or fixed, e.g.
  `port ansi parser states` or `fix: split escape sequences are held for completion`.
- Follow the style of the surrounding code; there are no external formatter
  dependencies beyond `cargo fmt` defaults.

## Reporting issues

- Describe the upstream Go behaviour expected and the Rust behaviour observed.
- Include the terminal emulator and `TERM` value when the issue is input/render related.
- Note the pinned upstream version in the report.

## License

[MIT](LICENSE) — same as the upstream project.
