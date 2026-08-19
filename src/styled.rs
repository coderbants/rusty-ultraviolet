//! Cleanroom Rust port of upstream Go source file: `styled.go`
//! Upstream Target Tag / Version: `v0.0.0-20260703014108-f5a850f9c2b7`
//!
//! <public-docs>
//! The styled string: decomposes a rendered ANSI string into styled lines
//! and cells that can be drawn into a buffer, parsing SGR and hyperlink
//! escape codes.
//! </public-docs>

use crate::buffer::{Line, Screen};
use crate::cell::{Cell, Link};
use crate::screen::Rectangle;
use crate::style::{Attr, Style};
use rusty_x_ansi::method::WidthMethod;
use rusty_x_ansi::parser::{
    decode_sequence, decode_sequence_wc, get_parser, has_csi_prefix, has_osc_prefix, Cmd, Params,
};
use rusty_x_ansi::style::{read_style_color, Color, Underline};

/// StyledString is a string that can be decomposed into a series of styled
/// lines and cells. It is used to disassemble a rendered string with ANSI
/// escape codes into a series of cells that can be used in a [crate::Buffer].
#[derive(Debug, Clone, Default)]
pub struct StyledString {
    /// Text is the original string that was used to create the styled string.
    pub text: String,
    /// Wrap determines whether the styled string should wrap to the next
    /// line.
    pub wrap: bool,
    /// Tail is the string that will be appended to the end of the line when
    /// the string is truncated i.e. when [StyledString::wrap] is false.
    pub tail: String,
}

/// NewStyledString creates a new [StyledString] for the given styled string.
pub fn new_styled_string(str_: &str) -> StyledString {
    StyledString {
        text: str_.to_string(),
        ..StyledString::default()
    }
}

impl crate::Drawable for StyledString {
    /// Draw renders the styled string on the screen for the given area.
    fn draw(&mut self, scr: &mut dyn Screen, area: Rectangle) {
        StyledString::draw(self, scr, area);
    }
}

impl StyledString {
    /// String returns the text of the styled string.
    pub fn string(&self) -> &str {
        &self.text
    }

    /// Lines returns the styled string decomposed into a slice of [Line]s.
    pub fn lines(&self, m: WidthMethod) -> Vec<Line> {
        print_string(
            None,
            m,
            0,
            0,
            Rectangle {
                min: (0, 0),
                max: (0, 0),
            },
            (0, 0),
            0,
            0,
            &self.text,
            false,
            "",
        )
    }

    /// Draw renders the styled string to the given buffer at the specified
    /// area.
    pub fn draw(&self, buf: &mut dyn Screen, area: Rectangle) {
        // Clear the area before drawing.
        for y in area.min.1..area.max.1 {
            for x in area.min.0..area.max.0 {
                buf.set_cell(x, y, None);
            }
        }
        let str_ = self.text.replace("\r\n", "\n");
        let method = buf.width_method();
        print_string(
            Some(buf),
            method,
            area.min.0 as i64,
            area.min.1,
            area,
            (area.min.0 as i64, area.min.1 as i64),
            area.dx(),
            area.dy(),
            &str_,
            !self.wrap,
            &self.tail,
        );
    }

    /// DrawAt renders the styled string to the given buffer starting at the
    /// given (possibly negative) origin, sized to the given width and height
    /// (mirrors the upstream `StyledString.Draw` with an off-screen origin,
    /// e.g. the layout example's dialog box). The area is intersected with
    /// the screen so the upstream's "all cells are in the area bounds" logic
    /// applies while off-screen cells are clipped by the screen itself.
    pub fn draw_at(&self, buf: &mut dyn Screen, x: i64, y: i64, w: usize, h: usize) {
        let str_ = self.text.replace("\r\n", "\n");
        let method = buf.width_method();
        let sb = buf.bounds();
        let x0 = x.max(0) as usize;
        let y0 = y.max(0) as usize;
        let x1 = ((x + w as i64).clamp(0, sb.max.0 as i64)) as usize;
        let y1 = ((y + h as i64).clamp(0, sb.max.1 as i64)) as usize;
        let bounds = Rectangle {
            min: (x0, y0),
            max: (x1.max(x0), y1.max(y0)),
        };
        print_string(
            Some(buf),
            method,
            x,
            y.max(0) as usize,
            bounds,
            (x, y),
            w,
            h,
            &str_,
            !self.wrap,
            &self.tail,
        );
    }

