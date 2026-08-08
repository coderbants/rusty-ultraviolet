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
pub mod decoder;
pub mod environ;
pub mod event;
pub mod key;
pub mod layout;
pub mod logger;
pub mod lru;
pub mod mouse;
pub mod poll;
pub mod screen;
pub mod screen_context;
pub mod style;
pub mod tabstop;
pub mod utils;
pub mod terminal_renderer;
pub mod terminal_screen;
pub mod window;

pub use buffer::{
    new_buffer, new_render_buffer, new_screen_buffer, trim_space, Buffer, Line, Lines,
    RenderBuffer, Screen, ScreenBuffer,
};
pub use cell::{empty_cell, new_link, Cell, Link};
pub use environ::Environ;
pub use event::{
    BackgroundColorEvent, BlurEvent, CapabilityEvent, CellSizeEvent, ClipboardEvent,
    CursorColorEvent, CursorPositionEvent, DarkColorSchemeEvent, FocusEvent,
    ForegroundColorEvent, KeyboardEnhancementsEvent, KeyPressEvent, KeyReleaseEvent,
    KittyGraphicsEvent, LightColorSchemeEvent, ModeReportEvent, ModifyOtherKeysEvent,
    MouseClickEvent, MouseMotionEvent, MouseReleaseEvent, MouseWheelEvent, MultiEvent,
    PasteEndEvent, PasteEvent, PasteStartEvent, PixelSizeEvent, PrimaryDeviceAttributesEvent,
    SecondaryDeviceAttributesEvent, Size, TerminalVersionEvent, TertiaryDeviceAttributesEvent,
    UnknownApcEvent, UnknownCsiEvent, UnknownDcsEvent, UnknownEvent, UnknownOscEvent,
    UnknownPmEvent, UnknownSosEvent, UnknownSs3Event, WindowOpEvent, WindowSizeEvent,
    PRIMARY_CLIPBOARD, SYSTEM_CLIPBOARD,
};
pub use key::{
    Key, KeyMod, KEY_BACKSPACE, KEY_BEGIN, KEY_CAPS_LOCK,
    KEY_DELETE, KEY_DOWN, KEY_END, KEY_ENTER, KEY_ESCAPE, KEY_EXTENDED, KEY_F1, KEY_F10,
    KEY_F11, KEY_F12, KEY_F13, KEY_F14, KEY_F15, KEY_F16, KEY_F17, KEY_F18, KEY_F19, KEY_F2,
    KEY_F20, KEY_F21, KEY_F22, KEY_F23, KEY_F24, KEY_F25, KEY_F26, KEY_F27, KEY_F28, KEY_F29,
    KEY_F3, KEY_F30, KEY_F31, KEY_F32, KEY_F33, KEY_F34, KEY_F35, KEY_F36, KEY_F37, KEY_F38,
    KEY_F39, KEY_F4, KEY_F40, KEY_F41, KEY_F42, KEY_F43, KEY_F44, KEY_F45, KEY_F46, KEY_F47,
    KEY_F48, KEY_F49, KEY_F5, KEY_F50, KEY_F51, KEY_F52, KEY_F53, KEY_F54, KEY_F55, KEY_F56,
    KEY_F57, KEY_F58, KEY_F59, KEY_F6, KEY_F60, KEY_F61, KEY_F62, KEY_F63, KEY_F7, KEY_F8,
    KEY_F9, KEY_FIND, KEY_HOME, KEY_INSERT, KEY_ISO_LEVEL3_SHIFT, KEY_ISO_LEVEL5_SHIFT,
    KEY_LEFT, KEY_LEFT_ALT, KEY_LEFT_CTRL, KEY_LEFT_HYPER, KEY_LEFT_META, KEY_LEFT_SHIFT,
    KEY_LEFT_SUPER, KEY_LOWER_VOL, KEY_MEDIA_FAST_FORWARD, KEY_MEDIA_NEXT, KEY_MEDIA_PAUSE,
    KEY_MEDIA_PLAY, KEY_MEDIA_PLAY_PAUSE, KEY_MEDIA_PREV, KEY_MEDIA_RECORD, KEY_MEDIA_REVERSE,
    KEY_MEDIA_STOP, KEY_MENU, KEY_MUTE, KEY_NUM_LOCK, KEY_PAUSE, KEY_PG_DOWN, KEY_PG_UP,
    KEY_PRINT_SCREEN, KEY_RAISE_VOL, KEY_RETURN, KEY_RIGHT, KEY_RIGHT_ALT, KEY_RIGHT_CTRL,
    KEY_RIGHT_HYPER, KEY_RIGHT_META, KEY_RIGHT_SHIFT, KEY_RIGHT_SUPER, KEY_SCROLL_LOCK,
    KEY_SELECT, KEY_SPACE, KEY_TAB, KEY_UP, MOD_ALT, MOD_CAPS_LOCK, MOD_CTRL, MOD_HYPER,
    MOD_META, MOD_NUM_LOCK, MOD_SCROLL_LOCK, MOD_SHIFT, MOD_SUPER,
};
pub use logger::{FileLogger, Logger};
pub use console::Winsize;
pub use mouse::{
    mouse_pixel_to_cell, Mouse, MouseEncoding, MouseMode, MOUSE_BACKWARD, MOUSE_BUTTON_10,
    MOUSE_BUTTON_11, MOUSE_FORWARD, MOUSE_LEFT, MOUSE_MIDDLE, MOUSE_NONE, MOUSE_RIGHT,
    MOUSE_WHEEL_DOWN, MOUSE_WHEEL_LEFT, MOUSE_WHEEL_RIGHT, MOUSE_WHEEL_UP,
};
pub use console::{Console, ConsoleError, FdFile, File, RawState};
pub use decoder::{DecodedEvent, EventDecoder, LegacyKeyEncoding};
pub use layout::{
    horizontal, new as new_layout, pad, vertical, Constraint, Direction, Flex, Layout, Padding,
    Splitted,
};
pub use poll::{new_fallback_reader, new_poll_reader, PollError, PollReader};
pub use screen::{clear, clear_area, clone_area, fill, fill_area, rect, Rectangle};
pub use screen_context::{new_context, new_context_with_width_method, Context};
pub use style::{style_diff, Attr, Style};
pub use terminal_screen::{new_terminal_screen, ColorProfile, TerminalScreen};
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
