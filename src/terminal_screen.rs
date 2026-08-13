//! Cleanroom Rust port of upstream Go source file: `terminal_screen.go`
//! Upstream Target Tag / Version: `v0.0.0-20260703014108-f5a850f9c2b7`
//!
//! <public-docs>
//! The terminal screen: owns the window buffer, the render buffer, the
//! output buffer, the cursor, and the terminal state (alternate screen,
//! bracketed paste, mouse modes, keyboard enhancements, colors, and more),
//! and drives the TerminalRenderer to transform the screen state into
//! escape sequences.
//! </public-docs>

use crate::buffer::{RenderBuffer, Screen};
use crate::cell::{empty_cell, Cell};
use crate::console::{FdFile, File};
use crate::environ::Environ;
use crate::screen::Rectangle;
use crate::window::{new_window, Window};
use crate::{
    new_cursor, Cursor, CursorShape, Drawable, KeyboardEnhancements, MouseEncoding, MouseMode,
    ProgressBar, ProgressBarState,
};
use rusty_x_ansi::background::{
    RESET_BACKGROUND_COLOR, RESET_CURSOR_COLOR, RESET_FOREGROUND_COLOR,
};
use rusty_x_ansi::cursor::{cursor_down, cursor_up, set_cursor_style};
use rusty_x_ansi::kitty::kitty_keyboard;
use rusty_x_ansi::method::WidthMethod;
use rusty_x_ansi::mode::{
    HIDE_CURSOR, RESET_MODE_ALT_SCREEN_SAVE_CURSOR, RESET_MODE_BRACKETED_PASTE,
    RESET_MODE_SYNCHRONIZED_OUTPUT, SET_MODE_ALT_SCREEN_SAVE_CURSOR, SET_MODE_BRACKETED_PASTE,
    SET_MODE_SYNCHRONIZED_OUTPUT, SHOW_CURSOR,
};
use rusty_x_ansi::progress::RESET_PROGRESS_BAR;
use std::any::Any;
use std::io::{self, Write};
use std::rc::Rc;

/// DECST8C: reset terminal tab stops to every 8 columns (upstream
/// `ansi.SetTabEvery8Columns`).
const SET_TAB_EVERY_8_COLUMNS: &str = "\x1b[?5W";

/// The color profile used for downsampling colors.
///
/// This is `rusty_colorprofile::Profile`; the upstream ultraviolet uses
/// the colorprofile package directly.
pub type ColorProfile = rusty_colorprofile::Profile;

/// TerminalRenderer is the internal interface of the terminal output
/// renderer.
///
/// NOTE: upstream this is `*TerminalRenderer` (terminal_renderer.go), which
/// is NOT ported yet. The screen isolates it behind this trait; the real
/// implementation will live in `src/terminal_renderer.rs`. The upstream
/// renderer owns the output writer (the screen's buffer); here the buffer is
/// passed as `out: &mut Vec<u8>` at each call site so the trait can be
/// boxed without a self-referential borrow.
pub(crate) trait TerminalRenderer {
    /// SetFullscreen sets whether the renderer is in fullscreen mode.
    fn set_fullscreen(&mut self, fullscreen: bool);
    /// SetRelativeCursor sets whether the renderer uses relative cursor
    /// movements.
    fn set_relative_cursor(&mut self, relative: bool);
    /// SetColorProfile sets the color profile of the renderer.
    fn set_color_profile(&mut self, profile: ColorProfile);
    /// SetLogger sets the logger of the renderer.
    fn set_logger(&mut self, logger: Option<Box<dyn crate::logger::Logger>>);
    /// SetTabStops sets the tab stop width of the renderer. A negative value
    /// disables tab stops.
    fn set_tab_stops(&mut self, every: i32);
    /// SetBackspace sets whether the renderer may use backspace movements.
    fn set_backspace(&mut self, backspace: bool);
    /// SetMapNewline sets whether the renderer maps newlines to CRLF.
    fn set_map_newline(&mut self, map: bool);
    /// SetWidthMethod sets the width method used by the renderer.
    fn set_width_method(&mut self, method: WidthMethod);
    /// SetGraphemeWidth sets whether the renderer measures grapheme width.
    fn set_grapheme_width(&mut self, grapheme: bool);
    /// Resize updates the renderer's terminal tab stops for the new width.
    fn resize(&mut self, width: usize, height: usize);
    /// Erase flags the renderer to clear the screen on the next [render].
    fn erase(&mut self);
    /// Render renders the given render buffer into the output buffer.
    fn render(&mut self, rbuf: &mut RenderBuffer, out: &mut Vec<u8>);
    /// Flush flushes the renderer's pending output into the output buffer.
    fn flush(&mut self, out: &mut Vec<u8>) -> io::Result<()>;
    /// MoveTo queues a cursor move to the given position.
    fn move_to(&mut self, x: usize, y: usize, out: &mut Vec<u8>);
    /// Position returns the renderer's current cursor position.
    fn position(&self) -> (usize, usize);
    /// SaveCursor saves the current cursor position.
    fn save_cursor(&mut self);
    /// RestoreCursor restores the saved cursor position.
    fn restore_cursor(&mut self);
    /// SetPosition sets the logical cursor position of the renderer.
    fn set_position(&mut self, x: usize, y: usize);
}

/// TerminalScreen represents a terminal screen, providing methods for
/// managing the screen state and rendering content.
pub struct TerminalScreen {
    win: Rc<Window>,
    w: Box<dyn Write>,
    buf: Vec<u8>,
    rend: Box<dyn TerminalRenderer>,
    rbuf: RenderBuffer,
    env: Environ,
    profile: ColorProfile,

    // Terminal state
    alt_screen: bool,
    keyboard_enhancements: Option<KeyboardEnhancements>,
    bracketed_paste: bool,
    mouse_mode: MouseMode,
    mouse_encoding: MouseEncoding,
    cursor: Option<Cursor>, // initial state is cursor hidden
    /// Whether the cursor position has been explicitly set. Mirrors the
    /// upstream `-1` position sentinel: a cursor created by
    /// [TerminalScreen::show_cursor], [TerminalScreen::set_cursor_style], or
    /// [TerminalScreen::set_cursor_color] has no known position.
    cursor_position_known: bool,
    background_color: Option<rusty_x_ansi::color::RGBColor>,
    foreground_color: Option<rusty_x_ansi::color::RGBColor>,
    progress_bar: Option<ProgressBar>,
    window_title: String,
    sync_updates: bool, // mode 2026
    reset_tabs: bool,   // DECST8C - reset terminal tabs on start
    /// Whether the application explicitly called
    /// [TerminalScreen::set_width_method]; suppresses the automatic mode-2027
    /// width-method negotiation.
    width_method_override: bool,
}

/// NewTerminalRenderer creates a new terminal renderer for the given
/// environment.
///
/// NOTE: upstream `NewTerminalRenderer(w, env)` also takes the output writer;
/// here the output buffer is passed to each trait method instead.
pub(crate) fn new_terminal_renderer(env: &Environ) -> Box<dyn TerminalRenderer> {
    crate::terminal_renderer::new_terminal_renderer(env)
}

/// NewTerminalScreen creates a new [TerminalScreen] with the given writer and
/// environment.
///
/// NOTE: upstream guards `Render`/`Flush` with a mutex for concurrent use
/// from the terminal event loop goroutine. In the port the methods take
/// `&mut self`, so the exclusivity is enforced by the type system; concurrent
/// use requires an external `Mutex<TerminalScreen>`.
pub fn new_terminal_screen<W: Write + 'static>(w: W, env: Environ) -> TerminalScreen {
    let fd = writer_fd(&w);
    let rend = new_terminal_renderer(&env);
    let mut s = TerminalScreen {
        win: new_window(0, 0, None),
        w: Box::new(w),
        buf: Vec::new(),
        rend,
        rbuf: crate::new_render_buffer(0, 0),
        env,
        profile: ColorProfile::NoTty,
        alt_screen: false,
        keyboard_enhancements: None,
        bracketed_paste: false,
        mouse_mode: MouseMode::MouseModeNone,
        mouse_encoding: MouseEncoding::MouseEncodingLegacy,
        cursor: None,
        cursor_position_known: false,
        background_color: None,
        foreground_color: None,
        progress_bar: None,
        window_title: String::new(),
        sync_updates: false,
        reset_tabs: false,
        width_method_override: false,
    };
    s.profile = detect_color_profile(fd, &s.env);
    s.rend.set_fullscreen(false); // by default, we start in inline mode
    s.rend.set_relative_cursor(true); // by default, we start in inline mode
    s.rend.set_color_profile(s.profile);

    let debug_file = s.env.getenv("UV_DEBUG");
    if !debug_file.is_empty() {
        if let Ok(f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&debug_file)
        {
            s.rend
                .set_logger(Some(Box::new(crate::logger::FileLogger(f))));
        }
    }

    // Configure renderer optimizations based on console settings.
    if let Some(fd) = fd {
        // NOTE: upstream is `term.GetState(fd); if err == nil || isWindows`.
        // On Windows GetState errors are ignored because Windows supports
        // tabs and backspace by default; [terminal_movement_hints] mirrors
        // that on non-Unix.
        if let Some((use_tabs, use_bspace)) = terminal_movement_hints(fd) {
            if use_tabs {
                s.rend.set_tab_stops(0); // the width will be set after calling TerminalScreen::resize
            } else {
                s.rend.set_tab_stops(-1);
            }
            s.rend.set_backspace(use_bspace);
            s.reset_tabs = use_tabs;
        }
    }
    // XXX: Do we still need map nl to crlf handling in the renderer?
    s.rend.set_map_newline(false);
    s
}

