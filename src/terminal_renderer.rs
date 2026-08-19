//! Cleanroom Rust port of upstream Go source file: `terminal_renderer.go`
//! Upstream Target Tag / Version: `v0.0.0-20260703014108-f5a850f9c2b7`
//!
//! Also covers (same upstream module):
//! `terminal_renderer_hardscroll.go`, `terminal_renderer_hashmap.go`
//!
//! <public-docs>
//! The terminal renderer: renders a [crate::RenderBuffer] to the terminal
//! with the minimal escape sequences, using cursor-movement, erase, insert,
//! hard-tab, hard-scroll, and line-hash optimizations. Output accumulates in
//! an internal buffer (mirroring Go's `bytes.Buffer`) and is drained into the
//! screen's output buffer on each trait call.
//! </public-docs>

use crate::buffer::{cell_equal, Line, LineData, RenderBuffer};
use crate::cell::{empty_cell, Cell, Link};
use crate::environ::Environ;
use crate::logger::Logger;
use crate::screen::rect;
use crate::style::{Attr, Style};
use crate::tabstop::{default_tab_stops, TabStops};
use crate::terminal_screen::ColorProfile;
use rusty_x_ansi::color::{ansi256_to_16, convert_16, convert_256};
use rusty_x_ansi::cursor::{cursor_backward_tab, REVERSE_INDEX};
use rusty_x_ansi::hyperlink::{reset_hyperlink, set_hyperlink};
use rusty_x_ansi::method::WidthMethod;
use rusty_x_ansi::mode::{
    RESET_MODE_AUTO_WRAP, RESET_MODE_INSERT_REPLACE, SET_MODE_AUTO_WRAP, SET_MODE_INSERT_REPLACE,
};
use rusty_x_ansi::parser::{DEL, US};
use rusty_x_ansi::style::Color;
use rusty_x_ansi::{
    cursor_backward, cursor_down, cursor_forward, cursor_horizontal_absolute, cursor_position,
    cursor_up, delete_character, delete_line, erase_character, insert_character, insert_line,
    repeat_previous_character, scroll_down, scroll_up, set_top_bottom_margins,
    vertical_position_absolute, CURSOR_HOME_POSITION, ERASE_ENTIRE_SCREEN, ERASE_LINE_LEFT,
    ERASE_LINE_RIGHT, ERASE_SCREEN_BELOW, RESET_STYLE,
};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::sync::Mutex;

/// The marker used for touched lines that have been processed (upstream uses
/// `-1, -1`).
const PROCESSED: LineData = LineData {
    first_cell: usize::MAX,
    last_cell: usize::MAX,
};

/// capabilities represents a mask of supported ANSI escape sequences.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Capabilities(u16);

impl Capabilities {
    const VPA: u16 = 1 << 0;
    const HPA: u16 = 1 << 1;
    const CHA: u16 = 1 << 2;
    const CHT: u16 = 1 << 3;
    const CBT: u16 = 1 << 4;
    const REP: u16 = 1 << 5;
    const ECH: u16 = 1 << 6;
    const ICH: u16 = 1 << 7;
    const SD: u16 = 1 << 8;
    const SU: u16 = 1 << 9;
    const HT: u16 = 1 << 10;
    const BS: u16 = 1 << 11;

    const ALL: u16 = Self::VPA
        | Self::HPA
        | Self::CHA
        | Self::CHT
        | Self::CBT
        | Self::REP
        | Self::ECH
        | Self::ICH
        | Self::SD
        | Self::SU;

    /// Set sets the given capabilities.
    pub fn set(&mut self, c: u16) {
        self.0 |= c;
    }

    /// Reset resets the given capabilities.
    pub fn reset(&mut self, c: u16) {
        self.0 &= !c;
    }

    /// Contains returns whether the capabilities contains the given
    /// capability.
    pub fn contains(&self, c: u16) -> bool {
        self.0 & c == c
    }
}

/// tFlag is a bitmask of terminal flags.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct TFlag(u8);

impl TFlag {
    const RELATIVE_CURSOR: u8 = 1 << 0;
    const FULLSCREEN: u8 = 1 << 1;
    const MAP_NEWLINE: u8 = 1 << 2;
    const SCROLL_OPTIM: u8 = 1 << 3;
    const GRAPHEME_WIDTH: u8 = 1 << 4;

    fn set(&mut self, c: u8) {
        self.0 |= c;
    }

    fn reset(&mut self, c: u8) {
        self.0 &= !c;
    }

    fn contains(&self, c: u8) -> bool {
        self.0 & c == c
    }
}

/// The cursor state: the pen cell plus the position. Positions use i64 so the
/// upstream `-1` "unknown" sentinel can be represented.
#[derive(Debug, Clone)]
struct RCursor {
    cell: Cell,
    x: i64,
    y: i64,
}

impl RCursor {
    fn new() -> RCursor {
        RCursor {
            cell: empty_cell(),
            x: -1,
            y: -1,
        }
    }
}

/// hashmap represents a single [Line] hash.
#[derive(Debug, Clone, Copy, Default)]
struct Hashmap {
    value: u64,
    oldcount: i32,
    newcount: i32,
    oldindex: i32,
    newindex: i32,
}

/// The value used to indicate lines created by insertions and scrolls.
const NEW_INDEX: i64 = -1;

/// ConvertStyle converts a style to respect the given color profile.
///
/// NOTE: upstream this lives in `cell.go` (ConvertStyle); ported here until
/// the mapping is reorganized.
pub(crate) fn convert_style(s: &Style, p: ColorProfile) -> Style {
    match p {
        ColorProfile::TrueColor => return s.clone(),
        ColorProfile::Unknown | ColorProfile::Ascii => {
            let mut s = s.clone();
            s.fg = None;
            s.bg = None;
            s.underline_color = None;
            return s;
        }
        ColorProfile::NoTty => return Style::default(),
        ColorProfile::Ansi | ColorProfile::Ansi256 => {}
    }

    let mut s = s.clone();
    if let Some(fg) = s.fg {
        s.fg = Some(convert_color(fg, p));
    }
    if let Some(bg) = s.bg {
        s.bg = Some(convert_color(bg, p));
    }
    if let Some(uc) = s.underline_color {
        s.underline_color = Some(convert_color(uc, p));
    }
    s
}

fn convert_color(c: Color, p: ColorProfile) -> Color {
    match (c, p) {
        (Color::RGB(rgb), ColorProfile::Ansi256) => {
            Color::Indexed(convert_256(rgb.r, rgb.g, rgb.b))
        }
        (Color::RGB(rgb), ColorProfile::Ansi) => Color::Basic(convert_16(rgb.r, rgb.g, rgb.b)),
        (Color::Indexed(i), ColorProfile::Ansi) => Color::Basic(ansi256_to_16(i)),
        (c, _) => c,
    }
}

/// ConvertLink converts a hyperlink to respect the given color profile.
///
/// NOTE: upstream this lives in `cell.go` (ConvertLink); see [convert_style].
pub(crate) fn convert_link(h: &Link, p: ColorProfile) -> Link {
    if p == ColorProfile::NoTty || p == ColorProfile::Unknown {
        return Link::default();
    }
    h.clone()
}

/// Detects the color profile from the environment, mirroring the env-based
/// part of `colorprofile.Detect`. The screen overrides this right after
/// construction with its own detection.
fn detect_profile(env: &Environ) -> ColorProfile {
    if env.lookup_env("NO_COLOR").is_some() && env.getenv("NO_COLOR") != "0" {
        return ColorProfile::NoTty;
    }
    if env.getenv("CLICOLOR") == "0" {
        return ColorProfile::Ascii;
    }
    if env.getenv("TERM") == "dumb" {
        return ColorProfile::Ascii;
    }
    if env.getenv("TERM").starts_with("xterm-256color") || env.lookup_env("COLORTERM").is_some() {
        return ColorProfile::TrueColor;
    }
    if env.getenv("TERM").starts_with("xterm") {
        return ColorProfile::Ansi256;
    }
    ColorProfile::Ansi
}

/// TerminalRenderer is a terminal screen render and lazy writer that buffers
/// the output until it is flushed. It handles rendering a screen from a
/// [RenderBuffer] to the terminal with the minimal necessary escape sequences
/// to transition the terminal to the new buffer state.
pub struct TerminalRenderer {
    /// The writer flushed to (upstream's `w io.Writer`).
    writer: Option<Mutex<Box<dyn Write + Send>>>,
    /// The internal output buffer (Go's `buf *bytes.Buffer`).
    buf: Vec<u8>,
    /// The current buffer, updated after each render.
    curbuf: RenderBuffer,
    /// Tab stops for hard-tab movement optimizations.
    tabs: Option<TabStops>,
    /// Line hash state for the hard-scroll optimizer.
    oldhash: Vec<u64>,
    newhash: Vec<u64>,
    hashtab: Vec<Hashmap>,
    oldnum: Vec<i64>,
    /// The current and saved cursors.
    cur: RCursor,
    saved: RCursor,
    /// Terminal writer flags.
    flags: TFlag,
    /// The width method used to measure cell width.
    method: WidthMethod,
    /// The terminal type.
    term: String,
    /// Whether to force clear the screen.
    clear: bool,
    /// Terminal control sequence capabilities.
    caps: Capabilities,
    /// Whether the cursor is out of bounds and at a phantom cell.
    at_phantom: bool,
    /// Whether the line currently being transformed contained a wide cell.
    line_had_wide: bool,
    /// The color profile used for downsampling colors.
    profile: ColorProfile,
}

/// NewTerminalRenderer returns a new [TerminalRenderer] using the given
/// environment. The renderer detects the color profile from the environment
/// and the terminal capabilities from the `TERM` variable.
pub(crate) fn new_terminal_renderer(
    env: &Environ,
) -> Box<dyn crate::terminal_screen::TerminalRenderer> {
    let term = env.getenv("TERM");
    Box::new(TerminalRenderer {
        writer: None,
        buf: Vec::new(),
        curbuf: crate::new_render_buffer(0, 0),
        tabs: None,
        oldhash: Vec::new(),
        newhash: Vec::new(),
        hashtab: Vec::new(),
        oldnum: Vec::new(),
        cur: RCursor::new(),
        saved: RCursor::new(),
        flags: TFlag::default(),
        method: WidthMethod::WcWidth,
        term: term.clone(),
        clear: false,
        caps: xterm_caps(&term),
        at_phantom: false,
        line_had_wide: false,
        profile: detect_profile(env),
    })
}

impl TerminalRenderer {
    /// NewTerminalRenderer returns a new [TerminalRenderer] that writes to
    /// the given writer, mirroring upstream `NewTerminalRenderer(w, env)`.
    ///
    /// The renderer detects the color profile from the environment and the
    /// terminal capabilities from the `TERM` variable.
    pub fn new(w: Box<dyn Write + Send>, env: &Environ) -> TerminalRenderer {
        let mut r = TerminalRenderer::new_inner(env);
        r.writer = Some(Mutex::new(w));
        r
    }

    /// NewWithoutWriter returns a renderer with no attached writer; use
    /// [TerminalRenderer::flush_into] to drain the buffer.
    pub fn new_without_writer(env: &Environ) -> TerminalRenderer {
        TerminalRenderer::new_inner(env)
    }

    fn new_inner(env: &Environ) -> TerminalRenderer {
        let term = env.getenv("TERM");
        TerminalRenderer {
            writer: None,
            buf: Vec::new(),
            curbuf: crate::new_render_buffer(0, 0),
            tabs: None,
            oldhash: Vec::new(),
            newhash: Vec::new(),
            hashtab: Vec::new(),
            oldnum: Vec::new(),
            cur: RCursor::new(),
            saved: RCursor::new(),
            flags: TFlag::default(),
            method: WidthMethod::WcWidth,
            term: term.clone(),
            clear: false,
            caps: xterm_caps(&term),
            at_phantom: false,
            line_had_wide: false,
            profile: detect_profile(env),
        }
    }

    /// Render renders changes of the screen to the internal buffer. Call
    /// [TerminalRenderer::flush] to flush pending changes to the writer.
    pub fn render_public(&mut self, newbuf: &mut RenderBuffer) {
        self.render_buffer(newbuf);
    }

    /// Flush flushes the buffer to the writer.
    pub fn flush_public(&mut self) -> std::io::Result<()> {
        if let Some(w) = &self.writer {
            let mut w = w.lock().unwrap();
            if !self.buf.is_empty() {
                w.write_all(&self.buf)?;
                self.buf.clear();
            }
        }
        Ok(())
    }

