//! Cleanroom Rust port of upstream Go source file: `doc.go` / `uv.go`
//! Upstream Target Tag / Version: `v0.0.0-20260703014108-f5a850f9c2b7`
//!
//! <public-docs>
//! A high-performance terminal rendering library for Rust, ported from
//! Charmbracelet's Ultraviolet. Provides cell buffers, screen rendering,
//! terminal modes, and the primitives used by Bubble Tea's terminal
//! renderer.
//! </public-docs>

pub mod buffer;
pub mod casso;
pub mod cell;
pub mod console;
pub mod layout;
pub mod lru;
pub mod poll;
pub mod screen;
pub mod screen_context;
pub mod style;
pub mod window;

pub use buffer::{
    new_buffer, new_render_buffer, new_screen_buffer, trim_space, Buffer, Line, Lines,
    RenderBuffer, Screen, ScreenBuffer,
};
pub use cell::{empty_cell, new_link, Cell, Link};
pub use console::{Console, ConsoleError, FdFile, File, RawState, Winsize};
pub use layout::{
    horizontal, new as new_layout, pad, vertical, Constraint, Direction, Flex, Layout, Padding,
    Splitted,
};
pub use poll::{new_fallback_reader, new_poll_reader, PollError, PollReader};
pub use screen::{clear, clear_area, clone_area, fill, fill_area, rect, Rectangle};
pub use screen_context::{new_context, new_context_with_width_method, Context};
pub use style::{style_diff, Attr, Style};
pub use window::{new_window, pos, Window};

use std::io::{self, Write};

/// ErrNotTerminal is an error that indicates that the file is not a terminal.
pub fn err_not_terminal() -> io::Error {
    io::Error::new(io::ErrorKind::NotConnected, "not a terminal")
}

/// ErrPlatformNotSupported is an error that indicates that the platform is
/// not supported.
pub fn err_platform_not_supported() -> io::Error {
    io::Error::new(io::ErrorKind::Unsupported, "platform not supported")
}

/// Drawable represents a drawable component on a [Screen].
pub trait Drawable {
    /// Draw renders the component on the screen for the given area.
    fn draw(&mut self, scr: &mut dyn Screen, area: Rectangle);
}

/// DrawableFunc is a function that implements the [Drawable] interface.
pub struct DrawableFunc<'a>(pub Box<dyn FnMut(&mut dyn Screen, Rectangle) + 'a>);

impl Drawable for DrawableFunc<'_> {
    fn draw(&mut self, scr: &mut dyn Screen, area: Rectangle) {
        (self.0)(scr, area)
    }
}

/// WidthMethod determines how many columns a grapheme occupies on the screen.
pub trait WidthMethod {
    /// StringWidth returns the width of the string in columns.
    fn string_width(&self, s: &str) -> usize;
}

impl WidthMethod for charming_x_ansi::method::WidthMethod {
    fn string_width(&self, s: &str) -> usize {
        charming_x_ansi::method::WidthMethod::string_width(self, s)
    }
}

/// CursorShape represents the shape of the terminal cursor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CursorShape {
    /// CursorBlock is a block cursor.
    #[default]
    CursorBlock = 0,
    /// CursorUnderline is an underline cursor.
    CursorUnderline,
    /// CursorBar is a bar cursor.
    CursorBar,
}

impl CursorShape {
    /// Encode returns the encoded value for the cursor shape.
    pub fn encode(&self, blink: bool) -> i32 {
        // We're using the ANSI escape sequence values for cursor styles. We
        // need to map both [style] and [steady] to the correct value.
        let s = (*self as i32 * 2) + 1;
        let s = if !blink { s + 1 } else { s };
        s
    }
}

/// Position represents a point on the terminal screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Position {
    /// The x coordinate.
    pub x: usize,
    /// The y coordinate.
    pub y: usize,
}

/// Cursor represents a cursor on the terminal screen.
#[derive(Debug, Clone, Default)]
pub struct Cursor {
    /// Position is a [Position] that determines the cursor's position on the
    /// screen relative to the top left corner of the frame.
    pub position: Position,