/// Returns the file descriptor of the underlying writer when it is a
/// terminal-backed file, mirroring the upstream `w.(term.File)` type
/// assertion.
///
/// NOTE: only [FdFile] and `Box<dyn File>` are detected; other wrapper types
/// can be added here as the console port grows.
fn writer_fd<W: Write + 'static>(w: &W) -> Option<i32> {
    let any: &dyn Any = w;
    if let Some(f) = any.downcast_ref::<FdFile>() {
        return Some(<FdFile as File>::fd(f) as i32);
    }
    if let Some(f) = any.downcast_ref::<Box<dyn File>>() {
        return Some(f.fd() as i32);
    }
    None
}

/// Mirrors `term.GetState` + `optimizeMovements` upstream: returns
/// `(useTabs, useBspace)` when the writer is a terminal.
///
/// `supportsHardTabs` is `oflag & TABDLY == TAB0` and `supportsBackspace` is
/// `lflag & BSDLY == BS0` (terminal_tabdly.go / terminal_bsdly.go).
#[cfg(unix)]
fn terminal_movement_hints(fd: i32) -> Option<(bool, bool)> {
    let mut t: libc::termios = unsafe { std::mem::zeroed() };
    if unsafe { libc::tcgetattr(fd, &mut t) } != 0 {
        return None;
    }
    Some((
        t.c_oflag & libc::TABDLY == libc::TAB0,
        t.c_lflag & libc::BSDLY == libc::BS0,
    ))
}

/// Non-Unix platforms mirror the upstream Windows path, where tabs and
/// backspace are supported by default and `GetState` errors are ignored.
#[cfg(not(unix))]
fn terminal_movement_hints(_fd: i32) -> Option<(bool, bool)> {
    Some((true, true))
}

/// Returns whether the file descriptor refers to a terminal, mirroring
/// `term.IsTerminal` (a successful `tcgetattr`).
#[cfg(unix)]
fn is_terminal(fd: i32) -> bool {
    let mut t: libc::termios = unsafe { std::mem::zeroed() };
    (unsafe { libc::tcgetattr(fd, &mut t) }) == 0
}

/// Non-Unix stub for [is_terminal].
#[cfg(not(unix))]
fn is_terminal(_fd: i32) -> bool {
    false
}

/// Returns whether `TTY_FORCE` is set in the environment (upstream
/// `isTTYForced`).
fn is_tty_forced(env: &Environ) -> bool {
    parse_bool(&env.getenv("TTY_FORCE"))
}

/// Mirrors Go's `strconv.ParseBool`: accepts 1, t, T, TRUE, true, True.
fn parse_bool(v: &str) -> bool {
    matches!(v, "1" | "t" | "T" | "TRUE" | "true" | "True")
}

/// DetectColorProfile detects the terminal color profile from the given
/// output file descriptor and environment (upstream
/// `colorprofile.Detect`).
pub fn detect_color_profile(fd: Option<i32>, env: &Environ) -> ColorProfile {
    let isatty = is_tty_forced(env) || fd.is_some_and(is_terminal);
    rusty_colorprofile::detect(isatty, &env.0)
}

/// BufferWrite lets the screen's byte output buffer use the `push_str`
/// ergonomics of the Go `bytes.Buffer` it ports.
trait BufferWrite {
    fn push_str(&mut self, s: &str);
    fn push_char(&mut self, c: char);
}

impl BufferWrite for Vec<u8> {
    fn push_str(&mut self, s: &str) {
        self.extend_from_slice(s.as_bytes());
    }
    fn push_char(&mut self, c: char) {
        let mut b = [0u8; 4];
        self.extend_from_slice(c.encode_utf8(&mut b).as_bytes());
    }
}

/// EncodeKeyboardEnhancements encodes the keyboard enhancements to the given
/// writer (upstream `uv.go`).
///
/// NOTE: upstream writes to an `io.Writer`; the port writes to the screen's
/// byte output buffer.
fn encode_keyboard_enhancements(out: &mut Vec<u8>, ke: Option<&KeyboardEnhancements>) {
    let flags = ke.map(|k| k.flags()).unwrap_or(0);
    out.push_str(&kitty_keyboard(flags as u8, 1));
}

/// EncodeWindowTitle encodes the window title to the given writer (upstream
/// `uv.go`); `\x1b]2;<title>\x07`.
fn encode_window_title(out: &mut Vec<u8>, title: &str) {
    out.push_str("\x1b]2;");
    out.push_str(title);
    out.push_char('\x07');
}

/// InsertLine returns a sequence to insert n lines at the current cursor
/// position (upstream `ansi.InsertLine`); `CSI Pn L`.
fn insert_line(n: i32) -> String {
    let mut s = String::from("\x1b[");
    if n > 1 {
        s.push_str(&n.to_string());
    }
    s.push('L');
    s
}

impl TerminalScreen {
    /// CellAt returns the cell at the specified x and y coordinates.
    pub fn cell_at(&self, x: usize, y: usize) -> Option<Cell> {
        self.win.cell_at(x, y)
    }

    /// SetCell sets the cell at the specified x and y coordinates.
    pub fn set_cell(&self, x: usize, y: usize, cell: Option<&Cell>) {
        let cell = cell.cloned().unwrap_or_else(empty_cell);
        self.win.set_cell(x, y, cell);
    }

    /// Bounds returns the bounds of the terminal screen as a rectangle.
    pub fn bounds(&self) -> Rectangle {
        self.win.bounds()
    }

    /// Width returns the width of the terminal screen.
    ///
    /// Note that this is not the actual width of the terminal window, but
    /// rather the width of the screen we're managing. The actual width of the
    /// terminal window can be obtained using `Terminal::get_size` or by
    /// reading the "COLUMNS" environment variable.
    pub fn width(&self) -> usize {
        self.win.bounds().dx()
    }

    /// Height returns the height of the terminal screen.
    ///
    /// Note that this is not the actual height of the terminal window, but
    /// rather the height of the screen we're managing. The actual height of
    /// the terminal window can be obtained using `Terminal::get_size` or by
    /// reading the "LINES" environment variable.
    pub fn height(&self) -> usize {
        self.win.bounds().dy()
    }

    /// StringWidth returns the cell width of the given string using the
    /// terminal screen's width method. This accounts for the configured
    /// [WidthMethod] (e.g. wcwidth vs grapheme width) so callers don't need
    /// to import ansi directly.
    pub fn string_width(&self, str: &str) -> usize {
        self.win.width_method().string_width(str)
    }

    /// WidthMethod returns the width method used by the terminal screen.
    pub fn width_method(&self) -> WidthMethod {
        self.win.width_method()
    }

    /// SetWidthMethod sets the width method for the terminal screen. This is
    /// an override that propagates to the window/buffer and the renderer so
    /// that all width measurements use the same method. Calling this marks
    /// the width method as explicitly overridden, which disables automatic
    /// mode-2027 negotiation.
    pub fn set_width_method(&mut self, method: WidthMethod) {
        self.width_method_override = true;
        self.set_width_method_internal(method);
    }

    /// setWidthMethod propagates the width method to the window/buffer and
    /// the renderer.
    ///
    /// NOTE: the ported [Window]'s setter takes `&mut self`, so the method
    /// applies through `Rc::get_mut`; if the window is aliased (a view was
    /// created) the window is left untouched.
    fn set_width_method_internal(&mut self, method: WidthMethod) {
        if let Some(win) = Rc::get_mut(&mut self.win) {
            win.set_width_method(method);
        }
        self.rend.set_width_method(method);
    }

    /// RequestGraphemeWidth queues a DECRQM request for Unicode core mode (DEC
    /// mode 2027) so the terminal reports whether it measures cell width using
    /// grapheme clustering. The response arrives as a mode report event.
    pub fn request_grapheme_width(&mut self) {
        self.buf
            .push_str(rusty_x_ansi::mode::REQUEST_MODE_UNICODE_CORE);
    }