    /// FlushInto flushes the buffer into the given output buffer (screen
    /// mode, used when no writer is attached).
    pub fn flush_into(&mut self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.buf);
        self.buf.clear();
    }

    /// SetFullscreen sets whether whole screen is being used.
    pub fn set_fullscreen_public(&mut self, fullscreen: bool) {
        if fullscreen {
            self.flags.set(TFlag::FULLSCREEN);
        } else {
            self.flags.reset(TFlag::FULLSCREEN);
        }
    }

    /// SetRelativeCursor sets whether to use relative cursor movements.
    pub fn set_relative_cursor_public(&mut self, relative: bool) {
        if relative {
            self.flags.set(TFlag::RELATIVE_CURSOR);
        } else {
            self.flags.reset(TFlag::RELATIVE_CURSOR);
        }
    }

    /// SetColorProfile sets the color profile of the renderer.
    pub fn set_color_profile_public(&mut self, profile: ColorProfile) {
        self.profile = profile;
    }

    /// SetTabStops sets the tab stops for the terminal.
    pub fn set_tab_stops_public(&mut self, every: i32) {
        if every < 0 || self.term.starts_with("linux") {
            self.caps.reset(Capabilities::HT);
        } else {
            self.caps.set(Capabilities::HT);
            let width = self.curbuf.width() as i32;
            self.tabs = Some(default_tab_stops(if width > 0 { width } else { every }));
        }
    }

    /// SetBackspace sets whether to use backspace as a movement
    /// optimization.
    pub fn set_backspace_public(&mut self, backspace: bool) {
        if backspace {
            self.caps.set(Capabilities::BS);
        } else {
            self.caps.reset(Capabilities::BS);
        }
    }

    /// SetMapNewline sets whether the terminal is currently mapping
    /// newlines to CRLF.
    pub fn set_map_newline_public(&mut self, map: bool) {
        if map {
            self.flags.set(TFlag::MAP_NEWLINE);
        } else {
            self.flags.reset(TFlag::MAP_NEWLINE);
        }
    }

    /// SetWidthMethod sets the width method the renderer uses.
    pub fn set_width_method_public(&mut self, method: WidthMethod) {
        self.method = method;
    }

    /// SetGraphemeWidth sets whether the terminal measures cell width using
    /// Unicode grapheme clustering.
    pub fn set_grapheme_width_public(&mut self, grapheme: bool) {
        if grapheme {
            self.flags.set(TFlag::GRAPHEME_WIDTH);
            self.method = WidthMethod::GraphemeWidth;
        } else {
            self.flags.reset(TFlag::GRAPHEME_WIDTH);
            self.method = WidthMethod::WcWidth;
        }
    }

    /// Resize updates the terminal screen tab stops.
    pub fn resize_public(&mut self, width: usize, _height: usize) {
        if let Some(tabs) = &mut self.tabs {
            tabs.resize(width as i32);
        }
    }

    /// Erase marks the screen to be fully erased on the next render.
    pub fn erase_public(&mut self) {
        self.clear = true;
    }

    /// SaveCursor saves the current cursor position and styles.
    pub fn save_cursor_public(&mut self) {
        self.saved = self.cur.clone();
    }

    /// RestoreCursor restores the saved cursor position and styles.
    pub fn restore_cursor_public(&mut self) {
        self.cur = self.saved.clone();
    }

    /// Position returns the cursor position in the screen buffer.
    pub fn position_public(&self) -> (usize, usize) {
        (self.cur.x.max(0) as usize, self.cur.y.max(0) as usize)
    }

    /// SetPosition changes the logical cursor position.
    pub fn set_position_public(&mut self, x: usize, y: usize) {
        self.cur.x = x as i64;
        self.cur.y = y as i64;
    }

    /// MoveTo calculates and writes the shortest sequence to move the cursor
    /// to the given position.
    pub fn move_to_public(&mut self, x: i64, y: i64) {
        self.move_to_pos(None, x, y);
    }

    /// Buffered returns the number of bytes buffered for the next flush.
    pub fn buffered_public(&self) -> usize {
        self.buf.len()
    }

    /// WriteString writes the given string to the renderer's buffer.
    pub fn write_string_public(&mut self, s: &str) -> std::io::Result<usize> {
        let n = s.len();
        self.push(s);
        Ok(n)
    }

    /// SetScrollOptim sets whether to use hard scroll optimizations.
    pub fn set_scroll_optim_public(&mut self, v: bool) {
        if v {
            self.flags.set(TFlag::SCROLL_OPTIM);
        } else {
            self.flags.reset(TFlag::SCROLL_OPTIM);
        }
    }

    /// Redraw forces a full redraw of the screen.
    pub fn redraw_public(&mut self, newbuf: &mut RenderBuffer) {
        self.clear = true;
        self.render_buffer(newbuf);
    }

    /// WriteByte writes a single byte to the renderer's buffer.
    pub fn write_byte_public(&mut self, b: u8) -> std::io::Result<usize> {
        self.push_byte(b);
        Ok(1)
    }
}

impl TerminalRenderer {
    fn push(&mut self, s: &str) {
        self.buf.extend_from_slice(s.as_bytes());
    }

    fn push_byte(&mut self, b: u8) {
        self.buf.push(b);
    }

    /// Buffered returns the number of bytes buffered for the next flush.
    pub fn buffered(&self) -> usize {
        self.buf.len()
    }

    /// move moves the cursor to the specified position in the buffer.
    ///
    /// It is safe to call this function with no buffer; in that case, it
    /// won't use any optimizations that depend on the buffer.
    fn move_to_pos(&mut self, newbuf: Option<&RenderBuffer>, x: i64, y: i64) {
        if !self.flags.contains(TFlag::FULLSCREEN)
            && self.flags.contains(TFlag::RELATIVE_CURSOR)
            && self.cur.x == -1
            && self.cur.y == -1
        {
            // First cursor movement in inline mode, move the cursor to the
            // first column before moving to the target position.
            self.push_byte(b'\r');
            self.cur.x = 0;
            self.cur.y = 0;
        }
        // XXX: Make sure we use the max height and width of the buffer in
        // case we're in the middle of a resize operation.
        let mut width = self.curbuf.width() as i64;
        let mut height = self.curbuf.height() as i64;
        if let Some(newbuf) = newbuf {
            width = width.max(newbuf.width() as i64);
            height = height.max(newbuf.height() as i64);
        }

        let (mut x, mut y) = (x, y);
        if width > 0 && x >= width {
            // Handle autowrap
            y += x / width;
            x %= width;
        }

        // XXX: Disable styles if there's any. Some move operations such as
        // [rusty_x_ansi::cursor_down] can apply styles to the new cursor
        // position, thus, we need to reset the styles before moving the
        // cursor.
        let blank = self.cur.cell.clone();
        let reset_pen = y != self.cur.y && !cell_equal(Some(&blank), Some(&empty_cell()));
        if reset_pen {
            self.update_pen(None);
        }

        // Reset wrap around (phantom cursor) state
        if self.at_phantom {
            self.cur.x = 0;
            self.push_byte(b'\r');
            self.at_phantom = false; // reset phantom cell state
        }

        if height > 0 {
            if self.cur.y > height - 1 {
                self.cur.y = height - 1;
            }
            if y > height - 1 {
                y = height - 1;
            }
        }

        if x == self.cur.x && y == self.cur.y {
            return;
        }

        // We set the new cursor in moveCursor.
        let seq = self.cursor_move(newbuf, x, y, true); // Overwrite cells if possible
        self.push(&seq);
        self.cur.x = x;
        self.cur.y = y;
    }

    /// moveCursor moves the cursor to the specified position.
    ///
    /// It is safe to call this function with no buffer; in that case, it
    /// won't use any optimizations that depend on the buffer.
    fn cursor_move(
        &self,
        newbuf: Option<&RenderBuffer>,
        x: i64,
        y: i64,
        overwrite: bool,
    ) -> String {
        let fx = self.cur.x;
        let fy = self.cur.y;

        if !self.flags.contains(TFlag::RELATIVE_CURSOR) {
            let mut width: i64 = -1; // Use -1 to indicate that we don't know the width of the screen.
            if let Some(tabs) = &self.tabs {
                width = tabs.width() as i64;
            }
            if let Some(newbuf) = newbuf {
                if width == -1 {
                    width = newbuf.width() as i64;
                }
            }
            // Method #0: Use CUP if the distance is long.
            let seq = cursor_position((x + 1) as i32, (y + 1) as i32);
            if fx == -1 || fy == -1 || width == -1 || not_local(width, fx, fy, x, y) {
                return seq;
            }
        }

        // Optimize based on options.
        let mut trials = 0;
        if self.caps.contains(Capabilities::HT) {
            trials |= 2; // 0b10 in binary
        }
        if self.caps.contains(Capabilities::BS) {
            trials |= 1; // 0b01 in binary
        }

        // Try all possible combinations of hard tabs and backspace
        // optimizations.
        let mut seq = String::new();
        for i in 0..=trials {
            // Skip combinations that are not enabled.
            if i & !trials != 0 {
                continue;
            }

            let use_hard_tabs = i & 2 != 0;
            let use_backspace = i & 1 != 0;

            // Method #1: Use local movement sequences.
            let nseq1 = self.relative_cursor_move(
                newbuf,
                fx,
                fy,
                x,
                y,
                overwrite,
                use_hard_tabs,
                use_backspace,
            );
            if (i == 0 && seq.is_empty()) || nseq1.len() < seq.len() {
                seq = nseq1;
            }

            // Method #2: Use CR and local movement sequences.
            let mut nseq2 = self.relative_cursor_move(
                newbuf,
                0,
                fy,
                x,
                y,
                overwrite,
                use_hard_tabs,
                use_backspace,
            );
            nseq2.insert(0, '\r');
            if nseq2.len() < seq.len() {
                seq = nseq2;
            }

            if !self.flags.contains(TFlag::RELATIVE_CURSOR) {
                // Method #3: Use CursorHomePosition and local movement
                // sequences.
                let mut nseq3 = self.relative_cursor_move(
                    newbuf,
                    0,
                    0,
                    x,
                    y,
                    overwrite,
                    use_hard_tabs,
                    use_backspace,
                );
                nseq3.insert_str(0, CURSOR_HOME_POSITION);
                if nseq3.len() < seq.len() {
                    seq = nseq3;
                }
            }
        }

        seq
    }