    /// Color is a color that determines the cursor's color.
    pub color: Option<charming_x_ansi::color::RGBColor>,

    /// Shape is a [CursorShape] that determines the cursor's shape.
    pub shape: CursorShape,

    /// Blink is a boolean that determines whether the cursor should blink.
    pub blink: bool,

    /// Hidden is a boolean that determines whether the cursor is hidden. You
    /// can use this if you want to hide the cursor but still want to change
    /// its position.
    pub hidden: bool,
}

/// NewCursor returns a new cursor with the default settings and the given
/// position.
pub fn new_cursor(x: usize, y: usize) -> Cursor {
    Cursor {
        position: Position { x, y },
        color: None,
        shape: CursorShape::CursorBlock,
        blink: true,
        hidden: false,
    }
}

/// ProgressBarState represents the state of the progress bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressBarState {
    /// ProgressBarNone state.
    ProgressBarNone,
    /// ProgressBarDefault state.
    ProgressBarDefault,
    /// ProgressBarError state.
    ProgressBarError,
    /// ProgressBarIndeterminate state.
    ProgressBarIndeterminate,
    /// ProgressBarWarning state.
    ProgressBarWarning,
}

impl ProgressBarState {
    /// String returns a human-readable value for the given [ProgressBarState].
    pub fn string(&self) -> &'static str {
        match self {
            ProgressBarState::ProgressBarNone => "None",
            ProgressBarState::ProgressBarDefault => "Default",
            ProgressBarState::ProgressBarError => "Error",
            ProgressBarState::ProgressBarIndeterminate => "Indeterminate",
            ProgressBarState::ProgressBarWarning => "Warning",
        }
    }
}

/// ProgressBar represents the terminal progress bar.
///
/// Support depends on the terminal.
///
/// See <https://learn.microsoft.com/en-us/windows/terminal/tutorials/progress-bar-sequences>
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProgressBar {
    /// State is the current state of the progress bar.
    pub state: ProgressBarState,
    /// Value is the current value of the progress bar. It should be between
    /// 0 and 100.
    pub value: i32,
}

/// NewProgressBar returns a new progress bar with the given state and value.
/// The value is ignored if the state is [ProgressBarState::ProgressBarNone] or
/// [ProgressBarState::ProgressBarIndeterminate].
pub fn new_progress_bar(state: ProgressBarState, value: i32) -> ProgressBar {
    ProgressBar {
        state,
        value: clamp(value, 0, 100),
    }
}

/// MouseMode represents the mouse tracking mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseMode {
    /// MouseModeNone disables mouse tracking.
    MouseModeNone,
    /// MouseModePress is press only (DEC mode 9). Reports button press
    /// events.
    MouseModePress,
    /// MouseModeClick is click tracking (DEC mode 1000). Reports button
    /// press and release.
    MouseModeClick,
    /// MouseModeDrag is drag tracking (DEC mode 1002). Reports press,
    /// release, and drag.
    MouseModeDrag,
    /// MouseModeMotion is motion tracking (DEC mode 1003). Reports all mouse
    /// events including motion.
    MouseModeMotion,
}

/// MouseEncoding represents the mouse encoding mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseEncoding {
    /// MouseEncodingLegacy is the legacy X10-compatible encoding. Coordinates
    /// limited to 223.
    MouseEncodingLegacy,
    /// MouseEncodingSGR is the SGR encoding (DEC mode 1006). No coordinate
    /// limit, distinguishes press/release.
    MouseEncodingSGR,
    /// MouseEncodingSGRPixel is the SGR-pixel encoding (DEC mode 1016).
    /// Reports pixel coordinates.
    MouseEncodingSGRPixel,
}