    /// Height returns the number of lines in the styled string.
    pub fn height(&self) -> usize {
        self.text.matches('\n').count() + 1
    }

    /// UnicodeWidth returns the cells width of the widest line in the styled
    /// string using the [WidthMethod::GraphemeWidth] method.
    pub fn unicode_width(&self) -> usize {
        self.width_height(WidthMethod::GraphemeWidth).0
    }

    /// WcWidth returns the cells width of the widest line in the styled
    /// string using the [WidthMethod::WcWidth] method.
    pub fn wc_width(&self) -> usize {
        self.width_height(WidthMethod::WcWidth).0
    }

    fn width_height(&self, m: WidthMethod) -> (usize, usize) {
        let lines: Vec<&str> = self.text.split('\n').collect();
        let h = lines.len();
        let mut w = 0;
        for l in &lines {
            w = w.max(m.string_width(l));
        }
        (w, h)
    }

    /// Bounds returns the minimum area that can contain the whole styled
    /// string.
    pub fn bounds(&self) -> Rectangle {
        let (w, h) = self.width_height(WidthMethod::GraphemeWidth);
        Rectangle {
            min: (0, 0),
            max: (w, h),
        }
    }
}

/// printString draws a string starting at the given position. If scr is None,
/// it will build and return a slice of [Line]s instead (unwrapped, ignoring
/// bounds).
/// Mirrors the upstream Go signature `printString(scr Screen, m WidthMethod,
/// x int64, y int, bounds Rectangle, origin Point, areaW, areaH int, str
/// string, truncate bool, tail string)` 1:1.
#[allow(clippy::too_many_arguments)]
fn print_string(
    mut scr: Option<&mut dyn Screen>,
    m: WidthMethod,
    mut x: i64,
    mut y: usize,
    bounds: Rectangle,
    origin: (i64, i64),
    area_w: usize,
    area_h: usize,
    str_: &str,
    truncate: bool,
    tail: &str,
) -> Vec<Line> {
    let mut p = get_parser();

    let mut tailc = Cell::default();
    if truncate && !tail.is_empty() {
        tailc = new_cell_with_method(m, tail);
    }

    let mut lines: Vec<Line> = Vec::new();
    let start_x = x;

    let mut cell = Cell::default();
    let mut style = Style::default();
    let mut link = Link::default();
    let mut state = rusty_x_ansi::parser::NORMAL_STATE;
    let mut rest = str_;
    while !rest.is_empty() {
        let d = if m == WidthMethod::GraphemeWidth {
            decode_sequence(rest.as_bytes(), state, Some(&mut p))
        } else {
            decode_sequence_wc(rest.as_bytes(), state, Some(&mut p))
        };
        let seq = d.seq;
        let width = d.width;
        let n = d.n;
        state = d.state;

        match width {
            1..=4 => {
                // wide cells can go up to 4 cells wide
                cell.width = width;
                cell.content = String::from_utf8_lossy(seq).into_owned();
                cell.style = style.clone();
                cell.link = if link.is_zero() {
                    None
                } else {
                    Some(link.clone())
                };

                match &mut scr {
                    None => {
                        // Building lines: unwrapped, no bounds
                        if y >= lines.len() {
                            lines.push(Line(Vec::new()));
                        }
                        lines[y].0.push(cell.clone());
                        x += width as i64;
                    }
                    Some(scr) => {
                        // Drawing to screen: handle wrapping, truncation, and
                        // bounds
                        if !truncate
                            && x + cell.width as i64 > bounds.max.0 as i64
                            && y + 1 < bounds.max.1
                        {
                            // Wrap the string to the width of the window
                            x = bounds.min.0 as i64;
                            y += 1;
                        }

                        let pos = crate::window::pos(x, y as i64);
                        // Cells are checked against the drawing area with its
                        // signed origin (the upstream `pos.In(bounds)` where
                        // bounds is the box area), and the screen itself clips
                        // out-of-bounds cells. The cursor advances for every
                        // in-area cell, mirroring the upstream.
                        let in_area = pos.x >= origin.0
                            && pos.x < origin.0 + area_w as i64
                            && pos.y >= origin.1
                            && pos.y < origin.1 + area_h as i64;
                        if in_area {
                            if pos.x >= 0 && pos.y >= 0 {
                                if truncate
                                    && tailc.width > 0
                                    && x + cell.width as i64
                                        > bounds.max.0 as i64 - tailc.width as i64
                                {
                                    // Truncate the string and append the tail
                                    // if any.
                                    let mut c = tailc.clone();
                                    c.style = style.clone();
                                    c.link = if link.is_zero() {
                                        None
                                    } else {
                                        Some(link.clone())
                                    };
                                    scr.set_cell(x as usize, y, Some(&c));
                                    x += tailc.width as i64;
                                } else {
                                    // Print the cell to the screen
                                    scr.set_cell(x as usize, y, Some(&cell));
                                    x += width as i64;
                                }
                            } else {
                                x += width as i64;
                            }
                        }
                    }
                }

                // Reset cell for next iteration
                cell = Cell::default();
            }
            _ => {
                // Valid sequences always have a non-zero Cmd.
                match () {
                    _ if has_csi_prefix(seq) && Cmd(p.command()).final_() == b'm' => {
                        // SGR - Select Graphic Rendition
                        read_style(Params(p.params().as_slice()), &mut style);
                    }
                    _ if has_osc_prefix(seq) && p.command() == 8 => {
                        // Hyperlinks
                        read_link(p.data(), &mut link);
                    }
                    _ if seq == b"\n" => {
                        if scr.is_none() {
                            // When building lines, we need to ensure empty
                            // lines are represented.
                            if y >= lines.len() {
                                lines.push(Line(Vec::new()));
                            }
                        }
                        y += 1;
                        // Always treat a NL as CR-LF similar to Termios ONLCR.
                        // Upstream resets to `bounds.Min.X`; the draw origin
                        // equals the bounds min for the regular draw path, so
                        // resetting to the initial x matches both paths (and
                        // keeps an off-screen origin like the layout example's
                        // dialog box).
                        if scr.is_none() {
                            x = 0;
                        } else {
                            x = start_x;
                        }
                    }
                    _ if seq == b"\r" => {
                        if scr.is_none() {
                            x = 0;
                        } else {
                            x = bounds.min.0 as i64;
                        }
                    }
                    _ => {
                        cell.content.push_str(&String::from_utf8_lossy(seq));
                    }
                }
            }
        }

        // Advance the state and data
        rest = &rest[n..];

        if y >= bounds.max.1 {
            // We've reached the bottom of the bounds, stop processing further
            // lines.
            break;
        }
    }

    // Make sure to set the last cell if it's not empty.
    if !cell.is_zero() && scr.is_some() {
        if let Some(scr) = scr {
            scr.set_cell(x as usize, y, Some(&cell));
        }
    }

    lines
}

