//! Cleanroom Rust port of upstream Go source file: `screen/context.go` (`screen/screen.go` is covered by `src/screen.rs`)
//! Upstream Target Tag / Version: `v0.0.0-20260703014108-f5a850f9c2b7`
//!
//! <public-docs>
//! A drawing context for rendering operations on a [Screen]. The context
//! carries a current style, hyperlink, and cursor position, and draws
//! grapheme-aware strings onto the screen with optional wrapping.
//!
//! NOTE: the upstream `Screen` interface at this pin exposes `Bounds`,
//! `CellAt`, `SetCell` and `WidthMethod`. The ported [Screen] trait in
//! `crate::buffer` predates that interface (it has `bounds`, `cell_at`,
//! `cell_at_mut`, `as_any_mut` but no `width_method`), so this Context:
//! - sets cells through `cell_at_mut` (the equivalent of `SetCell`), and
//! - holds its own [WidthMethod] (default [WidthMethod::WcWidth]) since the
//!   ported trait cannot answer `WidthMethod()`. Once the integrator's
//!   `uv.go` port exposes `width_method` on the screen, the Context should
//!   query the screen instead, matching upstream.
//!
//! </public-docs>
use std::io;

use rusty_x_ansi::method::WidthMethod;
use rusty_x_ansi::style::Underline;
use unicode_segmentation::UnicodeSegmentation;

use crate::buffer::Screen;
use crate::cell::{new_link, Cell, Link};
use crate::style::Style;
use crate::window::{pos, Position};

use std::fmt::Write as _;

// The attribute bits mirror `Attr` in `crate::style` (whose tuple field is
// private); they match the upstream `AttrBold`..`AttrStrikethrough` values.
const ATTR_BOLD: u8 = 1 << 0;
const ATTR_FAINT: u8 = 1 << 1;
const ATTR_ITALIC: u8 = 1 << 2;
const ATTR_BLINK: u8 = 1 << 3;
const ATTR_REVERSE: u8 = 1 << 5;
const ATTR_CONCEAL: u8 = 1 << 6;
const ATTR_STRIKETHROUGH: u8 = 1 << 7;

/// Context represents a drawing context for rendering operations on a screen.
pub struct Context<'a> {
    scr: Box<dyn Screen + 'a>,

    style: Style,
    link: Link,
    pos: Position,
    wm: WidthMethod,
}

/// NewContext creates a new drawing context for the given screen.
pub fn new_context<'a>(scr: Box<dyn Screen + 'a>) -> Context<'a> {
    let mut c = Context {
        scr,
        style: Style::default(),
        link: Link::default(),
        pos: pos(0, 0),
        wm: WidthMethod::default(),
    };
    c.reset();
    c
}

/// NewContextWithWidthMethod creates a new drawing context for the given
/// screen with an explicit width method.
///
/// Once the ported [Screen] trait exposes `width_method`, this constructor
/// will be subsumed by the screen's own method (see the module docs).
pub fn new_context_with_width_method<'a>(
    scr: Box<dyn Screen + 'a>,
    wm: WidthMethod,
) -> Context<'a> {
    let mut c = new_context(scr);
    c.wm = wm;
    c
}

impl<'a> Context<'a> {
    /// Reset resets the context to its default state.
    pub fn reset(&mut self) {
        self.style = Style::default();
        self.link = Link::default();
        self.pos = pos(0, 0);
    }

    /// SetStyle sets the style of the context.
    pub fn set_style(&mut self, style: Style) {
        self.style = style;
    }

