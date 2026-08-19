//! Cleanroom Rust port of upstream Go source file: `cell.go` (Style)
//! Upstream Target Tag / Version: `v0.0.0-20260703014108-f5a850f9c2b7`
//!
//! <public-docs>
//! The ultraviolet cell style: attributes, colors, underline styles, and SGR
//! diffing for efficient terminal rendering.
//! </public-docs>

use rusty_x_ansi::style::{Color, Underline, RESET_STYLE};
use rusty_x_ansi::Style as AnsiStyle;

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

    /// Returns the raw attribute bits.
    pub fn bits(&self) -> u8 {
        self.0
    }
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
        b.fg_color = self.fg.map(ansi_color);
        b.bg_color = self.bg.map(ansi_color);
        b.ul_color = self.underline_color.map(ansi_color);
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
fn ansi_color(c: Color) -> rusty_x_ansi::style::Color {
    c
}

/// StyleDiff returns the SGR ANSI sequence necessary to transition from the
/// "from" style to the "to" style.
pub fn style_diff(from: &Style, to: &Style) -> String {
    if from == to {
        return String::new();
    }
    // NOTE: upstream only short-circuits for a nil `from` pointer; a zero
    // (non-nil) style goes through the normal diff path (colors first).
    if to.is_zero() {
        return RESET_STYLE.to_string();
    }

    let mut b = AnsiStyle::default();

    // NOTE: upstream emits the default-color reset (SGR 39/49/59) when a
    // color transitions to nil (e.g. `ForegroundColor(nil)`); mirror that.
    if from.fg != to.fg {
        b.fg_color = Some(to.fg.map(ansi_color).unwrap_or(Color::Default));
    }
    if from.bg != to.bg {
        b.bg_color = Some(to.bg.map(ansi_color).unwrap_or(Color::Default));
    }
    if from.underline_color != to.underline_color {
        b.ul_color = Some(to.underline_color.map(ansi_color).unwrap_or(Color::Default));
    }

    let from_attrs = from.attrs;
    let to_attrs = to.attrs;
    let from_underline = from.underline != Underline::None;
    let to_underline = to.underline != Underline::None;

    let from_bold = from_attrs & Attr::BOLD.0 != 0;
    let from_faint = from_attrs & Attr::FAINT.0 != 0;
    let from_italic = from_attrs & Attr::ITALIC.0 != 0;
    let from_blink = from_attrs & Attr::BLINK.0 != 0;
    let from_rapid_blink = from_attrs & Attr::RAPID_BLINK.0 != 0;
    let from_reverse = from_attrs & Attr::REVERSE.0 != 0;
    let from_conceal = from_attrs & Attr::CONCEAL.0 != 0;
    let from_strikethrough = from_attrs & Attr::STRIKETHROUGH.0 != 0;
    let to_bold = to_attrs & Attr::BOLD.0 != 0;
    let to_faint = to_attrs & Attr::FAINT.0 != 0;
    let to_italic = to_attrs & Attr::ITALIC.0 != 0;
    let to_blink = to_attrs & Attr::BLINK.0 != 0;
    let to_rapid_blink = to_attrs & Attr::RAPID_BLINK.0 != 0;
    let to_reverse = to_attrs & Attr::REVERSE.0 != 0;
    let to_conceal = to_attrs & Attr::CONCEAL.0 != 0;
    let to_strikethrough = to_attrs & Attr::STRIKETHROUGH.0 != 0;

    let bold_changed = from_bold != to_bold;
    let faint_changed = from_faint != to_faint;
    let italic_changed = from_italic != to_italic;
    let underline_changed = from_underline != to_underline || from.underline != to.underline;
    let blink_changed = from_blink != to_blink;
    let rapid_blink_changed = from_rapid_blink != to_rapid_blink;
    let reverse_changed = from_reverse != to_reverse;
    let conceal_changed = from_conceal != to_conceal;
    let strikethrough_changed = from_strikethrough != to_strikethrough;

    // Build the SGR params in the upstream construction order: colors
    // first, then attribute resets, then attribute sets, then the
    // underline style.
    let mut params: Vec<String> = Vec::new();

    if let Some(c) = b.fg_color {
        params.push(rusty_x_ansi::style::color_seq(&c, 3));
    }
    if let Some(c) = b.bg_color {
        params.push(rusty_x_ansi::style::color_seq(&c, 4));
    }
    if let Some(c) = b.ul_color {
        params.push(rusty_x_ansi::style::color_seq(&c, 5));
    }

    if (bold_changed || faint_changed) && ((from_bold && !to_bold) || (from_faint && !to_faint)) {
        params.push("22".to_string());
    }
    if italic_changed && !to_italic {
        params.push("23".to_string());
    }
    if underline_changed && !to_underline {
        params.push("24".to_string());
    }
    if (blink_changed || rapid_blink_changed)
        && ((from_blink && !to_blink) || (from_rapid_blink && !to_rapid_blink))
    {
        params.push("25".to_string());
    }
    if reverse_changed && !to_reverse {
        params.push("27".to_string());
    }
    if conceal_changed && !to_conceal {
        params.push("8".to_string());
    }
    if strikethrough_changed && !to_strikethrough {
        params.push("29".to_string());
    }

    if bold_changed && to_bold {
        params.push("1".to_string());
    }
    if faint_changed && to_faint {
        params.push("2".to_string());
    }
    if italic_changed && to_italic {
        params.push("3".to_string());
    }
    if underline_changed && to_underline && to.underline == Underline::Single {
        params.push("4".to_string());
    }
    if blink_changed && to_blink {
        params.push("5".to_string());
    }
    if rapid_blink_changed && to_rapid_blink {
        params.push("6".to_string());
    }
    if reverse_changed && to_reverse {
        params.push("7".to_string());
    }
    if conceal_changed && to_conceal {
        params.push("8".to_string());
    }
    if strikethrough_changed && to_strikethrough {
        params.push("9".to_string());
    }

    if underline_changed
        && to_underline
        && to.underline != Underline::Single
        && to.underline != Underline::None
    {
        match to.underline {
            Underline::Double => params.push("21".to_string()),
            Underline::Curly => params.push("4:3".to_string()),
            Underline::Dotted => params.push("4:4".to_string()),
            Underline::Dashed => params.push("4:5".to_string()),
            _ => {}
        }
    }

    if params.is_empty() {
        return String::new();
    }
    format!("\x1b[{}m", params.join(";"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_style_string() {
        let s = Style {
            attrs: Attr::BOLD.0,
            ..Default::default()
        };
        assert_eq!(s.string(), "\x1b[1m");
        assert_eq!(s.styled("hi"), "\x1b[1mhi\x1b[m");
    }

    #[test]
    fn test_style_diff() {
        let from = Style {
            attrs: Attr::BOLD.0,
            ..Default::default()
        };
        let to = Style::default();
        assert_eq!(style_diff(&from, &to), "\x1b[m");
    }

    #[test]
    fn test_style_diff_attrs() {
        // Bold on.
        let from = Style::default();
        let to = Style {
            attrs: Attr::BOLD.0,
            ..Style::default()
        };
        assert_eq!(style_diff(&from, &to), "\x1b[1m");
        // Bold off (target keeps a fg so it is not zero).
        let from = Style {
            attrs: Attr::BOLD.0,
            ..Style::default()
        };
        let to = Style {
            fg: Some(Color::Basic(1)),
            ..Style::default()
        };
        assert_eq!(style_diff(&from, &to), "\x1b[31;22m");
        // Faint.
        assert_eq!(
            style_diff(
                &Style::default(),
                &Style {
                    attrs: Attr::FAINT.0,
                    ..Style::default()
                }
            ),
            "\x1b[2m"
        );
        // Italic.
        assert_eq!(
            style_diff(
                &Style::default(),
                &Style {
                    attrs: Attr::ITALIC.0,
                    ..Style::default()
                }
            ),
            "\x1b[3m"
        );
        // Blink / rapid blink.
        assert_eq!(
            style_diff(
                &Style::default(),
                &Style {
                    attrs: Attr::BLINK.0,
                    ..Style::default()
                }
            ),
            "\x1b[5m"
        );
        assert_eq!(
            style_diff(
                &Style::default(),
                &Style {
                    attrs: Attr::RAPID_BLINK.0,
                    ..Style::default()
                }
            ),
            "\x1b[6m"
        );
        // Reverse / conceal / strikethrough.
        assert_eq!(
            style_diff(
                &Style::default(),
                &Style {
                    attrs: Attr::REVERSE.0,
                    ..Style::default()
                }
            ),
            "\x1b[7m"
        );
        assert_eq!(
            style_diff(
                &Style::default(),
                &Style {
                    attrs: Attr::CONCEAL.0,
                    ..Style::default()
                }
            ),
            "\x1b[8m"
        );
        assert_eq!(
            style_diff(
                &Style::default(),
                &Style {
                    attrs: Attr::STRIKETHROUGH.0,
                    ..Style::default()
                }
            ),
            "\x1b[9m"
        );
        // Identical styles produce no output.
        let s = Style {
            attrs: Attr::BOLD.0,
            fg: Some(Color::Basic(1)),
            ..Style::default()
        };
        assert_eq!(style_diff(&s, &s), "");
        // A fully-zero target resets.
        assert_eq!(style_diff(&s, &Style::default()), "\x1b[m");
    }

    #[test]
    fn test_style_diff_colors() {
        // Foreground transition to default (target keeps an attr, non-zero).
        let from = Style {
            fg: Some(Color::Basic(1)),
            ..Style::default()
        };
        let to = Style {
            attrs: Attr::BOLD.0,
            ..Style::default()
        };
        assert_eq!(style_diff(&from, &to), "\x1b[39;1m");
        // Background transition to default (non-zero target).
        let from = Style {
            bg: Some(Color::Basic(2)),
            ..Style::default()
        };
        let to = Style {
            attrs: Attr::BOLD.0,
            ..Style::default()
        };
        assert_eq!(style_diff(&from, &to), "\x1b[49;1m");
        // Underline color transition to default (non-zero target).
        let from = Style {
            underline_color: Some(Color::Basic(3)),
            ..Style::default()
        };
        let to = Style {
            attrs: Attr::BOLD.0,
            ..Style::default()
        };
        assert_eq!(style_diff(&from, &to), "\x1b[59;1m");
        // Color set.
        let from = Style::default();
        let to = Style {
            fg: Some(Color::Basic(4)),
            ..Style::default()
        };
        assert_eq!(style_diff(&from, &to), "\x1b[34m");
        // Truecolor set.
        let to = Style {
            fg: Some(Color::RGB(rusty_x_ansi::color::RGBColor {
                r: 1,
                g: 2,
                b: 3,
            })),
            ..Style::default()
        };
        assert_eq!(style_diff(&from, &to), "\x1b[38;2;1;2;3m");
    }

    /// Style::string with all attributes and underline styles.
    #[test]
    fn test_style_string_all_attrs() {
        // All attributes together.
        let s = Style {
            attrs: Attr::BOLD.0
                | Attr::FAINT.0
                | Attr::ITALIC.0
                | Attr::BLINK.0
                | Attr::RAPID_BLINK.0
                | Attr::REVERSE.0
                | Attr::CONCEAL.0
                | Attr::STRIKETHROUGH.0,
            ..Style::default()
        };
        assert!(s.string().contains("1"));
        // Each attribute individually.
        let cases: &[(Attr, &str)] = &[
            (Attr::BOLD, "1"),
            (Attr::FAINT, "2"),
            (Attr::ITALIC, "3"),
            (Attr::BLINK, "5"),
            (Attr::RAPID_BLINK, "5"),
            (Attr::REVERSE, "7"),
            (Attr::STRIKETHROUGH, "9"),
        ];
        for (attr, code) in cases {
            let s = Style {
                attrs: attr.0,
                ..Style::default()
            };
            assert!(s.string().contains(code), "{attr:?} -> {code}");
        }
        // Underline styles.
        let cases: &[(Underline, &str)] = &[
            (Underline::Single, "4"),
            (Underline::Double, "21"),
            (Underline::Curly, "4:3"),
            (Underline::Dotted, "4:4"),
            (Underline::Dashed, "4:5"),
        ];
        for (u, code) in cases {
            let s = Style {
                underline: *u,
                ..Style::default()
            };
            assert!(s.string().contains(code), "{u:?} -> {code}");
        }
        // Colors flow through.
        let s = Style {
            fg: Some(Color::Basic(1)),
            bg: Some(Color::Basic(2)),
            underline_color: Some(Color::Basic(3)),
            ..Style::default()
        };
        let out = s.string();
        assert!(out.contains("31"));
        assert!(out.contains("42"));
        assert!(out.contains("53"));
    }
}