    /// EnableGraphemeWidth enables Unicode core mode (DEC mode 2027) on the
    /// terminal and switches the screen's width method to
    /// [WidthMethod::GraphemeWidth] so wide-glyph measurement matches the
    /// terminal. The change propagates to the window/buffer and renderer.
    ///
    /// If the application has explicitly set a width method via
    /// [TerminalScreen::set_width_method], the explicit choice is preserved
    /// and this is a no-op. The change is committed to the underlying writer
    /// through the screen's normal write path so it cannot race with
    /// render/flush.
    pub fn enable_grapheme_width(&mut self) {
        if self.width_method_override {
            return;
        }
        self.buf
            .push_str(rusty_x_ansi::mode::SET_MODE_UNICODE_CORE);
        self.set_width_method_internal(WidthMethod::GraphemeWidth);
        self.rend.set_grapheme_width(true);
        let _ = self.flush();
    }

    /// Applies the width-method switch of [TerminalScreen::enable_grapheme_width]
    /// without writing or flushing. Used when the SET_MODE_UNICODE_CORE
    /// sequence was already written by the terminal's event loop (the screen
    /// itself lives on the application thread).
    pub fn set_grapheme_width_enabled(&mut self) {
        if self.width_method_override {
            return;
        }
        self.set_width_method_internal(WidthMethod::GraphemeWidth);
        self.rend.set_grapheme_width(true);
    }

    /// SetColorProfile sets the color profile for the terminal screen. This
    /// is automatically detected when creating the terminal screen. However,
    /// you can override it using this method.
    pub fn set_color_profile(&mut self, profile: ColorProfile) {
        self.profile = profile;
        self.rend.set_color_profile(profile);
    }

    /// Resize resizes the terminal screen to the specified width and height,
    /// updating the render buffer and renderer accordingly.
    ///
    /// NOTE: the ported [Window]'s resize takes `&mut self`, so it applies
    /// through `Rc::get_mut`; if the window is aliased (a view was created)
    /// the window is left untouched.
    pub fn resize(&mut self, width: usize, height: usize) {
        if let Some(win) = Rc::get_mut(&mut self.win) {
            win.resize(width, height);
        }
        // NOTE: the ported [RenderBuffer] has no resize method; the buffer is
        // resized and the touched markers are cleared directly.
        self.rbuf.buffer.resize(width, height);
        self.rbuf.touched = vec![None; height];
        self.rend.resize(width, height);
        self.rend.erase();
    }

    /// Display clears the screen and draws the given [Drawable] onto the
    /// terminal screen and flushes the changes to the underlying writer.
    ///
    /// This is a convenience method that combines [TerminalScreen::render]
    /// and [TerminalScreen::flush].
    pub fn display(&mut self, d: Option<&mut dyn Drawable>) -> io::Result<()> {
        if let Some(d) = d {
            // NOTE: the ported [Window] has no clear method; the buffer is
            // cleared through set_cell.
            clear_window(&self.win);
            let bounds = self.win.bounds();
            d.draw(&mut *self, bounds);
        }
        self.render();
        self.flush()
    }

    /// Render renders changes that transform the screen from its current
    /// state to the state represented by the [TerminalScreen].
    ///
    /// The changes can be committed to the underlying writer by calling the
    /// [TerminalScreen::flush] method.
    ///
    /// NOTE: the actual diffing (cursor moves, transformLine, wide-cell
    /// placeholder handling, touched lines) is performed by the
    /// TerminalRenderer; the screen only copies the window's non-zero
    /// cells into the render buffer, skipping wide-cell continuation columns
    /// by advancing the column step by the cell width.
    pub fn render(&mut self) {
        let w = self.width();
        let h = self.height();
        for y in 0..h {
            let mut x = 0;
            while x < w {
                let cell = self.win.cell_at(x, y);
                match cell {
                    None => {
                        x += 1;
                        continue;
                    }
                    Some(c) if c.is_zero() => {
                        x += 1;
                        continue;
                    }
                    Some(c) => {
                        self.rbuf.set_cell(x, y, Some(&c));
                        let mut width = c.width;
                        if width == 0 {
                            width = 1;
                        }
                        x += width;
                    }
                }
            }
        }
        self.rend.render(&mut self.rbuf, &mut self.buf);
        let _ = self.rend.flush(&mut self.buf);
    }

    /// Flush writes any pending output to the underlying writer.
    pub fn flush(&mut self) -> io::Result<()> {
        let move_cursor = match &self.cursor {
            Some(c) => !c.hidden && self.cursor_position_known,
            None => false,
        };
        if move_cursor {
            let c = self.cursor.as_ref().unwrap();
            self.rend.move_to(c.position.x, c.position.y, &mut self.buf);
        } else if !self.alt_screen {
            // We don't want the cursor to be dangling at the end of the line
            // in inline mode because it can cause unwanted line wraps in some
            // terminals. So we move it to the beginning of the next line if
            // necessary.
            // This is only needed when the cursor is hidden because when it's
            // visible, we already set its position above.
            let (x, y) = self.rend.position();
            if x >= self.width().saturating_sub(1) {
                self.rend.move_to(0, y, &mut self.buf);
            }
        }

        let mut buf: Vec<u8> = Vec::new();

        if !self.buf.is_empty() {
            if self.sync_updates {
                // If synchronized updates are enabled, we need to wrap the
                // output in the appropriate control sequences to ensure that
                // the terminal treats it as a single atomic update. This is
                // necessary to prevent flickering and other visual artifacts
                // that can occur when multiple updates are sent separately.
                buf.push_str(SET_MODE_SYNCHRONIZED_OUTPUT);
            } else if let Some(c) = &self.cursor {
                if !c.hidden {
                    // If synchronized updates are not enabled, we need to
                    // ensure that the cursor is hidden before writing any
                    // output to prevent unwanted cursor visual artifacts.
                    buf.push_str(HIDE_CURSOR);
                }
            }

            buf.extend_from_slice(&self.buf);

            if self.sync_updates {
                buf.push_str(RESET_MODE_SYNCHRONIZED_OUTPUT);
            } else if let Some(c) = &self.cursor {
                if !c.hidden {
                    buf.push_str(SHOW_CURSOR);
                }
            }
        }

        self.w.write_all(&buf)?;
        self.buf.clear();
        Ok(())
    }

    /// EnterAltScreen switches the terminal to the alternate screen buffer,
    /// allowing applications to use a separate screen for their output
    /// without affecting the main screen.
    ///
    /// The changes can be committed to the underlying writer by calling the
    /// [TerminalScreen::flush] method.
    pub fn enter_alt_screen(&mut self) {
        let mut sb: Vec<u8> = Vec::new();
        sb.push_str(SET_MODE_ALT_SCREEN_SAVE_CURSOR);
        if self.cursor.is_none() || self.cursor.as_ref().is_some_and(|c| c.hidden) {
            sb.push_str(HIDE_CURSOR);
        } else if self.cursor.as_ref().is_some_and(|c| !c.hidden) {
            sb.push_str(SHOW_CURSOR);
        }
        if self.keyboard_enhancements.is_some() {
            encode_keyboard_enhancements(&mut sb, self.keyboard_enhancements.as_ref());
        }
        self.buf.extend_from_slice(&sb);

        if !self.alt_screen {
            self.rend.save_cursor();
            self.rend.erase();
            self.rend.set_fullscreen(true);
            self.rend.set_relative_cursor(false);
            self.alt_screen = true;
        }
    }

    /// ExitAltScreen switches the terminal back to the main screen buffer,
    /// restoring the previous screen state.
    ///
    /// The changes can be committed to the underlying writer by calling the
    /// [TerminalScreen::flush] method.
    pub fn exit_alt_screen(&mut self) {
        let mut sb: Vec<u8> = Vec::new();
        sb.push_str(RESET_MODE_ALT_SCREEN_SAVE_CURSOR);
        if self.cursor.is_none() || self.cursor.as_ref().is_some_and(|c| c.hidden) {
            sb.push_str(HIDE_CURSOR);
        } else if self.cursor.as_ref().is_some_and(|c| !c.hidden) {
            sb.push_str(SHOW_CURSOR);
        }
        if self.keyboard_enhancements.is_some() {
            encode_keyboard_enhancements(&mut sb, self.keyboard_enhancements.as_ref());
        }
        self.buf.extend_from_slice(&sb);

        if self.alt_screen {
            self.rend.restore_cursor();
            self.rend.erase();
            self.rend.set_fullscreen(false);
            self.rend.set_relative_cursor(true);
            self.alt_screen = false;
        }
    }

    /// AltScreen returns whether the terminal is currently in the alternate
    /// screen buffer.
    pub fn alt_screen(&self) -> bool {
        self.alt_screen
    }

