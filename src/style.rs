//! Cleanroom Rust port of upstream Go source file: `cell.go` (Style)
//! Upstream Target Tag / Version: `v0.0.0-20260703014108-f5a850f9c2b7`
//!
//! <public-docs>
//! The ultraviolet cell style: attributes, colors, underline styles, and SGR
//! diffing for efficient terminal rendering.
//! </public-docs>

use charming_x_ansi::style::{Color, Underline, RESET_STYLE};
use charming_x_ansi::Style as AnsiStyle;

/// These are the available text attributes that can be combined to create
/// different styles.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Attr(u8);

impl Attr {
    /// Bold attribute.
    pub const BOLD: Attr = Attr(1 << 0);
    /// Faint attribute.
    pub const FAINT: Attr = Attr(1 << 1);
    /// Italic attribute.
    pub const ITALIC: Attr = Attr(1 << 2);
    /// Blink attribute.
    pub const BLINK: Attr = Attr(1 << 3);
    /// Rapid blink attribute (not widely supported).
    pub const RAPID_BLINK: Attr = Attr(1 << 4);
    /// Reverse attribute.
    pub const REVERSE: Attr = Attr(1 << 5);
    /// Conceal attribute.
    pub const CONCEAL: Attr = Attr(1 << 6);
    /// Strikethrough attribute.
    pub const STRIKETHROUGH: Attr = Attr(1 << 7);
}

/// Style represents the style of a cell.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Style {
    /// Foreground color.
    pub fg: Option<Color>,
    /// Background color.
    pub bg: Option<Color>,
    /// Underline color.
    pub underline_color: Option<Color>,
    /// Underline style.
    pub underline: Underline,
    /// Text attribute bits.
    pub attrs: u8,
}

impl Style {
    /// Returns true if the style is equal to the other style.
    pub fn equal(&self, o: &Style) -> bool {
        self.attrs == o.attrs
            && self.underline == o.underline
            && self.fg == o.fg
            && self.bg == o.bg
            && self.underline_color == o.underline_color
    }

    /// Returns whether the style is zero (no attributes or colors).
    pub fn is_zero(&self) -> bool {
        self == &Style::default()
    }

    /// Wraps the given string with the style's ANSI sequences and resets.
    pub fn styled(&self, str: &str) -> String {
        if self.is_zero() {
            return str.to_string();
        }
        format!("{}{}{}", self.string(), str, RESET_STYLE)
    }

    /// Returns the ANSI SGR sequence for the style.
    pub fn string(&self) -> String {
        if self.is_zero() {
            return RESET_STYLE.to_string();
        }
        let mut b = AnsiStyle::default();
        let a = self.attrs;
        if a & Attr::BOLD.0 != 0 {
            b.bold = true;
        }
        if a & Attr::FAINT.0 != 0 {
            b.faint = true;
        }
        if a & Attr::ITALIC.0 != 0 {
            b.italic = true;
        }
        if a & Attr::BLINK.0 != 0 {
            b.blink = true;
        }
        if a & Attr::RAPID_BLINK.0 != 0 {
            b.blink = true;
        }
        if a & Attr::REVERSE.0 != 0 {
            b.reverse = true;
        }
        if a & Attr::CONCEAL.0 != 0 {
            // Conceal is not modeled by the base style; rendered as reverse
            // fallback is avoided — upstream emits SGR 8.
            b.strikethrough = false;
        }
        if a & Attr::STRIKETHROUGH.0 != 0 {
            b.strikethrough = true;
        }
        match self.underline {
            Underline::None => {}
            Underline::Single => {
                b.underline = true;
                b.underline_style = Underline::Single;
            }
            u => {
                b.underline = true;
                b.underline_style = u;
            }
        }
        b.fg_color = self.fg.map(|c| ansi_color(c));
        b.bg_color = self.bg.map(|c| ansi_color(c));
        b.ul_color = self.underline_color.map(|c| ansi_color(c));
        let s = b.string();
        if s.is_empty() {
            RESET_STYLE.to_string()
        } else {
            s
        }
    }

    /// Returns the ANSI sequence that sets the style as a diff from another
    /// style.
    pub fn diff(&self, from: &Style) -> String {
        style_diff(from, self)
    }
}