    /// WithStyle returns a copy of the context with the given style.
    pub fn with_style(mut self, style: Style) -> Context<'a> {
        self.set_style(style);
        self
    }

    /// SetLink sets the link of the context.
    pub fn set_link(&mut self, link: Link) {
        self.link = link;
    }

    /// WithLink returns a copy of the context with the given link.
    pub fn with_link(mut self, link: Link) -> Context<'a> {
        self.set_link(link);
        self
    }

    /// SetAttrs sets the attributes of the context.
    pub fn set_attrs(&mut self, attrs: u8) {
        self.style.attrs = attrs;
    }

    /// WithAttrs returns a copy of the context with the given attributes.
    pub fn with_attrs(mut self, attrs: u8) -> Context<'a> {
        self.set_attrs(attrs);
        self
    }

    /// SetBackground sets the background color of the context. Use None to
    /// reset to default.
    pub fn set_background(&mut self, bg: Option<rusty_x_ansi::style::Color>) {
        self.style.bg = bg;
    }

    /// WithBackground returns a copy of the context with the given background
    /// color.
    pub fn with_background(mut self, bg: Option<rusty_x_ansi::style::Color>) -> Context<'a> {
        self.set_background(bg);
        self
    }

    /// SetForeground sets the foreground color of the context. Use None to
    /// reset to default.
    pub fn set_foreground(&mut self, fg: Option<rusty_x_ansi::style::Color>) {
        self.style.fg = fg;
    }

    /// WithForeground returns a copy of the context with the given foreground
    /// color.
    pub fn with_foreground(mut self, fg: Option<rusty_x_ansi::style::Color>) -> Context<'a> {
        self.set_foreground(fg);
        self
    }

    /// SetBold sets whether the text in the context should be bold.
    pub fn set_bold(&mut self, bold: bool) {
        if bold {
            self.style.attrs |= ATTR_BOLD;
        } else {
            self.style.attrs &= !ATTR_BOLD;
        }
    }

    /// WithBold returns a copy of the context with the given bold attribute.
    pub fn with_bold(mut self, bold: bool) -> Context<'a> {
        self.set_bold(bold);
        self
    }

    /// SetItalic sets whether the text in the context should be italic.
    pub fn set_italic(&mut self, italic: bool) {
        if italic {
            self.style.attrs |= ATTR_ITALIC;
        } else {
            self.style.attrs &= !ATTR_ITALIC;
        }
    }

    /// WithItalic returns a copy of the context with the given italic
    /// attribute.
    pub fn with_italic(mut self, italic: bool) -> Context<'a> {
        self.set_italic(italic);
        self
    }

    /// SetStrikethrough sets whether the text in the context should be
    /// strikethrough.
    pub fn set_strikethrough(&mut self, strikethrough: bool) {
        if strikethrough {
            self.style.attrs |= ATTR_STRIKETHROUGH;
        } else {
            self.style.attrs &= !ATTR_STRIKETHROUGH;
        }
    }

    /// WithStrikethrough returns a copy of the context with the given
    /// strikethrough attribute.
    pub fn with_strikethrough(mut self, strikethrough: bool) -> Context<'a> {
        self.set_strikethrough(strikethrough);
        self
    }

    /// SetFaint sets whether the text in the context should be faint.
    pub fn set_faint(&mut self, faint: bool) {
        if faint {
            self.style.attrs |= ATTR_FAINT;
        } else {
            self.style.attrs &= !ATTR_FAINT;
        }
    }

    /// WithFaint returns a copy of the context with the given faint
    /// attribute.
    pub fn with_faint(mut self, faint: bool) -> Context<'a> {
        self.set_faint(faint);
        self
    }

    /// SetBlink sets whether the text in the context should blink.
    pub fn set_blink(&mut self, blink: bool) {
        if blink {
            self.style.attrs |= ATTR_BLINK;
        } else {
            self.style.attrs &= !ATTR_BLINK;
        }
    }

    /// WithBlink returns a copy of the context with the given blink
    /// attribute.
    pub fn with_blink(mut self, blink: bool) -> Context<'a> {
        self.set_blink(blink);
        self
    }

    /// SetReverse sets whether the text in the context should be reversed.
    pub fn set_reverse(&mut self, reverse: bool) {
        if reverse {
            self.style.attrs |= ATTR_REVERSE;
        } else {
            self.style.attrs &= !ATTR_REVERSE;
        }
    }

    /// WithReverse returns a copy of the context with the given reverse
    /// attribute.
    pub fn with_reverse(mut self, reverse: bool) -> Context<'a> {
        self.set_reverse(reverse);
        self
    }

    /// SetConceal sets whether the text in the context should be concealed.
    pub fn set_conceal(&mut self, conceal: bool) {
        if conceal {
            self.style.attrs |= ATTR_CONCEAL;
        } else {
            self.style.attrs &= !ATTR_CONCEAL;
        }
    }

    /// WithConceal returns a copy of the context with the given conceal
    /// attribute.
    pub fn with_conceal(mut self, conceal: bool) -> Context<'a> {
        self.set_conceal(conceal);
        self
    }

    /// SetUnderlineStyle sets the underline style of the context.
    pub fn set_underline_style(&mut self, u: Underline) {
        self.style.underline = u;
    }

    /// WithUnderlineStyle returns a copy of the context with the given
    /// underline style.
    pub fn with_underline_style(mut self, u: Underline) -> Context<'a> {
        self.set_underline_style(u);
        self
    }

    /// SetUnderline sets whether the text in the context should be
    /// underlined.
    ///
    /// This is a convenience method that sets the underline style to single
    /// or none. It is equivalent to calling [Context::set_underline_style]
    /// with [Underline::Single] or [Underline::None].
    pub fn set_underline(&mut self, underline: bool) {
        if underline {
            self.set_underline_style(Underline::Single);
        } else {
            self.set_underline_style(Underline::None);
        }
    }

    /// WithUnderline returns a copy of the context with the given underline
    /// attribute.
    pub fn with_underline(mut self, underline: bool) -> Context<'a> {
        self.set_underline(underline);
        self
    }

    /// SetUnderlineColor sets the underline color of the context. Use None to
    /// reset to default.
    pub fn set_underline_color(&mut self, color: Option<rusty_x_ansi::style::Color>) {
        self.style.underline_color = color;
    }

    /// WithUnderlineColor returns a copy of the context with the given
    /// underline color.
    pub fn with_underline_color(
        mut self,
        color: Option<rusty_x_ansi::style::Color>,
    ) -> Context<'a> {
        self.set_underline_color(color);
        self
    }

    /// SetURL sets the URL link for the context. Use an empty string to
    /// reset.
    pub fn set_url(&mut self, url: &str, params: &[&str]) {
        if url.is_empty() {
            self.link = Link::default();
            return;
        }
        self.link = new_link(url, params);
    }

    /// WithURL returns a copy of the context with the given URL link.
    pub fn with_url(mut self, url: &str, params: &[&str]) -> Context<'a> {
        self.set_url(url, params);
        self
    }

    /// Position returns the current position of the context.
    pub fn position(&self) -> (i64, i64) {
        (self.pos.x, self.pos.y)
    }

    /// SetPosition moves the current position of the context cursor to the
    /// given coordinates.
    ///
    /// This is an alias for [Context::move_to].
    pub fn set_position(&mut self, x: i64, y: i64) {
        self.move_to(x, y);
    }

    /// WithPosition returns a copy of the context with the given position.
    pub fn with_position(mut self, x: i64, y: i64) -> Context<'a> {
        self.move_to(x, y);
        self
    }

    /// MoveTo moves the current position of the context cursor to the given
    /// coordinates.
    pub fn move_to(&mut self, x: i64, y: i64) {
        self.pos.x = x;
        self.pos.y = y;
    }

    /// Print writes the formatted arguments to the screen at the current
    /// position, updating the position accordingly.
    pub fn print(&mut self, args: std::fmt::Arguments) -> std::fmt::Result {
        self.write_str(&args.to_string())
    }

    /// Println writes the formatted arguments to the screen at the current
    /// position, appending a newline, and updating the position accordingly.
    pub fn println(&mut self, args: std::fmt::Arguments) -> std::fmt::Result {
        self.write_str(&format!("{}\n", args))
    }

    /// Printf formats according to a format specifier and writes to the
    /// screen at the current position, updating the position accordingly.
    pub fn printf(&mut self, args: std::fmt::Arguments) -> std::fmt::Result {
        self.write_str(&args.to_string())
    }

    /// DrawString draws the given string at the given position with the
    /// current style and link, cropping the string when it reaches the edge
    /// of the screen.
    pub fn draw_string(&mut self, s: &str, x: i64, y: i64) {
        self.draw_string_at(s, x, y, false);
    }

    /// DrawStringWrapped draws the given string at the given position with
    /// the current style and link, wrapping the string when it reaches the
    /// edge of the screen.
    pub fn draw_string_wrapped(&mut self, s: &str, x: i64, y: i64) {
        self.draw_string_at(s, x, y, true);
    }

    fn draw_string_at(&mut self, s: &str, x: i64, y: i64, wrap: bool) {
        let (nx, ny) = draw_string_at(
            self.scr.as_mut(),
            s,
            x,
            y,
            &self.style,
            &self.link,
            wrap,
            self.wm,
        );
        self.pos = pos(nx, ny);
    }

    /// Returns the width method used by the context.
    pub fn width_method(&self) -> WidthMethod {
        self.wm
    }

    /// Sets the width method used by the context.
    pub fn set_width_method(&mut self, wm: WidthMethod) {
        self.wm = wm;
    }
}