    /// HideCursor hides the terminal cursor.
    ///
    /// The changes can be committed to the underlying writer by calling the
    /// [TerminalScreen::flush] method.
    pub fn hide_cursor(&mut self) {
        self.buf.push_str(HIDE_CURSOR);
        if let Some(c) = &mut self.cursor {
            c.hidden = true;
        }
    }

    /// ShowCursor shows the terminal cursor.
    ///
    /// The changes can be committed to the underlying writer by calling the
    /// [TerminalScreen::flush] method.
    pub fn show_cursor(&mut self) {
        self.buf.push_str(SHOW_CURSOR);
        match &mut self.cursor {
            Some(c) => {
                c.hidden = false;
            }
            None => {
                self.cursor = Some(new_cursor(0, 0));
                // NOTE: upstream creates the cursor at (-1, -1); the usize
                // position has no such sentinel, so a separate flag tracks
                // that the position is unknown.
                self.cursor_position_known = false;
            }
        }
    }

    /// CursorVisible returns whether the terminal cursor is currently visible.
    pub fn cursor_visible(&self) -> bool {
        self.cursor.as_ref().is_some_and(|c| !c.hidden)
    }

    /// SetCursorPosition sets the position of the terminal cursor to the
    /// specified coordinates.
    ///
    /// The changes can be committed to the underlying writer by calling the
    /// [TerminalScreen::flush] method.
    pub fn set_cursor_position(&mut self, x: usize, y: usize) {
        match &mut self.cursor {
            Some(c) => {
                c.position.x = x;
                c.position.y = y;
            }
            None => {
                self.cursor = Some(new_cursor(x, y));
                self.cursor.as_mut().unwrap().hidden = true;
            }
        }
        self.cursor_position_known = true;
    }

    /// CursorPosition returns the last set cursor position of the terminal.
    /// If the cursor position is not set, it returns None.
    ///
    /// This can be affected by [TerminalScreen::render] and
    /// [TerminalScreen::set_cursor_position] calls.
    ///
    /// NOTE: upstream returns (-1, -1) when the position is unset; the port
    /// returns None instead.
    pub fn cursor_position(&self) -> Option<(usize, usize)> {
        if !self.cursor_position_known {
            return None;
        }
        self.cursor.as_ref().map(|c| (c.position.x, c.position.y))
    }

    /// SetCursorStyle sets the style of the terminal cursor.
    ///
    /// The changes can be committed to the underlying writer by calling the
    /// [TerminalScreen::flush] method.
    pub fn set_cursor_style(&mut self, shape: CursorShape, blink: bool) {
        let _ = crate::encode_cursor_style(&mut self.buf, shape, blink);
        if self.cursor.is_none() {
            self.cursor = Some(new_cursor(0, 0));
            self.cursor_position_known = false;
        }
        if let Some(c) = &mut self.cursor {
            c.shape = shape;
            c.blink = blink;
        }
    }

    /// CursorStyle returns the current style of the terminal cursor.
    pub fn cursor_style(&self) -> (CursorShape, bool) {
        match &self.cursor {
            Some(c) => (c.shape, c.blink),
            None => (CursorShape::CursorBlock, true),
        }
    }

    /// SetCursorColor sets the color of the terminal cursor.
    ///
    /// The changes can be committed to the underlying writer by calling the
    /// [TerminalScreen::flush] method.
    pub fn set_cursor_color(&mut self, c: Option<rusty_x_ansi::color::RGBColor>) {
        let _ = crate::encode_cursor_color(&mut self.buf, c.as_ref());
        if self.cursor.is_none() {
            self.cursor = Some(new_cursor(0, 0));
            self.cursor_position_known = false;
        }
        if let Some(cur) = &mut self.cursor {
            cur.color = c;
        }
    }

    /// CursorColor returns the current color of the terminal cursor.
    ///
    /// A None color indicates that the cursor color is the default terminal
    /// cursor color.
    pub fn cursor_color(&self) -> Option<&rusty_x_ansi::color::RGBColor> {
        self.cursor.as_ref().and_then(|c| c.color.as_ref())
    }

    /// SetBackgroundColor sets the background color of the terminal.
    ///
    /// The changes can be committed to the underlying writer by calling the
    /// [TerminalScreen::flush] method.
    pub fn set_background_color(&mut self, c: Option<rusty_x_ansi::color::RGBColor>) {
        let _ = crate::encode_background_color(&mut self.buf, c.as_ref());
        self.background_color = c;
    }

    /// BackgroundColor returns the current background color of the terminal.
    ///
    /// A None color indicates that the background color is the default
    /// terminal background color.
    pub fn background_color(&self) -> Option<&rusty_x_ansi::color::RGBColor> {
        self.background_color.as_ref()
    }

    /// SetForegroundColor sets the foreground color of the terminal.
    ///
    /// The changes can be committed to the underlying writer by calling the
    /// [TerminalScreen::flush] method.
    pub fn set_foreground_color(&mut self, c: Option<rusty_x_ansi::color::RGBColor>) {
        let _ = crate::encode_foreground_color(&mut self.buf, c.as_ref());
        self.foreground_color = c;
    }

    /// ForegroundColor returns the current foreground color of the terminal.
    ///
    /// A None color indicates that the foreground color is the default
    /// terminal foreground color.
    pub fn foreground_color(&self) -> Option<&rusty_x_ansi::color::RGBColor> {
        self.foreground_color.as_ref()
    }

    /// EnableBracketedPaste enables bracketed paste mode, allowing the
    /// terminal to distinguish between pasted content and user input.
    ///
    /// The changes can be committed to the underlying writer by calling the
    /// [TerminalScreen::flush] method.
    pub fn enable_bracketed_paste(&mut self) {
        self.buf.push_str(SET_MODE_BRACKETED_PASTE);
        self.bracketed_paste = true;
    }

    /// DisableBracketedPaste disables bracketed paste mode.
    ///
    /// The changes can be committed to the underlying writer by calling the
    /// [TerminalScreen::flush] method.
    pub fn disable_bracketed_paste(&mut self) {
        self.buf.push_str(RESET_MODE_BRACKETED_PASTE);
        self.bracketed_paste = false;
    }

    /// BracketedPaste returns whether bracketed paste mode is currently
    /// enabled.
    pub fn bracketed_paste(&self) -> bool {
        self.bracketed_paste
    }

    /// SetSynchronizedUpdates sets whether to use synchronized updates (mode
    /// 2026), which allows applications to batch updates to the terminal
    /// screen and flush them all at once for improved performance.
    ///
    /// The changes can be committed to the underlying writer by calling the
    /// [TerminalScreen::flush] method.
    pub fn set_synchronized_updates(&mut self, enabled: bool) {
        self.sync_updates = enabled;
    }

    /// SynchronizedUpdates returns whether synchronized updates (mode 2026)
    /// are currently enabled.
    pub fn synchronized_updates(&self) -> bool {
        self.sync_updates
    }

    /// SetMouseMode sets the mouse tracking mode for the terminal, allowing
    /// applications to receive mouse events.
    ///
    /// The changes can be committed to the underlying writer by calling the
    /// [TerminalScreen::flush] method.
    pub fn set_mouse_mode(&mut self, mode: MouseMode) {
        let _ = crate::encode_mouse_mode(&mut self.buf, mode);
        self.mouse_mode = mode;
    }

    /// MouseMode returns the current mouse tracking mode of the terminal.
    pub fn mouse_mode(&self) -> MouseMode {
        self.mouse_mode
    }

    /// SetMouseEncoding sets the mouse encoding for the terminal. The
    /// encoding determines how mouse coordinates and buttons are reported.
    /// This is only meaningful when mouse tracking is enabled via
    /// [TerminalScreen::set_mouse_mode].
    ///
    /// The changes can be committed to the underlying writer by calling the
    /// [TerminalScreen::flush] method.
    pub fn set_mouse_encoding(&mut self, enc: MouseEncoding) {
        let _ = crate::encode_mouse_encoding(&mut self.buf, enc);
        self.mouse_encoding = enc;
    }

    /// MouseEncoding returns the current mouse encoding of the terminal.
    pub fn mouse_encoding(&self) -> MouseEncoding {
        self.mouse_encoding
    }

    /// SetWindowTitle sets the title of the terminal window.
    ///
    /// The changes can be committed to the underlying writer by calling the
    /// [TerminalScreen::flush] method.
    pub fn set_window_title(&mut self, title: &str) {
        encode_window_title(&mut self.buf, title);
        self.window_title = title.to_string();
    }

    /// WindowTitle returns the current title of the terminal window.
    pub fn window_title(&self) -> &str {
        &self.window_title
    }

    /// SetKeyboardEnhancements sets the keyboard enhancements for the
    /// terminal, allowing applications to receive enhanced keyboard input.
    ///
    /// The changes can be committed to the underlying writer by calling the
    /// [TerminalScreen::flush] method.
    pub fn set_keyboard_enhancements(&mut self, enh: Option<KeyboardEnhancements>) {
        encode_keyboard_enhancements(&mut self.buf, enh.as_ref());
        self.keyboard_enhancements = enh;
    }