    /// relativeCursorMove returns the relative cursor movement sequence using
    /// one or two of [cursor_up], [cursor_down], [cursor_forward],
    /// [cursor_backward], [vertical_position_absolute],
    /// [rusty_x_ansi::horizontal_position_absolute].
    /// Mirrors the upstream Go signature
    /// `relativeCursorMove(newbuf, fx, fy, tx, ty, overwrite, useTabs,
    /// useBackspace)` 1:1.
    #[allow(clippy::too_many_arguments)]
    fn relative_cursor_move(
        &self,
        newbuf: Option<&RenderBuffer>,
        mut fx: i64,
        fy: i64,
        tx: i64,
        ty: i64,
        mut overwrite: bool,
        use_tabs: bool,
        use_backspace: bool,
    ) -> String {
        let mut seq = String::new();
        if newbuf.is_none() {
            overwrite = false; // We can't overwrite the current buffer.
        }

        if ty != fy {
            let mut yseq = String::new();
            if self.caps.contains(Capabilities::VPA) && !self.flags.contains(TFlag::RELATIVE_CURSOR)
            {
                yseq = vertical_position_absolute((ty + 1) as i32);
            }

            if ty > fy {
                let n = ty - fy;
                let cud = cursor_down(n as i32);
                if yseq.is_empty() || cud.len() < yseq.len() {
                    yseq = cud;
                }
                if !self.flags.contains(TFlag::FULLSCREEN) || (n as usize) < yseq.len() {
                    yseq = "\n".repeat(n as usize);
                    if self.flags.contains(TFlag::MAP_NEWLINE) {
                        let _ = fx; // Go resets fx here; unused in this branch.
                    }
                }
            } else if ty < fy {
                let n = fy - ty;
                let cuu = cursor_up(n as i32);
                if yseq.is_empty() || cuu.len() < yseq.len() {
                    yseq = cuu;
                }
                if n == 1 && fy - 1 > 0 {
                    yseq = REVERSE_INDEX.to_string();
                }
            }

            seq.push_str(&yseq);
        }

        if tx != fx {
            let mut xseq = String::new();
            if !self.flags.contains(TFlag::RELATIVE_CURSOR) {
                if self.caps.contains(Capabilities::HPA) {
                    xseq = rusty_x_ansi::horizontal_position_absolute((tx + 1) as i32);
                } else if self.caps.contains(Capabilities::CHA) {
                    xseq = cursor_horizontal_absolute((tx + 1) as i32);
                }
            }

            if tx > fx {
                let mut n = tx - fx;
                let mut col = fx;
                if use_tabs {
                    if let Some(tabs) = self.tabs.as_ref() {
                        let mut tabs_count = 0;
                        while (tabs.next(col as i32) as i64) <= tx {
                            tabs_count += 1;
                            let next = tabs.next(col as i32) as i64;
                            if col == next || col >= tabs.width() as i64 - 1 {
                                break;
                            }
                            col = next;
                        }

                        if tabs_count > 0 {
                            seq.push_str(&"\t".repeat(tabs_count as usize));
                            n = tx - col;
                            // Mirror upstream: after emitting the tabs the cursor
                            // sits at the tab destination, not the original
                            // column; the overwrite scan below must start there.
                            fx = col;
                        }
                    }
                }

                let cuf = cursor_forward(n as i32);
                if xseq.is_empty() || cuf.len() < xseq.len() {
                    xseq = cuf;
                }

                // If we have no attribute and style changes, overwrite is
                // cheaper.
                let mut ovw = String::new();
                if overwrite && ty >= 0 {
                    let mut ok = true;
                    if let Some(newbuf) = newbuf {
                        let mut i: i64 = 0;
                        while i < n {
                            if let Some(cell) = newbuf.cell_at((fx + i) as usize, ty as usize) {
                                if cell.width > 0 {
                                    i += cell.width as i64 - 1;
                                    if !cell.style.equal(&self.cur.cell.style)
                                        || cell.link != self.cur.cell.link
                                    {
                                        ok = false;
                                        break;
                                    }
                                }
                            }
                            i += 1;
                        }
                    }
                    if !ok {
                        overwrite = false;
                    }
                }

                if overwrite && ty >= 0 {
                    if let Some(newbuf) = newbuf {
                        let mut i: i64 = 0;
                        while i < n {
                            if let Some(cell) = newbuf.cell_at((fx + i) as usize, ty as usize) {
                                if cell.width > 0 {
                                    ovw.push_str(&cell.content);
                                    i += cell.width as i64 - 1;
                                }
                            }
                            i += 1;
                        }
                    }
                }

                if overwrite && ovw.len() < xseq.len() {
                    xseq = ovw;
                }
            } else if tx < fx {
                let mut n = fx - tx;
                if use_tabs && self.caps.contains(Capabilities::CBT) {
                    // VT100 does not support backward tabs CBT.
                    if let Some(tabs) = self.tabs.as_ref() {
                        let mut col = fx;
                        let mut cbt = 0; // cursor backward tabs count
                        while (tabs.prev(col as i32) as i64) >= tx {
                            col = tabs.prev(col as i32) as i64;
                            cbt += 1;
                            if col == tabs.prev(col as i32) as i64 || col <= 0 {
                                break;
                            }
                        }

                        if cbt > 0 {
                            seq.push_str(&cursor_backward_tab(cbt));
                            n = col - tx;
                        }
                    }
                }

                let cub = cursor_backward(n as i32);
                if xseq.is_empty() || cub.len() < xseq.len() {
                    xseq = cub;
                }

                if use_backspace && (n as usize) < xseq.len() {
                    xseq = "\u{8}".repeat(n as usize);
                }
            }

            seq.push_str(&xseq);
        }

        seq
    }

    /// putCell draws a cell at the current cursor position.
    fn put_cell(&mut self, newbuf: &RenderBuffer, cell: Option<&Cell>) {
        let width = newbuf.width() as i64;
        let height = newbuf.height() as i64;
        if self.flags.contains(TFlag::FULLSCREEN)
            && self.cur.x == width - 1
            && self.cur.y == height - 1
        {
            self.put_cell_lr(newbuf, cell);
        } else {
            self.put_attr_cell(newbuf, cell);
        }
    }

    /// wrapCursor wraps the cursor to the next line.
    fn wrap_cursor(&mut self) {
        const AUTO_RIGHT_MARGIN: bool = true;
        if AUTO_RIGHT_MARGIN {
            // Assume we have auto wrap mode enabled.
            self.cur.x = 0;
            self.cur.y += 1;
        } else {
            self.cur.x -= 1;
        }
    }

    /// putAttrCell writes a cell at the current cursor position.
    fn put_attr_cell(&mut self, newbuf: &RenderBuffer, cell: Option<&Cell>) {
        if let Some(cell) = cell {
            if cell.width == 0 {
                // XXX: Zero width cells are special and should not be written
                // to the screen no matter what other attributes they have.
                return;
            }
        }

        // We're at pending wrap state (phantom cell), incoming cell should
        // wrap.
        if self.at_phantom {
            self.wrap_cursor();
            self.at_phantom = false;
        }

        self.update_pen(cell);
        let mut cell_width = 1;
        match cell {
            None => self.push_byte(b' '),
            Some(cell) => {
                self.push(&cell.content);
                cell_width = cell.width;
            }
        }

        self.cur.x += cell_width as i64;
        if self.cur.x >= newbuf.width() as i64 {
            self.at_phantom = true;
        }

        if cell_width > 1 {
            self.line_had_wide = true;
        }
    }

    /// putCellLR draws a cell at the lower right corner of the screen.
    fn put_cell_lr(&mut self, newbuf: &RenderBuffer, cell: Option<&Cell>) {
        // Optimize for the lower right corner cell.
        let cur_x = self.cur.x;
        if let Some(cell) = cell {
            if !cell.is_wide_placeholder() {
                self.push(RESET_MODE_AUTO_WRAP);
                self.put_attr_cell(newbuf, Some(cell));
                // Writing to lower-right corner cell should not wrap.
                self.at_phantom = false;
                self.cur.x = cur_x;
                self.push(SET_MODE_AUTO_WRAP);
            }
        } else {
            self.push(RESET_MODE_AUTO_WRAP);
            self.put_attr_cell(newbuf, None);
            // Writing to lower-right corner cell should not wrap.
            self.at_phantom = false;
            self.cur.x = cur_x;
            self.push(SET_MODE_AUTO_WRAP);
        }
    }

    /// updatePen updates the cursor pen styles.
    fn update_pen(&mut self, cell: Option<&Cell>) {
        match cell {
            None => {
                if !self.cur.cell.style.is_zero() {
                    self.push(RESET_STYLE);
                    self.cur.cell.style = Style::default();
                }
                if let Some(link) = &self.cur.cell.link {
                    if !link.is_zero() {
                        self.push(reset_hyperlink());
                    }
                }
            }
            Some(cell) => {
                // Downsample pen when we don't have a TrueColor profile,
                // otherwise, use the original style.
                let new_style = convert_style(&cell.style, self.profile);
                let new_link = convert_link(&cell.link.clone().unwrap_or_default(), self.profile);
                let old_style = convert_style(&self.cur.cell.style, self.profile);
                let old_link = convert_link(
                    &self.cur.cell.link.clone().unwrap_or_default(),
                    self.profile,
                );

                if !new_style.equal(&old_style) {
                    let mut seq = new_style.diff(&old_style);
                    if new_style.is_zero() && seq.len() > RESET_STYLE.len() {
                        seq = RESET_STYLE.to_string();
                    }
                    self.push(&seq);
                    self.cur.cell.style = cell.style.clone(); // Copy the original style
                }
                if new_link != old_link {
                    self.push(&set_hyperlink(&new_link.url, &new_link.params));
                    self.cur.cell.link = cell.link.clone();
                }
            }
        }
    }

    /// canClearWith checks whether the given cell can be used by clearing
    /// commands like [rusty_x_ansi::erase_line] to clear the screen.
    fn can_clear_with(c: Option<&Cell>) -> bool {
        match c {
            None => true,
            Some(c) => {
                if c.width != 1 || c.content.len() != 1 || c.content != " " {
                    return false;
                }
                // NOTE: This assumes that the terminal supports bce terminfo
                // capability.
                c.style.underline == rusty_x_ansi::style::Underline::None
                    && c.style.attrs
                        & !(Attr::BOLD.bits()
                            | Attr::FAINT.bits()
                            | Attr::ITALIC.bits()
                            | Attr::BLINK.bits()
                            | Attr::RAPID_BLINK.bits())
                        == 0
                    && c.link.as_ref().map(|l| l.is_zero()).unwrap_or(true)
            }
        }
    }

    /// emitRange emits a range of cells to the buffer. It is equivalent to
    /// calling [Self::put_cell] for each cell in the range. This is optimized
    /// to use [rusty_x_ansi::erase_character] and
    /// [rusty_x_ansi::repeat_previous_character].
    /// Returns whether the cursor is at the end of interval or somewhere in
    /// the middle.
    fn emit_range(&mut self, newbuf: &RenderBuffer, line: &[Cell], n: usize) -> bool {
        let has_ech = self.caps.contains(Capabilities::ECH);
        let has_rep = self.caps.contains(Capabilities::REP);
        let mut line = line;
        let mut n = n;

        if has_ech || has_rep {
            while n > 0 {
                let mut count;
                while n > 1 && !cell_equal(line.first(), line.get(1)) {
                    self.put_cell(newbuf, line.first());
                    line = &line[1..];
                    n -= 1;
                }

                let cell0 = line.first().cloned();
                if n == 1 {
                    self.put_cell(newbuf, cell0.as_ref());
                    return false;
                }

                count = 2;
                while count < n && cell_equal(line.get(count), cell0.as_ref()) {
                    count += 1;
                }

                let ech = erase_character(count as i32);
                let cup = cursor_position(
                    (self.cur.x + count as i64 + 1) as i32,
                    (self.cur.y + 1) as i32,
                );
                let rep = repeat_previous_character(count as i32);
                let cell0 = cell0.unwrap_or_else(empty_cell);
                if has_ech && count > ech.len() + cup.len() && Self::can_clear_with(Some(&cell0)) {
                    self.update_pen(Some(&cell0));
                    self.push(&ech);

                    // If this is the last cell, we don't need to move the
                    // cursor.
                    if count < n {
                        self.move_to_pos(Some(newbuf), self.cur.x + count as i64, self.cur.y);
                    } else {
                        return true; // cursor in the middle
                    }
                } else if has_rep
                    && count > rep.len()
                    && cell0.content.len() == 1
                    && cell0.content.as_bytes()[0] > US
                    && cell0.content.as_bytes()[0] < DEL
                {
                    // We only support ASCII characters.
                    let wrap_possible = self.cur.x + count as i64 >= newbuf.width() as i64;
                    let mut rep_count = count;
                    if wrap_possible {
                        rep_count -= 1;
                    }

                    self.update_pen(Some(&cell0));
                    self.put_cell(newbuf, Some(&cell0));
                    rep_count -= 1; // cell0 is a single width cell ASCII character

                    self.push(&repeat_previous_character(rep_count as i32));
                    self.cur.x += rep_count as i64;
                    if wrap_possible {
                        self.put_cell(newbuf, Some(&cell0));
                    }
                } else {
                    for i in 0..count {
                        self.put_cell(newbuf, line.get(i));
                    }
                }

                line = &line[count.clamp(0, line.len())..];
                n -= count;
            }

            return false;
        }

        for i in 0..n {
            self.put_cell(newbuf, line.get(i));
        }

        false
    }

    /// putRange puts a range of cells from the old line to the new line.
    /// Returns whether the cursor is at the end of interval or somewhere in
    /// the middle.
    fn put_range(
        &mut self,
        newbuf: &RenderBuffer,
        old_line: &[Cell],
        new_line: &[Cell],
        y: i64,
        start: usize,
        end: usize,
    ) -> bool {
        let inline = (cursor_position((start + 1) as i32, (y + 1) as i32)
            .len()
            .min(rusty_x_ansi::horizontal_position_absolute((start + 1) as i32).len()))
        .min(cursor_forward((start + 1) as i32).len());
        // Go tolerates out-of-order ranges (upstream's `emitRange` no-ops
        // on negative counts); mirror that instead of overflowing.
        if start > end {
            return false;
        }
        if end - start + 1 > inline {
            let mut j = start;
            let mut same = 0;
            let mut start = start;
            while j <= end {
                let old_cell = old_line.get(j);
                let new_cell = new_line.get(j);
                if same == 0
                    && old_cell.map(|c| c.is_wide_placeholder()).unwrap_or(false)
                    && new_cell.map(|c| c.is_wide_placeholder()).unwrap_or(false)
                {
                    j += 1;
                    continue;
                }
                if cell_equal(old_cell, new_cell) {
                    same += 1;
                } else {
                    if same > end - start {
                        self.emit_range(newbuf, &new_line[start..], j - same - start);
                        self.move_to_pos(Some(newbuf), j as i64, y);
                        start = j;
                    }
                    same = 0;
                }
                j += 1;
            }

            let i = self.emit_range(newbuf, &new_line[start..], j - same - start);

            // Always return 1 for the next move after a putRange if we found
            // identical characters at end of interval.
            if same == 0 {
                return i;
            }
            return true;
        }

        self.emit_range(newbuf, &new_line[start..], end - start + 1)
    }