/// Write implements the `std::io::Write` interface for the context, writing
/// the given byte slice to the screen at the current position, updating the
/// position accordingly.
impl<'a> io::Write for Context<'a> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let s =
            std::str::from_utf8(buf).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        self.write_str_impl(s);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// WriteString writes the given string to the screen at the current
/// position, updating the position accordingly.
impl<'a> std::fmt::Write for Context<'a> {
    fn write_str(&mut self, s: &str) -> std::fmt::Result {
        self.write_str_impl(s);
        Ok(())
    }
}

impl<'a> Context<'a> {
    /// Returns a mutable reference to the underlying screen.
    #[cfg(test)]
    fn scr_mut(&mut self) -> &mut dyn Screen {
        self.scr.as_mut()
    }

    fn write_str_impl(&mut self, s: &str) {
        let (nx, ny) = draw_string_at(
            self.scr.as_mut(),
            s,
            self.pos.x,
            self.pos.y,
            &self.style,
            &self.link,
            true,
            self.wm,
        );
        self.pos = pos(nx, ny);
    }
}

/// Mirrors the upstream Go signature `drawStringAt(scr Screen, s string, x,
/// y int64, style Style, link Link, wrap bool, wm WidthMethod)` 1:1.
#[allow(clippy::too_many_arguments)]
fn draw_string_at(
    scr: &mut dyn Screen,
    s: &str,
    x: i64,
    y: i64,
    style: &Style,
    link: &Link,
    wrap: bool,
    wm: WidthMethod,
) -> (i64, i64) {
    let mut bounds = scr.bounds();
    bounds.max.0 = bounds.max.0.saturating_sub(bounds.min.0);
    bounds.max.1 = bounds.max.1.saturating_sub(bounds.min.1);
    bounds.min.0 = 0;
    bounds.min.1 = 0;

    let mut pos = pos(x, y);
    if !pos.in_rect(bounds) {
        return (x, y);
    }

    for gr in s.graphemes(true) {
        if gr == "\n" {
            pos.x = bounds.min.0 as i64;
            pos.y += 1;
            continue;
        }

        let w = wm.string_width(gr) as i64;
        let mut p = pos;
        if pos.x + w > bounds.max.0 as i64 {
            if wrap {
                pos.x = bounds.min.0 as i64;
                pos.y += 1;
                p = pos;
            } else {
                break;
            }
        }
        if !p.in_rect(bounds) {
            break;
        }

        let c = Cell {
            content: gr.to_string(),
            width: w.max(0) as usize,
            style: style.clone(),
            link: Some(link.clone()),
        };
        scr.set_cell(p.x as usize, p.y as usize, Some(&c));

        pos.x += w;
        if wrap && pos.x >= bounds.max.0 as i64 {
            pos.x = bounds.min.0 as i64;
            pos.y += 1;
        }
    }

    (pos.x, pos.y)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::new_buffer;
    use std::io::Write as _;

    fn read_all(scr: &mut dyn Screen) -> String {
        let mut out = String::new();
        let b = scr.bounds();
        for y in b.min.1..b.max.1 {
            for x in b.min.0..b.max.0 {
                if let Some(c) = scr.cell_at(x, y) {
                    out.push_str(c.string());
                }
            }
        }
        out
    }

    #[test]
    fn test_write_string() {
        let buf = new_buffer(10, 1);
        let mut ctx = new_context(Box::new(buf));
        ctx.write_all(b"hello").unwrap();
        assert_eq!(read_all(ctx.scr_mut()), "hello     ");
        assert_eq!(ctx.position(), (5, 0));
    }

    #[test]
    fn test_write_string_multiline() {
        let buf = new_buffer(10, 2);
        let mut ctx = new_context(Box::new(buf));
        ctx.write_all(b"ab\ncd").unwrap();
        assert_eq!(ctx.position(), (2, 1));
        let out = read_all(ctx.scr_mut());
        assert!(out.starts_with("ab        "));
    }

    #[test]
    fn test_draw_string_crops() {
        let buf = new_buffer(5, 1);
        let mut ctx = new_context(Box::new(buf));
        ctx.draw_string("abcdef", 3, 0);
        let out = read_all(ctx.scr_mut());
        assert_eq!(out, "   ab");
    }

    #[test]
    fn test_draw_string_wrapped() {
        let buf = new_buffer(3, 3);
        let mut ctx = new_context(Box::new(buf));
        ctx.draw_string_wrapped("abcdef", 0, 0);
        let out = read_all(ctx.scr_mut());
        assert!(out.starts_with("abc"));
    }

    #[test]
    fn test_draw_string_out_of_bounds() {
        let buf = new_buffer(5, 1);
        let mut ctx = new_context(Box::new(buf));
        ctx.draw_string("abc", 10, 0);
        let out = read_all(ctx.scr_mut());
        assert_eq!(out, "     ");
    }

    #[test]
    fn test_new_context_resets() {
        let buf = new_buffer(1, 1);
        let mut ctx = new_context(Box::new(buf));
        ctx.set_bold(true);
        ctx.reset();
        assert!(ctx.style.is_zero());
        assert_eq!(ctx.position(), (0, 0));
    }

    #[test]
    fn test_set_url_and_attrs() {
        let buf = new_buffer(5, 1);
        let mut ctx = new_context(Box::new(buf));
        ctx.set_url("https://example.com", &["a", "b"]);
        assert_eq!(ctx.link.url, "https://example.com");
        assert_eq!(ctx.link.params, "a:b");
        ctx.set_url("", &[]);
        assert!(ctx.link.is_zero());
        ctx.set_italic(true);
        assert_ne!(ctx.style.attrs & ATTR_ITALIC, 0);
        ctx.set_italic(false);
        assert_eq!(ctx.style.attrs & ATTR_ITALIC, 0);
    }

    #[test]
    fn test_context_with_builder() {
        let buf = new_buffer(1, 1);
        let ctx = new_context(Box::new(buf));
        let ctx = ctx.with_bold(true).with_underline(true);
        assert_ne!(ctx.style.attrs & ATTR_BOLD, 0);
        assert_eq!(ctx.style.underline, Underline::Single);
    }

    #[test]
    fn test_print_and_printf() {
        let buf = new_buffer(10, 1);
        let mut ctx = new_context(Box::new(buf));
        ctx.printf(format_args!("{} {}", "a", "b")).unwrap();
        assert_eq!(ctx.position(), (3, 0));
        ctx.println(format_args!("")).unwrap();
        assert_eq!(ctx.position(), (0, 1));
    }

    #[test]
    fn test_style_applied_to_cells() {
        let buf = new_buffer(3, 1);
        let mut ctx = new_context(Box::new(buf));
        ctx.set_bold(true);
        ctx.write_all(b"ab").unwrap();
        let c = ctx.scr_mut().cell_at(0, 0).unwrap().clone();
        assert_ne!(c.style.attrs & ATTR_BOLD, 0);
        let b = ctx.scr_mut().cell_at(1, 0).unwrap().clone();
        assert_ne!(b.style.attrs & ATTR_BOLD, 0);
    }

    #[test]
    fn test_width_method_override() {
        let buf = new_buffer(4, 1);
        let mut ctx = new_context_with_width_method(Box::new(buf), WidthMethod::GraphemeWidth);
        ctx.write_all("👍🏽".as_bytes()).unwrap();
        // The emoji is 2 cells wide (verified against upstream Go
        // displaywidth), landing at x=2 without wrapping.
        assert_eq!(ctx.position(), (2, 0));
    }
}
