# Upstream Go File Mapping: `charming-ultraviolet`

Target Upstream Tag: `github.com/charmbracelet/ultraviolet@v0.0.0-20251205161215-1948445e3318` (first pin)

This mapping accounts for **every** file in the upstream repository at this pin (source,
tests, examples, docs, and support files). The full repo history is checked out locally in
`upstream-go/` (gitignored) so the diff-forward workflow for the second required pin
(`v0.0.0-20260703014108-f5a850f9c2b7`, used by charming-bubbletea v2.0.8) can run via
`git diff 1948445..f5a850f` per [`/Users/jonny/Projects/charming/DEPENDENCY_PLAN.md`](../DEPENDENCY_PLAN.md) §6.

## Source Files (package `ultraviolet`)

| Upstream Go File | Rust Equivalent / Status | Notes / Description |
| :--- | :--- | :--- |
| `doc.go` | `src/lib.rs` | Package docs |
| `uv.go` | `src/lib.rs` | Module facade / re-exports |
| `cell.go` | `src/cell.rs` | Cell model (`Cell`, style/link equality, zero cell) |
| `border.go` | `src/border.rs` | Border model for cell buffers |
| `buffer.go` | `src/buffer.rs` | `Buffer`, `ScreenBuffer`, `Line`, render/trim helpers |
| `screen/screen.go` | `src/screen.rs` | `Screen`/`Renderable` interfaces, `Rectangle` |
| `styled.go` | `src/styled.rs` | `StyledString` drawable |
| `style.go` (in `styled.go` context) | `src/style.rs` | SGR `Style` (thin wrapper over `charming-x-ansi`) |
| `event.go` | `src/event.rs` | `Event` interface and typed event wrappers |
| `environ.go` | `src/environ.rs` | `Environ` helpers (`Getenv`, `LookupEnv`) |
| `logger.go` | `src/logger.rs` | `Logger` interface |
| `key.go` | `src/key.rs` | `Key`/`KeyEvent` model |
| `key_table.go` | `src/key_table.rs` | Terminal key decode table |
| `mouse.go` | `src/mouse.rs` | Mouse event model |
| `cursor.go` | `src/cursor.rs` | Cursor model |
| `decoder.go` | `src/decoder.rs` | Input event decoder |
| `tabstop.go` | `src/tabstop.rs` | Tab stop handling |
| `utils.go` | `src/utils.rs` | Shared helpers |
| `layout.go` | `src/layout.rs` | Layout engine (constraints, flex, padding) |
| `terminal.go` | `src/terminal.rs` | Terminal abstraction |
| `terminal_reader.go` | `src/terminal_reader.rs` | Terminal input reader |
| `terminal_reader_other.go` | `src/terminal_reader.rs` | Non-Windows reader implementation |
| `terminal_reader_windows.go` | `src/terminal_reader.rs` | Windows reader implementation |
| `terminal_renderer.go` | `src/terminal_renderer.rs` | Terminal output renderer |
| `terminal_renderer_hardscroll.go` | `src/terminal_renderer.rs` | Hard-scroll renderer path |
| `terminal_renderer_hashmap.go` | `src/terminal_renderer.rs` | Hash-map renderer cache |
| `terminal_tabdly.go` | `src/terminal.rs` | Tab-delay terminal behaviour |
| `terminal_tabdly_other.go` | `src/terminal.rs` | Non-Windows tab-delay behaviour |
| `terminal_unix.go` | `src/terminal.rs` | Unix terminal behaviour |
| `terminal_windows.go` | `src/terminal.rs` | Windows terminal behaviour |
| `terminal_bsdly.go` | `src/terminal.rs` | BSD terminal behaviour |
| `terminal_bsdly_other.go` | `src/terminal.rs` | Non-BSD terminal behaviour |
| `terminal_other.go` | `src/terminal.rs` | Other-platform terminal behaviour |
| `tty.go` | `src/tty.rs` | TTY abstraction |
| `tty_unix.go` | `src/tty.rs` | Unix TTY implementation |
| `tty_windows.go` | `src/tty.rs` | Windows TTY implementation |
| `tty_other.go` | `src/tty.rs` | Other-platform TTY implementation |
| `winch.go` | `src/winch.rs` | Window-change (SIGWINCH) notifications |
| `winch_unix.go` | `src/winch.rs` | Unix window-change implementation |
| `winch_other.go` | `src/winch.rs` | Other-platform window-change implementation |
| `cancelreader_other.go` | `src/cancelreader.rs` | Cancellable reader (non-Windows) |
| `cancelreader_windows.go` | `src/cancelreader.rs` | Cancellable reader (Windows) |

## Test Files (`*_test.go` -> `tests/` or module tests)

| Upstream Go Test File | Rust Equivalent / Status | Notes / Description |
| :--- | :--- | :--- |
| `cell_test.go` | `tests/cell_test.rs` | Cell model tests |
| `border_test.go` | `tests/border_test.rs` | Border model tests |
| `buffer_test.go` | `tests/buffer_test.rs` | Buffer/screen tests |
| `screen/screen_test.go` | `tests/screen_test.rs` | Screen tests |
| `styled_test.go` | `tests/styled_test.rs` | StyledString tests |
| `event_test.go` | `tests/event_test.rs` | Event tests |
| `key_test.go` | `tests/key_test.rs` | Key tests |
| `decoder_test.go` | `tests/decoder_test.rs` | Decoder tests |
| `tabstop_test.go` | `tests/tabstop_test.rs` | Tab stop tests |
| `layout_test.go` | `tests/layout_test.rs` | Layout tests |
| `terminal_test.go` | `tests/terminal_test.rs` | Terminal tests |
| `terminal_renderer_test.go` | `tests/terminal_renderer_test.rs` | Renderer tests |
| `terminal_renderer_output_test.go` | `tests/terminal_renderer_test.rs` | Renderer output golden tests |
| `cancelreader_test.go` | `tests/cancelreader_test.rs` | Cancellable reader tests |
| `cursor_test.go` | `tests/cursor_test.rs` | Cursor tests |