    /// clearToEnd clears the screen from the current cursor position to the
    /// end of line.
    fn clear_to_end(&mut self, newbuf: &RenderBuffer, blank: &Cell, force: bool) {
        let mut force = force;
        if self.cur.y >= 0 {
            if let Some(curline) = self.curbuf.line(self.cur.y as usize).cloned() {
                // We use the newbuf width because the current buffer might be
                // smaller than the new buffer during a resize operation.
                let mut curline = curline;
                let mut force_any = false;
                for j in self.cur.x as usize..newbuf.width() {
                    if let Some(c) = curline.0.get(j) {
                        if !cell_equal(Some(c), Some(blank)) {
                            curline.0[j] = blank.clone();
                            force_any = true;
                        }
                    }
                }
                if force_any {
                    force = true;
                }
                self.curbuf.set_line(self.cur.y as usize, curline);
            }
        }

        if force {
            self.update_pen(Some(blank));
            let count = newbuf.width() as i64 - self.cur.x;
            if self.el0_cost() <= count as usize {
                self.push(ERASE_LINE_RIGHT);
            } else {
                for _ in 0..count {
                    self.put_cell(newbuf, Some(blank));
                }
            }
        }
    }

    /// el0Cost returns the cost of using [rusty_x_ansi::erase_line] 0.
    fn el0_cost(&self) -> usize {
        if self.caps.0 != 0 {
            return 0;
        }
        ERASE_LINE_RIGHT.len()
    }

    /// transformLine transforms the given line in the current window to the
    /// corresponding line in the new window.
    fn transform_line(&mut self, newbuf: &RenderBuffer, y: usize) {
        self.transform_line_inner(newbuf, y);
        // Upstream: `defer s.reanchorWideLine(newbuf)`.
        self.reanchor_wide_line(newbuf);
    }

    /// The body of [TerminalRenderer::transform_line]; kept separate so the
    /// re-anchor runs on every exit path like Go's deferred call.
    fn transform_line_inner(&mut self, newbuf: &RenderBuffer, y: usize) {
        let width = newbuf.width();
        let mut first_cell = 0usize;
        let mut o_last_cell: i64;
        let mut n_last_cell: i64;
        let old_line = self.curbuf.line(y).cloned().unwrap_or_default();
        let new_line = newbuf.line(y).cloned().unwrap_or_default();

        self.line_had_wide = false;

        // Find the first changed cell in the line
        let mut blank = new_line.0.first().cloned();

        // It might be cheaper to clear leading spaces with EraseLineLeft.
        if Self::can_clear_with(blank.as_ref()) {
            let mut o_first_cell = 0usize;
            let mut n_first_cell = 0usize;
            while o_first_cell < self.curbuf.width() {
                if !cell_equal(old_line.0.get(o_first_cell), blank.as_ref()) {
                    break;
                }
                o_first_cell += 1;
            }
            while n_first_cell < width {
                if !cell_equal(new_line.0.get(n_first_cell), blank.as_ref()) {
                    break;
                }
                n_first_cell += 1;
            }

            if n_first_cell == o_first_cell {
                first_cell = n_first_cell;

                // Find the first differing cell
                while first_cell < width
                    && cell_equal(old_line.0.get(first_cell), new_line.0.get(first_cell))
                {
                    first_cell += 1;
                }
            } else if o_first_cell > n_first_cell {
                first_cell = n_first_cell;
            } else if o_first_cell < n_first_cell {
                first_cell = o_first_cell;
                let el1_cost = ERASE_LINE_LEFT.len();
                if el1_cost < n_first_cell - o_first_cell {
                    if n_first_cell >= width {
                        self.move_to_pos(Some(newbuf), 0, y as i64);
                        self.update_pen(blank.as_ref());
                        self.push(ERASE_LINE_RIGHT);
                    } else {
                        self.move_to_pos(Some(newbuf), (n_first_cell - 1) as i64, y as i64);
                        self.update_pen(blank.as_ref());
                        self.push(ERASE_LINE_LEFT);
                    }

                    while first_cell < n_first_cell {
                        let mut old_line_mut = self.curbuf.line(y).cloned().unwrap_or_default();
                        if let Some(c) = old_line_mut.0.get_mut(first_cell) {
                            *c = blank.clone().unwrap_or_else(empty_cell);
                        }
                        self.curbuf.set_line(y, old_line_mut);
                        first_cell += 1;
                    }
                }
            }
        } else {
            // Find the first differing cell
            while first_cell < width
                && cell_equal(new_line.0.get(first_cell), old_line.0.get(first_cell))
            {
                first_cell += 1;
            }
        }

        // If we didn't find one, we're done
        if first_cell >= width {
            return;
        }

        blank = new_line.0.get(width - 1).cloned();
        if let Some(ref b) = blank {
            if !Self::can_clear_with(Some(b)) {
                // Find the last differing cell
                let mut n_last = width as i64 - 1;
                while n_last > first_cell as i64
                    && cell_equal(
                        new_line.0.get(n_last as usize),
                        old_line.0.get(n_last as usize),
                    )
                {
                    n_last -= 1;
                }

                if n_last >= first_cell as i64 {
                    self.move_to_pos(Some(newbuf), first_cell as i64, y as i64);
                    self.put_range(
                        newbuf,
                        &old_line.0,
                        &new_line.0,
                        y as i64,
                        first_cell,
                        n_last as usize,
                    );
                    self.copy_old_line(y, &new_line);
                }

                return;
            }
        }

        // Find last non-blank cell in the old line.
        o_last_cell = width as i64 - 1;
        while o_last_cell > first_cell as i64
            && cell_equal(old_line.0.get(o_last_cell as usize), blank.as_ref())
        {
            o_last_cell -= 1;
        }

        // Find last non-blank cell in the new line.
        n_last_cell = width as i64 - 1;
        while n_last_cell > first_cell as i64
            && cell_equal(new_line.0.get(n_last_cell as usize), blank.as_ref())
        {
            n_last_cell -= 1;
        }

        let blank_cell = blank.clone().unwrap_or_else(empty_cell);
        if n_last_cell == first_cell as i64
            && self.el0_cost() < (o_last_cell - n_last_cell) as usize
        {
            self.move_to_pos(Some(newbuf), first_cell as i64, y as i64);
            if !cell_equal(new_line.0.get(first_cell), blank.as_ref()) {
                self.put_cell(newbuf, new_line.0.get(first_cell));
            }
            self.clear_to_end(newbuf, &blank_cell, false);
        } else if n_last_cell != o_last_cell
            && !cell_equal(
                new_line.0.get(n_last_cell as usize),
                old_line.0.get(o_last_cell as usize),
            )
        {
            self.move_to_pos(Some(newbuf), first_cell as i64, y as i64);
            // Upstream uses signed ints; a negative difference falls through
            // to the else branch, which saturating subtraction reproduces.
            if o_last_cell.saturating_sub(n_last_cell) > self.el0_cost() as i64 {
                if self.put_range(
                    newbuf,
                    &old_line.0,
                    &new_line.0,
                    y as i64,
                    first_cell,
                    n_last_cell as usize,
                ) {
                    self.move_to_pos(Some(newbuf), n_last_cell + 1, y as i64);
                }
                self.clear_to_end(newbuf, &blank_cell, false);
            } else {
                let n = n_last_cell.max(o_last_cell) as usize;
                self.put_range(newbuf, &old_line.0, &new_line.0, y as i64, first_cell, n);
            }
        } else {
            let n_last_non_blank = n_last_cell;
            let o_last_non_blank = o_last_cell;

            // Find the last cells that really differ.
            // Can be -1 if no cells differ.
            while cell_equal(
                new_line.0.get(n_last_cell as usize),
                old_line.0.get(o_last_cell as usize),
            ) {
                // Upstream's `Line.At(-1)` returns nil; `cellEqual(nil, nil)`
                // is true, so the loop decrements through 0 down to -1 and
                // breaks. Mirror that with explicit bounds checks instead of
                // overflowing the usize cast.
                let new_before = if n_last_cell == 0 {
                    None
                } else {
                    new_line.0.get(n_last_cell as usize - 1)
                };
                let old_before = if o_last_cell == 0 {
                    None
                } else {
                    old_line.0.get(o_last_cell as usize - 1)
                };
                if !cell_equal(new_before, old_before) {
                    break;
                }
                n_last_cell -= 1;
                o_last_cell -= 1;
                if n_last_cell == -1 || o_last_cell == -1 {
                    break;
                }
            }

            let mut n = o_last_cell.min(n_last_cell);
            if n >= first_cell as i64 {
                self.move_to_pos(Some(newbuf), first_cell as i64, y as i64);
                self.put_range(
                    newbuf,
                    &old_line.0,
                    &new_line.0,
                    y as i64,
                    first_cell,
                    n as usize,
                );
            }

            if o_last_cell < n_last_cell {
                let m = n_last_non_blank.max(o_last_non_blank);
                if n != 0 {
                    while n > 0 {
                        let wide = new_line.0.get(n as usize + 1);
                        if !wide.map(|c| c.is_wide_placeholder()).unwrap_or(false) {
                            break;
                        }
                        n -= 1;
                        o_last_cell -= 1;
                    }
                } else if n >= first_cell as i64
                    && new_line
                        .0
                        .get(n as usize)
                        .map(|c| c.width > 1)
                        .unwrap_or(false)
                {
                    let mut next = new_line.0.get(n as usize + 1).cloned();
                    while next
                        .as_ref()
                        .map(|c| c.is_wide_placeholder())
                        .unwrap_or(false)
                    {
                        n += 1;
                        o_last_cell += 1;
                        next = new_line.0.get(n as usize + 1).cloned();
                    }
                }

                self.move_to_pos(Some(newbuf), n + 1, y as i64);
                let ich_cost = 3 + n_last_cell - o_last_cell;
                if self.caps.contains(Capabilities::ICH)
                    && (n_last_cell < n_last_non_blank || ich_cost > m - n)
                {
                    self.put_range(
                        newbuf,
                        &old_line.0,
                        &new_line.0,
                        y as i64,
                        (n + 1) as usize,
                        m as usize,
                    );
                } else {
                    self.insert_cells(
                        newbuf,
                        &new_line.0[(n + 1) as usize..],
                        (n_last_cell - o_last_cell) as usize,
                    );
                }
            } else if o_last_cell > n_last_cell {
                self.move_to_pos(Some(newbuf), n + 1, y as i64);
                let dch_cost = 3 + o_last_cell - n_last_cell;
                if dch_cost > ERASE_LINE_RIGHT.len() as i64 + n_last_non_blank - (n + 1) {
                    if self.put_range(
                        newbuf,
                        &old_line.0,
                        &new_line.0,
                        y as i64,
                        (n + 1) as usize,
                        n_last_non_blank as usize,
                    ) {
                        self.move_to_pos(Some(newbuf), n_last_non_blank + 1, y as i64);
                    }
                    self.clear_to_end(newbuf, &blank_cell, false);
                } else {
                    self.update_pen(blank.as_ref());
                    self.delete_cells(o_last_cell - n_last_cell);
                }
            }
        }

        // Update the old line with the new line
        self.copy_old_line(y, &new_line);
    }

    /// Copies the new line into the current buffer's line at y.
    fn copy_old_line(&mut self, y: usize, new_line: &Line) {
        match self.curbuf.line(y).cloned() {
            Some(mut old) => {
                let n = old.0.len().min(new_line.0.len());
                for k in 0..n {
                    old.0[k] = new_line.0[k].clone();
                }
                self.curbuf.set_line(y, old);
            }
            None => {
                self.curbuf.set_line(y, new_line.clone());
            }
        }
    }

    /// reanchorWideLine re-anchors the cursor with a single absolute
    /// horizontal move after a line that contained a wide cell.
    fn reanchor_wide_line(&mut self, newbuf: &RenderBuffer) {
        if !self.line_had_wide || self.flags.contains(TFlag::GRAPHEME_WIDTH) {
            return;
        }
        self.line_had_wide = false;
        if self.at_phantom || self.cur.x < 0 || self.cur.x >= newbuf.width() as i64 {
            return;
        }
        self.push(&cursor_horizontal_absolute((self.cur.x + 1) as i32));
    }

    /// deleteCells deletes the count cells at the current cursor position.
    fn delete_cells(&mut self, count: i64) {
        self.push(&delete_character(count as i32));
    }

    /// insertCells inserts the count cells pointed by the given line at the
    /// current cursor position.
    fn insert_cells(&mut self, newbuf: &RenderBuffer, line: &[Cell], count: usize) {
        let supports_ich = self.caps.contains(Capabilities::ICH);
        if supports_ich {
            // Use ICH as an optimization.
            self.push(&insert_character(count as i32));
        } else {
            // Otherwise, use IRM mode.
            self.push(SET_MODE_INSERT_REPLACE);
        }

        let mut count = count;
        let mut i = 0;
        while count > 0 {
            self.put_attr_cell(newbuf, line.get(i));
            count -= 1;
            i += 1;
        }

        if !supports_ich {
            self.push(RESET_MODE_INSERT_REPLACE);
        }
    }