/// KeyboardEnhancements defines different keyboard enhancement features that
/// can be requested from the terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct KeyboardEnhancements {
    /// DisambiguateEscapeCodes requests the terminal to report ambiguous keys
    /// such as Ctrl+i and Tab, and Ctrl+m and Enter, and others as distinct
    /// key code sequences.
    pub disambiguate_escape_codes: bool,
    /// ReportEventTypes requests the terminal to report key repeat and
    /// release events.
    pub report_event_types: bool,
    /// ReportAlternateKeys requests the terminal to report alternate key
    /// values in addition to the main ones.
    pub report_alternate_keys: bool,
    /// ReportAllKeysAsEscapeCodes requests the terminal to report all key
    /// events, including plain text keys, as escape codes.
    pub report_all_keys_as_escape_codes: bool,
    /// ReportAssociatedText requests the terminal to report the text
    /// associated with key events.
    pub report_associated_text: bool,
}

/// NewKeyboardEnhancements returns a new [KeyboardEnhancements] with the
/// given options as flags.
///
/// A zero, or negative, flags value is treated as no enhancements.
pub fn new_keyboard_enhancements(flags: i32) -> KeyboardEnhancements {
    if flags <= 0 {
        return KeyboardEnhancements::default();
    }
    let f = flags as u8;
    KeyboardEnhancements {
        disambiguate_escape_codes: f & charming_x_ansi::kitty::KITTY_DISAMBIGUATE_ESCAPE_CODES != 0,
        report_event_types: f & charming_x_ansi::kitty::KITTY_REPORT_EVENT_TYPES != 0,
        report_alternate_keys: f & charming_x_ansi::kitty::KITTY_REPORT_ALTERNATE_KEYS != 0,
        report_all_keys_as_escape_codes: f
            & charming_x_ansi::kitty::KITTY_REPORT_ALL_KEYS_AS_ESCAPE_CODES
            != 0,
        report_associated_text: f & charming_x_ansi::kitty::KITTY_REPORT_ASSOCIATED_KEYS != 0,
    }
}

impl KeyboardEnhancements {
    /// Flags returns the keyboard enhancements as bits that can be used to
    /// set the appropriate terminal modes.
    pub fn flags(&self) -> i32 {
        let mut bits: u8 = 0;
        if self.disambiguate_escape_codes {
            bits |= charming_x_ansi::kitty::KITTY_DISAMBIGUATE_ESCAPE_CODES;
        }
        if self.report_event_types {
            bits |= charming_x_ansi::kitty::KITTY_REPORT_EVENT_TYPES;
        }
        if self.report_alternate_keys {
            bits |= charming_x_ansi::kitty::KITTY_REPORT_ALTERNATE_KEYS;
        }
        if self.report_all_keys_as_escape_codes {
            bits |= charming_x_ansi::kitty::KITTY_REPORT_ALL_KEYS_AS_ESCAPE_CODES;
        }
        if self.report_associated_text {
            bits |= charming_x_ansi::kitty::KITTY_REPORT_ASSOCIATED_KEYS;
        }
        bits as i32
    }
}

/// EncodeBackgroundColor encodes the background color to the given writer.
/// Use None to reset the background color to the default.
pub fn encode_background_color(
    w: &mut dyn Write,
    c: Option<&charming_x_ansi::color::RGBColor>,
) -> io::Result<()> {
    let seq = match c {
        None => charming_x_ansi::background::RESET_BACKGROUND_COLOR.to_string(),
        Some(col) => charming_x_ansi::background::set_background_color(&col.hex()),
    };
    w.write_all(seq.as_bytes())?;
    Ok(())
}

/// EncodeForegroundColor encodes the foreground color to the given writer.
/// Use None to reset the foreground color to the default.
pub fn encode_foreground_color(
    w: &mut dyn Write,
    c: Option<&charming_x_ansi::color::RGBColor>,
) -> io::Result<()> {
    let seq = match c {
        None => charming_x_ansi::background::RESET_FOREGROUND_COLOR.to_string(),
        Some(col) => charming_x_ansi::background::set_foreground_color(&col.hex()),
    };
    w.write_all(seq.as_bytes())?;
    Ok(())
}