/// NewCell creates a new cell from the given string grapheme using the given
/// width method.
fn new_cell_with_method(method: WidthMethod, gr: &str) -> Cell {
    if gr.is_empty() {
        return Cell::default();
    }
    if gr == " " {
        return crate::cell::empty_cell();
    }
    Cell {
        content: gr.to_string(),
        width: method.string_width(gr),
        ..Cell::default()
    }
}

/// ReadStyle reads a Select Graphic Rendition (SGR) escape sequence from a
/// list of parameters into pen.
pub fn read_style(params: Params<'_>, pen: &mut Style) {
    if params.as_slice().is_empty() {
        *pen = Style::default();
        return;
    }

    let mut i = 0usize;
    while i < params.as_slice().len() {
        let (param, has_more, _) = params.param(i, 0);
        match param {
            0 => {
                // Reset
                *pen = Style::default();
            }
            1 => {
                // Bold
                pen.attrs |= Attr::BOLD.bits();
            }
            2 => {
                // Dim/Faint
                pen.attrs |= Attr::FAINT.bits();
            }
            3 => {
                // Italic
                pen.attrs |= Attr::ITALIC.bits();
            }
            4 => {
                // Underline
                let (next_param, _, ok) = params.param(i + 1, 0);
                if has_more && ok {
                    // Only accept subparameters i.e. separated by ":"
                    if let 0..=5 = next_param {
                        i += 1;
                        pen.underline = match next_param {
                            0 => Underline::None,
                            1 => Underline::Single,
                            2 => Underline::Double,
                            3 => Underline::Curly,
                            4 => Underline::Dotted,
                            5 => Underline::Dashed,
                            _ => unreachable!(),
                        };
                    }
                } else {
                    // Single Underline
                    pen.underline = Underline::Single;
                }
            }
            5 => {
                // Slow Blink
                pen.attrs |= Attr::BLINK.bits();
            }
            6 => {
                // Rapid Blink
                pen.attrs |= Attr::RAPID_BLINK.bits();
            }
            7 => {
                // Reverse
                pen.attrs |= Attr::REVERSE.bits();
            }
            8 => {
                // Conceal
                pen.attrs |= Attr::CONCEAL.bits();
            }
            9 => {
                // Crossed-out/Strikethrough
                pen.attrs |= Attr::STRIKETHROUGH.bits();
            }
            22 => {
                // Normal Intensity (not bold or faint)
                pen.attrs &= !(Attr::BOLD.bits() | Attr::FAINT.bits());
            }
            23 => {
                // Not italic, not Fraktur
                pen.attrs &= !Attr::ITALIC.bits();
            }
            24 => {
                // Not underlined
                pen.underline = Underline::None;
            }
            25 => {
                // Blink off
                pen.attrs &= !(Attr::BLINK.bits() | Attr::RAPID_BLINK.bits());
            }
            27 => {
                // Positive (not reverse)
                pen.attrs &= !Attr::REVERSE.bits();
            }
            28 => {
                // Reveal
                pen.attrs &= !Attr::CONCEAL.bits();
            }
            29 => {
                // Not crossed out
                pen.attrs &= !Attr::STRIKETHROUGH.bits();
            }
            30..=37 => {
                // Set foreground
                pen.fg = Some(Color::Basic((param - 30) as u8));
            }
            38 => {
                // Set foreground 256 or truecolor
                let mut c = None;
                let n = read_style_color(&params.as_slice()[i..], &mut c);
                if n > 0 {
                    pen.fg = c;
                    i += n - 1;
                }
            }
            39 => {
                // Default foreground
                pen.fg = None;
            }
            40..=47 => {
                // Set background
                pen.bg = Some(Color::Basic((param - 40) as u8));
            }
            48 => {
                // Set background 256 or truecolor
                let mut c = None;
                let n = read_style_color(&params.as_slice()[i..], &mut c);
                if n > 0 {
                    pen.bg = c;
                    i += n - 1;
                }
            }
            49 => {
                // Default Background
                pen.bg = None;
            }
            58 => {
                // Set underline color
                let mut c = None;
                let n = read_style_color(&params.as_slice()[i..], &mut c);
                if n > 0 {
                    pen.underline_color = c;
                    i += n - 1;
                }
            }
            59 => {
                // Default underline color
                pen.underline_color = None;
            }
            90..=97 => {
                // Set bright foreground
                pen.fg = Some(Color::Basic((8 + param - 90) as u8));
            }
            100..=107 => {
                // Set bright background
                pen.bg = Some(Color::Basic((8 + param - 100) as u8));
            }
            _ => {}
        }
        i += 1;
    }
}