    /// clearToBottom clears the screen from the current cursor position to
    /// the end of the screen.
    fn clear_to_bottom(&mut self, blank: &Cell) {
        let mut row = self.cur.y;
        let col = self.cur.x;
        if row < 0 {
            row = 0;
        }

        self.update_pen(Some(blank));
        self.push(ERASE_SCREEN_BELOW);
        // Clear the rest of the current line. Upstream uses signed ints and
        // lets ClearArea clamp negative widths; saturating subtraction
        // mirrors that without overflowing.
        self.curbuf.clear_area(rect(
            col.max(0) as usize,
            row as usize,
            self.curbuf.width().saturating_sub(col.max(0) as usize),
            1,
        ));
        // Clear everything below the current line
        self.curbuf.clear_area(rect(
            0,
            row as usize + 1,
            self.curbuf.width(),
            self.curbuf.height().saturating_sub(row as usize + 1),
        ));
    }

    /// clearBottom tests if clearing the end of the screen would satisfy part
    /// of the screen update. It returns the top line.
    fn clear_bottom(&mut self, newbuf: &RenderBuffer, total: usize) -> usize {
        if total == 0 {
            return 0;
        }

        let mut top = total;
        let last = self.curbuf.width().min(newbuf.width());
        let blank = self.cur.cell.clone();
        let can_clear_with_blank = Self::can_clear_with(Some(&blank));

        if can_clear_with_blank {
            let mut row: i64 = total as i64 - 1;
            while row >= 0 {
                let old_line = self.curbuf.line(row as usize).cloned().unwrap_or_default();
                let new_line = newbuf.line(row as usize).cloned().unwrap_or_default();

                let mut ok = true;
                let mut col = 0usize;
                while ok && col < last {
                    ok = cell_equal(new_line.0.get(col), Some(&blank));
                    col += 1;
                }
                if !ok {
                    break;
                }

                col = 0;
                while ok && col < last {
                    ok = cell_equal(old_line.0.get(col), Some(&blank));
                    col += 1;
                }
                if !ok {
                    top = row as usize;
                }

                row -= 1;
            }

            if top < total {
                self.move_to_pos(Some(newbuf), 0, top.saturating_sub(1) as i64); // top is 1-based
                self.clear_to_bottom(&blank);
                if !self.oldhash.is_empty() && !self.newhash.is_empty() && row >= 0 {
                    for r in top..newbuf.height() {
                        if r < self.oldhash.len() && r < self.newhash.len() {
                            self.oldhash[r] = self.newhash[r];
                        }
                    }
                }
            }
        }

        top
    }

    /// clearScreen clears the screen and put cursor at home.
    fn clear_screen(&mut self, blank: &Cell) {
        self.update_pen(Some(blank));
        self.push(CURSOR_HOME_POSITION);
        self.push(ERASE_ENTIRE_SCREEN);
        self.cur.x = 0;
        self.cur.y = 0;
        self.curbuf.fill(Some(blank));
    }

    /// clearBelow clears everything below and including the row.
    fn clear_below(&mut self, newbuf: &RenderBuffer, blank: Option<&Cell>, row: i64) {
        self.move_to_pos(Some(newbuf), 0, row);
        let blank = match blank {
            Some(b) => b.clone(),
            None => self.cur.cell.clone(),
        };
        self.clear_to_bottom(&blank);
    }

    /// clearUpdate forces a screen redraw.
    fn clear_update(&mut self, newbuf: &RenderBuffer) {
        let blank = self.cur.cell.clone();
        let mut non_empty;
        if self.flags.contains(TFlag::FULLSCREEN) {
            // XXX: We're using the maximum height of the two buffers to
            // ensure we write newly added lines to the screen.
            non_empty = self.curbuf.height().max(newbuf.height());
            self.clear_screen(&blank);
        } else {
            non_empty = newbuf.height();
            self.clear_below(newbuf, Some(&blank), 0);
        }
        non_empty = self.clear_bottom(newbuf, non_empty);
        for i in 0..non_empty.min(newbuf.height()) {
            self.transform_line(newbuf, i);
        }
    }

    /// scrollOptimize optimizes the screen to transform the old buffer into
    /// the new buffer.
    fn scroll_optimize(&mut self, newbuf: &RenderBuffer) {
        let height = newbuf.height();
        if self.oldnum.len() < height {
            self.oldnum.resize(height, 0);
        }

        // Calculate the indices
        self.update_hashmap(newbuf);
        if self.hashtab.len() < height {
            return;
        }

        // Pass 1 - from top to bottom scrolling up
        let mut i: i64 = 0;
        while i < height as i64 {
            while i < height as i64
                && (self.oldnum[i as usize] == NEW_INDEX || self.oldnum[i as usize] <= i)
            {
                i += 1;
            }
            if i >= height as i64 {
                break;
            }

            let shift = self.oldnum[i as usize] - i; // shift > 0
            let start = i;

            i += 1;
            while i < height as i64
                && self.oldnum[i as usize] != NEW_INDEX
                && self.oldnum[i as usize] - i == shift
            {
                i += 1;
            }
            let end = i - 1 + shift;

            if !self.scrolln(newbuf, shift, start, end, height as i64 - 1) {
                continue;
            }
        }

        // Pass 2 - from bottom to top scrolling down
        let mut i: i64 = height as i64 - 1;
        while i >= 0 {
            while i >= 0 && (self.oldnum[i as usize] == NEW_INDEX || self.oldnum[i as usize] >= i) {
                i -= 1;
            }
            if i < 0 {
                break;
            }

            let shift = self.oldnum[i as usize] - i; // shift < 0
            let end = i;

            i -= 1;
            while i >= 0
                && self.oldnum[i as usize] != NEW_INDEX
                && self.oldnum[i as usize] - i == shift
            {
                i -= 1;
            }

            let start = i + 1 - (-shift);
            if !self.scrolln(newbuf, shift, start, end, height as i64 - 1) {
                continue;
            }
        }
    }

    /// scrolln scrolls the screen up by n lines.
    fn scrolln(&mut self, newbuf: &RenderBuffer, n: i64, top: i64, bot: i64, max_y: i64) -> bool {
        let blank = self.cur.cell.clone();
        let mut v;
        if n > 0 {
            // Scroll up (forward)
            v = self.scroll_up(newbuf, n, top, bot, 0, max_y, &blank);
            if !v {
                self.push(&set_top_bottom_margins((top + 1) as i32, (bot + 1) as i32));

                // XXX: How should we handle this in inline mode when not
                // using alternate screen?
                self.cur.x = -1;
                self.cur.y = -1;
                v = self.scroll_up(newbuf, n, top, bot, top, bot, &blank);
                self.push(&set_top_bottom_margins(1, (max_y + 1) as i32));
                self.cur.x = -1;
                self.cur.y = -1;
            }

            if !v {
                v = self.scroll_idl(newbuf, n, top, bot - n + 1, &blank);
            }
        } else if n < 0 {
            // Scroll down (backward)
            v = self.scroll_down(newbuf, -n, top, bot, 0, max_y, &blank);
            if !v {
                self.push(&set_top_bottom_margins((top + 1) as i32, (bot + 1) as i32));

                // XXX: How should we handle this in inline mode when not
                // using alternate screen?
                self.cur.x = -1;
                self.cur.y = -1;
                v = self.scroll_down(newbuf, -n, top, bot, top, bot, &blank);
                self.push(&set_top_bottom_margins(1, (max_y + 1) as i32));
                self.cur.x = -1;
                self.cur.y = -1;

                if !v {
                    v = self.scroll_idl(newbuf, -n, bot + n + 1, top, &blank);
                }
            }
        } else {
            return false;
        }

        if !v {
            return false;
        }

        self.scroll_buffer(n, top, bot, &blank);
        self.scroll_oldhash(n, top, bot);

        true
    }

    /// scrollBuffer scrolls the buffer by n lines.
    fn scroll_buffer(&mut self, n: i64, top: i64, bot: i64, blank: &Cell) {
        let height = self.curbuf.height() as i64;
        if top < 0 || bot < top || bot >= height {
            // Nothing to scroll
            return;
        }

        if n < 0 {
            // shift n lines downwards
            let limit = top - n;
            let mut line = bot;
            while line >= limit && line >= 0 && line >= top {
                self.curbuf.copy_line(line as usize, (line + n) as usize);
                line -= 1;
            }
            let mut line = top;
            while line < limit && line < height && line <= bot {
                self.curbuf
                    .buffer
                    .fill_area(Some(blank), rect(0, line as usize, self.curbuf.width(), 1));
                line += 1;
            }
        }

        if n > 0 {
            // shift n lines upwards
            let limit = bot - n;
            let mut line = top;
            while line <= limit && line < height && line <= bot {
                self.curbuf.copy_line(line as usize, (line + n) as usize);
                line += 1;
            }
            let mut line = bot;
            while line > limit && line >= 0 && line >= top {
                self.curbuf
                    .buffer
                    .fill_area(Some(blank), rect(0, line as usize, self.curbuf.width(), 1));
                line -= 1;
            }
        }

        self.touch_line(top as usize, (bot - top + 1) as usize, true);
    }

    /// touchLine marks the line as touched.
    fn touch_line(&mut self, y: usize, n: usize, changed: bool) {
        let height = self.curbuf.height();
        if n == 0 || y >= height {
            return; // Nothing to touch
        }

        let width = self.curbuf.width();
        let mut i = y;
        while i < y + n && i < height {
            if changed {
                self.curbuf.touch_line(0, i, width);
            } else {
                self.curbuf.touched[i] = None;
            }
            i += 1;
        }
    }

    /// Mirrors the upstream Go signature `scrollUp(newbuf, n, top, bot,
    /// minY, maxY, blank)` 1:1.
    #[allow(clippy::too_many_arguments)]
    fn scroll_up(
        &mut self,
        newbuf: &RenderBuffer,
        n: i64,
        top: i64,
        bot: i64,
        min_y: i64,
        max_y: i64,
        blank: &Cell,
    ) -> bool {
        if n == 1 && top == min_y && bot == max_y {
            self.move_to_pos(Some(newbuf), 0, bot);
            self.update_pen(Some(blank));
            self.push_byte(b'\n');
        } else if n == 1 && bot == max_y {
            self.move_to_pos(Some(newbuf), 0, top);
            self.update_pen(Some(blank));
            self.push(&delete_line(1));
        } else if top == min_y && bot == max_y {
            let supports_su = self.caps.contains(Capabilities::SU);
            self.move_to_pos(Some(newbuf), 0, bot);
            self.update_pen(Some(blank));
            if supports_su {
                self.push(&scroll_up(n as i32));
            } else {
                self.push(&"\n".repeat(n as usize));
            }
        } else if bot == max_y {
            self.move_to_pos(Some(newbuf), 0, top);
            self.update_pen(Some(blank));
            self.push(&delete_line(n as i32));
        } else {
            return false;
        }
        true
    }

    /// Mirrors the upstream Go signature `scrollDown(newbuf, n, top, bot,
    /// minY, maxY, blank)` 1:1.
    #[allow(clippy::too_many_arguments)]
    fn scroll_down(
        &mut self,
        newbuf: &RenderBuffer,
        n: i64,
        top: i64,
        bot: i64,
        min_y: i64,
        max_y: i64,
        blank: &Cell,
    ) -> bool {
        if n == 1 && top == min_y && bot == max_y {
            self.move_to_pos(Some(newbuf), 0, top);
            self.update_pen(Some(blank));
            self.push(REVERSE_INDEX);
        } else if n == 1 && bot == max_y {
            self.move_to_pos(Some(newbuf), 0, top);
            self.update_pen(Some(blank));
            self.push(&insert_line(1));
        } else if top == min_y && bot == max_y {
            self.move_to_pos(Some(newbuf), 0, top);
            self.update_pen(Some(blank));
            if self.caps.contains(Capabilities::SD) {
                self.push(&scroll_down(n as i32));
            } else {
                self.push(&REVERSE_INDEX.repeat(n as usize));
            }
        } else if bot == max_y {
            self.move_to_pos(Some(newbuf), 0, top);
            self.update_pen(Some(blank));
            self.push(&insert_line(n as i32));
        } else {
            return false;
        }
        true
    }

