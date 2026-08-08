# Agent Instructions for `charming-ultraviolet`

> [!IMPORTANT]
> **Subsequent Cycle Requirement**: On every development cycle, before doing any work, the agent MUST inspect [`UPSTREAM_MAPPING.md`](file:///Users/jonny/Projects/charming/charming-ultraviolet/UPSTREAM_MAPPING.md) to verify that all upstream Go files and examples are accounted for. When adding, modifying, or refactoring files, the agent MUST update [`UPSTREAM_MAPPING.md`](file:///Users/jonny/Projects/charming/charming-ultraviolet/UPSTREAM_MAPPING.md) to reflect the current state.
>
> Run `scripts/verify_mapping.sh` to mechanically verify that every file in `upstream-go/` is accounted for in `UPSTREAM_MAPPING.md`.

## Core Rules & Workflow
1. Refer to the workspace-level rule in [`/Users/jonny/Projects/charming/AGENTS.md`](file:///Users/jonny/Projects/charming/AGENTS.md).
2. Maintain 100% rustdoc documentation.
3. Every ported file MUST include the guiding comment header:
   ```rust
   //! Cleanroom Rust port of upstream Go source file: `<upstream-go-filepath>`
   //! Upstream Target Tag / Version: `v0.0.0-20251205161215-1948445e3318`
   ```
4. Verify all tests pass with `cargo test --all-targets` before committing.
5. Multi-version rule: the dependency tree requires this repo at two pins
   (`20251205161215` for charming-lipgloss v2.0.5, `20260703014108` for charming-bubbletea
   v2.0.8). Port the earliest pin first, then diff-forward (`git diff` between the two
   commits inside `upstream-go/`) to produce the later pin. See
   `/Users/jonny/Projects/charming/DEPENDENCY_PLAN.md` §6.
