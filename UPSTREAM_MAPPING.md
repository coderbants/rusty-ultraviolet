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
| `border.go` | `src/border.rs` — **Ported** | Border styles + Draw | Border model for cell buffers |
| `buffer.go` | `src/buffer.rs` | `Buffer`, `ScreenBuffer`, `Line`, render/trim helpers |
| `screen/screen.go` | `src/screen.rs` | `Screen`/`Renderable` interfaces, `Rectangle` |
| `styled.go` | `src/styled.rs` — **Ported** | StyledString + printString + ReadStyle/ReadLink (SGR/hyperlink parsing; Go-verified zero-bounds Lines quirk) | `StyledString` drawable |
| `style.go` (in `styled.go` context) | `src/style.rs` | SGR `Style` (thin wrapper over `charming-x-ansi`) |
| `event.go` | `src/event.rs` | `Event` interface and typed event wrappers |
| `environ.go` | `src/environ.rs` | `Environ` helpers (`Getenv`, `LookupEnv`) |
| `logger.go` | `src/logger.rs` | `Logger` interface |
| `key.go` | `src/key.rs` | `Key`/`KeyEvent` model |
| `key_table.go` | `src/key_table.rs` | Terminal key decode table |
| `mouse.go` | `src/mouse.rs` | Mouse event model |
| `cursor.go` | `src/cursor.rs` | Cursor model |
| `decoder.go` | `src/decoder.rs` | Input event decoder |
| `tabstop.go` | `src/tabstop.rs` — **Ported** | Tab stops (bitmask, find/next/prev/resize) |
| `utils.go` | `src/utils.rs` | Shared helpers |
| `layout.go` | ~~`src/layout.rs`~~ | **DELETED upstream at second pin**; replaced by the `layout/` subpackage (see Second Pin table) which is ported to `src/layout.rs` |
| `terminal.go` | `src/terminal.rs` — **Ported** | Terminal: raw mode, input/event/winch threads, grapheme-width negotiation, winsize reports | Terminal abstraction |
| `terminal_reader.go` | `src/terminal_reader.rs` — **Ported** | TerminalReader + EventScanner: lookup table, bracketed paste, ESC timeout (reader thread + recv_timeout) | Terminal input reader |
| `terminal_reader_other.go` | `src/terminal_reader.rs` — **Ported** | Unix reader path | Non-Windows reader implementation |
| `terminal_reader_windows.go` | `src/terminal_reader.rs` — Deferred (Windows Console API input) | Windows reader implementation |
| `terminal_renderer.go` | `src/terminal_renderer.rs` — **Ported** | Full renderer: cursor-move optimizer (CUP/local/CR/home + hard tabs + backspace + overwrite), transformLine/putRange/emitRange (ECH/REP), clear optimizations, insert/delete cells, profile-aware pen |
| `terminal_renderer_hardscroll.go` | `src/terminal_renderer.rs` — **Ported** | scrollOptimize/scrolln/scrollUp/scrollDown/scrollIdl + DECSTBM margins |
| `terminal_renderer_hashmap.go` | `src/terminal_renderer.rs` — **Ported** | Line hashing + hunk growing/cost-effectiveness for scroll optimization |
| `terminal_tabdly.go` | `src/terminal.rs` | Tab-delay terminal behaviour |
| `terminal_tabdly_other.go` | `src/terminal.rs` | Non-Windows tab-delay behaviour |
| `terminal_unix.go` | `src/terminal.rs` | Unix terminal behaviour |
| `terminal_windows.go` | `src/terminal.rs` | Windows terminal behaviour |
| `terminal_bsdly.go` | `src/terminal.rs` | BSD terminal behaviour |
| `terminal_bsdly_other.go` | `src/terminal.rs` | Non-BSD terminal behaviour |
| `terminal_other.go` | `src/terminal.rs` | Other-platform terminal behaviour |
| `tty.go` | `src/tty.rs` — **Ported** | OpenTTY/Suspend/NotifyWinch (self-pipe + signal handlers) | TTY abstraction |
| `tty_unix.go` | `src/tty.rs` — **Ported** | Unix /dev/tty + SIGTSTP/SIGWINCH | Unix TTY implementation |
| `tty_windows.go` | `src/tty.rs` — Deferred (Windows) | Windows TTY implementation |
| `tty_other.go` | `src/tty.rs` — **Ported** | Non-Unix stubs | Other-platform TTY implementation |
| `winch.go` | `src/winch.rs` — **Ported** | SizeNotifier | Window-change (SIGWINCH) notifications |
| `winch_unix.go` | `src/winch.rs` — **Ported** | Unix SIGWINCH + TIOCGWINSZ | Unix window-change implementation |
| `winch_other.go` | `src/winch.rs` — **Ported** | Non-Unix stubs | Other-platform window-change implementation |
| `cancelreader_other.go` | `src/cancelreader.rs` — **Ported** | new_cancel_reader → poll reader | Cancellable reader (non-Windows) |
| `cancelreader_windows.go` | `src/cancelreader.rs` — Deferred (Windows) | Cancellable reader (Windows) |

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
| `terminal_renderer_test.go`, `terminal_renderer_output_test.go` | `src/terminal_renderer.rs` (tests) | Renderer tests: Go-verified byte vectors (initial/modify/revert/wide/erase/leading/repeat/resize scenarios) |
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