    /// scrollIdl scrolls the screen n lines by using
    /// [rusty_x_ansi::delete_line] at del and using
    /// [rusty_x_ansi::insert_line] at ins.
    fn scroll_idl(
        &mut self,
        newbuf: &RenderBuffer,
        n: i64,
        del: i64,
        ins: i64,
        blank: &Cell,
    ) -> bool {
        if n < 0 {
            return false;
        }

        // Delete lines
        self.move_to_pos(Some(newbuf), 0, del);
        self.update_pen(Some(blank));
        self.push(&delete_line(n as i32));

        // Insert lines
        self.move_to_pos(Some(newbuf), 0, ins);
        self.update_pen(Some(blank));
        self.push(&insert_line(n as i32));

        true
    }

    /// updateHashmap updates the hashmap with the new hash value.
    fn update_hashmap(&mut self, newbuf: &RenderBuffer) {
        let height = newbuf.height();

        if self.oldhash.len() >= height && self.newhash.len() >= height {
            // rehash changed lines
            for i in 0..height {
                if newbuf.touched.get(i).map(|t| t.is_some()).unwrap_or(true) {
                    self.oldhash[i] = hash_line(self.curbuf.line(i).unwrap_or(&Line(Vec::new())));
                    self.newhash[i] = hash_line(newbuf.line(i).unwrap_or(&Line(Vec::new())));
                }
            }
        } else {
            // rehash all
            if self.oldhash.len() != height {
                self.oldhash = vec![0; height];
            }
            if self.newhash.len() != height {
                self.newhash = vec![0; height];
            }
            for i in 0..height {
                self.oldhash[i] = hash_line(self.curbuf.line(i).unwrap_or(&Line(Vec::new())));
                self.newhash[i] = hash_line(newbuf.line(i).unwrap_or(&Line(Vec::new())));
            }
        }

        self.hashtab = vec![Hashmap::default(); (height + 1) * 2];
        for i in 0..height {
            let hashval = self.oldhash[i];

            // Find matching hash or empty slot
            let mut idx = 0usize;
            while idx < self.hashtab.len() && self.hashtab[idx].value != 0 {
                if self.hashtab[idx].value == hashval {
                    break;
                }
                idx += 1;
            }

            self.hashtab[idx].value = hashval; // in case this is a new hash
            self.hashtab[idx].oldcount += 1;
            self.hashtab[idx].oldindex = i as i32;
        }
        for i in 0..height {
            let hashval = self.newhash[i];

            // Find matching hash or empty slot
            let mut idx = 0usize;
            while idx < self.hashtab.len() && self.hashtab[idx].value != 0 {
                if self.hashtab[idx].value == hashval {
                    break;
                }
                idx += 1;
            }

            self.hashtab[idx].value = hashval; // in case this is a new hash
            self.hashtab[idx].newcount += 1;
            self.hashtab[idx].newindex = i as i32;
            self.oldnum[i] = NEW_INDEX; // init old indices slice
        }

        // Mark line pair corresponding to unique hash pairs.
        for i in 0..self.hashtab.len() {
            if self.hashtab[i].value == 0 {
                break;
            }
            let hsp = self.hashtab[i];
            if hsp.oldcount == 1 && hsp.newcount == 1 && hsp.oldindex != hsp.newindex {
                self.oldnum[hsp.newindex as usize] = hsp.oldindex as i64;
            }
        }

        self.grow_hunks(newbuf);

        // Eliminate bad or impossible shifts.
        let mut i = 0usize;
        while i < height {
            let mut start;

            while i < height && self.oldnum[i] == NEW_INDEX {
                i += 1;
            }
            if i >= height {
                break;
            }
            start = i;
            let shift = self.oldnum[i] - i as i64;
            i += 1;
            while i < height && self.oldnum[i] != NEW_INDEX && self.oldnum[i] - i as i64 == shift {
                i += 1;
            }
            let size = i - start;
            if size < 3 || (size as i64) + ((size / 8).min(2) as i64) < shift.abs() {
                while start < i {
                    self.oldnum[start] = NEW_INDEX;
                    start += 1;
                }
            }
        }

        // After clearing invalid hunks, try grow the rest.
        self.grow_hunks(newbuf);
    }

    /// scrollOldhash scrolls the oldhash slice by 'n' lines between 'top' and
    /// 'bot'.
    fn scroll_oldhash(&mut self, n: i64, top: i64, bot: i64) {
        if self.oldhash.is_empty() {
            return;
        }

        let size = bot - top + 1 - n.abs();
        if n > 0 {
            // Move existing hashes up
            let src = (top + n) as usize;
            let dst = top as usize;
            for k in 0..size as usize {
                self.oldhash[dst + k] = self.oldhash[src + k];
            }
            // Recalculate hashes for newly shifted-in lines
            let mut i = bot;
            while i > bot - n {
                self.oldhash[i as usize] =
                    hash_line(self.curbuf.line(i as usize).unwrap_or(&Line(Vec::new())));
                i -= 1;
            }
        } else {
            // Move existing hashes down
            let src = top as usize;
            let dst = (top - n) as usize;
            for k in 0..size as usize {
                self.oldhash[dst + k] = self.oldhash[src + k];
            }
            // Recalculate hashes for newly shifted-in lines
            let mut i = top;
            while i < top - n {
                self.oldhash[i as usize] =
                    hash_line(self.curbuf.line(i as usize).unwrap_or(&Line(Vec::new())));
                i += 1;
            }
        }
    }

    /// growHunks grows hunks forward and backward where hash-matching or cost
    /// effective.
    fn grow_hunks(&mut self, newbuf: &RenderBuffer) {
        let height = newbuf.height();
        let mut back_limit: i64 = 0; // limits for cells to fill
        let mut back_ref_limit: i64 = 0; // limit for references
        let mut i: i64 = 0;
        let mut next_hunk: i64;

        while i < height as i64 && self.oldnum[i as usize] == NEW_INDEX {
            i += 1;
        }
        while (i as usize) < height {
            let start = i;
            let shift = self.oldnum[i as usize] - i;

            // get forward limit
            i = start + 1;
            while (i as usize) < height
                && self.oldnum[i as usize] != NEW_INDEX
                && self.oldnum[i as usize] - i == shift
            {
                i += 1;
            }

            let end: i64 = i;
            while (i as usize) < height && self.oldnum[i as usize] == NEW_INDEX {
                i += 1;
            }

            next_hunk = i;
            let mut forward_limit = i;
            let forward_ref_limit: i64 = if (i as usize) >= height || self.oldnum[i as usize] >= i {
                i
            } else {
                self.oldnum[i as usize]
            };

            i = start - 1;

            // grow back
            if shift < 0 {
                back_limit = back_ref_limit + (-shift);
            }
            while i >= back_limit {
                if self.newhash[i as usize] == self.oldhash[(i + shift) as usize]
                    || self.cost_effective(newbuf, i + shift, i, shift < 0)
                {
                    self.oldnum[i as usize] = i + shift;
                } else {
                    break;
                }
                i -= 1;
            }

            i = end;
            // grow forward
            if shift > 0 {
                forward_limit = forward_ref_limit - shift;
            }
            while i < forward_limit {
                if self.newhash[i as usize] == self.oldhash[(i + shift) as usize]
                    || self.cost_effective(newbuf, i + shift, i, shift > 0)
                {
                    self.oldnum[i as usize] = i + shift;
                } else {
                    break;
                }
                i += 1;
            }

            back_limit = i;
            back_ref_limit = back_limit;
            if shift > 0 {
                back_ref_limit += shift;
            }
            i = next_hunk;
        }
    }

    /// costEffective returns true if the cost of moving line 'from' to line
    /// 'to' seems to be cost effective.
    fn cost_effective(&mut self, newbuf: &RenderBuffer, from: i64, to: i64, blank: bool) -> bool {
        if from == to {
            return false;
        }

        let mut new_from = self.oldnum[from as usize];
        if new_from == NEW_INDEX {
            new_from = from;
        }

        let oto = self.curbuf.line(to as usize).cloned().unwrap_or_default();
        let nto = newbuf.line(to as usize).cloned().unwrap_or_default();
        let ofrom = self.curbuf.line(from as usize).cloned().unwrap_or_default();
        let nfrom = newbuf.line(from as usize).cloned().unwrap_or_default();
        let onew_from = self
            .curbuf
            .line(new_from as usize)
            .cloned()
            .unwrap_or_default();

        // Calculate costs before moving.
        let mut cost_before_move;
        if blank {
            // Cost of updating blank line at destination.
            cost_before_move = self.update_cost_blank(&nto);
        } else {
            // Cost of updating exiting line at destination.
            cost_before_move = self.update_cost(&oto, &nto);
        }

        // Add cost of updating source line. (Upstream uses the line the
        // source *moves from* — `curbuf.Line(newFrom)` — not the source's
        // current row.)
        cost_before_move += self.update_cost(&onew_from, &nfrom);

        // Calculate costs after moving.
        let mut cost_after_move;
        if new_from == from {
            // Source becomes blank after move
            cost_after_move = self.update_cost_blank(&nfrom);
        } else {
            // Source gets updated from another line
            cost_after_move = self.update_cost(&onew_from, &nfrom);
        }

        // Add cost of moving source line to destination
        cost_after_move += self.update_cost(&ofrom, &nto);

        // Return true if moving is cost effective (costs less or equal)
        cost_before_move >= cost_after_move
    }

    fn update_cost(&mut self, from: &Line, to: &Line) -> i32 {
        let mut cost = 0;
        let mut fidx = 0usize;
        let mut tidx = 0usize;
        let w = self.curbuf.width();
        let mut i = w;
        while i > 0 {
            if !cell_equal(from.0.get(fidx), to.0.get(tidx)) {
                cost += 1;
            }
            i -= 1;
            fidx += 1;
            tidx += 1;
        }
        cost
    }

    fn update_cost_blank(&mut self, to: &Line) -> i32 {
        // This assumes bce capability.
        let blank = self.cur.cell.clone();
        let mut cost = 0;
        let mut tidx = 0usize;
        let w = self.curbuf.width();
        let mut i = w;
        while i > 0 {
            if !cell_equal(Some(&blank), to.0.get(tidx)) {
                cost += 1;
            }
            i -= 1;
            tidx += 1;
        }
        cost
    }

    /// The main render loop.
    fn render_buffer(&mut self, newbuf: &mut RenderBuffer) {
        // Do we need to render anything?
        let touched_lines = newbuf.touched_lines();
        if !self.clear && touched_lines == 0 {
            return;
        }

        if self.curbuf.width() == 0 || self.curbuf.height() == 0 {
            // Initialize the current buffer
            self.curbuf = crate::new_render_buffer(newbuf.width(), newbuf.height());
        }

        let new_width = newbuf.width();
        let new_height = newbuf.height();
        let cur_width = self.curbuf.width();
        let cur_height = self.curbuf.height();

        if cur_width != new_width || cur_height != new_height {
            self.oldhash = Vec::new();
            self.newhash = Vec::new();
        }

        let mut non_empty;

        // XXX: In inline mode, after a screen resize, we need to clear the
        // extra lines at the bottom of the screen.
        let partial_clear = !self.flags.contains(TFlag::FULLSCREEN)
            && self.cur.x != -1
            && self.cur.y != -1
            && cur_width == new_width
            && cur_height > 0
            && cur_height > new_height;

        if !self.clear && partial_clear {
            self.clear_below(newbuf, None, new_height as i64 - 1);
        }

        if self.clear {
            self.clear_update(newbuf);
            self.clear = false;
        } else if touched_lines > 0 {
            if self.flags.contains(TFlag::SCROLL_OPTIM) && self.flags.contains(TFlag::FULLSCREEN) {
                self.scroll_optimize(newbuf);
            }

            if self.flags.contains(TFlag::FULLSCREEN) {
                non_empty = cur_height.min(new_height);
            } else {
                non_empty = new_height;
            }

            non_empty = self.clear_bottom(newbuf, non_empty);
            if std::env::var("UV_DEBUG").is_ok() {
                eprintln!(
                    "NONEMPTY: {} height: {} touched: {:?}",
                    non_empty,
                    new_height,
                    newbuf
                        .touched
                        .iter()
                        .map(|t| t.as_ref().map(|ld| (ld.first_cell, ld.last_cell)))
                        .collect::<Vec<_>>()
                );
                for y in 0..new_height {
                    if let Some(line) = newbuf.line(y) {
                        let content: String = line.0.iter().map(|c| c.content.clone()).collect();
                        eprintln!("LINE {}: {:?}", y, content);
                    }
                }
            }
            let mut i = 0usize;
            while i < non_empty.min(new_height) {
                let touched = newbuf.touched.get(i);
                let transform = match touched {
                    None => true,
                    Some(None) => false,
                    Some(Some(ld)) => ld.first_cell != usize::MAX || ld.last_cell != usize::MAX,
                };
                if transform {
                    self.transform_line(newbuf, i);
                }

                // Mark line changed successfully.
                if i < newbuf.touched.len() {
                    newbuf.touched[i] = Some(PROCESSED);
                }
                if i < self.curbuf.touched.len() && i < self.curbuf.height().saturating_sub(1) {
                    self.curbuf.touched[i] = Some(PROCESSED);
                }

                i += 1;
            }
        }

        if !self.flags.contains(TFlag::FULLSCREEN)
            && (cur_width != new_width || cur_height != new_height)
        {
            self.move_to_pos(Some(newbuf), 0, new_height as i64 - 1);
        }

        // Sync windows and screen
        newbuf.touched = vec![Some(PROCESSED); new_height];
        self.curbuf.touched = vec![Some(PROCESSED); self.curbuf.height()];

        if cur_width != new_width || cur_height != new_height {
            // Resize the old buffer to match the new buffer.
            self.curbuf.buffer.resize(new_width, new_height);
            // Sync new lines to old lines
            for i in cur_height.saturating_sub(1)..new_height {
                let nl = newbuf.line(i).cloned();
                if let (Some(nl), Some(cl)) = (nl, self.curbuf.line(i)) {
                    let n = cl.0.len().min(nl.0.len());
                    let mut cl = cl.clone();
                    for k in 0..n {
                        cl.0[k] = nl.0[k].clone();
                    }
                    self.curbuf.set_line(i, cl);
                }
            }
        }

        self.update_pen(None); // nil indicates a blank cell with no styles
    }
}

