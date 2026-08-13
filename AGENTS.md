# Agent Instructions for `rusty-ultraviolet`

> [!IMPORTANT]
> **Subsequent Cycle Requirement**: On every development cycle, before doing any work, the agent MUST inspect [`UPSTREAM_MAPPING.md`](file:///Users/jonny/Projects/rusty/rusty-ultraviolet/UPSTREAM_MAPPING.md) to verify that all upstream Go files and examples are accounted for. When adding, modifying, or refactoring files, the agent MUST update [`UPSTREAM_MAPPING.md`](file:///Users/jonny/Projects/rusty/rusty-ultraviolet/UPSTREAM_MAPPING.md) to reflect the current state.
>
> Run `scripts/verify_mapping.sh` to mechanically verify that every file in `upstream-go/` is accounted for in `UPSTREAM_MAPPING.md`.

## Core Rules & Workflow
1. Refer to the workspace-level rule in [`/Users/jonny/Projects/rusty/AGENTS.md`](file:///Users/jonny/Projects/rusty/AGENTS.md).
2. Maintain 100% rustdoc documentation.
3. Every ported file MUST include the guiding comment header:
   ```rust
   //! Cleanroom Rust port of upstream Go source file: `<upstream-go-filepath>`
   //! Upstream Target Tag / Version: `v0.0.0-20251205161215-1948445e3318`
   ```
4. Verify all tests pass with `cargo test --all-targets` before committing.
5. Multi-version rule: the dependency tree requires this repo at two pins
   (`20251205161215` for rusty-lipgloss v2.0.5, `20260703014108` for rusty-bubbletea
   v2.0.8). Port the earliest pin first, then diff-forward (`git diff` between the two
   commits inside `upstream-go/`) to produce the later pin. See
   `/Users/jonny/Projects/rusty/DEPENDENCY_PLAN.md` §6.

## Releases
- **Version policy: the crate version and every release tag MUST equal the tracked upstream
  version exactly** — for untagged upstreams that means the full upstream pseudo-version
  including its commit suffix (the `Upstream Target Tag / Version` header in `src/lib.rs`,
  e.g. `v0.0.0-20260703014108-f5a850f9c2b7`) — never ahead, never behind.
  `scripts/verify_upstream_version.sh` enforces this in CI and in the publish workflow.
- GitHub Releases MUST match upstream: every upstream release tag tracked by this port must
  exist as a `v*` tag and a GitHub release on this repo (the publish workflow creates the
  release automatically from the tag). If upstream has no tagged releases (pseudo-version
  pins), no GitHub release is required.
- To release: push the `v*` tag (e.g. `git tag v2.0.8 && git push origin v2.0.8`); the
  workflow runs tests, example parity, creates the GitHub Release, and attempts the
  crates.io publish (non-fatal without a registry token).
- The crates.io publish step is tag-gated; dev pushes only run tests and parity.
