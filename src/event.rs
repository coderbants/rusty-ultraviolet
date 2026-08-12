//! Cleanroom Rust port of upstream Go source file: `event.go`
//! Upstream Target Tag / Version: `v0.0.0-20260703014108-f5a850f9c2b7`
//!
//! <public-docs>
//! Input event types: key press/release, mouse, paste, focus, color, size,
//! and terminal response events produced by the decoder.
//! </public-docs>

use crate::key::Key;
use crate::mouse::Mouse;
use crate::screen::Rectangle;
use charming_x_ansi::kitty::{
    KITTY_DISAMBIGUATE_ESCAPE_CODES, KITTY_REPORT_ALL_KEYS_AS_ESCAPE_CODES,
    KITTY_REPORT_ALTERNATE_KEYS, KITTY_REPORT_EVENT_TYPES,
};
use charming_x_ansi::mode::ModeSetting;

/// Event represents an input event that can be received from an input source.
pub trait Event: std::fmt::Debug {}

/// UnknownEvent represents an unknown event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownEvent(pub String);

/// UnknownCsiEvent represents an unknown CSI event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownCsiEvent(pub String);

/// UnknownSs3Event represents an unknown SS3 event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownSs3Event(pub String);

/// UnknownOscEvent represents an unknown OSC event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownOscEvent(pub String);

/// UnknownDcsEvent represents an unknown DCS event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownDcsEvent(pub String);

/// UnknownSosEvent represents an unknown SOS event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownSosEvent(pub String);

/// UnknownPmEvent represents an unknown PM event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownPmEvent(pub String);

/// UnknownApcEvent represents an unknown APC event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownApcEvent(pub String);

/// MultiEvent represents multiple messages event.
#[derive(Debug, Default)]
pub struct MultiEvent(pub Vec<Box<dyn Event>>);

/// Size represents the size of the terminal window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Size {
    /// The width of the window.
    pub width: usize,
    /// The height of the window.
    pub height: usize,
}

impl Size {
    /// Bounds returns the bounds corresponding to the size.
    pub fn bounds(&self) -> Rectangle {
        Rectangle {
            min: (0, 0),
            max: (self.width, self.height),
        }
    }
}

/// WindowSizeEvent represents the window size in cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WindowSizeEvent(pub Size);

impl WindowSizeEvent {
    /// Bounds returns the bounds corresponding to the size.
    pub fn bounds(&self) -> Rectangle {
        self.0.bounds()
    }
}

/// PixelSizeEvent represents the window size in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PixelSizeEvent(pub Size);

impl PixelSizeEvent {
    /// Bounds returns the bounds corresponding to the size.
    pub fn bounds(&self) -> Rectangle {
        self.0.bounds()
    }
}

/// CellSizeEvent represents the cell size in pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CellSizeEvent(pub Size);

impl CellSizeEvent {
    /// Bounds returns the bounds corresponding to the size.
    pub fn bounds(&self) -> Rectangle {
        self.0.bounds()
    }
}

/// KeyEvent represents a key event. This can be either a key press or a key
/// release event.
pub trait KeyEvent {
    /// Key returns the underlying key event.
    fn key(&self) -> &Key;
}

/// KeyPressEvent represents a key press event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyPressEvent(pub Key);

impl KeyEvent for KeyPressEvent {
    fn key(&self) -> &Key {
        &self.0
    }
}

impl KeyPressEvent {
    /// MatchString returns true if the key matches one of the given strings.
    pub fn match_string(&self, strings: &[&str]) -> bool {
        self.0.match_string(strings)
    }

    /// String returns the textual representation of the key event.
    pub fn string(&self) -> String {
        self.0.string()
    }

    /// Keystroke returns the keystroke representation of the key.
    pub fn keystroke(&self) -> String {
        self.0.keystroke()
    }
}

/// KeyReleaseEvent represents a key release event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyReleaseEvent(pub Key);

impl KeyEvent for KeyReleaseEvent {
    fn key(&self) -> &Key {
        &self.0
    }
}

impl KeyReleaseEvent {
    /// MatchString returns true if the key matches one of the given strings.
    pub fn match_string(&self, strings: &[&str]) -> bool {
        self.0.match_string(strings)
    }

    /// String returns the textual representation of the key event.
    pub fn string(&self) -> String {
        self.0.string()
    }

    /// Keystroke returns the keystroke representation of the key.
    pub fn keystroke(&self) -> String {
        self.0.keystroke()
    }
}

/// MouseEvent represents a mouse message.
pub trait MouseEvent {
    /// Mouse returns the underlying mouse event.
    fn mouse(&self) -> &Mouse;
}

/// MouseClickEvent represents a mouse button click event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MouseClickEvent(pub Mouse);

impl MouseEvent for MouseClickEvent {
    fn mouse(&self) -> &Mouse {
        &self.0
    }
}

/// MouseReleaseEvent represents a mouse button release event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MouseReleaseEvent(pub Mouse);

impl MouseEvent for MouseReleaseEvent {
    fn mouse(&self) -> &Mouse {
        &self.0
    }
}