impl crate::terminal_screen::TerminalRenderer for TerminalRenderer {
    fn set_fullscreen(&mut self, fullscreen: bool) {
        if fullscreen {
            self.flags.set(TFlag::FULLSCREEN);
        } else {
            self.flags.reset(TFlag::FULLSCREEN);
        }
    }

    fn set_relative_cursor(&mut self, relative: bool) {
        if relative {
            self.flags.set(TFlag::RELATIVE_CURSOR);
        } else {
            self.flags.reset(TFlag::RELATIVE_CURSOR);
        }
    }

    fn set_color_profile(&mut self, profile: ColorProfile) {
        self.profile = profile;
    }

    fn set_logger(&mut self, _logger: Option<Box<dyn Logger>>) {
        // NOTE: upstream logs renderer output; the ported screen logger is
        // accepted but no output logging is performed.
    }

    fn set_tab_stops(&mut self, every: i32) {
        if every < 0 || self.term.starts_with("linux") {
            // Linux terminal does not support hard tabs.
            self.caps.reset(Capabilities::HT);
        } else {
            self.caps.set(Capabilities::HT);
            let width = self.curbuf.width() as i32;
            self.tabs = Some(default_tab_stops(if width > 0 { width } else { every }));
        }
    }

    fn set_backspace(&mut self, backspace: bool) {
        if backspace {
            self.caps.set(Capabilities::BS);
        } else {
            self.caps.reset(Capabilities::BS);
        }
    }

    fn set_map_newline(&mut self, map: bool) {
        if map {
            self.flags.set(TFlag::MAP_NEWLINE);
        } else {
            self.flags.reset(TFlag::MAP_NEWLINE);
        }
    }

    fn set_width_method(&mut self, method: WidthMethod) {
        self.method = method;
    }

    fn set_grapheme_width(&mut self, grapheme: bool) {
        if grapheme {
            self.flags.set(TFlag::GRAPHEME_WIDTH);
            self.method = WidthMethod::GraphemeWidth;
        } else {
            self.flags.reset(TFlag::GRAPHEME_WIDTH);
            self.method = WidthMethod::WcWidth;
        }
    }

    fn resize(&mut self, width: usize, _height: usize) {
        if let Some(tabs) = &mut self.tabs {
            tabs.resize(width as i32);
        }
    }

    fn erase(&mut self) {
        self.clear = true;
    }

    fn render(&mut self, rbuf: &mut RenderBuffer, out: &mut Vec<u8>) {
        self.render_buffer(rbuf);
        out.extend_from_slice(&self.buf);
        self.buf.clear();
    }

    fn flush(&mut self, out: &mut Vec<u8>) -> std::io::Result<()> {
        out.extend_from_slice(&self.buf);
        self.buf.clear();
        Ok(())
    }

    fn move_to(&mut self, x: usize, y: usize, _out: &mut Vec<u8>) {
        if !self.flags.contains(TFlag::FULLSCREEN)
            && self.flags.contains(TFlag::RELATIVE_CURSOR)
            && self.cur.x == -1
            && self.cur.y == -1
        {
            // First cursor movement in inline mode, move the cursor to the
            // first column before moving to the target position.
            self.push_byte(b'\r');
            self.cur.x = 0;
            self.cur.y = 0;
        }
        let seq = self.cursor_move(None, x as i64, y as i64, false);
        self.push(&seq);
        self.cur.x = x as i64;
        self.cur.y = y as i64;
        // NOTE: mirroring upstream, the renderer's output lands in its own
        // internal buffer and only reaches the screen's buffer via
        // [TerminalRenderer::flush] (called from the screen's render path).
        // A move queued during [crate::terminal_screen::TerminalScreen::flush]
        // is therefore invisible until the next render, exactly like Go's
        // `TerminalRenderer.MoveTo` writing to the renderer buffer.
    }

    fn position(&self) -> (usize, usize) {
        (self.cur.x.max(0) as usize, self.cur.y.max(0) as usize)
    }

    fn save_cursor(&mut self) {
        self.saved = self.cur.clone();
    }

    fn restore_cursor(&mut self) {
        self.cur = self.saved.clone();
    }

    fn set_position(&mut self, x: usize, y: usize) {
        self.cur.x = x as i64;
        self.cur.y = y as i64;
    }
}

/// notLocal returns whether the coordinates are not considered local
/// movement using the defined thresholds.
fn not_local(cols: i64, fx: i64, fy: i64, tx: i64, ty: i64) -> bool {
    // The typical distance for a CUP sequence.
    const LONG_DIST: i64 = 8 - 1;
    (tx > LONG_DIST)
        && (tx < cols - 1 - LONG_DIST)
        && ((ty - fy).abs() + (tx - fx).abs() > LONG_DIST)
}

/// xtermCaps returns a list of control sequence capabilities for the given
/// terminal type.
fn xterm_caps(term_type: &str) -> Capabilities {
    let mut v = Capabilities::default();
    let mut parts = term_type.split('-');
    let first = parts.next().unwrap_or("");
    if first.is_empty() {
        return v;
    }

    match first {
        "contour" | "foot" | "ghostty" | "kitty" | "rio" | "st" | "tmux" | "wezterm" => {
            v.0 = Capabilities::ALL;
        }
        "xterm" => {
            match parts.next().unwrap_or("") {
                "ghostty" | "kitty" | "rio" => {
                    v.0 = Capabilities::ALL;
                }
                _ => {
                    // NOTE: We exclude capHPA from allCaps because terminals
                    // like Konsole don't support it.
                    v.0 = Capabilities::ALL;
                    v.reset(Capabilities::HPA);
                    v.reset(Capabilities::CHT);
                    v.reset(Capabilities::REP);
                }
            }
        }
        "alacritty" => {
            v.0 = Capabilities::ALL;
            v.reset(Capabilities::CHT);
        }
        "screen" => {
            v.0 = Capabilities::ALL;
            v.reset(Capabilities::REP);
        }
        "linux" => {
            v.0 = Capabilities::VPA
                | Capabilities::CHA
                | Capabilities::HPA
                | Capabilities::ECH
                | Capabilities::ICH;
        }
        _ => {}
    }

    v
}

/// hashLine returns the hash value of a [Line]. Deterministic within the
/// process (Go uses the randomized `maphash`; only relative comparisons
/// matter).
fn hash_line(l: &Line) -> u64 {
    let mut h = DefaultHasher::new();
    for c in &l.0 {
        c.content.hash(&mut h);
    }
    h.finish()
}