    /// KeyboardEnhancements returns the current keyboard enhancements of the
    /// terminal.
    ///
    /// A None value indicates that no keyboard enhancements are currently
    /// enabled.
    pub fn keyboard_enhancements(&self) -> Option<&KeyboardEnhancements> {
        self.keyboard_enhancements.as_ref()
    }

    /// SetProgressBar sets the progress bar for the terminal, allowing
    /// applications to display progress information.
    ///
    /// The changes can be committed to the underlying writer by calling the
    /// [TerminalScreen::flush] method.
    pub fn set_progress_bar(&mut self, pb: Option<ProgressBar>) {
        let _ = crate::encode_progress_bar(&mut self.buf, pb.as_ref());
        self.progress_bar = pb;
    }

    /// ProgressBar returns the current progress bar of the terminal.
    ///
    /// A None value indicates that no progress bar is currently set.
    pub fn progress_bar(&self) -> Option<&ProgressBar> {
        self.progress_bar.as_ref()
    }

    /// Reset resets the terminal screen to its default state, clearing the
    /// screen, switching back to the main screen buffer if necessary, and
    /// resetting all terminal settings to their defaults.
    ///
    /// The changes can be committed to the underlying writer by calling the
    /// [TerminalScreen::flush] method.
    pub fn reset(&mut self) {
        let mut sb: Vec<u8> = Vec::new();

        let has_keyboard_enhancements = self.keyboard_enhancements.is_some();

        if self.alt_screen {
            if has_keyboard_enhancements {
                sb.push_str(&kitty_keyboard(0, 1));
            }
            sb.push_str(RESET_MODE_ALT_SCREEN_SAVE_CURSOR);
        }
        if has_keyboard_enhancements {
            sb.push_str(&kitty_keyboard(0, 1));
        }
        if self.mouse_mode != MouseMode::MouseModeNone {
            let _ = crate::encode_mouse_mode(&mut sb, MouseMode::MouseModeNone);
        }
        if self.mouse_encoding != MouseEncoding::MouseEncodingLegacy {
            let _ = crate::encode_mouse_encoding(&mut sb, MouseEncoding::MouseEncodingLegacy);
        }

        if self.cursor.is_none() || !self.cursor.as_ref().unwrap().hidden {
            sb.push_str(SHOW_CURSOR);
        }
        if let Some(c) = &self.cursor {
            if c.shape != CursorShape::CursorBlock || !c.blink {
                sb.push_str(&set_cursor_style(0));
            }
            if c.color.is_some() {
                sb.push_str(RESET_CURSOR_COLOR);
            }
        }
        if self.background_color.is_some() {
            sb.push_str(RESET_BACKGROUND_COLOR);
        }
        if self.foreground_color.is_some() {
            sb.push_str(RESET_FOREGROUND_COLOR);
        }
        if self.bracketed_paste {
            sb.push_str(RESET_MODE_BRACKETED_PASTE);
        }
        if !self.window_title.is_empty() {
            encode_window_title(&mut sb, "");
        }
        if let Some(pb) = &self.progress_bar {
            if pb.state != ProgressBarState::ProgressBarNone {
                sb.push_str(RESET_PROGRESS_BAR);
            }
        }

        self.buf.extend_from_slice(&sb);

        // Go to the bottom of the screen.
        // We need to go to the bottom of the screen regardless of whether
        // we're in alt screen mode or not to avoid leaving the cursor in the
        // middle in terminals that don't support alt screen mode.
        //
        // This comes after resetting the screen state to ensure that moving
        // the cursor is the last thing we do, preventing any unwanted cursor
        // movements after resetting the screen.
        //
        // Note that both the renderer and the screen write to the same output
        // buffer.
        self.rend
            .move_to(0, self.height().saturating_sub(1), &mut self.buf);
    }

    /// Restore restores the terminal screen to its previous state, applying
    /// any previous settings and state that were reset by the
    /// [TerminalScreen::reset] method.
    ///
    /// The changes can be committed to the underlying writer by calling the
    /// [TerminalScreen::flush] method.
    pub fn restore(&mut self) {
        let mut sb: Vec<u8> = Vec::new();

        if self.reset_tabs {
            sb.push_str(SET_TAB_EVERY_8_COLUMNS);
        }
        if self.alt_screen {
            sb.push_str(SET_MODE_ALT_SCREEN_SAVE_CURSOR);
        }
        if self.cursor.as_ref().is_some_and(|c| !c.hidden) {
            sb.push_str(SHOW_CURSOR);
        } else {
            // Hide the cursor by default.
            sb.push_str(HIDE_CURSOR);
        }
        if let Some(ke) = &self.keyboard_enhancements {
            encode_keyboard_enhancements(&mut sb, Some(ke));
        }
        if self.mouse_mode != MouseMode::MouseModeNone {
            let _ = crate::encode_mouse_mode(&mut sb, self.mouse_mode);
        }
        if self.mouse_encoding != MouseEncoding::MouseEncodingLegacy {
            let _ = crate::encode_mouse_encoding(&mut sb, self.mouse_encoding);
        }
        if let Some(c) = &self.cursor {
            if c.shape != CursorShape::CursorBlock || !c.blink {
                let _ = crate::encode_cursor_style(&mut sb, c.shape, c.blink);
            }
            if c.color.is_some() {
                let _ = crate::encode_cursor_color(&mut sb, c.color.as_ref());
            }
        }
        if let Some(c) = &self.background_color {
            let _ = crate::encode_background_color(&mut sb, Some(c));
        }
        if let Some(c) = &self.foreground_color {
            let _ = crate::encode_foreground_color(&mut sb, Some(c));
        }
        if self.bracketed_paste {
            sb.push_str(SET_MODE_BRACKETED_PASTE);
        }
        if !self.window_title.is_empty() {
            encode_window_title(&mut sb, &self.window_title);
        }
        if let Some(pb) = &self.progress_bar {
            if pb.state != ProgressBarState::ProgressBarNone {
                let _ = crate::encode_progress_bar(&mut sb, Some(pb));
            }
        }

        self.buf.extend_from_slice(&sb);

        // This needs to be called after restoring the screen state and
        // writing to the buffer.
        //
        // [TerminalScreen::render] will write to the output buffer, so we
        // need to call it after writing the restore commands to the buffer to
        // ensure that the restore commands are included in the render output.
        // This ensures that the screen is properly restored before rendering
        // any changes.
        self.render();

        // Cursor position will be restored by the caller after calling
        // [TerminalScreen::flush].
    }

    /// Write writes data to the underlying buffer queuing it for output.
    ///
    /// The changes can be committed to the underlying writer by calling the
    /// [TerminalScreen::flush] method.
    ///
    /// NOTE: upstream returns `(n int, err error)` from a bytes.Buffer; the
    /// port returns the byte count only (a byte buffer never fails).
    pub fn write(&mut self, p: &[u8]) -> usize {
        self.buf.extend_from_slice(p);
        p.len()
    }

    /// WriteString writes a string to the underlying buffer queuing it for
    /// output.
    ///
    /// The changes can be committed to the underlying writer by calling the
    /// [TerminalScreen::flush] method.
    ///
    /// NOTE: upstream returns `(n int, err error)` from a bytes.Buffer; the
    /// port returns the byte count only (a byte buffer never fails).
    pub fn write_string(&mut self, str: &str) -> usize {
        self.buf.push_str(str);
        str.len()
    }

    /// InsertAbove inserts content above the screen pushing the current
    /// content down.
    ///
    /// This is useful for inserting content above the current screen content
    /// without affecting the current cursor position or screen state.
    ///
    /// Note that this won't have any visible effect if the screen is in alt
    /// screen mode, as the content will be inserted above the alt screen
    /// buffer, which is not visible. However, if the screen is in inline
    /// mode, the content will be inserted above and will not be managed by
    /// the renderer.
    ///
    /// Unlike other methods that modify the screen state, this method writes
    /// directly to the underlying writer, so there is no need to call
    /// [TerminalScreen::flush] after calling this method.
    pub fn insert_above(&mut self, content: &str) -> io::Result<()> {
        if content.is_empty() {
            return Ok(());
        }

        let mut sb: Vec<u8> = Vec::new();
        let w = self.width();
        let h = self.height();
        let (_, y) = self.rend.position();

        // We need to scroll the screen up by the number of lines in the
        // queue.
        sb.push_char('\r');
        let down = h.saturating_sub(y).saturating_sub(1);
        if down > 0 {
            sb.push_str(&cursor_down(down as i32));
        }

        let lines: Vec<&str> = content.split('\n').collect();
        let mut offset = lines.len();
        for line in &lines {
            let line_width = self.win.width_method().string_width(line);
            if w > 0 && line_width > w {
                offset += line_width / w;
            }
        }

        // Scroll the screen up by the offset to make room for the new lines.
        sb.push_str(&"\n".repeat(offset));

        // XXX: Now go to the top of the screen, insert new lines, and write
        // the queued strings. It is important to use moveCursor instead of
        // move because we don't want to perform any checks on the cursor
        // position.
        let up = offset + h - 1;
        sb.push_str(&cursor_up(up as i32));
        sb.push_str(&insert_line(offset as i32));
        for line in &lines {
            sb.push_str(line);
            sb.push_str("\x1b[K");
            sb.push_str("\r\n");
        }

        self.rend.set_position(0, 0);

        self.w.write_all(&sb)?;
        Ok(())
    }
}