/// MouseWheelEvent represents a mouse wheel message event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MouseWheelEvent(pub Mouse);

impl MouseEvent for MouseWheelEvent {
    fn mouse(&self) -> &Mouse {
        &self.0
    }
}

/// MouseMotionEvent represents a mouse motion event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MouseMotionEvent(pub Mouse);

impl MouseEvent for MouseMotionEvent {
    fn mouse(&self) -> &Mouse {
        &self.0
    }
}

/// CursorPositionEvent represents a cursor position event. Where X is the
/// zero-based column and Y is the zero-based row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CursorPositionEvent {
    /// The zero-based column.
    pub x: usize,
    /// The zero-based row.
    pub y: usize,
}

/// FocusEvent represents a terminal focus event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FocusEvent;

/// BlurEvent represents a terminal blur event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BlurEvent;

/// DarkColorSchemeEvent is sent when the operating system is using a dark
/// color scheme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DarkColorSchemeEvent;

/// LightColorSchemeEvent is sent when the operating system is using a light
/// color scheme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LightColorSchemeEvent;

/// PasteEvent is a message that is emitted when a terminal receives pasted
/// text using bracketed-paste.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PasteEvent {
    /// Content is the pasted text content.
    pub content: String,
}

/// PasteStartEvent is a message that is emitted when the terminal starts the
/// bracketed-paste text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PasteStartEvent;

/// PasteEndEvent is a message that is emitted when the terminal ends the
/// bracketed-paste text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PasteEndEvent;

/// TerminalVersionEvent is a message that represents the terminal version.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalVersionEvent {
    /// The terminal version name.
    pub name: String,
}

/// ModifyOtherKeysEvent represents a modifyOtherKeys event.
///
/// - `0`: disable
/// - `1`: enable mode 1
/// - `2`: enable mode 2
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModifyOtherKeysEvent {
    /// The mode value.
    pub mode: i32,
}

/// KittyGraphicsEvent represents a Kitty Graphics response event.
///
/// NOTE: the upstream `kitty.Options` type lives in the deferred
/// `ansi/kitty` subpackage; the options are kept as an opaque byte payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KittyGraphicsEvent {
    /// The kitty graphics options payload.
    pub options: Vec<u8>,
    /// The payload.
    pub payload: Vec<u8>,
}

/// KeyboardEnhancementsEvent represents a keyboard enhancements report event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyboardEnhancementsEvent {
    /// Flags are the Kitty Keyboard Enhancement flags.
    pub flags: i32,
}

impl KeyboardEnhancementsEvent {
    /// Contains reports whether m contains the given enhancements.
    pub fn contains(&self, enhancements: i32) -> bool {
        self.flags & enhancements == enhancements
    }

    /// SupportsKeyDisambiguation returns whether the terminal supports
    /// reporting disambiguated keys as escape codes.
    pub fn supports_key_disambiguation(&self) -> bool {
        self.flags & KITTY_DISAMBIGUATE_ESCAPE_CODES as i32 != 0
    }

    /// SupportsKeyReleases returns whether the terminal supports key release
    /// events.
    pub fn supports_key_releases(&self) -> bool {
        self.flags & KITTY_REPORT_EVENT_TYPES as i32 != 0
    }

    /// SupportsUniformKeyLayout returns whether the terminal supports
    /// reporting key events as though they were on a PC-101 layout.
    pub fn supports_uniform_key_layout(&self) -> bool {
        self.supports_key_disambiguation()
            && self.flags & KITTY_REPORT_ALTERNATE_KEYS as i32 != 0
            && self.flags & KITTY_REPORT_ALL_KEYS_AS_ESCAPE_CODES as i32 != 0
    }
}

/// PrimaryDeviceAttributesEvent is an event that represents the terminal
/// primary device attributes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrimaryDeviceAttributesEvent(pub Vec<i32>);

/// SecondaryDeviceAttributesEvent is an event that represents the terminal
/// secondary device attributes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecondaryDeviceAttributesEvent(pub Vec<i32>);

/// TertiaryDeviceAttributesEvent is an event that represents the terminal
/// tertiary device attributes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TertiaryDeviceAttributesEvent(pub String);

/// ModeReportEvent is a message that represents a mode report event (DECRPM).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModeReportEvent {
    /// Mode is the mode number.
    pub mode: i32,
    /// Value is the mode value.
    pub value: ModeSetting,
}

/// ForegroundColorEvent represents a foreground color event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ForegroundColorEvent(pub Option<charming_x_ansi::color::RGBColor>);

impl ForegroundColorEvent {
    /// String returns the hex representation of the color.
    pub fn string(&self) -> String {
        color_to_hex(self.0)
    }

    /// IsDark returns whether the color is dark.
    pub fn is_dark(&self) -> bool {
        is_dark_color(self.0)
    }
}

/// BackgroundColorEvent represents a background color event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackgroundColorEvent(pub Option<charming_x_ansi::color::RGBColor>);