/// EncodeCursorColor encodes the cursor color to the given writer. Use None
/// to reset the cursor color to the default.
pub fn encode_cursor_color(
    w: &mut dyn Write,
    c: Option<&charming_x_ansi::color::RGBColor>,
) -> io::Result<()> {
    let seq = match c {
        None => charming_x_ansi::background::RESET_CURSOR_COLOR.to_string(),
        Some(col) => charming_x_ansi::background::set_cursor_color(&col.hex()),
    };
    w.write_all(seq.as_bytes())?;
    Ok(())
}

/// EncodeCursorStyle encodes the cursor style to the given writer.
pub fn encode_cursor_style(w: &mut dyn Write, shape: CursorShape, blink: bool) -> io::Result<()> {
    let seq = charming_x_ansi::cursor::set_cursor_style(shape.encode(blink));
    w.write_all(seq.as_bytes())?;
    Ok(())
}

/// EncodeBracketedPaste encodes the bracketed paste mode to the given
/// writer.
pub fn encode_bracketed_paste(w: &mut dyn Write, enable: bool) -> io::Result<()> {
    let seq = if enable {
        charming_x_ansi::mode::SET_MODE_BRACKETED_PASTE
    } else {
        charming_x_ansi::mode::RESET_MODE_BRACKETED_PASTE
    };
    w.write_all(seq.as_bytes())?;
    Ok(())
}

/// EncodeMouseMode encodes the mouse tracking mode to the given writer.
pub fn encode_mouse_mode(w: &mut dyn Write, mode: MouseMode) -> io::Result<()> {
    let seq = match mode {
        MouseMode::MouseModeNone => {
            charming_x_ansi::mode::RESET_MODE_MOUSE_X10.to_owned()
                + charming_x_ansi::mode::RESET_MODE_MOUSE_NORMAL
                + charming_x_ansi::mode::RESET_MODE_MOUSE_BUTTON_EVENT
                + charming_x_ansi::mode::RESET_MODE_MOUSE_ANY_EVENT
        }
        MouseMode::MouseModePress => charming_x_ansi::mode::SET_MODE_MOUSE_X10.to_string(),
        MouseMode::MouseModeClick => charming_x_ansi::mode::SET_MODE_MOUSE_NORMAL.to_string(),
        MouseMode::MouseModeDrag => {
            charming_x_ansi::mode::SET_MODE_MOUSE_BUTTON_EVENT.to_string()
        }
        MouseMode::MouseModeMotion => {
            charming_x_ansi::mode::SET_MODE_MOUSE_ANY_EVENT.to_string()
        }
    };
    w.write_all(seq.as_bytes())?;
    Ok(())
}

/// EncodeMouseEncoding encodes the mouse encoding mode to the given writer.
/// When enc is [MouseEncoding::MouseEncodingLegacy], all extended encodings
/// are reset.
pub fn encode_mouse_encoding(w: &mut dyn Write, enc: MouseEncoding) -> io::Result<()> {
    let seq = match enc {
        MouseEncoding::MouseEncodingLegacy => {
            charming_x_ansi::mode::RESET_MODE_MOUSE_EXT_SGR.to_owned()
                + charming_x_ansi::mode::RESET_MODE_MOUSE_EXT_URXVT
                + charming_x_ansi::mode::RESET_MODE_MOUSE_EXT_SGR_PIXEL
        }
        MouseEncoding::MouseEncodingSGR => charming_x_ansi::mode::SET_MODE_MOUSE_EXT_SGR.to_string(),
        MouseEncoding::MouseEncodingSGRPixel => {
            charming_x_ansi::mode::SET_MODE_MOUSE_EXT_SGR_PIXEL.to_string()
        }
    };
    w.write_all(seq.as_bytes())?;
    Ok(())
}