/// Clears the window buffer with space cells, mirroring `Window.Clear`
/// upstream.
///
/// NOTE: the ported [Window] has no clear method; this writes an empty cell
/// to every position instead.
fn clear_window(win: &Rc<Window>) {
    let b = win.bounds();
    for y in b.min.1..b.max.1 {
        for x in b.min.0..b.max.0 {
            win.set_cell(x, y, empty_cell());
        }
    }
}

impl Screen for TerminalScreen {
    /// Bounds returns the bounds of the screen.
    fn bounds(&self) -> Rectangle {
        self.win.bounds()
    }

    /// CellAt returns the cell at the given position.
    ///
    /// NOTE: the window cells live behind an `Rc<RefCell<Buffer>>`, so a
    /// borrowed cell cannot be returned from this interface; None is
    /// returned, which all existing consumers treat as an empty cell (the
    /// same way upstream treats a nil cell).
    fn cell_at(&self, _x: usize, _y: usize) -> Option<&Cell> {
        None
    }

    /// SetCell sets the cell at the given position.
    fn set_cell(&mut self, x: usize, y: usize, c: Option<&Cell>) {
        TerminalScreen::set_cell(self, x, y, c);
    }

    /// WidthMethod returns the width method used by the screen.
    fn width_method(&self) -> WidthMethod {
        self.win.width_method()
    }