## Second Pin: Source Files (added or restructured after the first pin)

| Upstream Go File | Rust Equivalent / Status | Notes / Description |
| :--- | :--- | :--- |
| `poll.go`, `poll_bsd.go`, `poll_linux.go`, `poll_select.go`, `poll_solaris.go`, `poll_windows.go`, `poll_fallback.go`, `poll_default.go` | `src/poll.rs` — **Ported** | `PollReader` trait + platform selection. Upstream: linux→epoll, bsd→kqueue (+`/dev/tty`→select), solaris→select, windows→conReader, other→fallback. Port unifies the epoll/kqueue/select variants into one POSIX `libc::poll` + self-pipe implementation on Unix; non-Unix uses the fallback stub. The kqueue `/dev/tty` special-case is unnecessary under `poll(2)`. |
| `window.go` | `src/window.rs` — **Ported** | `Window` over a shared `Rc<RefCell<Buffer>>` (buffer sharing for views); also hosts root-package geometry `Position`/`pos`/`rect` (from `buffer.go`). Mutating methods take `&mut self` (callers use `Rc::get_mut`). Negative coordinates clamp to 0 (usize `Rectangle`). Pending: implement the ported `Screen`/`Drawable` interfaces for `Window` once the integrator's `uv.go` port defines them; delegate `clone_area` to `Buffer` if `buffer.rs` gains it. |
| `console.go`, `console_unix.go`, `console_windows.go` | `src/console.rs` — **Ported** | `Console` I/O abstraction, `File` trait, `Winsize`, `RawState`. Build-tag split (`TTY`/`WinCon`) via type aliases. Raw mode (`tcgetattr`/`cfmakeraw`/`tcsetattr`) and `TIOCGWINSZ` implemented directly on `libc` (upstream uses `charmbracelet/x/term`, which is not a dependency). Std streams exposed as `FdFile` (fd-based Read/Write); `TTY`/`WinCon` names kept as aliases for API parity. |
| `internal/casso/math.go`, `internal/casso/solver.go` | `src/casso.rs` — **Ported** | Cassowary constraint solver (port of `lithdew/casso`). Exact deterministic port: `Symbol` counter (note: Go's `atomic.AddUint64` returns the new value — mirrored with `fetch_add + 1`), `Solver::add`/`val`, priorities as `f64`. Go map iteration is order-independent for these cases; verified against Go on 4900 layout splits. Divergence: on solver error the marker symbol is not returned (Go returns `(marker, err)`); callers discard it. |
| `internal/lru/lru.go` | `src/lru.rs` — **Ported** | Deterministic `Lru<K, V>` (Go: hash map + doubly-linked list; port: `Vec` eviction list + `HashMap<K, usize>` index, O(n) ops). API: `new(size)` (panics on negative), `get`, `add` (returns evicted flag), `len`. `Get` returns `Option<&V>` instead of Go's `(V, bool)` copy. |
| `layout/cache.go` | `src/layout.rs` — **Ported** | Global 500-entry split cache (`OnceLock<Mutex<Lru<CacheKey, CacheValue>>>`); FNV-1a 64 hashing reimplemented std-only. `CacheKey` stores the area as min/max tuples because the ported `Rectangle` has no `Hash` impl. |
| `layout/constraint.go` | `src/layout.rs` — **Ported** | `Constraint` as a sealed Rust enum (`Min`/`Max`/`Len`/`Percent`/`Ratio`/`Fill`, i64) with `Display` (`String()` upstream). |
| `layout/flex.go` | `src/layout.rs` — **Ported** | `Flex` enum (`FlexStart`/`FlexLegacy`/`FlexEnd`/`FlexCenter`/`FlexSpaceBetween`/`FlexSpaceEvenly`/`FlexSpaceAround`) with `Display`. |
| `layout/layout.go` | `src/layout.rs` — **Ported** | `Layout` (direction/constraints/padding/spacing/flex), `Splitted` newtype with `assign`/`iter`/`Index`, `new`/`vertical`/`horizontal`, `with_*` builders, `split`/`split_with_spacers`. Verified byte-for-byte against Go on 4900 (flex × spacing × width × constraint-set) splits. |
| `layout/padding.go` | `src/layout.rs` — **Ported** | `Padding` (CSS shorthand via `pad(&[i64])`) applied to the area before solving. |
| `screen/context.go` | `src/screen_context.rs` — **Ported** | Drawing `Context` (style/link/position, grapheme-aware `draw_string`/`draw_string_wrapped`, `fmt::Write`+`io::Write`). Pending: the ported `Screen` trait lacks `width_method()`/`SetCell` — the Context holds its own `WidthMethod` (default WcWidth) and writes through `cell_at_mut`; switch to the screen's `width_method` when the integrator's `uv.go` port exposes it. |
| `screen/screen.go` | `src/screen.rs` (first pin) + **delta pending** | The `uv.Screen` interface (Bounds/CellAt/SetCell/WidthMethod) and the `FillArea`/`CloneArea` cell-width stepping delta belong to the `screen.rs`/`buffer.rs` owners; `screen_context.rs` documents the coupling. |
| `terminal_screen.go` | `src/terminal_screen.rs` — **Ported** | Moved out of `terminal.go` upstream (was `terminal.go` → `src/terminal.rs` at first pin); the `TerminalScreen` owns the window/render/output buffers, cursor, and terminal state. `TerminalRenderer` calls are isolated behind a local `pub(crate)` trait; the real `terminal_renderer.rs` implements it (byte-verified against Go for render/move scenarios). `Environ`, `Logger`, and `ColorProfile` are implemented locally (upstream `environ.go`, `logger.go`, `colorprofile`); the env-based `Detect` subset is ported (terminfo/tmux upgrades omitted). The Go `sync.Mutex` is dropped: all methods take `&mut self`. |
| `layout.go` (root, first pin) | **DELETED upstream** | Old root-package layout engine; replaced by the `layout/` subpackage (mapped above). The old `src/layout.rs` entry no longer applies. |

## Second Pin: Test Files

| Upstream Go Test File | Rust Equivalent / Status | Notes / Description |
| :--- | :--- | :--- |
| `poll_test.go`, `poll_default_test.go`, `poll_fallback_test.go` | `src/poll.rs` (module tests) — **Ported** | `TestReaderNonFile` ported; fallback read/poll/cancel covered in module tests. |
| `screen/screen_test.go` | `tests/screen_test.rs` — Pending | Exercises the new `uv.Screen` interface + `Context`. |
| `internal/casso/casso_test.go` | `src/casso.rs` (module tests) — **Ported** | `TestSymbol`, `TestConstraint`, `TestConstraintRequiringArtificialVariable` ported and passing. |
| `internal/lru/lru_test.go` | `src/lru.rs` (module tests) — **Ported** | `TestLRU` ported and passing (incl. negative-size panic). |
| `layout/layout_test.go` | `src/layout.rs` (module tests) — **Ported** | `TestPriorityIsValid`, `TestLength`, `TestPercent`, `TestRatio`, `TestMin`/`Max`/`Len`, `TestFlexConstraint`, `TestFlexSpacing`, `TestEdgeCases` cases ported; Go-verified expectations. |
| `cell_iszero_test.go`, `grapheme_width_test.go`, `transformline_bug_test.go`, `widecell_placeholder_test.go` | `tests/*_test.rs` — Pending | Second-pin cell/width regression tests; belong to the `cell`/`buffer` owners. `transformline_bug_test.go` and `widecell_placeholder_test.go` are covered by `src/terminal_screen.rs` module tests. |
| `terminal_screen_test.go` | (none) — N/A | No upstream test file exists for `terminal_screen.go` at this pin; the screen behavior is covered by the module tests in `src/terminal_screen.rs`. |

## Second Pin: Example Applications

| Upstream Go Example | Rust Equivalent / Status | Notes / Description |
| :--- | :--- | :--- |
| `examples/mouse/main.go` | `examples/mouse.rs` — Pending | Mouse demo (new at second pin). |
| `examples/advanced/boxes/main.go` | `examples/advanced_boxes.rs` — Pending | New advanced example. |
| `examples/advanced/image/main.go` | `examples/advanced_image.rs` — Pending | Moved from `examples/image`. |
| `examples/advanced/layout/main.go` | `examples/advanced_layout.rs` — Pending | Moved from `examples/layout`. |
| `examples/advanced/rgbimage/main.go` | `examples/advanced_rgbimage.rs` — Pending | New advanced example. |
| `examples/advanced/space/main.go` | `examples/advanced_space.rs` — Pending | Moved from `examples/space`. |
| `examples/advanced/splits/main.go` | `examples/advanced_splits.rs` — Pending | New advanced example. |
| `examples/advanced/tv/main.go` | `examples/advanced_tv.rs` — Pending | Moved from `examples/tv`. |

## Porting Status

| Module group | Status |
| --- | --- |
| `cell`, `buffer`/`screen`, `styled`, `border` | Ported & Tested |
| `event`, `key`, `key_table`, `mouse`, `cursor`, `environ`, `logger`, `utils` | Ported & Tested |
| `decoder`, `tabstop` | Ported & Tested |
| `terminal*`, `tty*`, `winch*`, `cancelreader*` | In progress |
| `examples/*` | In progress (PTY harness in place) |
| Second pin (20260703) | In progress: `poll`, `console`, `window`, `casso`, `lru`, `layout` subpackage, `screen_context` ported & verified; `terminal_screen`, `uv.go` facade, second-pin tests/examples pending |