/// EncodeProgressBar encodes the progress bar to the given writer.
pub fn encode_progress_bar(w: &mut dyn Write, pb: Option<&ProgressBar>) -> io::Result<()> {
    let pb = match pb {
        None => ProgressBar {
            state: ProgressBarState::ProgressBarNone,
            value: 0,
        },
        Some(pb) => *pb,
    };

    let seq = {
        let percent = clamp(pb.value, 0, 100);
        match pb.state {
            ProgressBarState::ProgressBarNone => {
                charming_x_ansi::progress::RESET_PROGRESS_BAR.to_string()
            }
            ProgressBarState::ProgressBarDefault => {
                charming_x_ansi::progress::set_progress_bar(percent)
            }
            ProgressBarState::ProgressBarError => {
                charming_x_ansi::progress::set_error_progress_bar(percent)
            }
            ProgressBarState::ProgressBarIndeterminate => {
                charming_x_ansi::progress::SET_INDETERMINATE_PROGRESS_BAR.to_string()
            }
            ProgressBarState::ProgressBarWarning => {
                charming_x_ansi::progress::set_warning_progress_bar(percent)
            }
        }
    };
    w.write_all(seq.as_bytes())?;
    Ok(())
}

fn clamp(v: i32, min: i32, max: i32) -> i32 {
    if v < min {
        min
    } else if v > max {
        max
    } else {
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cursor_shape_encode() {
        assert_eq!(CursorShape::CursorBlock.encode(true), 1);
        assert_eq!(CursorShape::CursorBlock.encode(false), 2);
        assert_eq!(CursorShape::CursorUnderline.encode(true), 3);
        assert_eq!(CursorShape::CursorBar.encode(true), 5);
    }

    #[test]
    fn test_keyboard_enhancements_flags() {
        let ke = new_keyboard_enhancements(1);
        assert!(ke.disambiguate_escape_codes);
        assert_eq!(ke.flags(), 1);
        let ke = new_keyboard_enhancements(31);
        assert_eq!(ke.flags(), 31);
        let ke = new_keyboard_enhancements(0);
        assert_eq!(ke.flags(), 0);
    }

    #[test]
    fn test_new_progress_bar_clamps() {
        let pb = new_progress_bar(ProgressBarState::ProgressBarDefault, 150);
        assert_eq!(pb.value, 100);
        let pb = new_progress_bar(ProgressBarState::ProgressBarDefault, -5);
        assert_eq!(pb.value, 0);
    }

    #[test]
    fn test_encode_sequences() {
        let mut out = Vec::new();
        encode_bracketed_paste(&mut out, true).unwrap();
        assert_eq!(out, b"\x1b[?2004h");
        encode_bracketed_paste(&mut out, false).unwrap();
        assert_eq!(out, b"\x1b[?2004h\x1b[?2004l");
        out.clear();
        encode_mouse_mode(&mut out, MouseMode::MouseModeNone).unwrap();
        assert_eq!(out, b"\x1b[?9l\x1b[?1000l\x1b[?1002l\x1b[?1003l");
        out.clear();
        encode_mouse_encoding(&mut out, MouseEncoding::MouseEncodingSGR).unwrap();
        assert_eq!(out, b"\x1b[?1006h");
        out.clear();
        encode_progress_bar(&mut out, None).unwrap();
        assert_eq!(out, b"\x1b]9;4;0\x07");
        out.clear();
        encode_cursor_style(&mut out, CursorShape::CursorBlock, true).unwrap();
        assert_eq!(out, b"\x1b[1 q");
        out.clear();
        encode_foreground_color(&mut out, None).unwrap();
        assert_eq!(out, b"\x1b]110\x07");
        let rgb = charming_x_ansi::color::RGBColor { r: 255, g: 0, b: 0 };
        encode_foreground_color(&mut out, Some(&rgb)).unwrap();
        assert_eq!(out, b"\x1b]110\x07\x1b]10;#ff0000\x07");
    }

    #[test]
    fn test_drawable_func() {
        let mut f = DrawableFunc(Box::new(|_scr: &mut dyn Screen, _area: Rectangle| {}));
        let mut b = new_buffer(2, 2);
        f.draw(&mut b, Rectangle { min: (0, 0), max: (2, 2) });
    }
}