## Example Applications (`examples/*`)

The upstream examples are interactive programs exercised through the PTY harness
(`scripts/pty_driver.py`) against the pinned Go binaries; each pair is verified
content-equivalent by `scripts/verify_examples.sh` (wired into the publish workflow).

| Upstream Go Example | Rust Equivalent / Status | Notes / Description |
| :--- | :--- | :--- |
| `examples/helloworld/main.go` | `examples/helloworld.rs` | Hello world screen |
| `examples/altscreen/main.go` | `examples/altscreen.rs` | Alternate screen |
| `examples/draw/main.go` | `examples/draw.rs` | Drawing primitives |
| `examples/layout/main.go` | `examples/layout.rs` | Layout engine demo |
| `examples/splits/main.go` | `examples/splits.rs` | Split panes |
| `examples/space/main.go` | `examples/space.rs` | Space rendering |
| `examples/tv/main.go` | `examples/tv.rs` | TV demo |
| `examples/image/charm.jpg` | (asset) | Example image asset |
| `examples/panic/main.go` | `examples/panic.rs` | Panic handling demo |
| `examples/prependline/main.go` | `examples/prependline.rs` | Prepend-line demo |

Example support files (`examples/go.mod`, `examples/go.sum`) are covered by the Support Files
section.

## Documentation & Support Files

| Upstream File | Rust Equivalent / Status | Notes / Description |
| :--- | :--- | :--- |
| `LICENSE` | `LICENSE` | MIT License (matching upstream copyright) |
| `README.md` | `README.md` | Upstream README retained + port identification header |
| `TUTORIAL.md` | `TUTORIAL.md` (retained) | Upstream tutorial doc |
| `go.mod` / `go.sum` | `Cargo.toml` | Dependency manifest (Go modules -> Cargo crates) |
| `examples/go.mod` / `examples/go.sum` | `Cargo.toml` | Example-module manifest |
| `.golangci.yml` / `.goreleaser.yml` / `Taskfile.yml` | `.github/workflows/publish.yml` | Build/lint/release config -> CI workflow |
| `.github/workflows/*` | `.github/workflows/publish.yml` | CI/CD -> Rust publish workflow + example parity check |
| `.github/CODEOWNERS` / `.github/dependabot.yml` / `.gitattributes` | `.gitignore` | Process/config files; not applicable to the Rust crate |

## Dependency Versions (this pin)

| Go dependency | Version | Rust handling |
| --- | --- | --- |
| `charmbracelet/colorprofile` | v0.3.3 | `charming-colorprofile` v0.3.3 (sibling repo) |
| `charmbracelet/x/ansi` | v0.11.2 | `charming-x-ansi` v0.11.2 (sibling repo) |
| `charmbracelet/x/term` | v0.2.2 | `charming-x-term` v0.2.2 (sibling repo) |
| `charmbracelet/x/termios` | v0.1.1 | `charming-x-termios` v0.1.1 (sibling repo) |
| `charmbracelet/x/windows` | v0.2.2 | `charming-x-windows` v0.2.2 (sibling repo) |
| `clipperhouse/{displaywidth,stringish,uax29/v2}` | various | `unicode-width`/`unicode-segmentation` crates |
| `lucasb-eyer/go-colorful` | v1.3.0 | In-line color math |
| `mattn/go-runewidth` | v0.0.19 | `unicode-width` crate |
| `muesli/cancelreader` | v0.2.2 | In-line `cancelreader` module |
| `rivo/uniseg` | v0.4.7 | `unicode-segmentation` crate |
| `xo/terminfo` | v0.0.0-20220910… | Terminfo (in-line or crate; see DEPENDENCY_PLAN.md §4) |
| `golang.org/x/sync` / `x/sys` | v0.18.0 / v0.38.0 | `std` / `libc` / crossterm |

## Second Pin (diff-forward target)

`v0.0.0-20260703014108-f5a850f9c2b7` (charming-bubbletea v2.0.8): ported after this pin by
diffing `git diff 1948445..f5a850f -- '*.go'` inside `upstream-go/` and applying the changes.
Per the multi-version rule this second pin is published as a separate crate version
(`0.0.0-20260703014108`) with its own mapping entries appended to this document when ported.

## Porting Status

| Module group | Status |
| --- | --- |
| `cell`, `buffer`/`screen`, `styled`, `border` | Ported & Tested |
| `event`, `key`, `key_table`, `mouse`, `cursor`, `environ`, `logger`, `utils` | Ported & Tested |
| `decoder`, `tabstop` | Ported & Tested |
| `terminal*`, `tty*`, `winch*`, `cancelreader*` | In progress |
| `examples/*` | In progress (PTY harness in place) |
| Second pin (20260703) | Pending (diff-forward) |