impl BackgroundColorEvent {
    /// String returns the hex representation of the color.
    pub fn string(&self) -> String {
        color_to_hex(self.0)
    }

    /// IsDark returns whether the color is dark.
    pub fn is_dark(&self) -> bool {
        is_dark_color(self.0)
    }
}

/// CursorColorEvent represents a cursor color change event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CursorColorEvent(pub Option<charming_x_ansi::color::RGBColor>);

impl CursorColorEvent {
    /// String returns the hex representation of the color.
    pub fn string(&self) -> String {
        color_to_hex(self.0)
    }

    /// IsDark returns whether the color is dark.
    pub fn is_dark(&self) -> bool {
        is_dark_color(self.0)
    }
}

/// WindowOpEvent is a window operation (XTWINOPS) report event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowOpEvent {
    /// The operation code.
    pub op: i32,
    /// The operation arguments.
    pub args: Vec<i32>,
}

/// CapabilityEvent represents a Termcap/Terminfo response event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityEvent {
    /// The capability content.
    pub content: String,
}

/// ClipboardSelection represents a clipboard selection.
pub type ClipboardSelection = u8;

/// Clipboard selections.
/// System clipboard selection ('c').
pub const SYSTEM_CLIPBOARD: ClipboardSelection = b'c';
/// Primary clipboard selection ('p').
pub const PRIMARY_CLIPBOARD: ClipboardSelection = b'p';

/// ClipboardEvent is a clipboard read message event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipboardEvent {
    /// The clipboard content.
    pub content: String,
    /// The clipboard selection.
    pub selection: ClipboardSelection,
}

/// colorToHex returns the hex representation of the color.
pub(crate) fn color_to_hex(c: Option<charming_x_ansi::color::RGBColor>) -> String {
    match c {
        None => String::new(),
        Some(c) => format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b),
    }
}

/// isDarkColor returns whether the given color is dark.
pub(crate) fn is_dark_color(c: Option<charming_x_ansi::color::RGBColor>) -> bool {
    let (r, g, b) = match c {
        None => return true,
        Some(c) => (c.r, c.g, c.b),
    };
    let (_, _, l) = rgb_to_hsl(r, g, b);
    l < 0.5
}

/// rgbToHSL converts an RGB triple to an HSL triple.
pub(crate) fn rgb_to_hsl(r: u8, g: u8, b: u8) -> (f64, f64, f64) {
    let rnot = f64::from(r) / 255.0;
    let gnot = f64::from(g) / 255.0;
    let bnot = f64::from(b) / 255.0;
    let (cmax, cmin) = get_max_min(rnot, gnot, bnot);
    let delta = cmax - cmin;
    // Lightness calculation:
    let l = (cmax + cmin) / 2.0;
    // Hue and Saturation Calculation:
    let (h, s) = if delta == 0.0 {
        (0.0, 0.0)
    } else {
        let h = if cmax == rnot {
            60.0 * (((gnot - bnot) / delta).rem_euclid(6.0))
        } else if cmax == gnot {
            60.0 * (((bnot - rnot) / delta) + 2.0)
        } else {
            60.0 * (((rnot - gnot) / delta) + 4.0)
        };
        let h = if h < 0.0 { h + 360.0 } else { h };
        let s = delta / (1.0 - (2.0 * l - 1.0).abs());
        (h, s)
    };

    (h, round(s), round(l))
}

fn get_max_min(a: f64, b: f64, c: f64) -> (f64, f64) {
    let (ma, mi) = if a > b { (a, b) } else { (b, a) };
    if c > ma {
        (c, mi)
    } else if c < mi {
        (ma, c)
    } else {
        (ma, mi)
    }
}

fn round(x: f64) -> f64 {
    (x * 1000.0).round() / 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_hex() {
        let c = charming_x_ansi::color::RGBColor { r: 255, g: 0, b: 0 };
        assert_eq!(color_to_hex(Some(c)), "#ff0000");
        assert_eq!(color_to_hex(None), "");
    }

    #[test]
    fn test_is_dark() {
        let black = charming_x_ansi::color::RGBColor { r: 0, g: 0, b: 0 };
        assert!(is_dark_color(Some(black)));
        assert!(is_dark_color(None));
        let white = charming_x_ansi::color::RGBColor {
            r: 255,
            g: 255,
            b: 255,
        };
        assert!(!is_dark_color(Some(white)));
    }

    #[test]
    fn test_size_bounds() {
        let s = Size {
            width: 10,
            height: 5,
        };
        assert_eq!(
            s.bounds(),
            Rectangle {
                min: (0, 0),
                max: (10, 5)
            }
        );
    }

    #[test]
    fn test_keyboard_enhancements() {
        let e = KeyboardEnhancementsEvent { flags: 1 };
        assert!(e.supports_key_disambiguation());
        assert!(!e.supports_key_releases());
        assert!(!e.supports_uniform_key_layout());
        let e = KeyboardEnhancementsEvent {
            flags: 1 | 2 | 4 | 8,
        };
        assert!(e.supports_uniform_key_layout());
    }
}