/// Erase is exposed for parity; use the trait's [erase] instead.
#[allow(unused)]
pub fn erase(r: &mut TerminalRenderer) {
    r.clear = true;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env() -> Environ {
        Environ(vec![
            "TERM=xterm-256color".to_string(),
            "COLORTERM=truecolor".to_string(),
        ])
    }

    fn frame1() -> RenderBuffer {
        let mut nb = crate::new_render_buffer(20, 3);
        let space = empty_cell();
        for y in 0..3 {
            for x in 0..20 {
                nb.set_cell(x, y, Some(&space));
            }
        }
        nb.set_cell(0, 0, Some(&Cell::new("H")));
        nb.set_cell(1, 0, Some(&Cell::new("i")));
        nb.set_cell(0, 1, Some(&Cell::new("w")));
        nb.set_cell(1, 1, Some(&Cell::new("o")));
        nb.set_cell(2, 1, Some(&Cell::new("r")));
        nb.set_cell(3, 1, Some(&Cell::new("l")));
        nb.set_cell(4, 1, Some(&Cell::new("d")));
        nb
    }

    /// Renders the buffer and returns the renderer's output.
    fn render(
        r: &mut Box<dyn crate::terminal_screen::TerminalRenderer>,
        nb: &mut RenderBuffer,
    ) -> Vec<u8> {
        let mut out = Vec::new();
        r.render(nb, &mut out);
        r.flush(&mut out).unwrap();
        out
    }

    #[test]
    fn test_render_diff_scenarios_match_go() {
        let mut r = new_terminal_renderer(&env());
        r.set_fullscreen(true);

        // Frame 1: initial content (Go: "\x1b[HHi\r\nworld").
        let mut nb = frame1();
        assert_eq!(render(&mut r, &mut nb), b"\x1b[HHi\r\nworld");

        // Frame 2: modify one cell (Go: "\rwoXl").
        let mut nb2 = frame1();
        nb2.set_cell(2, 1, Some(&Cell::new("X")));
        assert_eq!(render(&mut r, &mut nb2), b"\rwoXl");

        // Frame 3: back to the original content (Go: "\rworl").
        let mut nb3 = frame1();
        assert_eq!(render(&mut r, &mut nb3), b"\rworl");

        // Frame 4: a wide cell (Go: "\x1b[H\x1b[J\x1b[7G").
        let mut nb4 = crate::new_render_buffer(20, 3);
        nb4.set_cell(5, 0, Some(&Cell::new("界")));
        nb4.set_cell(6, 0, Some(&Cell::default()));
        assert_eq!(render(&mut r, &mut nb4), b"\x1b[H\x1b[J\x1b[7G");

        // Frame 5: erase to an empty buffer (Go: "").
        let mut nb5 = crate::new_render_buffer(20, 3);
        assert_eq!(render(&mut r, &mut nb5), b"");
    }

    #[test]
    fn test_render_harder_scenarios_match_go() {
        let mut r = new_terminal_renderer(&env());
        r.set_fullscreen(true);

        // Leading change on line 0 (Go: "\x1b[HZi").
        let mut nb = frame1();
        r.render(&mut nb, &mut Vec::new());
        let mut out = Vec::new();
        r.flush(&mut out).unwrap();
        let mut nb2 = frame1();
        nb2.set_cell(0, 0, Some(&Cell::new("Z")));
        out.clear();
        r.render(&mut nb2, &mut out);
        r.flush(&mut out).unwrap();
        assert_eq!(out, b"\x1b[HZi");

        // Repeated chars with xterm caps (no REP) (Go: "\r\x1b[J**********").
        let mut nb3 = crate::new_render_buffer(20, 3);
        let space = empty_cell();
        for y in 0..3 {
            for x in 0..20 {
                nb3.set_cell(x, y, Some(&space));
            }
        }
        for x in 0..10 {
            nb3.set_cell(x, 0, Some(&Cell::new("*")));
        }
        out.clear();
        r.render(&mut nb3, &mut out);
        r.flush(&mut out).unwrap();
        assert_eq!(out, b"\r\x1b[J**********");

        // Resize smaller (Go: "\rnew\x1b[K").
        let mut nb4 = crate::new_render_buffer(10, 2);
        for y in 0..2 {
            for x in 0..10 {
                nb4.set_cell(x, y, Some(&space));
            }
        }
        nb4.set_cell(0, 0, Some(&Cell::new("n")));
        nb4.set_cell(1, 0, Some(&Cell::new("e")));
        nb4.set_cell(2, 0, Some(&Cell::new("w")));
        out.clear();
        r.render(&mut nb4, &mut out);
        r.flush(&mut out).unwrap();
        assert_eq!(out, b"\rnew\x1b[K");
    }

    #[test]
    fn test_move_to_matches_go() {
        let mut r = new_terminal_renderer(&env());
        r.set_fullscreen(false);
        r.set_relative_cursor(true);
        let mut out = Vec::new();
        r.move_to(5, 3, &mut out);
        // Go: "\r\n\n\n\x1b[5C" — queued in the renderer buffer, invisible
        // until flush.
        assert!(out.is_empty());
        // Draining via flush reveals the queued move.
        r.flush(&mut out).unwrap();
        assert_eq!(out, b"\r\n\n\n\x1b[5C");
    }

    #[test]
    fn test_xterm_caps() {
        assert_eq!(
            xterm_caps("xterm-256color").0,
            Capabilities::ALL & !(Capabilities::HPA | Capabilities::CHT | Capabilities::REP)
        );
        assert_eq!(xterm_caps("kitty").0, Capabilities::ALL);
        assert_eq!(
            xterm_caps("linux").0,
            Capabilities::VPA
                | Capabilities::CHA
                | Capabilities::HPA
                | Capabilities::ECH
                | Capabilities::ICH
        );
        assert_eq!(xterm_caps("").0, 0);
    }

    /// Ported from upstream `TestRendererRelativeCursor`.
    #[test]
    fn test_renderer_relative_cursor() {
        let mut r = new_terminal_renderer(&env());
        r.set_fullscreen(true);
        r.set_relative_cursor(true);
        let mut nb = crate::new_render_buffer(10, 3);
        let space = empty_cell();
        for y in 0..3 {
            for x in 0..10 {
                nb.set_cell(x, y, Some(&space));
            }
        }
        nb.set_cell(5, 1, Some(&Cell::new("X")));
        let mut out = Vec::new();
        r.render(&mut nb, &mut out);
        r.flush(&mut out).unwrap();
        assert!(out.contains(&b'X'));

        // Disabling relative cursor still renders (unchanged buffer: no diff).
        r.set_relative_cursor(false);
        let mut out = Vec::new();
        r.render(&mut nb, &mut out);
        r.flush(&mut out).unwrap();
    }

    /// Ported from upstream `TestRendererScrollOptimization`.
    #[test]
    fn test_renderer_scroll_optimization() {
        let mut r = new_terminal_renderer(&env());
        r.set_fullscreen(true);
        let mut nb = crate::new_render_buffer(10, 5);
        for y in 0..5 {
            for x in 0..10 {
                nb.set_cell(
                    x,
                    y,
                    Some(&Cell::new(&((b'A' + y as u8) as char).to_string())),
                );
            }
        }
        let mut out = Vec::new();
        r.render(&mut nb, &mut out);
        r.flush(&mut out).unwrap();
        out.clear();

        // Scroll content up by one and add a new row at the bottom.
        let mut nb2 = crate::new_render_buffer(10, 5);
        for y in 0..4 {
            for x in 0..10 {
                nb2.set_cell(
                    x,
                    y,
                    Some(&Cell::new(&((b'A' + y as u8 + 1) as char).to_string())),
                );
            }
        }
        for x in 0..10 {
            nb2.set_cell(x, 4, Some(&Cell::new("F")));
        }
        r.render(&mut nb2, &mut out);
        r.flush(&mut out).unwrap();
        assert!(out.contains(&b'F'));
    }

    /// Ported from upstream `TestRendererPosition`.
    #[test]
    fn test_renderer_position() {
        let mut r = new_terminal_renderer(&env());
        assert_eq!(r.position(), (0, 0));
        r.set_position(5, 10);
        assert_eq!(r.position(), (5, 10));
    }

    /// Ported from upstream `TestRendererMoveTo` and `TestRendererWriteString`.
    #[test]
    fn test_renderer_move_and_write() {
        let mut r = new_terminal_renderer(&env());
        let mut out = Vec::new();
        r.move_to(5, 3, &mut out);
        r.flush(&mut out).unwrap();
        assert!(out.contains(&0x1b));

        // write via the concrete struct's write_string_public.
        let mut cr = TerminalRenderer::new_without_writer(&env());
        let n = cr.write_string_public("Hello, World!").unwrap();
        assert_eq!(n, 13);
        let mut out = Vec::new();
        cr.flush_into(&mut out);
        assert!(String::from_utf8_lossy(&out).contains("Hello, World!"));
    }

    /// Ported from upstream `TestRendererRedraw` and `TestRendererErase`.
    #[test]
    fn test_renderer_redraw_and_erase() {
        let mut r = new_terminal_renderer(&env());
        let mut nb = crate::new_render_buffer(3, 1);
        nb.set_cell(0, 0, Some(&Cell::new("X")));
        let mut out = Vec::new();
        r.render(&mut nb, &mut out);
        r.flush(&mut out).unwrap();
        assert!(out.contains(&b'X'));

        // Erase forces a full clear on the next render.
        r.erase();
        let mut out = Vec::new();
        r.render(&mut nb, &mut out);
        r.flush(&mut out).unwrap();
        assert!(out.contains(&b'X'));
    }

    /// Ported from upstream `TestRendererResize`.
    #[test]
    fn test_renderer_resize() {
        let mut r = new_terminal_renderer(&env());
        r.resize(80, 24);
        let mut nb = crate::new_render_buffer(80, 24);
        let mut out = Vec::new();
        r.render(&mut nb, &mut out);
        r.flush(&mut out).unwrap();
    }

    /// Ported from upstream `TestRendererTabStops`.
    #[test]
    fn test_renderer_tab_stops() {
        let mut r = new_terminal_renderer(&env());
        r.set_tab_stops(5);
        r.set_backspace(true);
        r.set_map_newline(true);
        let mut nb = crate::new_render_buffer(20, 2);
        nb.set_cell(0, 0, Some(&Cell::new("H")));
        nb.set_cell(1, 0, Some(&Cell::new("i")));
        let mut out = Vec::new();
        r.render(&mut nb, &mut out);
        r.flush(&mut out).unwrap();
        assert!(out.contains(&b'H'));
        // Disable tab stops.
        r.set_tab_stops(-1);
        let mut out = Vec::new();
        r.render(&mut nb, &mut out);
        r.flush(&mut out).unwrap();
    }

    /// Ported from upstream `TestRendererWideCharacters` and
    /// `TestRendererZeroWidthCharacters`.
    #[test]
    fn test_renderer_wide_and_zero_width() {
        let mut r = new_terminal_renderer(&env());
        r.set_fullscreen(true);
        r.set_width_method(WidthMethod::WcWidth);
        let mut nb = crate::new_render_buffer(10, 2);
        let space = empty_cell();
        for y in 0..2 {
            for x in 0..10 {
                nb.set_cell(x, y, Some(&space));
            }
        }
        nb.set_cell(0, 0, Some(&Cell::new("界")));
        nb.set_cell(1, 0, Some(&Cell::default()));
        nb.set_cell(2, 0, Some(&Cell::new("界")));
        nb.set_cell(3, 0, Some(&Cell::default()));
        let mut out = Vec::new();
        r.render(&mut nb, &mut out);
        r.flush(&mut out).unwrap();
        // The wide cells only require cursor moves (no visible content diff).
        assert!(!out.is_empty());
    }

    /// Ported from upstream `TestRendererSwitchBuffer`.
    #[test]
    fn test_renderer_switch_buffer() {
        let mut r = new_terminal_renderer(&env());
        r.set_fullscreen(true);
        let mut nb = crate::new_render_buffer(5, 3);
        nb.set_cell(0, 0, Some(&Cell::new("X")));
        let mut out = Vec::new();
        r.render(&mut nb, &mut out);
        r.flush(&mut out).unwrap();
        // Switching to inline mode.
        r.set_fullscreen(false);
        let mut out = Vec::new();
        r.render(&mut nb, &mut out);
        r.flush(&mut out).unwrap();
    }

    /// Ported from upstream `TestRendererStyledText`: styled cells emit SGR.
    #[test]
    fn test_renderer_styled_text() {
        let mut r = new_terminal_renderer(&env());
        r.set_fullscreen(true);
        let mut nb = crate::new_render_buffer(10, 1);
        let space = empty_cell();
        for x in 0..10 {
            nb.set_cell(x, 0, Some(&space));
        }
        let styles = [
            Style {
                attrs: Attr::BOLD.bits(),
                ..Style::default()
            },
            Style {
                fg: Some(Color::Basic(1)),
                ..Style::default()
            },
            Style {
                bg: Some(Color::Basic(2)),
                ..Style::default()
            },
            Style {
                attrs: Attr::BOLD.bits(),
                fg: Some(Color::Basic(4)),
                ..Style::default()
            },
        ];
        for (i, style) in styles.iter().enumerate() {
            nb.set_cell(
                i,
                0,
                Some(&Cell {
                    content: "X".to_string(),
                    width: 1,
                    style: style.clone(),
                    ..Cell::default()
                }),
            );
        }
        let mut out = Vec::new();
        r.render(&mut nb, &mut out);
        r.flush(&mut out).unwrap();
        assert!(out.contains(&0x1b));
    }

    /// Ported from upstream `TestRendererHyperlinks`.
    #[test]
    fn test_renderer_hyperlinks() {
        let mut r = new_terminal_renderer(&env());
        r.set_fullscreen(true);
        let mut nb = crate::new_render_buffer(10, 1);
        let space = empty_cell();
        for x in 0..10 {
            nb.set_cell(x, 0, Some(&space));
        }
        let link = crate::cell::Link {
            url: "https://example.com".to_string(),
            params: String::new(),
        };
        nb.set_cell(
            0,
            0,
            Some(&Cell {
                content: "l".to_string(),
                width: 1,
                link: Some(link),
                ..Cell::default()
            }),
        );
        let mut out = Vec::new();
        r.render(&mut nb, &mut out);
        r.flush(&mut out).unwrap();
        assert!(String::from_utf8_lossy(&out).contains("l"));
    }

    /// Multi-line scroll in fullscreen triggers the hard-scroll optimizer.
    #[test]
    fn test_renderer_scroll_multiline() {
        let mut r = new_terminal_renderer(&env());
        r.set_fullscreen(true);
        let mut nb = crate::new_render_buffer(5, 5);
        for y in 0..5 {
            for x in 0..5 {
                nb.set_cell(
                    x,
                    y,
                    Some(&Cell::new(&((b'A' + y as u8) as char).to_string())),
                );
            }
        }
        let mut out = Vec::new();
        r.render(&mut nb, &mut out);
        r.flush(&mut out).unwrap();
        out.clear();
        // Shift content up by two rows.
        let mut nb2 = crate::new_render_buffer(5, 5);
        for y in 0..3 {
            for x in 0..5 {
                nb2.set_cell(
                    x,
                    y,
                    Some(&Cell::new(&((b'A' + y as u8 + 2) as char).to_string())),
                );
            }
        }
        for y in 3..5 {
            for x in 0..5 {
                nb2.set_cell(
                    x,
                    y,
                    Some(&Cell::new(&((b'X' + y as u8) as char).to_string())),
                );
            }
        }
        r.render(&mut nb2, &mut out);
        r.flush(&mut out).unwrap();
        assert!(!out.is_empty());
    }

    /// Inserting a line in the middle of a fullscreen buffer.
    #[test]
    fn test_renderer_insert_line_middle() {
        let mut r = new_terminal_renderer(&env());
        r.set_fullscreen(true);
        let mut nb = crate::new_render_buffer(5, 4);
        for y in 0..4 {
            for x in 0..5 {
                nb.set_cell(
                    x,
                    y,
                    Some(&Cell::new(&((b'A' + y as u8) as char).to_string())),
                );
            }
        }
        let mut out = Vec::new();
        r.render(&mut nb, &mut out);
        r.flush(&mut out).unwrap();
        out.clear();
        // Insert a blank line at row 1.
        let mut nb2 = crate::new_render_buffer(5, 4);
        let space = empty_cell();
        for x in 0..5 {
            nb2.set_cell(x, 0, Some(&Cell::new("A")));
        }
        for x in 0..5 {
            nb2.set_cell(x, 1, Some(&space));
        }
        for y in 2..4 {
            for x in 0..5 {
                nb2.set_cell(
                    x,
                    y,
                    Some(&Cell::new(&((b'A' + y as u8 - 1) as char).to_string())),
                );
            }
        }
        r.render(&mut nb2, &mut out);
        r.flush(&mut out).unwrap();
        assert!(!out.is_empty());
    }

    /// Repeated characters with REP support (kitty caps).
    #[test]
    fn test_renderer_repeat_character() {
        let mut env = env();
        env.0.push("TERM=kitty".to_string());
        let mut r = new_terminal_renderer(&env);
        r.set_fullscreen(true);
        let mut nb = crate::new_render_buffer(20, 2);
        let space = empty_cell();
        for y in 0..2 {
            for x in 0..20 {
                nb.set_cell(x, y, Some(&space));
            }
        }
        for x in 0..10 {
            nb.set_cell(x, 0, Some(&Cell::new("*")));
        }
        let mut out = Vec::new();
        r.render(&mut nb, &mut out);
        r.flush(&mut out).unwrap();
        assert!(!out.is_empty());
    }
}