/// Maps the ultraviolet color to the base ansi style color.
fn ansi_color(c: Color) -> charming_x_ansi::style::Color {
    c
}

/// StyleDiff returns the SGR ANSI sequence necessary to transition from the
/// "from" style to the "to" style.
pub fn style_diff(from: &Style, to: &Style) -> String {
    if from == to {
        return String::new();
    }
    if from.is_zero() {
        return to.string();
    }
    if to.is_zero() {
        return RESET_STYLE.to_string();
    }

    let mut b = AnsiStyle::default();

    if from.fg != to.fg {
        b.fg_color = to.fg.map(ansi_color);
    }
    if from.bg != to.bg {
        b.bg_color = to.bg.map(ansi_color);
    }
    if from.underline_color != to.underline_color {
        b.ul_color = to.underline_color.map(ansi_color);
    }

    let from_attrs = from.attrs;
    let to_attrs = to.attrs;
    let from_underline = from.underline != Underline::None;
    let to_underline = to.underline != Underline::None;

    // Resets first: bold/faint/italic/underline/blink/reverse/conceal/strike.
    if from_attrs & Attr::BOLD.0 != 0 && to_attrs & Attr::BOLD.0 == 0 {
        b.bold = false;
        // "22" is the normal-intensity reset in the base style.
        b.faint = false;
    }
    if from_attrs & Attr::FAINT.0 != 0 && to_attrs & Attr::FAINT.0 == 0 {
        b.faint = false;
    }
    if from_attrs & Attr::ITALIC.0 != 0 && to_attrs & Attr::ITALIC.0 == 0 {
        b.italic = false;
    }
    if from_underline && !to_underline {
        b.underline = false;
    }
    if from_attrs & Attr::BLINK.0 != 0 && to_attrs & Attr::BLINK.0 == 0 {
        b.blink = false;
    }
    if from_attrs & Attr::REVERSE.0 != 0 && to_attrs & Attr::REVERSE.0 == 0 {
        b.reverse = false;
    }
    if from_attrs & Attr::CONCEAL.0 != 0 && to_attrs & Attr::CONCEAL.0 == 0 {
        b.strikethrough = false;
    }
    if from_attrs & Attr::STRIKETHROUGH.0 != 0 && to_attrs & Attr::STRIKETHROUGH.0 == 0 {
        b.strikethrough = false;
    }

    // Then the attributes that are being set.
    if to_attrs & Attr::BOLD.0 != 0 && from_attrs & Attr::BOLD.0 == 0 {
        b.bold = true;
    }
    if to_attrs & Attr::FAINT.0 != 0 && from_attrs & Attr::FAINT.0 == 0 {
        b.faint = true;
    }
    if to_attrs & Attr::ITALIC.0 != 0 && from_attrs & Attr::ITALIC.0 == 0 {
        b.italic = true;
    }
    if to_underline && !from_underline {
        b.underline = true;
        b.underline_style = to.underline;
    }
    if to_attrs & Attr::BLINK.0 != 0 && from_attrs & Attr::BLINK.0 == 0 {
        b.blink = true;
    }
    if to_attrs & Attr::REVERSE.0 != 0 && from_attrs & Attr::REVERSE.0 == 0 {
        b.reverse = true;
    }
    if to_attrs & Attr::CONCEAL.0 != 0 && from_attrs & Attr::CONCEAL.0 == 0 {
        b.strikethrough = false;
    }
    if to_attrs & Attr::STRIKETHROUGH.0 != 0 && from_attrs & Attr::STRIKETHROUGH.0 == 0 {
        b.strikethrough = true;
    }

    b.string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_style_string() {
        let mut s = Style::default();
        s.attrs = Attr::BOLD.0;
        assert_eq!(s.string(), "\x1b[1m");
        assert_eq!(s.styled("hi"), "\x1b[1mhi\x1b[m");
    }

    #[test]
    fn test_style_diff() {
        let mut from = Style::default();
        from.attrs = Attr::BOLD.0;
        let to = Style::default();
        assert_eq!(style_diff(&from, &to), "\x1b[m");
    }
}