    /// Provides access to the underlying concrete type for downcasting.
    fn as_any_mut(&mut self) -> Option<&mut dyn Any> {
        Some(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::Line;
    use rusty_x_ansi::color::RGBColor;
    use rusty_x_ansi::cursor_position;

    /// A test renderer that writes markers into the output buffer to record
    /// which methods the screen drove.
    #[derive(Default)]
    struct RecordingRenderer {
        cur: (usize, usize),
    }

    impl TerminalRenderer for RecordingRenderer {
        fn set_fullscreen(&mut self, _fullscreen: bool) {}
        fn set_relative_cursor(&mut self, _relative: bool) {}
        fn set_color_profile(&mut self, _profile: ColorProfile) {}
        fn set_logger(&mut self, _logger: Option<Box<dyn crate::logger::Logger>>) {}
        fn set_tab_stops(&mut self, _every: i32) {}
        fn set_backspace(&mut self, _backspace: bool) {}
        fn set_map_newline(&mut self, _map: bool) {}
        fn set_width_method(&mut self, _method: WidthMethod) {}
        fn set_grapheme_width(&mut self, _grapheme: bool) {}
        fn resize(&mut self, _width: usize, _height: usize) {}
        fn erase(&mut self) {}
        fn render(&mut self, _rbuf: &mut RenderBuffer, out: &mut Vec<u8>) {
            out.push_char('R');
        }
        fn flush(&mut self, out: &mut Vec<u8>) -> io::Result<()> {
            out.push_char('F');
            Ok(())
        }
        fn move_to(&mut self, x: usize, y: usize, out: &mut Vec<u8>) {
            out.push_str(&cursor_position((x + 1) as i32, (y + 1) as i32));
            self.cur = (x, y);
        }
        fn position(&self) -> (usize, usize) {
            self.cur
        }
        fn save_cursor(&mut self) {}
        fn restore_cursor(&mut self) {}
        fn set_position(&mut self, x: usize, y: usize) {
            self.cur = (x, y);
        }
    }

    fn rgb(r: u8, g: u8, b: u8) -> RGBColor {
        RGBColor { r, g, b }
    }

    /// A writer that shares its buffer so tests can inspect the bytes the
    /// screen writes to the underlying writer.
    #[derive(Clone, Debug, Default)]
    struct SharedWriter(Rc<std::cell::RefCell<Vec<u8>>>);

    impl Write for SharedWriter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.borrow_mut().extend_from_slice(buf);
            Ok(buf.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    /// Creates a screen backed by a shared writer so tests can inspect the
    /// written bytes.
    fn test_screen() -> (TerminalScreen, Rc<std::cell::RefCell<Vec<u8>>>) {
        let w = Rc::new(std::cell::RefCell::new(Vec::new()));
        let s = new_terminal_screen(SharedWriter(w.clone()), Environ::default());
        (s, w)
    }

    fn test_screen_sized(
        width: usize,
        height: usize,
    ) -> (TerminalScreen, Rc<std::cell::RefCell<Vec<u8>>>) {
        let (mut s, w) = test_screen();
        s.resize(width, height);
        (s, w)
    }

    /// Port of `TestTransformLine_IsZeroInfiniteLoop` upstream
    /// (transformline_bug_test.go): the transformLine loop must re-read the
    /// next cell from the line inside the loop, or it spins forever on a
    /// trailing zero cell.
    #[test]
    fn test_transform_line_zero_skip() {
        let new_line = Line(vec![
            Cell {
                content: "世".to_string(),
                width: 2,
                ..Cell::default()
            }, // wide char at index 0
            Cell::default(), // zero cell at index 1 (trailing)
            Cell::default(), // zero cell at index 2
            Cell {
                content: "a".to_string(),
                width: 1,
                ..Cell::default()
            }, // normal char at index 3
        ]);

        let mut n = 0;
        let mut next = new_line.get(n + 1).cloned(); // reads trailing cell (zero)

        // The FIXED loop — updates `next` inside the loop.
        let mut iterations = 0;
        while let Some(c) = next {
            if !c.is_zero() {
                break;
            }
            n += 1;
            next = new_line.get(n + 1).cloned(); // fix: re-read from buffer
            iterations += 1;
            if iterations > 1000 {
                panic!("loop still hangs — fix did not work");
            }
        }

        assert_eq!(n, 2, "expected loop to skip 2 zero cells");
    }

    /// Port of the "unstyled wide cell" subtest of
    /// `TestStyledWideCellPlaceholderDetection` upstream
    /// (widecell_placeholder_test.go).
    #[test]
    fn test_wide_cell_placeholder_unstyled() {
        let mut l = Line(vec![Cell::default(); 5]);
        l.set(
            0,
            Cell {
                content: "你".to_string(),
                width: 2,
                ..Cell::default()
            },
        );

        let ph = &l[1];
        assert_eq!(ph.width, 0, "expected placeholder Width == 0");
        assert!(ph.is_zero(), "expected unstyled placeholder to be IsZero()");
    }

    /// Port of the "styled wide cell" subtest of
    /// `TestStyledWideCellPlaceholderDetection` upstream
    /// (widecell_placeholder_test.go).
    ///
    /// At this upstream pin, Line::set overwrites the continuation columns
    /// with plain zero cells, so the placeholder IsZero() check still
    /// agrees with the Width == 0 check and the test is skipped, mirroring
    /// the upstream `t.Skip`.
    #[test]
    fn test_wide_cell_placeholder_styled() {
        let mut l = Line(vec![Cell::default(); 5]);
        l.set(
            0,
            Cell {
                content: "你".to_string(),
                width: 2,
                style: crate::style::Style {
                    bg: Some(rusty_x_ansi::style::Color::Basic(
                        rusty_x_ansi::color::RED,
                    )),
                    ..Default::default()
                },
                ..Cell::default()
            },
        );

        let ph = &l[1];
        assert_eq!(ph.width, 0, "expected placeholder Width == 0");
        if ph.is_zero() {
            // Mirrors upstream: "placeholder is zero; PR #124 style
            // inheritance not present".
            return;
        }
        panic!("styled wide-cell placeholder has Width==0 but IsZero()==false");
    }

    #[test]
    fn test_new_screen_initial_state() {
        let (s, _) = test_screen();
        assert_eq!(s.width(), 0);
        assert_eq!(s.height(), 0);
        assert_eq!(
            s.bounds(),
            Rectangle {
                min: (0, 0),
                max: (0, 0)
            }
        );
        assert!(!s.alt_screen());
        assert!(!s.cursor_visible());
        assert_eq!(s.cursor_position(), None);
        assert_eq!(s.cursor_style(), (CursorShape::CursorBlock, true));
        assert!(s.cursor_color().is_none());
        assert_eq!(s.mouse_mode(), MouseMode::MouseModeNone);
        assert_eq!(s.mouse_encoding(), MouseEncoding::MouseEncodingLegacy);
        assert!(!s.bracketed_paste());
        assert!(!s.synchronized_updates());
        assert_eq!(s.window_title(), "");
        assert!(s.progress_bar().is_none());
        assert!(s.keyboard_enhancements().is_none());
        assert!(s.background_color().is_none());
        assert!(s.foreground_color().is_none());
    }

    /// The screen's Render must copy non-zero window cells into the render
    /// buffer, skipping wide-cell continuation columns by stepping over the
    /// cell width, and hand the buffer to the renderer.
    #[test]
    fn test_render_scans_window_into_render_buffer() {
        let (mut s, _) = test_screen_sized(10, 2);
        s.rend = Box::new(RecordingRenderer::default());
        s.set_cell(0, 0, Some(&Cell::new("a")));
        s.set_cell(
            2,
            0,
            Some(&Cell {
                content: "界".to_string(),
                width: 2,
                ..Cell::default()
            }),
        );
        s.set_cell(5, 1, Some(&Cell::new("b")));

        s.render();

        // Cells copied into the render buffer.
        assert_eq!(s.rbuf.cell_at(0, 0).unwrap().content, "a");
        assert_eq!(s.rbuf.cell_at(2, 0).unwrap().content, "界");
        // The wide-cell continuation column is skipped by the width step and
        // remains an untouched zero-width placeholder cell in the render
        // buffer (upstream marks it with Cell{}).
        assert_eq!(s.rbuf.cell_at(3, 0).unwrap().content, "");
        assert_eq!(s.rbuf.cell_at(3, 0).unwrap().width, 0);
        assert_eq!(s.rbuf.cell_at(5, 1).unwrap().content, "b");
        // Touched lines marked for both changed rows.
        assert!(s.rbuf.touched[0].is_some());
        assert!(s.rbuf.touched[1].is_some());

        // The renderer received the render buffer and was flushed, in order.
        assert_eq!(s.buf, b"RF");
    }

    #[test]
    fn test_flush_plain_output() {
        let (mut s, w) = test_screen();
        s.write_string("hello");
        s.flush().unwrap();
        assert_eq!(w.borrow().as_slice(), b"hello");
        assert!(s.buf.is_empty());
        // Nothing is written when the buffer is empty.
        w.borrow_mut().clear();
        s.flush().unwrap();
        assert!(w.borrow().is_empty());
    }

    #[test]
    fn test_flush_synchronized_updates() {
        let (mut s, w) = test_screen();
        s.set_synchronized_updates(true);
        s.write_string("x");
        s.flush().unwrap();
        assert_eq!(w.borrow().as_slice(), b"\x1b[?2026hx\x1b[?2026l");
    }

    #[test]
    fn test_flush_visible_cursor_sequence() {
        let (mut s, w) = test_screen();
        s.set_cursor_position(5, 3);
        s.show_cursor();
        s.write_string("hi");
        s.flush().unwrap();
        // The cursor move is queued into the renderer's own buffer and is
        // only written on the next Render (Go-verified: the screen flush
        // output contains no move); the buffer is wrapped in hide/show.
        assert_eq!(w.borrow().as_slice(), b"\x1b[?25l\x1b[?25hhi\x1b[?25h");
    }

    /// In inline mode with a hidden cursor, a dangling cursor at the last
    /// column is moved to the beginning of the line before flushing.
    #[test]
    fn test_flush_dangling_cursor_inline_mode() {
        let (mut s, w) = test_screen_sized(80, 24);
        s.rend.set_position(79, 0);
        s.write_string("x");
        s.flush().unwrap();
        // Go-verified: the dangling-cursor move goes to the renderer buffer,
        // invisible in the screen flush output.
        assert_eq!(w.borrow().as_slice(), b"x");

        // In alt screen mode the cursor is not moved.
        let (mut s2, w2) = test_screen_sized(80, 24);
        s2.rend.set_position(79, 0);
        s2.enter_alt_screen();
        s2.buf.clear();
        s2.write_string("x");
        s2.flush().unwrap();
        assert_eq!(w2.borrow().as_slice(), b"x");

        // A hidden cursor that exists (e.g. after show+hide) also gets the
        // dangling-cursor fix.
        let (mut s3, w3) = test_screen_sized(80, 24);
        s3.rend.set_position(79, 0);
        s3.show_cursor();
        s3.hide_cursor();
        s3.write_string("y");
        s3.flush().unwrap();
        assert_eq!(w3.borrow().as_slice(), b"\x1b[?25h\x1b[?25ly");
    }

    #[test]
    fn test_reset_output_sequences() {
        let (mut s, w) = test_screen_sized(10, 5);
        s.set_mouse_mode(MouseMode::MouseModeDrag);
        s.set_mouse_encoding(MouseEncoding::MouseEncodingSGR);
        s.enable_bracketed_paste();
        s.set_background_color(Some(rgb(1, 2, 3)));
        s.set_foreground_color(Some(rgb(4, 5, 6)));
        s.set_window_title("hello");
        s.set_progress_bar(Some(ProgressBar {
            state: ProgressBarState::ProgressBarDefault,
            value: 42,
        }));
        s.show_cursor();
        s.set_cursor_style(CursorShape::CursorBar, true);
        s.set_cursor_color(Some(rgb(7, 8, 9)));
        s.reset();
        s.flush().unwrap();

        let out = String::from_utf8(w.borrow().clone()).unwrap();
        // Mouse mode and encoding resets.
        assert!(out.contains("\x1b[?9l\x1b[?1000l\x1b[?1002l\x1b[?1003l"));
        assert!(out.contains("\x1b[?1006l\x1b[?1015l\x1b[?1016l"));
        // Cursor visible -> ShowCursor.
        assert!(out.contains("\x1b[?25h"));
        // Non-default cursor style and color resets.
        assert!(out.contains("\x1b[0 q"));
        assert!(out.contains("\x1b]112\x07"));
        // Background/foreground resets.
        assert!(out.contains("\x1b]111\x07"));
        assert!(out.contains("\x1b]110\x07"));
        // Bracketed paste reset and empty window title.
        assert!(out.contains("\x1b[?2004l"));
        assert!(out.contains("\x1b]2;\x07"));
        // Progress bar reset.
        assert!(out.contains("\x1b]9;4;0\x07"));
        // Final cursor move to the bottom of the screen is queued in the
        // renderer buffer (invisible here, Go-verified); the output ends
        // with the ShowCursor wrapper of the visible cursor.
        assert!(out.ends_with("\x1b[?25h"));
    }

    #[test]
    fn test_restore_output_sequences() {
        let (mut s, w) = test_screen_sized(10, 5);
        s.reset_tabs = true;
        s.alt_screen = true;
        s.set_keyboard_enhancements(Some(KeyboardEnhancements {
            disambiguate_escape_codes: true,
            ..Default::default()
        }));
        s.set_mouse_mode(MouseMode::MouseModeDrag);
        s.set_mouse_encoding(MouseEncoding::MouseEncodingSGR);
        s.show_cursor();
        s.set_cursor_style(CursorShape::CursorUnderline, false);
        s.set_cursor_color(Some(rgb(1, 2, 3)));
        s.set_background_color(Some(rgb(4, 5, 6)));
        s.set_foreground_color(Some(rgb(5, 6, 7)));
        s.enable_bracketed_paste();
        s.set_window_title("hi");
        s.set_progress_bar(Some(ProgressBar {
            state: ProgressBarState::ProgressBarError,
            value: 10,
        }));
        s.restore();
        s.flush().unwrap();

        let out = String::from_utf8(w.borrow().clone()).unwrap();
        // Tab stop reset, alt screen restore, cursor visibility.
        assert!(out.contains("\x1b[?5W"));
        assert!(out.contains("\x1b[?1049h"));
        assert!(out.contains("\x1b[?25h"));
        // Keyboard enhancements (flag 1), mouse mode and encoding.
        assert!(out.contains("\x1b[=1;1u"));
        assert!(out.contains("\x1b[?1002h"));
        assert!(out.contains("\x1b[?1006h"));
        // Cursor style (steady underline = 4), cursor color.
        assert!(out.contains("\x1b[4 q"));
        assert!(out.contains("\x1b]12;#010203\x07"));
        // Background/foreground.
        assert!(out.contains("\x1b]11;#040506\x07"));
        assert!(out.contains("\x1b]10;#050607\x07"));
        // Bracketed paste, window title, error progress bar.
        assert!(out.contains("\x1b[?2004h"));
        assert!(out.contains("\x1b]2;hi\x07"));
        assert!(out.contains("\x1b]9;4;2;10\x07"));
        // The output is wrapped in hide/show because the cursor is visible.
        assert!(out.starts_with("\x1b[?25l"));
        assert!(out.ends_with("\x1b[?25h"));
    }

    /// InsertAbove writes exactly to the underlying writer: CR, cursor
    /// down to the bottom, the queued newlines, cursor up, insert lines, and
    /// each line erased to the right.
    #[test]
    fn test_insert_above_exact_output() {
        let (mut s, w) = test_screen_sized(10, 3);
        // Renderer position defaults to (0, 0); down = 3 - 0 - 1 = 2.
        s.insert_above("a\nb").unwrap();
        assert_eq!(
            w.borrow().as_slice(),
            b"\r\x1b[2B\n\n\x1b[4A\x1b[2La\x1b[K\r\nb\x1b[K\r\n"
        );
        assert_eq!(s.rend.position(), (0, 0));
    }

    #[test]
    fn test_insert_above_wrapping_lines() {
        let (mut s, w) = test_screen_sized(10, 3);
        // A 25-column line wraps: 25 / 10 = 2 extra lines.
        let long = "x".repeat(25);
        s.insert_above(&long).unwrap();
        // offset = 1 + 2 = 3; down = 2; up = 3 + 3 - 1 = 5.
        let expected = format!("\r\x1b[2B\n\n\n\x1b[5A\x1b[3L{long}\x1b[K\r\n");
        assert_eq!(w.borrow().as_slice(), expected.as_bytes());
        assert_eq!(s.rend.position(), (0, 0));
    }

    #[test]
    fn test_insert_above_empty() {
        let (mut s, w) = test_screen_sized(10, 3);
        s.insert_above("").unwrap();
        assert!(w.borrow().is_empty());
    }

    #[test]
    fn test_string_width() {
        let (s, _) = test_screen();
        assert_eq!(s.string_width("hello"), 5);
        assert_eq!(s.string_width("界"), 2);
        assert_eq!(s.string_width("\x1b[31mred\x1b[0m"), 3);
    }

    #[test]
    fn test_width_method_override() {
        let (mut s, _) = test_screen();
        assert_eq!(s.width_method(), WidthMethod::WcWidth);
        s.set_width_method(WidthMethod::GraphemeWidth);
        assert_eq!(s.width_method(), WidthMethod::GraphemeWidth);
        assert_eq!(s.string_width("e\u{301}"), 1);
    }

    #[test]
    fn test_cursor_position_semantics() {
        let (mut s, _) = test_screen();
        assert_eq!(s.cursor_position(), None);
        // ShowCursor creates a cursor with no known position.
        s.show_cursor();
        assert!(s.cursor_visible());
        assert_eq!(s.cursor_position(), None);
        // SetCursorPosition sets the position.
        s.set_cursor_position(3, 4);
        assert_eq!(s.cursor_position(), Some((3, 4)));
        // Setting a style on an existing cursor keeps the position.
        s.set_cursor_style(CursorShape::CursorBar, false);
        assert_eq!(s.cursor_position(), Some((3, 4)));
        assert_eq!(s.cursor_style(), (CursorShape::CursorBar, false));
    }

    #[test]
    fn test_alt_screen_enter_exit() {
        let (mut s, w) = test_screen();
        s.enter_alt_screen();
        assert!(s.alt_screen());
        // Cursor is hidden by default: 1049h + hide cursor.
        assert_eq!(s.buf, b"\x1b[?1049h\x1b[?25l");
        s.flush().unwrap();
        assert_eq!(w.borrow().as_slice(), b"\x1b[?1049h\x1b[?25l");

        w.borrow_mut().clear();
        s.buf.clear();
        s.exit_alt_screen();
        assert!(!s.alt_screen());
        s.flush().unwrap();
        assert_eq!(w.borrow().as_slice(), b"\x1b[?1049l\x1b[?25l");
    }

    #[test]
    fn test_display_with_drawable() {
        let (mut s, w) = test_screen_sized(5, 1);
        s.rend = Box::new(RecordingRenderer::default());
        let mut d = crate::DrawableFunc(Box::new(|scr: &mut dyn Screen, area: Rectangle| {
            scr.set_cell(area.min.0, area.min.1, Some(&Cell::new("x")));
            scr.set_cell(area.min.0 + 1, area.min.1, Some(&Cell::new("y")));
        }));
        s.display(Some(&mut d)).unwrap();
        // Cells written through the drawable.
        assert_eq!(s.cell_at(0, 0).unwrap().content, "x");
        assert_eq!(s.cell_at(1, 0).unwrap().content, "y");
        // The renderer rendered and flushed the render buffer.
        assert_eq!(w.borrow().as_slice(), b"RF");
    }

    #[test]
    fn test_resize_updates_bounds_and_render_buffer() {
        let (mut s, _) = test_screen();
        assert_eq!((s.width(), s.height()), (0, 0));
        s.resize(20, 10);
        assert_eq!((s.width(), s.height()), (20, 10));
        assert_eq!(s.rbuf.width(), 20);
        assert_eq!(s.rbuf.height(), 10);
        assert!(s.rbuf.touched.iter().all(|t| t.is_none()));
    }

    #[test]
    fn test_mouse_bracketed_title_roundtrip() {
        let (mut s, w) = test_screen();
        s.set_mouse_mode(MouseMode::MouseModeMotion);
        s.set_mouse_encoding(MouseEncoding::MouseEncodingSGRPixel);
        s.enable_bracketed_paste();
        s.set_window_title("t");
        s.disable_bracketed_paste();
        s.set_mouse_mode(MouseMode::MouseModeClick);
        s.flush().unwrap();
        let out = String::from_utf8(w.borrow().clone()).unwrap();
        assert!(out.contains("\x1b[?1003h"));
        assert!(out.contains("\x1b[?1016h"));
        assert!(out.contains("\x1b]2;t\x07"));
        assert!(out.contains("\x1b[?2004h"));
        assert!(out.contains("\x1b[?2004l"));
        assert!(out.contains("\x1b[?1000h"));
        assert_eq!(s.mouse_mode(), MouseMode::MouseModeClick);
        assert_eq!(s.mouse_encoding(), MouseEncoding::MouseEncodingSGRPixel);
        assert_eq!(s.window_title(), "t");
        assert!(!s.bracketed_paste());
    }

    #[test]
    fn test_write_and_write_string() {
        let (mut s, w) = test_screen();
        assert_eq!(s.write_string("abc"), 3);
        assert_eq!(s.write(b"def"), 3);
        s.flush().unwrap();
        assert_eq!(w.borrow().as_slice(), b"abcdef");
    }

    #[test]
    fn test_color_profile_detection() {
        let mut env = Environ::default();
        // No TERM and not a tty: NoTTY.
        assert_eq!(detect_color_profile(None, &env), ColorProfile::NoTty);
        // TERM=xterm-256color is still NoTTY when the output is not a tty.
        env.0.push("TERM=xterm-256color".to_string());
        assert_eq!(detect_color_profile(None, &env), ColorProfile::NoTty);
        // COLORTERM=truecolor only upgrades a real terminal.
        env.0.push("COLORTERM=truecolor".to_string());
        assert_eq!(detect_color_profile(None, &env), ColorProfile::NoTty);
        env.0.push("TTY_FORCE=1".to_string());
        assert_eq!(detect_color_profile(None, &env), ColorProfile::TrueColor);
        // NO_COLOR downgrades to Ascii when the output is a tty.
        let mut env = Environ(vec![
            "TERM=xterm-256color".to_string(),
            "NO_COLOR=1".to_string(),
            "TTY_FORCE=1".to_string(),
        ]);
        assert_eq!(detect_color_profile(Some(1), &env), ColorProfile::Ascii);
        // Without NO_COLOR, TTY_FORCE plus a 256-color TERM is ANSI256.
        env.0.retain(|e| !e.starts_with("NO_COLOR="));
        assert_eq!(detect_color_profile(None, &env), ColorProfile::Ansi256);
        // TERM=dumb is NoTTY even with TTY_FORCE.
        let env = Environ(vec!["TERM=dumb".to_string(), "TTY_FORCE=1".to_string()]);
        assert_eq!(detect_color_profile(None, &env), ColorProfile::NoTty);
        // CLICOLOR_FORCE=1 upgrades NoTTY to ANSI.
        let env = Environ(vec!["CLICOLOR_FORCE=1".to_string()]);
        assert_eq!(detect_color_profile(None, &env), ColorProfile::Ansi);
    }

    #[test]
    fn test_environ_lookup() {
        let env = Environ(vec![
            "A=1".to_string(),
            "B=2".to_string(),
            "A=3".to_string(),
        ]);
        assert_eq!(env.getenv("A"), "3");
        assert_eq!(env.lookup_env("B"), Some("2".to_string()));
        assert_eq!(env.lookup_env("C"), None);
        assert_eq!(env.getenv("C"), "");
    }
}