/// ReadLink reads a hyperlink escape sequence from a data buffer into link.
pub fn read_link(p: &[u8], link: &mut Link) {
    let params: Vec<&[u8]> = p.split(|&c| c == b';').collect();
    if params.len() != 3 {
        return;
    }
    link.params = String::from_utf8_lossy(params[1]).into_owned();
    link.url = String::from_utf8_lossy(params[2]).into_owned();
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusty_x_ansi::parser::HAS_MORE_FLAG;

    #[test]
    fn test_styled_string_lines_zero_bounds_quirk() {
        // Go-verified: Lines() calls printString with a zero Rectangle{}, so
        // the y >= bounds.Max.Y early break fires after the first grapheme.
        let ss = new_styled_string("Hello\nWorld");
        let lines = ss.lines(WidthMethod::WcWidth);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].0.len(), 1);
        assert_eq!(lines[0].0[0].content, "H");
    }

    #[test]
    fn test_styled_string_draw_with_style() {
        let mut b = crate::new_buffer(20, 3);
        let ss = new_styled_string("\x1b[1mBold\x1b[0m");
        ss.draw(
            &mut b,
            Rectangle {
                min: (0, 0),
                max: (20, 3),
            },
        );
        assert_eq!(b.cell_at(0, 0).unwrap().content, "B");
        assert_ne!(b.cell_at(0, 0).unwrap().style.attrs & Attr::BOLD.bits(), 0);
        assert_eq!(b.cell_at(3, 0).unwrap().content, "d");
        // The 'd' cell is written before the reset SGR arrives, so it is
        // still bold (the reset affects only subsequent cells).
        assert_ne!(b.cell_at(3, 0).unwrap().style.attrs & Attr::BOLD.bits(), 0);
        // Text after the reset is not bold.
        let mut b2 = crate::new_buffer(20, 3);
        let ss2 = new_styled_string("\x1b[1mBold\x1b[0mX");
        ss2.draw(
            &mut b2,
            Rectangle {
                min: (0, 0),
                max: (20, 3),
            },
        );
        assert_eq!(b2.cell_at(4, 0).unwrap().content, "X");
        assert_eq!(b2.cell_at(4, 0).unwrap().style.attrs & Attr::BOLD.bits(), 0);
    }

    #[test]
    fn test_styled_string_draw_colors() {
        let mut b = crate::new_buffer(20, 3);
        let ss = new_styled_string("\x1b[38;5;196mRed");
        ss.draw(
            &mut b,
            Rectangle {
                min: (0, 0),
                max: (20, 3),
            },
        );
        assert_eq!(b.cell_at(0, 0).unwrap().style.fg, Some(Color::Indexed(196)));
    }

    #[test]
    fn test_styled_string_multiline_draw() {
        let mut b = crate::new_buffer(20, 3);
        let ss = new_styled_string("Hello\nWorld");
        ss.draw(
            &mut b,
            Rectangle {
                min: (0, 0),
                max: (20, 3),
            },
        );
        assert_eq!(b.cell_at(4, 0).unwrap().content, "o");
        assert_eq!(b.cell_at(0, 1).unwrap().content, "W");
        assert_eq!(b.cell_at(4, 1).unwrap().content, "d");
    }

    #[test]
    fn test_styled_string_height_width() {
        let ss = new_styled_string("ab\ncdef");
        assert_eq!(ss.height(), 2);
        assert_eq!(ss.unicode_width(), 4);
        assert_eq!(ss.wc_width(), 4);
    }

    #[test]
    fn test_styled_string_draw() {
        let mut b = crate::new_buffer(20, 2);
        let ss = new_styled_string("Hi");
        ss.draw(
            &mut b,
            Rectangle {
                min: (0, 0),
                max: (20, 2),
            },
        );
        assert_eq!(b.cell_at(0, 0).unwrap().content, "H");
        assert_eq!(b.cell_at(1, 0).unwrap().content, "i");
    }

    #[test]
    fn test_read_style() {
        let mut style = Style::default();
        let params = vec![1, 3, 38, 2, 100, 200, 50];
        read_style(Params(&params), &mut style);
        assert_ne!(style.attrs & Attr::BOLD.bits(), 0);
        assert_ne!(style.attrs & Attr::ITALIC.bits(), 0);
        assert_eq!(
            style.fg,
            Some(Color::RGB(rusty_x_ansi::color::RGBColor {
                r: 100,
                g: 200,
                b: 50
            }))
        );
    }

    #[test]
    fn test_read_link() {
        let mut link = Link::default();
        read_link(b"8;id=1;https://example.com", &mut link);
        assert_eq!(link.url, "https://example.com");
        assert_eq!(link.params, "id=1");
    }

    /// Full SGR attribute coverage through `read_style`.
    #[test]
    fn test_read_style_attrs() {
        // Empty params reset the style.
        let mut style = Style {
            fg: Some(Color::Basic(1)),
            ..Style::default()
        };
        read_style(Params(&[]), &mut style);
        assert!(style.fg.is_none());

        // All attribute setters.
        let params = vec![1, 2, 3, 5, 6, 7, 8, 9];
        let mut style = Style::default();
        read_style(Params(&params), &mut style);
        assert_ne!(style.attrs & Attr::BOLD.bits(), 0);
        assert_ne!(style.attrs & Attr::FAINT.bits(), 0);
        assert_ne!(style.attrs & Attr::ITALIC.bits(), 0);
        assert_ne!(style.attrs & Attr::BLINK.bits(), 0);
        assert_ne!(style.attrs & Attr::RAPID_BLINK.bits(), 0);
        assert_ne!(style.attrs & Attr::REVERSE.bits(), 0);
        assert_ne!(style.attrs & Attr::CONCEAL.bits(), 0);
        assert_ne!(style.attrs & Attr::STRIKETHROUGH.bits(), 0);

        // Turning the attributes off.
        let mut style = Style {
            attrs: u8::MAX,
            underline: Underline::Double,
            ..Style::default()
        };
        let params = vec![22, 23, 24, 25, 27, 28, 29];
        read_style(Params(&params), &mut style);
        assert_eq!(style.attrs & Attr::BOLD.bits(), 0);
        assert_eq!(style.attrs & Attr::FAINT.bits(), 0);
        assert_eq!(style.attrs & Attr::ITALIC.bits(), 0);
        assert_eq!(style.attrs & Attr::BLINK.bits(), 0);
        assert_eq!(style.attrs & Attr::RAPID_BLINK.bits(), 0);
        assert_eq!(style.attrs & Attr::REVERSE.bits(), 0);
        assert_eq!(style.attrs & Attr::CONCEAL.bits(), 0);
        assert_eq!(style.attrs & Attr::STRIKETHROUGH.bits(), 0);
        assert_eq!(style.underline, Underline::None);
    }

    /// Foreground/background/underline color codes.
    #[test]
    fn test_read_style_colors() {
        // Basic fg/bg.
        let mut style = Style::default();
        let params = vec![30, 31, 40, 41];
        read_style(Params(&params), &mut style);
        assert_eq!(style.fg, Some(Color::Basic(1)));
        assert_eq!(style.bg, Some(Color::Basic(1)));
        // Bright fg/bg.
        let mut style = Style::default();
        let params = vec![90, 91, 100, 101];
        read_style(Params(&params), &mut style);
        assert_eq!(style.fg, Some(Color::Basic(9)));
        assert_eq!(style.bg, Some(Color::Basic(9)));
        // Defaults clear colors.
        let mut style = Style {
            fg: Some(Color::Basic(1)),
            bg: Some(Color::Basic(2)),
            underline_color: Some(Color::Basic(3)),
            ..Style::default()
        };
        let params = vec![39, 49, 59];
        read_style(Params(&params), &mut style);
        assert!(style.fg.is_none());
        assert!(style.bg.is_none());
        assert!(style.underline_color.is_none());
        // 256-color fg/bg/underline.
        let mut style = Style::default();
        let params = vec![38, 5, 196, 48, 5, 42, 58, 5, 1];
        read_style(Params(&params), &mut style);
        assert_eq!(style.fg, Some(Color::Indexed(196)));
        assert_eq!(style.bg, Some(Color::Indexed(42)));
        assert_eq!(style.underline_color, Some(Color::Indexed(1)));
    }

    /// Underline styles via sub-params.
    #[test]
    fn test_read_style_underline_subparams() {
        let cases: &[(i32, Underline)] = &[
            (0, Underline::None),
            (1, Underline::Single),
            (2, Underline::Double),
            (3, Underline::Curly),
            (4, Underline::Dotted),
            (5, Underline::Dashed),
        ];
        for &(sub, want) in cases {
            // Param 4 with a sub-parameter requires has_more on the 4 and the
            // next param to be valid (0..=5).
            let mut style = Style::default();
            read_style(Params(&[4 | HAS_MORE_FLAG, sub]), &mut style);
            assert_eq!(style.underline, want, "sub {sub}");
        }
        // Out-of-range sub-param falls back to single underline.
        let mut style = Style::default();
        read_style(Params(&[4, 6]), &mut style);
        assert_eq!(style.underline, Underline::Single);
        // Plain 4 (no sub-param).
        let mut style = Style::default();
        read_style(Params(&[4]), &mut style);
        assert_eq!(style.underline, Underline::Single);
    }

    /// draw_at with an off-screen origin clips correctly.
    #[test]
    fn test_draw_at_offscreen() {
        let mut b = crate::new_buffer(10, 3);
        let ss = new_styled_string("Hello");
        ss.draw_at(&mut b, -2, 0, 10, 3);
        // With origin -2, 'H' is at x=-2, so the first visible cell is 'l'.
        assert_eq!(b.cell_at(0, 0).unwrap().content, "l");
        assert_eq!(b.cell_at(1, 0).unwrap().content, "l");
        assert_eq!(b.cell_at(2, 0).unwrap().content, "o");
    }

    /// CR in the text resets the x position to the bounds start.
    #[test]
    fn test_draw_cr_sequence() {
        let mut b = crate::new_buffer(10, 2);
        let ss = new_styled_string("AB\rCD");
        ss.draw(
            &mut b,
            Rectangle {
                min: (0, 0),
                max: (10, 2),
            },
        );
        // After \r, C overwrites A.
        assert_eq!(b.cell_at(0, 0).unwrap().content, "C");
        assert_eq!(b.cell_at(1, 0).unwrap().content, "D");
    }

    /// Wrap=false truncates with the tail.
    #[test]
    fn test_draw_no_wrap_tail() {
        let mut b = crate::new_buffer(4, 1);
        let mut ss = new_styled_string("Hello World");
        ss.wrap = false;
        ss.tail = "…".to_string();
        ss.draw(
            &mut b,
            Rectangle {
                min: (0, 0),
                max: (4, 1),
            },
        );
        assert_eq!(b.cell_at(0, 0).unwrap().content, "H");
        // The string is truncated to fit and the tail is appended.
        assert_eq!(b.cell_at(3, 0).unwrap().content, "…");
    }

    /// A styled string with an OSC 8 hyperlink populates cell links.
    #[test]
    fn test_draw_hyperlink() {
        let mut b = crate::new_buffer(20, 2);
        let ss = new_styled_string("\x1b]8;id=1;https://x.dev\x07Link\x1b]8;;\x07");
        ss.draw(
            &mut b,
            Rectangle {
                min: (0, 0),
                max: (20, 2),
            },
        );
        assert_eq!(b.cell_at(0, 0).unwrap().content, "L");
        let link = b.cell_at(0, 0).unwrap().link.clone();
        assert!(link.is_some());
        if let Some(l) = link {
            assert_eq!(l.url, "https://x.dev");
            assert_eq!(l.params, "id=1");
        }
    }
}
