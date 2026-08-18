//! Cleanroom Rust port of upstream Go source file: `decoder.go`
//! Upstream Target Tag / Version: `v0.0.0-20260703014108-f5a850f9c2b7`
//!
//! <public-docs>
//! The input event decoder: turns raw terminal byte streams into typed
//! events (key presses/releases, mouse, paste, focus, color, size, and
//! terminal responses), including the legacy and Kitty keyboard encodings.
//! </public-docs>

use crate::event::{KeyPressEvent, Size};
use crate::key::{
    Key, KeyMod, KEY_BACKSPACE, KEY_CAPS_LOCK, KEY_DELETE, KEY_DOWN, KEY_END, KEY_ENTER,
    KEY_ESCAPE, KEY_EXTENDED, KEY_F1, KEY_F11, KEY_F13, KEY_F15, KEY_F17, KEY_F21, KEY_F3, KEY_F6,
    KEY_FIND, KEY_HOME, KEY_INSERT, KEY_ISO_LEVEL3_SHIFT, KEY_ISO_LEVEL5_SHIFT, KEY_KP_0, KEY_KP_1,
    KEY_KP_2, KEY_KP_3, KEY_KP_4, KEY_KP_5, KEY_KP_6, KEY_KP_7, KEY_KP_8, KEY_KP_9, KEY_KP_BEGIN,
    KEY_KP_COMMA, KEY_KP_DECIMAL, KEY_KP_DELETE, KEY_KP_DIVIDE, KEY_KP_DOWN, KEY_KP_END,
    KEY_KP_ENTER, KEY_KP_EQUAL, KEY_KP_HOME, KEY_KP_INSERT, KEY_KP_LEFT, KEY_KP_MINUS,
    KEY_KP_MULTIPLY, KEY_KP_PG_DOWN, KEY_KP_PG_UP, KEY_KP_PLUS, KEY_KP_RIGHT, KEY_KP_SEP,
    KEY_KP_UP, KEY_LEFT, KEY_LEFT_ALT, KEY_LEFT_CTRL, KEY_LEFT_HYPER, KEY_LEFT_META,
    KEY_LEFT_SHIFT, KEY_LEFT_SUPER, KEY_LOWER_VOL, KEY_MEDIA_FAST_FORWARD, KEY_MEDIA_NEXT,
    KEY_MEDIA_PAUSE, KEY_MEDIA_PLAY, KEY_MEDIA_PLAY_PAUSE, KEY_MEDIA_PREV, KEY_MEDIA_RECORD,
    KEY_MEDIA_REVERSE, KEY_MEDIA_REWIND, KEY_MEDIA_STOP, KEY_MENU, KEY_MUTE, KEY_NUM_LOCK,
    KEY_PAUSE, KEY_PG_DOWN, KEY_PG_UP, KEY_PRINT_SCREEN, KEY_RAISE_VOL, KEY_RIGHT, KEY_RIGHT_ALT,
    KEY_RIGHT_CTRL, KEY_RIGHT_HYPER, KEY_RIGHT_META, KEY_RIGHT_SHIFT, KEY_RIGHT_SUPER,
    KEY_SCROLL_LOCK, KEY_SELECT, KEY_SPACE, KEY_TAB, KEY_UP, MOD_ALT, MOD_CAPS_LOCK, MOD_CTRL,
    MOD_HYPER, MOD_META, MOD_NUM_LOCK, MOD_SCROLL_LOCK, MOD_SHIFT, MOD_SUPER,
};
use crate::mouse::{
    Mouse, MOUSE_BACKWARD, MOUSE_LEFT, MOUSE_NONE, MOUSE_WHEEL_RIGHT, MOUSE_WHEEL_UP,
};
use rusty_x_ansi::mouse::MouseButton;
use rusty_x_ansi::parser::{HAS_MORE_FLAG, MISSING_PARAM};
use unicode_segmentation::UnicodeSegmentation;

// Flags to control the behavior of the parser.
const FLAG_CTRL_AT: u32 = 1 << 0;
const FLAG_CTRL_I: u32 = 1 << 1;
const FLAG_CTRL_M: u32 = 1 << 2;
const FLAG_CTRL_OPEN_BRACKET: u32 = 1 << 3;
const FLAG_BACKSPACE: u32 = 1 << 4;
const FLAG_FIND: u32 = 1 << 5;
const FLAG_SELECT: u32 = 1 << 6;
const FLAG_F_KEYS: u32 = 1 << 7;

/// A ST-terminated payload decoder callback (upstream `parseStTerminated`'s
/// `st *parser`).
type StTerminatedFn = Option<fn(&[u8]) -> Option<DecodedEvent>>;

/// LegacyKeyEncoding is a set of flags that control the behavior of legacy
/// terminal key encodings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct LegacyKeyEncoding(pub u32);

impl LegacyKeyEncoding {
    /// CtrlAt returns a [LegacyKeyEncoding] with whether NUL (0x00) is
    /// mapped to ctrl+at instead of ctrl+space.
    pub fn ctrl_at(&self, v: bool) -> LegacyKeyEncoding {
        if v {
            LegacyKeyEncoding(self.0 | FLAG_CTRL_AT)
        } else {
            LegacyKeyEncoding(self.0 & !FLAG_CTRL_AT)
        }
    }

    /// CtrlI returns a [LegacyKeyEncoding] with whether HT (0x09) is mapped
    /// to ctrl+i instead of the tab key.
    pub fn ctrl_i(&self, v: bool) -> LegacyKeyEncoding {
        if v {
            LegacyKeyEncoding(self.0 | FLAG_CTRL_I)
        } else {
            LegacyKeyEncoding(self.0 & !FLAG_CTRL_I)
        }
    }

    /// CtrlM returns a [LegacyKeyEncoding] with whether CR (0x0D) is mapped
    /// to ctrl+m instead of the enter key.
    pub fn ctrl_m(&self, v: bool) -> LegacyKeyEncoding {
        if v {
            LegacyKeyEncoding(self.0 | FLAG_CTRL_M)
        } else {
            LegacyKeyEncoding(self.0 & !FLAG_CTRL_M)
        }
    }

    /// CtrlOpenBracket returns a [LegacyKeyEncoding] with whether ESC (0x1B)
    /// is mapped to ctrl+[ instead of the escape key.
    pub fn ctrl_open_bracket(&self, v: bool) -> LegacyKeyEncoding {
        if v {
            LegacyKeyEncoding(self.0 | FLAG_CTRL_OPEN_BRACKET)
        } else {
            LegacyKeyEncoding(self.0 & !FLAG_CTRL_OPEN_BRACKET)
        }
    }

    /// Backspace returns a [LegacyKeyEncoding] with whether the backspace
    /// key is mapped to BS (0x08) instead of DEL (0x7F).
    pub fn backspace(&self, v: bool) -> LegacyKeyEncoding {
        if v {
            LegacyKeyEncoding(self.0 | FLAG_BACKSPACE)
        } else {
            LegacyKeyEncoding(self.0 & !FLAG_BACKSPACE)
        }
    }

    /// Find returns a [LegacyKeyEncoding] with whether the legacy find key is
    /// mapped to the home key.
    pub fn find(&self, v: bool) -> LegacyKeyEncoding {
        if v {
            LegacyKeyEncoding(self.0 | FLAG_FIND)
        } else {
            LegacyKeyEncoding(self.0 & !FLAG_FIND)
        }
    }

    /// Select returns a [LegacyKeyEncoding] with whether the legacy select
    /// key is mapped to the end key.
    pub fn select(&self, v: bool) -> LegacyKeyEncoding {
        if v {
            LegacyKeyEncoding(self.0 | FLAG_SELECT)
        } else {
            LegacyKeyEncoding(self.0 & !FLAG_SELECT)
        }
    }

    /// FKeys returns a [LegacyKeyEncoding] with whether high function keys
    /// are mapped to high function keys instead of Function+modifiers.
    pub fn f_keys(&self, v: bool) -> LegacyKeyEncoding {
        if v {
            LegacyKeyEncoding(self.0 | FLAG_F_KEYS)
        } else {
            LegacyKeyEncoding(self.0 & !FLAG_F_KEYS)
        }
    }
}

/// A decoded input event.
#[derive(Debug, Clone, PartialEq)]
pub enum DecodedEvent {
    /// UnknownEvent represents an unknown event.
    Unknown(String),
    /// UnknownCsiEvent represents an unknown CSI event.
    UnknownCsi(String),
    /// UnknownSs3Event represents an unknown SS3 event.
    UnknownSs3(String),
    /// UnknownOscEvent represents an unknown OSC event.
    UnknownOsc(String),
    /// UnknownDcsEvent represents an unknown DCS event.
    UnknownDcs(String),
    /// UnknownSosEvent represents an unknown SOS event.
    UnknownSos(String),
    /// UnknownPmEvent represents an unknown PM event.
    UnknownPm(String),
    /// UnknownApcEvent represents an unknown APC event.
    UnknownApc(String),
    /// MultiEvent represents multiple events.
    Multi(Vec<DecodedEvent>),
    /// WindowSizeEvent represents the window size in cells.
    WindowSize(Size),
    /// PixelSizeEvent represents the window size in pixels.
    PixelSize(Size),
    /// CellSizeEvent represents the cell size in pixels.
    CellSize(Size),
    /// KeyPressEvent represents a key press event.
    KeyPress(Key),
    /// KeyReleaseEvent represents a key release event.
    KeyRelease(Key),
    /// MouseClickEvent represents a mouse click event.
    MouseClick(Mouse),
    /// MouseReleaseEvent represents a mouse release event.
    MouseRelease(Mouse),
    /// MouseWheelEvent represents a mouse wheel event.
    MouseWheel(Mouse),
    /// MouseMotionEvent represents a mouse motion event.
    MouseMotion(Mouse),
    /// CursorPositionEvent represents a cursor position event.
    CursorPosition { x: i32, y: i32 },
    /// FocusEvent represents a terminal focus event.
    Focus,
    /// BlurEvent represents a terminal blur event.
    Blur,
    /// DarkColorSchemeEvent is sent when the OS uses a dark color scheme.
    DarkColorScheme,
    /// LightColorSchemeEvent is sent when the OS uses a light color scheme.
    LightColorScheme,
    /// PasteEvent is emitted when a terminal receives pasted text.
    Paste(String),
    /// PasteStartEvent is emitted when bracketed-paste starts.
    PasteStart,
    /// PasteEndEvent is emitted when bracketed-paste ends.
    PasteEnd,
    /// TerminalVersionEvent represents the terminal version.
    TerminalVersion(String),
    /// ModifyOtherKeysEvent represents a modifyOtherKeys event.
    ModifyOtherKeys(i32),
    /// KittyGraphicsEvent represents a Kitty Graphics response event.
    KittyGraphics { options: Vec<u8>, payload: Vec<u8> },
    /// KeyboardEnhancementsEvent represents a keyboard enhancements report.
    KeyboardEnhancements(i32),
    /// PrimaryDeviceAttributesEvent represents the terminal primary device
    /// attributes.
    PrimaryDeviceAttributes(Vec<i32>),
    /// SecondaryDeviceAttributesEvent represents the terminal secondary
    /// device attributes.
    SecondaryDeviceAttributes(Vec<i32>),
    /// TertiaryDeviceAttributesEvent represents the terminal tertiary device
    /// attributes.
    TertiaryDeviceAttributes(String),
    /// ModeReportEvent represents a mode report event (DECRPM).
    ModeReport { mode: i32, value: u8 },
    /// ForegroundColorEvent represents a foreground color event.
    ForegroundColor(Option<rusty_x_ansi::color::RGBColor>),
    /// BackgroundColorEvent represents a background color event.
    BackgroundColor(Option<rusty_x_ansi::color::RGBColor>),
    /// CursorColorEvent represents a cursor color event.
    CursorColor(Option<rusty_x_ansi::color::RGBColor>),
    /// WindowOpEvent is a window operation report event.
    WindowOp { op: i32, args: Vec<i32> },
    /// CapabilityEvent represents a Termcap/Terminfo response event.
    Capability(String),
    /// ClipboardEvent is a clipboard read message event.
    Clipboard { content: String, selection: u8 },
    /// ignoredEvent represents a sequence event that is ignored.
    Ignored(String),
}

/// EventDecoder decodes terminal input events from a byte buffer.
#[derive(Debug, Clone, Default)]
pub struct EventDecoder {
    /// Legacy is the legacy key encoding flags.
    pub legacy: LegacyKeyEncoding,
    /// UseTerminfo is a flag that controls whether to use the terminal type
    /// Terminfo database to map escape sequences to key events.
    pub use_terminfo: bool,

    /// The last control key state for the previous key event record.
    last_cks: u32,
}

impl EventDecoder {
    /// Decode finds the first recognized event sequence and returns it along
    /// with its length.
    ///
    /// It will return zero and None if no sequence is recognized or when the
    /// buffer is empty. If a sequence is not supported, an
    /// [DecodedEvent::Unknown] is returned.
    pub fn decode(&mut self, buf: &[u8]) -> (usize, Option<DecodedEvent>) {
        if buf.is_empty() {
            return (0, None);
        }

        match buf[0] {
            0x1B => {
                // ESC
                if buf.len() == 1 {
                    // Escape key
                    return (
                        1,
                        Some(DecodedEvent::KeyPress(Key {
                            code: KEY_ESCAPE,
                            ..Key::default()
                        })),
                    );
                }

                match buf[1] {
                    b'O' => self.parse_ss3(buf),
                    b'P' => self.parse_dcs(buf),
                    b'[' => self.parse_csi(buf),
                    b']' => self.parse_osc(buf),
                    b'_' => self.parse_apc(buf),
                    b'^' => self.parse_st_terminated(0x9E, b'^', None, buf),
                    b'X' => self.parse_st_terminated(0x98, b'X', None, buf),
                    _ => {
                        let (n, e) = self.decode(&buf[1..]);
                        if let Some(DecodedEvent::KeyPress(mut k)) = e {
                            k.text = String::new();
                            k.mod_.0 |= MOD_ALT.0;
                            return (n + 1, Some(DecodedEvent::KeyPress(k)));
                        }

                        // Not a key sequence, nor an alt modified key
                        // sequence. In that case, just report a single
                        // escape key.
                        (
                            1,
                            Some(DecodedEvent::KeyPress(Key {
                                code: KEY_ESCAPE,
                                ..Key::default()
                            })),
                        )
                    }
                }
            }
            0x8F => self.parse_ss3(buf),
            0x90 => self.parse_dcs(buf),
            0x9B => self.parse_csi(buf),
            0x9D => self.parse_osc(buf),
            0x9F => self.parse_apc(buf),
            0x9E => self.parse_st_terminated(0x9E, b'^', None, buf),
            0x98 => self.parse_st_terminated(0x98, b'X', None, buf),
            b => {
                if b <= 0x1F || b == 0x7F || b == 0x20 {
                    return (1, Some(self.parse_control(b)));
                } else if (0x80..=0x9F).contains(&b) {
                    // C1 control code
                    // UTF-8 never starts with a C1 control code
                    // Encode these as Ctrl+Alt+<code - 0x40>
                    let code = (b as u32) - 0x40;
                    return (
                        1,
                        Some(DecodedEvent::KeyPress(Key {
                            code,
                            mod_: KeyMod(MOD_CTRL.0 | MOD_ALT.0),
                            ..Key::default()
                        })),
                    );
                }
                self.parse_utf8(buf)
            }
        }
    }

    /// parseCsi parses a CSI sequence.
    fn parse_csi(&mut self, b: &[u8]) -> (usize, Option<DecodedEvent>) {
        if b.len() == 2 && b[0] == 0x1B {
            // short cut if this is an alt+[ key
            return (
                2,
                Some(DecodedEvent::KeyPress(Key {
                    code: b[1] as u32,
                    mod_: MOD_ALT,
                    ..Key::default()
                })),
            );
        }

        let mut cmd: i32 = 0;
        let mut params: [i32; 32] = [MISSING_PARAM; 32];
        let mut params_len = 0usize;

        let mut i = 0usize;
        if b[i] == 0x9B || b[i] == 0x1B {
            i += 1;
        }
        if i < b.len() && i > 0 && b[i - 1] == 0x1B && b[i] == b'[' {
            i += 1;
        }

        // Initial CSI byte
        if i < b.len() && b[i] >= b'<' && b[i] <= b'?' {
            cmd |= (b[i] as i32) << 8;
        }

        // Scan parameter bytes in the range 0x30-0x3F
        let mut j = 0usize;
        while i < b.len() && params_len < params.len() && b[i] >= 0x30 && b[i] <= 0x3F {
            if b[i] >= b'0' && b[i] <= b'9' {
                if params[params_len] == MISSING_PARAM {
                    params[params_len] = 0;
                }
                params[params_len] *= 10;
                params[params_len] += (b[i] - b'0') as i32;
            }
            if b[i] == b':' {
                params[params_len] |= HAS_MORE_FLAG;
            }
            if b[i] == b';' || b[i] == b':' {
                params_len += 1;
                if params_len < params.len() {
                    // Don't overflow the params slice
                    params[params_len] = MISSING_PARAM;
                }
            }
            i += 1;
            j += 1;
        }

        if j > 0 && params_len < params.len() {
            // has parameters
            params_len += 1;
        }

        // Scan intermediate bytes in the range 0x20-0x2F
        let mut intermed: u8 = 0;
        while i < b.len() && b[i] >= 0x20 && b[i] <= 0x2F {
            intermed = b[i];
            i += 1;
        }

        // Set the intermediate byte
        cmd |= (intermed as i32) << 16;

        // Scan final byte in the range 0x40-0x7E
        if i >= b.len() || b[i] < 0x40 || b[i] > 0x7E {
            // Special case for URxvt keys
            // CSI <number> $ is an invalid sequence, but URxvt uses it for
            // shift modified keys.
            if intermed == b'$' && b[i - 1] == b'$' {
                let mut buf2 = b[..i - 1].to_vec();
                buf2.push(b'~');
                let (n, ev) = self.parse_csi(&buf2);
                if let Some(DecodedEvent::KeyPress(mut k)) = ev {
                    k.mod_.0 |= MOD_SHIFT.0;
                    return (n, Some(DecodedEvent::KeyPress(k)));
                }
            }
            return (
                i,
                Some(DecodedEvent::UnknownCsi(
                    String::from_utf8_lossy(&b[..i]).into_owned(),
                )),
            );
        }

        // Add the final byte
        cmd |= b[i] as i32;
        i += 1;

        let pa = &params[..params_len];

        let packed = |prefix: u8, inter: u8, final_: u8| -> i32 {
            let mut c = final_ as i32;
            c |= (prefix as i32) << 8;
            c |= (inter as i32) << 16;
            c
        };

        match cmd {
            _ if cmd == packed(b'?', b'$', b'y') => {
                // Report Mode (DECRPM)
                let (mode, _, ok) = param_get(pa, 0, -1);
                if !ok || mode == -1 {
                    return (
                        i,
                        Some(DecodedEvent::UnknownCsi(
                            String::from_utf8_lossy(&b[..i]).into_owned(),
                        )),
                    );
                }
                let (value, _, ok) = param_get(pa, 1, 0);
                if !ok {
                    return (
                        i,
                        Some(DecodedEvent::UnknownCsi(
                            String::from_utf8_lossy(&b[..i]).into_owned(),
                        )),
                    );
                }
                return (
                    i,
                    Some(DecodedEvent::ModeReport {
                        mode,
                        value: value as u8,
                    }),
                );
            }
            _ if cmd == packed(b'?', 0, b'c') => {
                // Primary Device Attributes
                return (i, Some(parse_primary_dev_attrs(pa)));
            }
            _ if cmd == packed(b'>', 0, b'c') => {
                // Secondary Device Attributes
                return (i, Some(parse_secondary_dev_attrs(pa)));
            }
            _ if cmd == packed(b'?', 0, b'u') => {
                // Kitty keyboard flags
                let (flags, _, _) = param_get(pa, 0, -1);
                return (i, Some(DecodedEvent::KeyboardEnhancements(flags)));
            }
            _ if cmd == packed(b'?', 0, b'R') => {
                // This report may return a third parameter representing the
                // page number, but we don't really need it.
                let (row, _, _) = param_get(pa, 0, 1);
                let (col, _, ok) = param_get(pa, 1, 1);
                if !ok {
                    return (
                        i,
                        Some(DecodedEvent::UnknownCsi(
                            String::from_utf8_lossy(&b[..i]).into_owned(),
                        )),
                    );
                }
                return (
                    i,
                    Some(DecodedEvent::CursorPosition {
                        x: col - 1,
                        y: row - 1,
                    }),
                );
            }
            _ if cmd == packed(b'<', 0, b'm') || cmd == packed(b'<', 0, b'M') => {
                // Handle SGR mouse
                if params_len == 3 {
                    return (i, Some(parse_sgr_mouse_event(cmd, pa)));
                }
            }
            _ if cmd == packed(b'>', 0, b'm') => {
                // XTerm modifyOtherKeys
                let (mok, _, ok) = param_get(pa, 0, 0);
                if !ok || mok != 4 {
                    return (
                        i,
                        Some(DecodedEvent::UnknownCsi(
                            String::from_utf8_lossy(&b[..i]).into_owned(),
                        )),
                    );
                }
                let (val, _, ok) = param_get(pa, 1, -1);
                if !ok || val == -1 {
                    return (
                        i,
                        Some(DecodedEvent::UnknownCsi(
                            String::from_utf8_lossy(&b[..i]).into_owned(),
                        )),
                    );
                }
                return (i, Some(DecodedEvent::ModifyOtherKeys(val)));
            }
            _ if cmd == packed(b'?', 0, b'n') => {
                let (report, _, _) = param_get(pa, 0, -1);
                let (dark_light, _, _) = param_get(pa, 1, -1);
                if report == 997 {
                    match dark_light {
                        1 => return (i, Some(DecodedEvent::DarkColorScheme)),
                        2 => return (i, Some(DecodedEvent::LightColorScheme)),
                        _ => {}
                    }
                }
            }
            _ if cmd == b'I' as i32 => {
                return (i, Some(DecodedEvent::Focus));
            }
            _ if cmd == b'O' as i32 => {
                return (i, Some(DecodedEvent::Blur));
            }
            _ if cmd == b'R' as i32 => {
                // Cursor position report OR modified F3
                let (row, _, rok) = param_get(pa, 0, 1);
                let (col, _, cok) = param_get(pa, 1, 1);
                if params_len == 2 && rok && cok {
                    let m = DecodedEvent::CursorPosition {
                        x: col - 1,
                        y: row - 1,
                    };
                    if row == 1 && col - 1 <= (MOD_META.0 | MOD_SHIFT.0 | MOD_ALT.0 | MOD_CTRL.0) {
                        // XXX: We cannot differentiate between cursor position
                        // report and CSI 1 ; <mod> R (which is modified F3)
                        // when the cursor is at the row 1.
                        return (
                            i,
                            Some(DecodedEvent::Multi(vec![
                                DecodedEvent::KeyPress(Key {
                                    code: KEY_F3,
                                    mod_: KeyMod(col - 1),
                                    ..Key::default()
                                }),
                                m,
                            ])),
                        );
                    }

                    return (i, Some(m));
                }

                if params_len != 0 {
                    return (
                        i,
                        Some(DecodedEvent::UnknownCsi(
                            String::from_utf8_lossy(&b[..i]).into_owned(),
                        )),
                    );
                }

                // Unmodified key F3 (CSI R)
                let mut k = key_press_from_cmd(cmd);
                let (id, _, _) = param_get(pa, 0, 1);
                let (mod_, _, _) = param_get(pa, 1, 1);
                if params_len > 2 && !has_more(pa, 1) || id != 1 {
                    return (
                        i,
                        Some(DecodedEvent::UnknownCsi(
                            String::from_utf8_lossy(&b[..i]).into_owned(),
                        )),
                    );
                }
                if params_len > 1 && id == 1 && mod_ != -1 {
                    // CSI 1 ; <modifiers> A
                    k.0.mod_.0 |= mod_ - 1;
                }
                // Don't forget to handle Kitty keyboard protocol
                return (i, Some(parse_kitty_keyboard_ext(pa, k)));
            }
            _ if cmd == b'M' as i32 => {
                // Handle X10 mouse
                if i + 3 > b.len() {
                    return (
                        i,
                        Some(DecodedEvent::UnknownCsi(
                            String::from_utf8_lossy(&b[..i]).into_owned(),
                        )),
                    );
                }
                let mut buf2 = b[..i].to_vec();
                buf2.extend_from_slice(&b[i..i + 3]);
                return (i + 3, Some(parse_x10_mouse_event(&buf2)));
            }
            _ if cmd == packed(0, b'$', b'y') => {
                // Report Mode (DECRPM)
                let (mode, _, ok) = param_get(pa, 0, -1);
                if !ok || mode == -1 {
                    return (
                        i,
                        Some(DecodedEvent::UnknownCsi(
                            String::from_utf8_lossy(&b[..i]).into_owned(),
                        )),
                    );
                }
                let (val, _, ok) = param_get(pa, 1, 0);
                if !ok {
                    return (
                        i,
                        Some(DecodedEvent::UnknownCsi(
                            String::from_utf8_lossy(&b[..i]).into_owned(),
                        )),
                    );
                }
                return (
                    i,
                    Some(DecodedEvent::ModeReport {
                        mode,
                        value: val as u8,
                    }),
                );
            }
            _ if cmd == b'u' as i32 => {
                // Kitty keyboard protocol & CSI u (fixterms)
                if params_len == 0 {
                    return (
                        i,
                        Some(DecodedEvent::UnknownCsi(
                            String::from_utf8_lossy(&b[..i]).into_owned(),
                        )),
                    );
                }
                return (i, Some(parse_kitty_keyboard(pa)));
            }
            _ if matches!(
                (cmd & 0xff) as u8,
                b'a' | b'b'
                    | b'c'
                    | b'd'
                    | b'A'
                    | b'B'
                    | b'C'
                    | b'D'
                    | b'E'
                    | b'F'
                    | b'H'
                    | b'P'
                    | b'Q'
                    | b'S'
                    | b'Z'
            ) =>
            {
                // Simple CSI keys (up/down/left/right/home/end/f1-f4/tab).
                let mut k = key_press_from_cmd(cmd);
                let (id, _, _) = param_get(pa, 0, 1);
                let (mod_, _, _) = param_get(pa, 1, 1);
                if params_len > 2 && !has_more(pa, 1) || id != 1 {
                    return (
                        i,
                        Some(DecodedEvent::UnknownCsi(
                            String::from_utf8_lossy(&b[..i]).into_owned(),
                        )),
                    );
                }
                if params_len > 1 && id == 1 && mod_ != -1 {
                    // CSI 1 ; <modifiers> A
                    k.0.mod_.0 |= mod_ - 1;
                }
                // Don't forget to handle Kitty keyboard protocol
                return (i, Some(parse_kitty_keyboard_ext(pa, k)));
            }
            _ if cmd == b'_' as i32 => {
                // Win32 Input Mode
                if params_len != 6 {
                    return (
                        i,
                        Some(DecodedEvent::UnknownCsi(
                            String::from_utf8_lossy(&b[..i]).into_owned(),
                        )),
                    );
                }

                let (vk, _, _) = param_get(pa, 0, 0);
                let (sc, _, _) = param_get(pa, 1, 0);
                let (uc, _, _) = param_get(pa, 2, 0);
                let (kd, _, _) = param_get(pa, 3, 0);
                let (cs, _, _) = param_get(pa, 4, 0);
                let (rc, _, _) = param_get(pa, 5, 0);
                let event = self.parse_win32_input_key_event(
                    vk as u16,
                    sc as u16,
                    uc as u32,
                    kd == 1,
                    cs as u32,
                    (rc.max(1)) as u16,
                );

                return (i, Some(event));
            }
            _ if cmd == b'@' as i32 || cmd == b'^' as i32 || cmd == b'~' as i32 => {
                if params_len == 0 {
                    return (
                        i,
                        Some(DecodedEvent::UnknownCsi(
                            String::from_utf8_lossy(&b[..i]).into_owned(),
                        )),
                    );
                }

                let (param, _, _) = param_get(pa, 0, 0);
                if cmd == b'~' as i32 {
                    match param {
                        27 => {
                            // XTerm modifyOtherKeys 2
                            if params_len != 3 {
                                return (
                                    i,
                                    Some(DecodedEvent::UnknownCsi(
                                        String::from_utf8_lossy(&b[..i]).into_owned(),
                                    )),
                                );
                            }
                            return (i, Some(parse_xterm_modify_other_keys(pa)));
                        }
                        200 => {
                            // bracketed-paste start
                            return (i, Some(DecodedEvent::PasteStart));
                        }
                        201 => {
                            // bracketed-paste end
                            return (i, Some(DecodedEvent::PasteEnd));
                        }
                        _ => {}
                    }
                }

                match param {
                    1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 11 | 12 | 13 | 14 | 15 | 17 | 18 | 19 | 20
                    | 21 | 23 | 24 | 25 | 26 | 28 | 29 | 31 | 32 | 33 | 34 => {
                        let mut k = KeyPressEvent(Key {
                            code: 0,
                            ..Key::default()
                        });
                        match param {
                            1 => {
                                if self.legacy.0 & FLAG_FIND != 0 {
                                    k.0.code = KEY_FIND;
                                } else {
                                    k.0.code = KEY_HOME;
                                }
                            }
                            2 => k.0.code = KEY_INSERT,
                            3 => k.0.code = KEY_DELETE,
                            4 => {
                                if self.legacy.0 & FLAG_SELECT != 0 {
                                    k.0.code = KEY_SELECT;
                                } else {
                                    k.0.code = KEY_END;
                                }
                            }
                            5 => k.0.code = KEY_PG_UP,
                            6 => k.0.code = KEY_PG_DOWN,
                            7 => k.0.code = KEY_HOME,
                            8 => k.0.code = KEY_END,
                            11..=15 => k.0.code = KEY_F1 + (param - 11) as u32,
                            17..=21 => k.0.code = KEY_F6 + (param - 17) as u32,
                            23..=26 => k.0.code = KEY_F11 + (param - 23) as u32,
                            28..=29 => k.0.code = KEY_F15 + (param - 28) as u32,
                            31..=34 => k.0.code = KEY_F17 + (param - 31) as u32,
                            _ => {}
                        }

                        // modifiers
                        let (mod_, _, _) = param_get(pa, 1, -1);
                        if params_len > 1 && mod_ != -1 {
                            k.0.mod_.0 |= mod_ - 1;
                        }

                        // Handle URxvt weird keys
                        match cmd {
                            _ if cmd == b'~' as i32 => {
                                // Don't forget to handle Kitty keyboard
                                // protocol
                                return (i, Some(parse_kitty_keyboard_ext(pa, k)));
                            }
                            _ if cmd == b'^' as i32 => {
                                k.0.mod_.0 |= MOD_CTRL.0;
                            }
                            _ if cmd == b'@' as i32 => {
                                k.0.mod_.0 |= MOD_CTRL.0 | MOD_SHIFT.0;
                            }
                            _ => {}
                        }

                        return (i, Some(DecodedEvent::KeyPress(k.0)));
                    }
                    _ => {}
                }
            }
            _ if cmd == b't' as i32 => {
                let (param, _, ok) = param_get(pa, 0, 0);
                if !ok {
                    return (
                        i,
                        Some(DecodedEvent::UnknownCsi(
                            String::from_utf8_lossy(&b[..i]).into_owned(),
                        )),
                    );
                }

                match param {
                    4 => {
                        // Report Terminal window size in pixels.
                        if params_len == 3 {
                            let (height, _, h_ok) = param_get(pa, 1, 0);
                            let (width, _, w_ok) = param_get(pa, 2, 0);
                            if !h_ok || !w_ok {
                                return (
                                    i,
                                    Some(DecodedEvent::UnknownCsi(
                                        String::from_utf8_lossy(&b[..i]).into_owned(),
                                    )),
                                );
                            }
                            return (
                                i,
                                Some(DecodedEvent::PixelSize(Size {
                                    width: width as usize,
                                    height: height as usize,
                                })),
                            );
                        }
                    }
                    6 => {
                        // Report Terminal character cell size.
                        if params_len == 3 {
                            let (height, _, h_ok) = param_get(pa, 1, 0);
                            let (width, _, w_ok) = param_get(pa, 2, 0);
                            if !h_ok || !w_ok {
                                return (
                                    i,
                                    Some(DecodedEvent::UnknownCsi(
                                        String::from_utf8_lossy(&b[..i]).into_owned(),
                                    )),
                                );
                            }
                            return (
                                i,
                                Some(DecodedEvent::CellSize(Size {
                                    width: width as usize,
                                    height: height as usize,
                                })),
                            );
                        }
                    }
                    8 => {
                        // Report Terminal Window size in cells.
                        if params_len == 3 {
                            let (height, _, h_ok) = param_get(pa, 1, 0);
                            let (width, _, w_ok) = param_get(pa, 2, 0);
                            if !h_ok || !w_ok {
                                return (
                                    i,
                                    Some(DecodedEvent::UnknownCsi(
                                        String::from_utf8_lossy(&b[..i]).into_owned(),
                                    )),
                                );
                            }
                            return (
                                i,
                                Some(DecodedEvent::WindowSize(Size {
                                    width: width as usize,
                                    height: height as usize,
                                })),
                            );
                        }
                    }
                    48 if params_len == 5 => {
                        // In band terminal size report.
                        let (cell_height, _, ch_ok) = param_get(pa, 1, 0);
                        let (cell_width, _, cw_ok) = param_get(pa, 2, 0);
                        let (pixel_height, _, ph_ok) = param_get(pa, 3, 0);
                        let (pixel_width, _, pw_ok) = param_get(pa, 4, 0);
                        if !ch_ok || !cw_ok || !ph_ok || !pw_ok {
                            return (
                                i,
                                Some(DecodedEvent::UnknownCsi(
                                    String::from_utf8_lossy(&b[..i]).into_owned(),
                                )),
                            );
                        }
                        return (
                            i,
                            Some(DecodedEvent::Multi(vec![
                                DecodedEvent::WindowSize(Size {
                                    width: cell_width as usize,
                                    height: cell_height as usize,
                                }),
                                DecodedEvent::PixelSize(Size {
                                    width: pixel_width as usize,
                                    height: pixel_height as usize,
                                }),
                            ])),
                        );
                    }
                    _ => {}
                }

                // Any other window operation event.
                let mut args = Vec::new();
                for j in 1..params_len {
                    let (val, _, ok) = param_get(pa, j, 0);
                    if ok {
                        args.push(val);
                    }
                }

                return (i, Some(DecodedEvent::WindowOp { op: param, args }));
            }
            _ => {}
        }
        (
            i,
            Some(DecodedEvent::UnknownCsi(
                String::from_utf8_lossy(&b[..i]).into_owned(),
            )),
        )
    }

    /// parseSs3 parses a SS3 sequence.
    fn parse_ss3(&mut self, b: &[u8]) -> (usize, Option<DecodedEvent>) {
        if b.len() == 2 && b[0] == 0x1B {
            // short cut if this is an alt+O key
            let c = b[1].to_ascii_lowercase() as u32;
            return (
                2,
                Some(DecodedEvent::KeyPress(Key {
                    code: c,
                    mod_: KeyMod(MOD_SHIFT.0 | MOD_ALT.0),
                    ..Key::default()
                })),
            );
        }

        let mut i = 0usize;
        if b[i] == 0x8F || b[i] == 0x1B {
            i += 1;
        }
        if i < b.len() && i > 0 && b[i - 1] == 0x1B && b[i] == b'O' {
            i += 1;
        }

        // Scan numbers from 0-9
        let mut mod_: i32 = 0;
        while i < b.len() && b[i].is_ascii_digit() {
            mod_ *= 10;
            mod_ += (b[i] - b'0') as i32;
            i += 1;
        }

        // Scan a GL character
        if i >= b.len() || b[i] < 0x21 || b[i] > 0x7E {
            return (
                i,
                Some(DecodedEvent::Unknown(
                    String::from_utf8_lossy(&b[..i]).into_owned(),
                )),
            );
        }

        // GL character(s)
        let gl = b[i];
        i += 1;

        let mut k = KeyPressEvent(Key::default());
        match gl {
            b'a' | b'b' | b'c' | b'd' => {
                k.0.code = KEY_UP + (gl - b'a') as u32;
                k.0.mod_ = MOD_CTRL;
            }
            b'A' | b'B' | b'C' | b'D' => {
                k.0.code = KEY_UP + (gl - b'A') as u32;
            }
            b'E' => k.0.code = KEY_BEGIN,
            b'F' => k.0.code = KEY_END,
            b'H' => k.0.code = KEY_HOME,
            b'P' | b'Q' | b'R' | b'S' => {
                k.0.code = KEY_F1 + (gl - b'P') as u32;
            }
            b'M' => k.0.code = KEY_KP_ENTER,
            b'X' => k.0.code = KEY_KP_EQUAL,
            b'j' | b'k' | b'l' | b'm' | b'n' | b'o' | b'p' | b'q' | b'r' | b's' | b't' | b'u'
            | b'v' | b'w' | b'x' | b'y' => {
                k.0.code = KEY_KP_MULTIPLY + (gl - b'j') as u32;
            }
            _ => {
                return (
                    i,
                    Some(DecodedEvent::UnknownSs3(
                        String::from_utf8_lossy(&b[..i]).into_owned(),
                    )),
                );
            }
        }

        // Handle weird SS3 <modifier> Func
        if mod_ > 0 {
            k.0.mod_.0 |= mod_ - 1;
        }

        (i, Some(DecodedEvent::KeyPress(k.0)))
    }

    /// parseOsc parses an OSC sequence.
    fn parse_osc(&mut self, b: &[u8]) -> (usize, Option<DecodedEvent>) {
        let default_key = || -> DecodedEvent {
            DecodedEvent::KeyPress(Key {
                code: b[1] as u32,
                mod_: MOD_ALT,
                ..Key::default()
            })
        };
        if b.len() == 2 && b[0] == 0x1B {
            // short cut if this is an alt+] key
            return (2, Some(default_key()));
        }

        let mut i = 0usize;
        if b[i] == 0x9D || b[i] == 0x1B {
            i += 1;
        }
        if i < b.len() && i > 0 && b[i - 1] == 0x1B && b[i] == b']' {
            i += 1;
        }

        // Parse OSC command
        let mut start = 0usize;

        let mut cmd: i32 = -1;
        while i < b.len() && b[i].is_ascii_digit() {
            if cmd == -1 {
                cmd = 0;
            } else {
                cmd *= 10;
            }
            cmd += (b[i] - b'0') as i32;
            i += 1;
        }

        if i < b.len() && b[i] == b';' {
            // mark the start of the sequence data
            i += 1;
            start = i;
        }

        while i < b.len() {
            // advance to the end of the sequence
            if [0x07u8, 0x1B, 0x9C, 0x18, 0x1A].contains(&b[i]) {
                break;
            }
            i += 1;
        }

        if i >= b.len() {
            return (
                i,
                Some(DecodedEvent::Unknown(
                    String::from_utf8_lossy(&b[..i]).into_owned(),
                )),
            );
        }

        let end = i; // end of the sequence data
        i += 1;

        // Check 7-bit ST (string terminator) character
        match b[i - 1] {
            0x18 | 0x1A => {
                return (
                    i,
                    Some(DecodedEvent::Ignored(
                        String::from_utf8_lossy(&b[..i]).into_owned(),
                    )),
                );
            }
            0x1B => {
                if i >= b.len() || b[i] != b'\\' {
                    if cmd == -1 || (start == 0 && end == 2) {
                        return (2, Some(default_key()));
                    }

                    // If we don't have a valid ST terminator, then this is a
                    // cancelled sequence and should be ignored.
                    return (
                        i,
                        Some(DecodedEvent::Ignored(
                            String::from_utf8_lossy(&b[..i]).into_owned(),
                        )),
                    );
                }

                i += 1;
            }
            _ => {}
        }

        if end <= start {
            return (
                i,
                Some(DecodedEvent::Unknown(
                    String::from_utf8_lossy(&b[..i]).into_owned(),
                )),
            );
        }

        let data = &b[start..end];
        match cmd {
            10 => {
                return (
                    i,
                    Some(DecodedEvent::ForegroundColor(parse_xparse_color(data))),
                );
            }
            11 => {
                return (
                    i,
                    Some(DecodedEvent::BackgroundColor(parse_xparse_color(data))),
                );
            }
            12 => {
                return (i, Some(DecodedEvent::CursorColor(parse_xparse_color(data))));
            }
            52 => {
                let parts: Vec<&[u8]> = data.split(|&c| c == b';').collect();
                if parts.len() != 2 || parts[0].is_empty() {
                    return (
                        i,
                        Some(DecodedEvent::Clipboard {
                            content: String::new(),
                            selection: 0,
                        }),
                    );
                }

                let b64 = parts[1];
                match base64_decode(b64) {
                    Some(bts) => {
                        let sel = parts[0][0];
                        return (
                            i,
                            Some(DecodedEvent::Clipboard {
                                content: String::from_utf8_lossy(&bts).into_owned(),
                                selection: sel,
                            }),
                        );
                    }
                    None => {
                        return (
                            i,
                            Some(DecodedEvent::Clipboard {
                                content: String::from_utf8_lossy(b64).into_owned(),
                                selection: 0,
                            }),
                        );
                    }
                }
            }
            _ => {}
        }

        (
            i,
            Some(DecodedEvent::UnknownOsc(
                String::from_utf8_lossy(&b[..i]).into_owned(),
            )),
        )
    }

    /// parseStTerminated parses a control sequence that gets terminated by a
    /// ST character.
    fn parse_st_terminated(
        &mut self,
        intro8: u8,
        intro7: u8,
        f: StTerminatedFn,
        b: &[u8],
    ) -> (usize, Option<DecodedEvent>) {
        let default_key = |b: &[u8]| -> (usize, Option<DecodedEvent>) {
            match intro8 {
                0x98 => {
                    let c = b[1].to_ascii_lowercase() as u32;
                    (
                        2,
                        Some(DecodedEvent::KeyPress(Key {
                            code: c,
                            mod_: KeyMod(MOD_SHIFT.0 | MOD_ALT.0),
                            ..Key::default()
                        })),
                    )
                }
                0x9E | 0x9F => (
                    2,
                    Some(DecodedEvent::KeyPress(Key {
                        code: b[1] as u32,
                        mod_: MOD_ALT,
                        ..Key::default()
                    })),
                ),
                _ => (0, None),
            }
        };
        if b.len() == 2 && b[0] == 0x1B {
            return default_key(b);
        }

        let mut i = 0usize;
        if b[i] == intro8 || b[i] == 0x1B {
            i += 1;
        }
        if i < b.len() && i > 0 && b[i - 1] == 0x1B && b[i] == intro7 {
            i += 1;
        }

        // Scan control sequence
        let start = i;
        while i < b.len() {
            if [0x1Bu8, 0x9C, 0x18, 0x1A].contains(&b[i]) {
                break;
            }
            i += 1;
        }

        if i >= b.len() {
            return (
                i,
                Some(DecodedEvent::Unknown(
                    String::from_utf8_lossy(&b[..i]).into_owned(),
                )),
            );
        }

        let end = i; // end of the sequence data
        i += 1;

        // Check 7-bit ST (string terminator) character
        match b[i - 1] {
            0x18 | 0x1A => {
                return (
                    i,
                    Some(DecodedEvent::Ignored(
                        String::from_utf8_lossy(&b[..i]).into_owned(),
                    )),
                );
            }
            0x1B => {
                if i >= b.len() || b[i] != b'\\' {
                    if start == end {
                        return default_key(b);
                    }

                    // If we don't have a valid ST terminator, then this is a
                    // cancelled sequence and should be ignored.
                    return (
                        i,
                        Some(DecodedEvent::Ignored(
                            String::from_utf8_lossy(&b[..i]).into_owned(),
                        )),
                    );
                }

                i += 1;
            }
            _ => {}
        }

        // Call the function to parse the sequence and return the result
        if let Some(f) = f {
            if let Some(e) = f(&b[start..end]) {
                return (i, Some(e));
            }
        }

        match intro8 {
            0x9E => (
                i,
                Some(DecodedEvent::UnknownPm(
                    String::from_utf8_lossy(&b[..i]).into_owned(),
                )),
            ),
            0x98 => (
                i,
                Some(DecodedEvent::UnknownSos(
                    String::from_utf8_lossy(&b[..i]).into_owned(),
                )),
            ),
            0x9F => (
                i,
                Some(DecodedEvent::UnknownApc(
                    String::from_utf8_lossy(&b[..i]).into_owned(),
                )),
            ),
            _ => (
                i,
                Some(DecodedEvent::Unknown(
                    String::from_utf8_lossy(&b[..i]).into_owned(),
                )),
            ),
        }
    }

    /// parseDcs parses a DCS sequence.
    fn parse_dcs(&mut self, b: &[u8]) -> (usize, Option<DecodedEvent>) {
        if b.len() == 2 && b[0] == 0x1B {
            // short cut if this is an alt+P key
            let c = b[1].to_ascii_lowercase() as u32;
            return (
                2,
                Some(DecodedEvent::KeyPress(Key {
                    code: c,
                    mod_: KeyMod(MOD_SHIFT.0 | MOD_ALT.0),
                    ..Key::default()
                })),
            );
        }

        let mut params: [i32; 16] = [MISSING_PARAM; 16];
        let mut params_len = 0usize;
        let mut cmd: i32 = 0;

        let mut i = 0usize;
        if b[i] == 0x90 || b[i] == 0x1B {
            i += 1;
        }
        if i < b.len() && i > 0 && b[i - 1] == 0x1B && b[i] == b'P' {
            i += 1;
        }

        // initial DCS byte
        if i < b.len() && b[i] >= b'<' && b[i] <= b'?' {
            cmd |= (b[i] as i32) << 8;
        }

        // Scan parameter bytes in the range 0x30-0x3F
        let mut j = 0usize;
        while i < b.len() && params_len < params.len() && b[i] >= 0x30 && b[i] <= 0x3F {
            if b[i] >= b'0' && b[i] <= b'9' {
                if params[params_len] == MISSING_PARAM {
                    params[params_len] = 0;
                }
                params[params_len] *= 10;
                params[params_len] += (b[i] - b'0') as i32;
            }
            if b[i] == b':' {
                params[params_len] |= HAS_MORE_FLAG;
            }
            if b[i] == b';' || b[i] == b':' {
                params_len += 1;
                if params_len < params.len() {
                    params[params_len] = MISSING_PARAM;
                }
            }
            i += 1;
            j += 1;
        }

        if j > 0 && params_len < params.len() {
            params_len += 1;
        }

        // Scan intermediate bytes in the range 0x20-0x2F
        let mut intermed: u8 = 0;
        while i < b.len() && b[i] >= 0x20 && b[i] <= 0x2F {
            intermed = b[i];
            i += 1;
        }

        // set intermediate byte
        cmd |= (intermed as i32) << 16;

        // Scan final byte in the range 0x40-0x7E
        if i >= b.len() || b[i] < 0x40 || b[i] > 0x7E {
            return (
                i,
                Some(DecodedEvent::Unknown(
                    String::from_utf8_lossy(&b[..i]).into_owned(),
                )),
            );
        }

        // Add the final byte
        cmd |= b[i] as i32;
        i += 1;

        let start = i; // start of the sequence data
        while i < b.len() {
            if b[i] == 0x9C || b[i] == 0x1B {
                break;
            }
            i += 1;
        }

        if i >= b.len() {
            return (
                i,
                Some(DecodedEvent::Unknown(
                    String::from_utf8_lossy(&b[..i]).into_owned(),
                )),
            );
        }

        let end = i; // end of the sequence data
        i += 1;

        // Check 7-bit ST (string terminator) character
        if i < b.len() && b[i - 1] == 0x1B && b[i] == b'\\' {
            i += 1;
        }

        let pa = &params[..params_len];
        let packed = |inter: u8, final_: u8| -> i32 {
            let mut c = final_ as i32;
            c |= (inter as i32) << 16;
            c
        };

        match cmd {
            _ if cmd == packed(b'+', b'r') => {
                // XTGETTCAP responses
                let (param, _, _) = param_get(pa, 0, 0);
                if param == 1 {
                    // 1 means valid response, 0 means invalid response
                    let tc = parse_termcap(&b[start..end]);
                    return (i, Some(DecodedEvent::Capability(tc)));
                }
            }
            _ if cmd == packed(0, b'|') || cmd == ((b'>' as i32) << 8 | b'|' as i32) => {
                // XTVersion response
                return (
                    i,
                    Some(DecodedEvent::TerminalVersion(
                        String::from_utf8_lossy(&b[start..end]).into_owned(),
                    )),
                );
            }
            _ if cmd == packed(b'!', b'|') => {
                // Tertiary Device Attributes
                return (i, Some(parse_tertiary_dev_attrs(&b[start..end])));
            }
            _ => {}
        }

        (
            i,
            Some(DecodedEvent::UnknownDcs(
                String::from_utf8_lossy(&b[..i]).into_owned(),
            )),
        )
    }

    /// parseApc parses an APC sequence.
    fn parse_apc(&mut self, b: &[u8]) -> (usize, Option<DecodedEvent>) {
        if b.len() == 2 && b[0] == 0x1B {
            // short cut if this is an alt+_ key
            return (
                2,
                Some(DecodedEvent::KeyPress(Key {
                    code: b[1] as u32,
                    mod_: MOD_ALT,
                    ..Key::default()
                })),
            );
        }

        self.parse_st_terminated(0x9F, b'_', Some(parse_apc_data), b)
    }

    /// parseUtf8 parses a UTF-8 sequence.
    fn parse_utf8(&mut self, b: &[u8]) -> (usize, Option<DecodedEvent>) {
        if b.is_empty() {
            return (0, None);
        }

        let c = b[0];
        if c <= 0x1F || c == 0x7F {
            // Control codes get handled by parseControl
            return (1, Some(self.parse_control(c)));
        } else if c > 0x1F && c < 0x7F {
            // ASCII printable characters
            let code = c as u32;
            let mut k = Key {
                code,
                text: (c as char).to_string(),
                ..Key::default()
            };
            if (c as char).is_uppercase() {
                // Convert upper case letters to lower case + shift modifier
                k.code = (c as char).to_ascii_lowercase() as u32;
                k.shifted_code = code;
                k.mod_.0 |= MOD_SHIFT.0;
            }

            return (1, Some(DecodedEvent::KeyPress(k)));
        }

        let s = std::str::from_utf8(b).unwrap_or("\u{FFFD}");
        let first = s.graphemes(true).next().unwrap_or("\u{FFFD}");
        let cluster = first.as_bytes();
        let mut code: u32 = 0;
        if let Some(ch) = first.chars().next() {
            code = ch as u32;
        }
        let text = first.to_string();
        if first.chars().count() > 1 {
            // Use [KEY_EXTENDED] for multi-rune graphemes
            code = KEY_EXTENDED;
        }

        (
            cluster.len(),
            Some(DecodedEvent::KeyPress(Key {
                code,
                text,
                ..Key::default()
            })),
        )
    }

    /// parseControl parses a control character.
    fn parse_control(&self, b: u8) -> DecodedEvent {
        match b {
            0x00 => {
                if self.legacy.0 & FLAG_CTRL_AT != 0 {
                    return DecodedEvent::KeyPress(Key {
                        code: b'@' as u32,
                        mod_: MOD_CTRL,
                        ..Key::default()
                    });
                }
                DecodedEvent::KeyPress(Key {
                    code: KEY_SPACE,
                    mod_: MOD_CTRL,
                    ..Key::default()
                })
            }
            0x08 => DecodedEvent::KeyPress(Key {
                code: b'h' as u32,
                mod_: MOD_CTRL,
                ..Key::default()
            }),
            0x09 => {
                if self.legacy.0 & FLAG_CTRL_I != 0 {
                    return DecodedEvent::KeyPress(Key {
                        code: b'i' as u32,
                        mod_: MOD_CTRL,
                        ..Key::default()
                    });
                }
                DecodedEvent::KeyPress(Key {
                    code: KEY_TAB,
                    ..Key::default()
                })
            }
            0x0D => {
                if self.legacy.0 & FLAG_CTRL_M != 0 {
                    return DecodedEvent::KeyPress(Key {
                        code: b'm' as u32,
                        mod_: MOD_CTRL,
                        ..Key::default()
                    });
                }
                DecodedEvent::KeyPress(Key {
                    code: KEY_ENTER,
                    ..Key::default()
                })
            }
            0x1B => {
                if self.legacy.0 & FLAG_CTRL_OPEN_BRACKET != 0 {
                    return DecodedEvent::KeyPress(Key {
                        code: b'[' as u32,
                        mod_: MOD_CTRL,
                        ..Key::default()
                    });
                }
                DecodedEvent::KeyPress(Key {
                    code: KEY_ESCAPE,
                    ..Key::default()
                })
            }
            0x7F => {
                if self.legacy.0 & FLAG_BACKSPACE != 0 {
                    return DecodedEvent::KeyPress(Key {
                        code: KEY_DELETE,
                        ..Key::default()
                    });
                }
                DecodedEvent::KeyPress(Key {
                    code: KEY_BACKSPACE,
                    ..Key::default()
                })
            }
            0x20 => DecodedEvent::KeyPress(Key {
                code: KEY_SPACE,
                text: " ".to_string(),
                ..Key::default()
            }),
            _ => {
                if (0x01..=0x1A).contains(&b) {
                    // Use lower case letters for control codes
                    let code = (b + 0x60) as u32;
                    DecodedEvent::KeyPress(Key {
                        code,
                        mod_: MOD_CTRL,
                        ..Key::default()
                    })
                } else if (0x1C..=0x1F).contains(&b) {
                    let code = (b + 0x40) as u32;
                    DecodedEvent::KeyPress(Key {
                        code,
                        mod_: MOD_CTRL,
                        ..Key::default()
                    })
                } else {
                    DecodedEvent::Unknown((b as char).to_string())
                }
            }
        }
    }

    /// parseWin32InputKeyEvent converts a Windows Input Record Key Event into
    /// a key event.
    fn parse_win32_input_key_event(
        &mut self,
        vkc: u16,
        _sc: u16,
        r: u32,
        key_down: bool,
        cks: u32,
        repeat_count: u16,
    ) -> DecodedEvent {
        let mut event = self.parse_win32_key(vkc, r, key_down, cks);
        if vkc != 0 {
            self.last_cks = cks;
        }
        if repeat_count > 1 {
            let mut multi = Vec::new();
            for _ in 0..repeat_count {
                multi.push(event.clone());
            }
            event = DecodedEvent::Multi(multi);
        }
        event
    }

    fn parse_win32_key(&mut self, vkc: u16, r: u32, key_down: bool, cks: u32) -> DecodedEvent {
        let mut key = Key::default();
        match vkc {
            0 => {
                // This is either a UTF-16 encoded pair, or an escape sequence
                // waiting to be decoded.
                let mod_ = translate_control_key_state(cks);
                if key_down {
                    return DecodedEvent::KeyPress(Key {
                        code: 0,
                        base_code: r,
                        mod_,
                        ..Key::default()
                    });
                }
                return DecodedEvent::KeyRelease(Key {
                    code: 0,
                    base_code: r,
                    mod_,
                    ..Key::default()
                });
            }
            VK_BACK => key.base_code = KEY_BACKSPACE,
            VK_TAB => key.base_code = KEY_TAB,
            VK_RETURN => key.base_code = KEY_ENTER,
            VK_SHIFT => {
                if cks & SHIFT_PRESSED != 0 {
                    if cks & ENHANCED_KEY != 0 {
                        key.base_code = KEY_RIGHT_SHIFT;
                    } else {
                        key.base_code = KEY_LEFT_SHIFT;
                    }
                } else if self.last_cks & SHIFT_PRESSED != 0 {
                    if self.last_cks & ENHANCED_KEY != 0 {
                        key.base_code = KEY_RIGHT_SHIFT;
                    } else {
                        key.base_code = KEY_LEFT_SHIFT;
                    }
                }
            }
            VK_CONTROL => {
                if cks & LEFT_CTRL_PRESSED != 0 {
                    key.base_code = KEY_LEFT_CTRL;
                } else if cks & RIGHT_CTRL_PRESSED != 0 {
                    key.base_code = KEY_RIGHT_CTRL;
                } else if self.last_cks & LEFT_CTRL_PRESSED != 0 {
                    key.base_code = KEY_LEFT_CTRL;
                } else if self.last_cks & RIGHT_CTRL_PRESSED != 0 {
                    key.base_code = KEY_RIGHT_CTRL;
                }
            }
            VK_MENU => {
                if cks & LEFT_ALT_PRESSED != 0 {
                    key.base_code = KEY_LEFT_ALT;
                } else if cks & RIGHT_ALT_PRESSED != 0 {
                    key.base_code = KEY_RIGHT_ALT;
                } else if self.last_cks & LEFT_ALT_PRESSED != 0 {
                    key.base_code = KEY_LEFT_ALT;
                } else if self.last_cks & RIGHT_ALT_PRESSED != 0 {
                    key.base_code = KEY_RIGHT_ALT;
                }
            }
            VK_PAUSE => key.base_code = KEY_PAUSE,
            VK_CAPITAL => key.base_code = KEY_CAPS_LOCK,
            VK_ESCAPE => key.base_code = KEY_ESCAPE,
            VK_SPACE => key.base_code = KEY_SPACE,
            VK_PRIOR => key.base_code = KEY_PG_UP,
            VK_NEXT => key.base_code = KEY_PG_DOWN,
            VK_END => key.base_code = KEY_END,
            VK_HOME => key.base_code = KEY_HOME,
            VK_LEFT => key.base_code = KEY_LEFT,
            VK_UP => key.base_code = KEY_UP,
            VK_RIGHT => key.base_code = KEY_RIGHT,
            VK_DOWN => key.base_code = KEY_DOWN,
            VK_SELECT => key.base_code = KEY_SELECT,
            VK_SNAPSHOT => key.base_code = KEY_PRINT_SCREEN,
            VK_INSERT => key.base_code = KEY_INSERT,
            VK_DELETE => key.base_code = KEY_DELETE,
            0x30..=0x39 => key.base_code = vkc as u32,
            0x41..=0x5A => {
                // Convert to lowercase.
                key.base_code = (vkc + 32) as u32;
            }
            VK_LWIN => key.base_code = KEY_LEFT_SUPER,
            VK_RWIN => key.base_code = KEY_RIGHT_SUPER,
            VK_APPS => key.base_code = KEY_MENU,
            0x60..=0x69 => {
                key.base_code = KEY_KP_0 + (vkc - 0x60) as u32;
                key.text = ((b'0' + (vkc - 0x60) as u8) as char).to_string();
            }
            VK_MULTIPLY => {
                key.base_code = KEY_KP_MULTIPLY;
                key.text = "*".to_string();
            }
            VK_ADD => {
                key.base_code = KEY_KP_PLUS;
                key.text = "+".to_string();
            }
            VK_SEPARATOR => {
                key.base_code = KEY_KP_COMMA;
                key.text = ",".to_string();
            }
            VK_SUBTRACT => {
                key.base_code = KEY_KP_MINUS;
                key.text = "-".to_string();
            }
            VK_DECIMAL => {
                key.base_code = KEY_KP_DECIMAL;
                key.text = ".".to_string();
            }
            VK_DIVIDE => {
                key.base_code = KEY_KP_DIVIDE;
                key.text = "/".to_string();
            }
            0x70..=0x87 => {
                key.base_code = KEY_F1 + (vkc - 0x70) as u32;
            }
            VK_NUMLOCK => key.base_code = KEY_NUM_LOCK,
            VK_SCROLL => key.base_code = KEY_SCROLL_LOCK,
            VK_LSHIFT => key.base_code = KEY_LEFT_SHIFT,
            VK_RSHIFT => key.base_code = KEY_RIGHT_SHIFT,
            VK_LCONTROL => key.base_code = KEY_LEFT_CTRL,
            VK_RCONTROL => key.base_code = KEY_RIGHT_CTRL,
            VK_LMENU => key.base_code = KEY_LEFT_ALT,
            VK_RMENU => key.base_code = KEY_RIGHT_ALT,
            VK_VOLUME_MUTE => key.base_code = KEY_MUTE,
            VK_VOLUME_DOWN => key.base_code = KEY_LOWER_VOL,
            VK_VOLUME_UP => key.base_code = KEY_RAISE_VOL,
            VK_MEDIA_NEXT_TRACK => key.base_code = KEY_MEDIA_NEXT,
            VK_MEDIA_PREV_TRACK => key.base_code = KEY_MEDIA_PREV,
            VK_MEDIA_STOP => key.base_code = KEY_MEDIA_STOP,
            VK_MEDIA_PLAY_PAUSE => key.base_code = KEY_MEDIA_PLAY_PAUSE,
            VK_OEM_1 => key.base_code = b';' as u32,
            VK_OEM_PLUS => key.base_code = b'+' as u32,
            VK_OEM_COMMA => key.base_code = b',' as u32,
            VK_OEM_MINUS => key.base_code = b'-' as u32,
            VK_OEM_PERIOD => key.base_code = b'.' as u32,
            VK_OEM_2 => key.base_code = b'/' as u32,
            VK_OEM_3 => key.base_code = b'`' as u32,
            VK_OEM_4 => key.base_code = b'[' as u32,
            VK_OEM_5 => key.base_code = b'\\' as u32,
            VK_OEM_6 => key.base_code = b']' as u32,
            VK_OEM_7 => key.base_code = b'\'' as u32,
            _ => {}
        }

        // AltGr is left ctrl + right alt.
        const ALT_GR_PRESSED: u32 = LEFT_CTRL_PRESSED | RIGHT_ALT_PRESSED;
        let alt_gr = cks & ALT_GR_PRESSED == ALT_GR_PRESSED;

        // Remove these lock keys from the control key state from now on.
        let cks = cks & !NUMLOCK_ON & !SCROLLLOCK_ON;
        key.code = key.base_code;
        if !char::from_u32(r).map(|c| c.is_control()).unwrap_or(true) {
            key.code = r;
            if char::from_u32(r).map(|c| c.is_control()).unwrap_or(true) {
                // unreachable: handled above
            } else if char::from_u32(r)
                .map(|c| c.is_alphanumeric() || c.is_ascii_punctuation() || c.is_whitespace())
                .unwrap_or(false)
                && (cks == 0
                    || cks == SHIFT_PRESSED
                    || cks == CAPSLOCK_ON
                    || cks == (SHIFT_PRESSED | CAPSLOCK_ON)
                    || alt_gr)
            {
                // If the control key state is 0, shift is pressed, or caps
                // lock then the key event is a printable event.
                if let Some(c) = char::from_u32(key.code) {
                    key.text = c.to_string();
                }
            }
        }

        key.mod_ = translate_control_key_state(cks);
        key = ensure_key_case(key, cks);
        if key_down {
            DecodedEvent::KeyPress(key)
        } else {
            DecodedEvent::KeyRelease(key)
        }
    }
}

/// key_press_from_cmd builds the key event for the simple CSI final-byte
/// keys (a/b/c/d/A/B/C/D/E/F/H/P/Q/R/S/Z).
fn key_press_from_cmd(cmd: i32) -> KeyPressEvent {
    let f = (cmd & 0xff) as u8;
    let mut k = KeyPressEvent(Key::default());
    match f {
        b'a' | b'b' | b'c' | b'd' => {
            k.0.code = KEY_UP + (f - b'a') as u32;
            k.0.mod_ = MOD_SHIFT;
        }
        b'A' | b'B' | b'C' | b'D' => {
            k.0.code = KEY_UP + (f - b'A') as u32;
        }
        b'E' => k.0.code = KEY_BEGIN,
        b'F' => k.0.code = KEY_END,
        b'H' => k.0.code = KEY_HOME,
        b'P' | b'Q' | b'R' | b'S' => {
            k.0.code = KEY_F1 + (f - b'P') as u32;
        }
        b'Z' => {
            k.0.code = KEY_TAB;
            k.0.mod_ = MOD_SHIFT;
        }
        _ => {}
    }
    k
}

/// param_get returns the unpacked parameter at the given index with the
/// given default value.
fn param_get(params: &[i32], i: usize, def: i32) -> (i32, bool, bool) {
    match params.get(i) {
        None => (def, false, false),
        Some(p) => {
            let packed = *p;
            let value = packed & !HAS_MORE_FLAG;
            let has_more = packed & HAS_MORE_FLAG != 0;
            let value = if value == MISSING_PARAM { def } else { value };
            (value, has_more, true)
        }
    }
}

fn has_more(params: &[i32], i: usize) -> bool {
    params
        .get(i)
        .map(|p| p & HAS_MORE_FLAG != 0)
        .unwrap_or(false)
}

/// parseXTermModifyOtherKeys parses an XTerm modifyOtherKeys sequence.
fn parse_xterm_modify_other_keys(params: &[i32]) -> DecodedEvent {
    // XTerm modify other keys starts with ESC [ 27 ; <modifier> ; <code> ~
    let (xmod, _, _) = param_get(params, 1, 1);
    let (xrune, _, _) = param_get(params, 2, 1);
    let mod_ = KeyMod(xmod - 1);
    let r = xrune as u32;

    let code = match r {
        0x08 => KEY_BACKSPACE,
        0x09 => KEY_TAB,
        0x0D => KEY_ENTER,
        0x1B => KEY_ESCAPE,
        0x7F => KEY_BACKSPACE,
        _ => 0,
    };
    if code != 0 {
        return DecodedEvent::KeyPress(Key {
            code,
            mod_,
            ..Key::default()
        });
    }

    // CSI 27 ; <modifier> ; <code> ~ keys defined in XTerm modifyOtherKeys
    let mut k = Key {
        code: r,
        mod_,
        ..Key::default()
    };
    if mod_.0 <= MOD_SHIFT.0 {
        if let Some(c) = char::from_u32(r) {
            k.text = c.to_string();
        }
    }

    DecodedEvent::KeyPress(k)
}

/// kittyKeyMap maps kitty keyboard protocol key codes to keys.
fn kitty_key_map(code: i32) -> Option<Key> {
    // These are some faulty C0 mappings some terminals such as WezTerm have
    // and doesn't follow the specs (upstream `init()`).
    if code == 0x00 {
        return Some(Key {
            code: KEY_SPACE,
            mod_: MOD_CTRL,
            ..Key::default()
        });
    }
    if (0x01..=0x1A).contains(&code) {
        return Some(Key {
            code: (code + 0x60) as u32,
            mod_: MOD_CTRL,
            ..Key::default()
        });
    }
    if (0x1C..=0x1F).contains(&code) {
        return Some(Key {
            code: (code + 0x40) as u32,
            mod_: MOD_CTRL,
            ..Key::default()
        });
    }
    Some(Key {
        code: match code {
            0x08 => KEY_BACKSPACE,
            0x09 => KEY_TAB,
            0x0D => KEY_ENTER,
            0x1B => KEY_ESCAPE,
            0x7F => KEY_BACKSPACE,
            57344 => KEY_ESCAPE,
            57345 => KEY_ENTER,
            57346 => KEY_TAB,
            57347 => KEY_BACKSPACE,
            57348 => KEY_INSERT,
            57349 => KEY_DELETE,
            57350 => KEY_LEFT,
            57351 => KEY_RIGHT,
            57352 => KEY_UP,
            57353 => KEY_DOWN,
            57354 => KEY_PG_UP,
            57355 => KEY_PG_DOWN,
            57356 => KEY_HOME,
            57357 => KEY_END,
            57358 => KEY_CAPS_LOCK,
            57359 => KEY_SCROLL_LOCK,
            57360 => KEY_NUM_LOCK,
            57361 => KEY_PRINT_SCREEN,
            57362 => KEY_PAUSE,
            57363 => KEY_MENU,
            57364..=57375 => KEY_F1 + (code - 57364) as u32,
            57376..=57383 => KEY_F13 + (code - 57376) as u32,
            57384..=57398 => KEY_F21 + (code - 57384) as u32,
            57399 => KEY_KP_0,
            57400 => KEY_KP_1,
            57401 => KEY_KP_2,
            57402 => KEY_KP_3,
            57403 => KEY_KP_4,
            57404 => KEY_KP_5,
            57405 => KEY_KP_6,
            57406 => KEY_KP_7,
            57407 => KEY_KP_8,
            57408 => KEY_KP_9,
            57409 => KEY_KP_DECIMAL,
            57410 => KEY_KP_DIVIDE,
            57411 => KEY_KP_MULTIPLY,
            57412 => KEY_KP_MINUS,
            57413 => KEY_KP_PLUS,
            57414 => KEY_KP_ENTER,
            57415 => KEY_KP_EQUAL,
            57416 => KEY_KP_SEP,
            57417 => KEY_KP_LEFT,
            57418 => KEY_KP_RIGHT,
            57419 => KEY_KP_UP,
            57420 => KEY_KP_DOWN,
            57421 => KEY_KP_PG_UP,
            57422 => KEY_KP_PG_DOWN,
            57423 => KEY_KP_HOME,
            57424 => KEY_KP_END,
            57425 => KEY_KP_INSERT,
            57426 => KEY_KP_DELETE,
            57427 => KEY_KP_BEGIN,
            57428 => KEY_MEDIA_PLAY,
            57429 => KEY_MEDIA_PAUSE,
            57430 => KEY_MEDIA_PLAY_PAUSE,
            57431 => KEY_MEDIA_REVERSE,
            57432 => KEY_MEDIA_STOP,
            57433 => KEY_MEDIA_FAST_FORWARD,
            57434 => KEY_MEDIA_REWIND,
            57435 => KEY_MEDIA_NEXT,
            57436 => KEY_MEDIA_PREV,
            57437 => KEY_MEDIA_RECORD,
            57438 => KEY_LOWER_VOL,
            57439 => KEY_RAISE_VOL,
            57440 => KEY_MUTE,
            57441 => KEY_LEFT_SHIFT,
            57442 => KEY_LEFT_CTRL,
            57443 => KEY_LEFT_ALT,
            57444 => KEY_LEFT_SUPER,
            57445 => KEY_LEFT_HYPER,
            57446 => KEY_LEFT_META,
            57447 => KEY_RIGHT_SHIFT,
            57448 => KEY_RIGHT_CTRL,
            57449 => KEY_RIGHT_ALT,
            57450 => KEY_RIGHT_SUPER,
            57451 => KEY_RIGHT_HYPER,
            57452 => KEY_RIGHT_META,
            57453 => KEY_ISO_LEVEL3_SHIFT,
            57454 => KEY_ISO_LEVEL5_SHIFT,
            _ => return None,
        },
        ..Key::default()
    })
}

/// fromKittyMod converts a kitty protocol modifier to a [KeyMod].
fn from_kitty_mod(mod_: i32) -> KeyMod {
    let mut m = 0i32;
    if mod_ & KITTY_SHIFT != 0 {
        m |= MOD_SHIFT.0;
    }
    if mod_ & KITTY_ALT != 0 {
        m |= MOD_ALT.0;
    }
    if mod_ & KITTY_CTRL != 0 {
        m |= MOD_CTRL.0;
    }
    if mod_ & KITTY_SUPER != 0 {
        m |= MOD_SUPER.0;
    }
    if mod_ & KITTY_HYPER != 0 {
        m |= MOD_HYPER.0;
    }
    if mod_ & KITTY_META != 0 {
        m |= MOD_META.0;
    }
    if mod_ & KITTY_CAPS_LOCK != 0 {
        m |= MOD_CAPS_LOCK.0;
    }
    if mod_ & KITTY_NUM_LOCK != 0 {
        m |= MOD_NUM_LOCK.0;
    }
    KeyMod(m)
}

/// parseKittyKeyboard parses a Kitty Keyboard Protocol sequence.
fn parse_kitty_keyboard(params: &[i32]) -> DecodedEvent {
    let mut is_release = false;
    let mut key = Key::default();

    // The index of parameters separated by semicolons ';'. Sub parameters
    // are separated by colons ':'.
    let mut param_idx = 0usize;
    let mut sud_idx = 0usize; // The sub parameter index
    for p in params {
        // Kitty Keyboard Protocol has 3 optional components.
        match param_idx {
            0 => match sud_idx {
                0 => {
                    let code = Param_(*p).param(1); // CSI u has a default value of 1
                    if let Some(k) = kitty_key_map(code) {
                        key = k;
                    } else {
                        key.code = code as u32;
                    }
                }
                2 => {
                    // shifted key + base key
                    let b = Param_(*p).param(1) as u32;
                    if let Some(c) = char::from_u32(b) {
                        if !c.is_control() {
                            key.base_code = b;
                        }
                    }
                    // fallthrough to case 1
                    let s = Param_(*p).param(1) as u32;
                    if let Some(c) = char::from_u32(s) {
                        if !c.is_control() {
                            // XXX: We swap keys here because we want the
                            // shifted key to be the Rune that is returned.
                            key.shifted_code = s;
                        }
                    }
                }
                1 => {
                    // shifted key
                    let s = Param_(*p).param(1) as u32;
                    if let Some(c) = char::from_u32(s) {
                        if !c.is_control() {
                            key.shifted_code = s;
                        }
                    }
                }
                _ => {}
            },
            1 => match sud_idx {
                0 => {
                    let mod_ = Param_(*p).param(1);
                    if mod_ > 1 {
                        key.mod_ = from_kitty_mod(mod_ - 1);
                        if key.mod_.0 > MOD_SHIFT.0 {
                            // XXX: We need to clear the text if we have a
                            // modifier key other than a [MOD_SHIFT] key.
                            key.text = String::new();
                        }
                    }
                }
                1 => match Param_(*p).param(1) {
                    2 => key.is_repeat = true,
                    3 => is_release = true,
                    _ => {}
                },
                _ => {}
            },
            2 => {
                let code = Param_(*p).param(0);
                if code != 0 {
                    if let Some(c) = char::from_u32(code as u32) {
                        key.text.push(c);
                    }
                }
            }
            _ => {}
        }

        sud_idx += 1;
        if *p & HAS_MORE_FLAG == 0 {
            param_idx += 1;
            sud_idx = 0;
        }
    }

    let key_mod = key.mod_;

    // Remove these lock modifiers from now on since they don't affect the
    // text.
    let key_mod = KeyMod(key_mod.0 & !MOD_NUM_LOCK.0);

    let print_mod = key_mod.0 <= MOD_SHIFT.0
        || key_mod == MOD_CAPS_LOCK
        || key_mod == KeyMod(MOD_SHIFT.0 | MOD_CAPS_LOCK.0);
    let print_key_pad = key.code >= KEY_KP_EQUAL && key.code <= KEY_KP_SEP;
    if key.text.is_empty() && print_key_pad && print_mod {
        match key.code {
            KEY_KP_0..=KEY_KP_9 => {
                key.text = ((b'0' + (key.code - KEY_KP_0) as u8) as char).to_string();
            }
            KEY_KP_EQUAL => key.text = "=".to_string(),
            KEY_KP_MULTIPLY => key.text = "*".to_string(),
            KEY_KP_PLUS => key.text = "+".to_string(),
            KEY_KP_MINUS => key.text = "-".to_string(),
            KEY_KP_DECIMAL => key.text = ".".to_string(),
            KEY_KP_DIVIDE => key.text = "/".to_string(),
            KEY_KP_SEP => key.text = ",".to_string(),
            _ => {}
        }
    }

    if key.text.is_empty() && key.code <= 0x10FFFF && print_mod {
        if let Some(c) = char::from_u32(key.code) {
            if !c.is_control() {
                if key_mod.0 == 0 {
                    key.text = c.to_string();
                } else {
                    let desired_case =
                        if key_mod.contains(MOD_SHIFT) || key_mod.contains(MOD_CAPS_LOCK) {
                            c.to_uppercase().collect::<String>()
                        } else {
                            c.to_lowercase().collect::<String>()
                        };
                    if key.shifted_code != 0 {
                        if let Some(s) = char::from_u32(key.shifted_code) {
                            key.text = s.to_string();
                        }
                    } else {
                        key.text = desired_case;
                    }
                }
            }
        }
    }

    if is_release {
        DecodedEvent::KeyRelease(key)
    } else {
        DecodedEvent::KeyPress(key)
    }
}

/// Param_ is a small wrapper replicating the packed param accessors.
#[derive(Clone, Copy)]
struct Param_(i32);

impl Param_ {
    fn param(&self, def: i32) -> i32 {
        let p = self.0 & !HAS_MORE_FLAG;
        if p == MISSING_PARAM {
            return def;
        }
        p
    }
    #[allow(dead_code)]
    fn has_more(&self) -> bool {
        self.0 & HAS_MORE_FLAG != 0
    }
}

/// parseKittyKeyboardExt parses Kitty Keyboard Protocol sequence extensions
/// for non CSI u sequences.
fn parse_kitty_keyboard_ext(params: &[i32], k: KeyPressEvent) -> DecodedEvent {
    // Handle Kitty keyboard protocol
    if params.len() > 2 && params[1] & HAS_MORE_FLAG != 0 {
        // The second parameter is a subparameter (separated by a ":")
        match Param_(params[2]).param(1) {
            // The third parameter is the event type (defaults to 1)
            2 => {
                let mut k = k;
                k.0.is_repeat = true;
                DecodedEvent::KeyPress(k.0)
            }
            3 => DecodedEvent::KeyRelease(k.0),
            _ => DecodedEvent::KeyPress(k.0),
        }
    } else {
        DecodedEvent::KeyPress(k.0)
    }
}

/// parsePrimaryDevAttrs parses the terminal primary device attributes.
fn parse_primary_dev_attrs(params: &[i32]) -> DecodedEvent {
    // Primary Device Attributes
    let mut da1 = Vec::new();
    for p in params {
        if p & HAS_MORE_FLAG == 0 {
            da1.push(Param_(*p).param(0));
        }
    }
    DecodedEvent::PrimaryDeviceAttributes(da1)
}

/// parseSecondaryDevAttrs parses the terminal secondary device attributes.
fn parse_secondary_dev_attrs(params: &[i32]) -> DecodedEvent {
    // Secondary Device Attributes
    let mut da2 = Vec::new();
    for p in params {
        if p & HAS_MORE_FLAG == 0 {
            da2.push(Param_(*p).param(0));
        }
    }
    DecodedEvent::SecondaryDeviceAttributes(da2)
}

/// parseTertiaryDevAttrs parses the terminal tertiary device attributes.
fn parse_tertiary_dev_attrs(b: &[u8]) -> DecodedEvent {
    // Tertiary Device Attributes
    // The response is a 4-digit hexadecimal number.
    match hex_decode(b) {
        Some(bts) => {
            DecodedEvent::TertiaryDeviceAttributes(String::from_utf8_lossy(&bts).into_owned())
        }
        None => DecodedEvent::UnknownDcs(format!("\x1bP!|{}\x1b\\", String::from_utf8_lossy(b))),
    }
}

/// parseSGRMouseEvent parses SGR-encoded mouse events.
fn parse_sgr_mouse_event(cmd: i32, params: &[i32]) -> DecodedEvent {
    let (mut x, _, ok) = param_get(params, 1, 1);
    if !ok {
        x = 1;
    }
    let (mut y, _, ok) = param_get(params, 2, 1);
    if !ok {
        y = 1;
    }
    let release = (cmd & 0xff) as u8 == b'm';
    let (b, _, _) = param_get(params, 0, 0);
    let (mod_, btn, _, is_motion) = parse_mouse_button(b);

    // (1,1) is the upper left. We subtract 1 to normalize it to (0,0).
    x -= 1;
    y -= 1;

    let m = Mouse {
        x,
        y,
        button: btn,
        mod_,
    };

    // Wheel buttons don't have release events
    // Motion can be reported as a release event in some terminals
    if is_wheel(m.button) {
        DecodedEvent::MouseWheel(m)
    } else if !is_motion && release {
        DecodedEvent::MouseRelease(m)
    } else if is_motion {
        DecodedEvent::MouseMotion(m)
    } else {
        DecodedEvent::MouseClick(m)
    }
}

/// parseX10MouseEvent parses X10-encoded mouse events.
fn parse_x10_mouse_event(buf: &[u8]) -> DecodedEvent {
    let v = &buf[3..6];
    let mut b = v[0] as i32;
    if b >= X10_MOUSE_BYTE_OFFSET {
        b -= X10_MOUSE_BYTE_OFFSET;
    }

    let (mod_, btn, is_release, is_motion) = parse_mouse_button(b);

    // (1,1) is the upper left. We subtract 1 to normalize it to (0,0).
    let x = v[1] as i32 - X10_MOUSE_BYTE_OFFSET - 1;
    let y = v[2] as i32 - X10_MOUSE_BYTE_OFFSET - 1;

    let m = Mouse {
        x,
        y,
        button: btn,
        mod_,
    };
    if is_wheel(m.button) {
        DecodedEvent::MouseWheel(m)
    } else if is_motion {
        DecodedEvent::MouseMotion(m)
    } else if is_release {
        DecodedEvent::MouseRelease(m)
    } else {
        DecodedEvent::MouseClick(m)
    }
}

const X10_MOUSE_BYTE_OFFSET: i32 = 32;

/// parseMouseButton decodes the mouse button code.
fn parse_mouse_button(b: i32) -> (KeyMod, MouseButton, bool, bool) {
    // mouse bit shifts
    const BIT_SHIFT: i32 = 0b0000_0100;
    const BIT_ALT: i32 = 0b0000_1000;
    const BIT_CTRL: i32 = 0b0001_0000;
    const BIT_MOTION: i32 = 0b0010_0000;
    const BIT_WHEEL: i32 = 0b0100_0000;
    const BIT_ADD: i32 = 0b1000_0000; // additional buttons 8-11

    const BITS_MASK: i32 = 0b0000_0011;

    let mut mod_ = KeyMod(0);

    // Modifiers
    if b & BIT_ALT != 0 {
        mod_.0 |= MOD_ALT.0;
    }
    if b & BIT_CTRL != 0 {
        mod_.0 |= MOD_CTRL.0;
    }
    if b & BIT_SHIFT != 0 {
        mod_.0 |= MOD_SHIFT.0;
    }

    let mut is_release = false;
    let mut btn;
    if b & BIT_ADD != 0 {
        btn = MouseButton(MOUSE_BACKWARD.0 + (b & BITS_MASK) as u8);
    } else if b & BIT_WHEEL != 0 {
        btn = MouseButton(MOUSE_WHEEL_UP.0 + (b & BITS_MASK) as u8);
    } else {
        btn = MouseButton(MOUSE_LEFT.0 + (b & BITS_MASK) as u8);
        // X10 reports a button release as 0b0000_0011 (3)
        if b & BITS_MASK == BITS_MASK {
            btn = MouseButton(MOUSE_NONE.0);
            is_release = true;
        }
    }

    // Motion bit doesn't get reported for wheel events.
    let mut is_motion = false;
    if b & BIT_MOTION != 0 && !is_wheel(btn) {
        is_motion = true;
    }

    (mod_, btn, is_release, is_motion)
}

/// isWheel returns true if the mouse event is a wheel event.
fn is_wheel(btn: MouseButton) -> bool {
    btn.0 >= MOUSE_WHEEL_UP.0 && btn.0 <= MOUSE_WHEEL_RIGHT.0
}

/// parseTermcap parses XTGETTCAP responses.
fn parse_termcap(data: &[u8]) -> String {
    // XTGETTCAP
    if data.is_empty() {
        return String::new();
    }

    let mut tc = String::new();
    let split: Vec<&[u8]> = data.split(|&c| c == b';').collect();
    for s in split {
        let mut parts = s.splitn(2, |&c| c == b'=');
        let name_part = parts.next().unwrap_or(&[]);
        let value_part = parts.next();

        let name = hex_decode(name_part);
        match name {
            Some(name) if !name.is_empty() => {
                let mut value: Option<Vec<u8>> = None;
                if let Some(vp) = value_part {
                    match hex_decode(vp) {
                        Some(v) => value = Some(v),
                        None => continue,
                    }
                }

                if !tc.is_empty() {
                    tc.push(';');
                }
                tc.push_str(&String::from_utf8_lossy(&name));
                if let Some(v) = value {
                    if !v.is_empty() {
                        tc.push('=');
                        tc.push_str(&String::from_utf8_lossy(&v));
                    }
                }
            }
            _ => continue,
        }
    }

    tc
}

/// parseApcData parses APC sequence data.
fn parse_apc_data(b: &[u8]) -> Option<DecodedEvent> {
    if b.is_empty() {
        return None;
    }

    match b[0] {
        b'G' => {
            // Kitty Graphics Protocol
            let parts: Vec<&[u8]> = b[1..].split(|&c| c == b';').collect();
            let options = parts[0].to_vec();
            let payload = if parts.len() > 1 {
                parts[1].to_vec()
            } else {
                Vec::new()
            };
            Some(DecodedEvent::KittyGraphics { options, payload })
        }
        _ => None,
    }
}

/// parseXParseColor parses an XParseColor string.
fn parse_xparse_color(data: &[u8]) -> Option<rusty_x_ansi::color::RGBColor> {
    let s = std::str::from_utf8(data).unwrap_or("");
    rusty_x_ansi::util::x_parse_color(s)
}

/// hex_decode decodes a hex string.
fn hex_decode(b: &[u8]) -> Option<Vec<u8>> {
    let s = std::str::from_utf8(b).ok()?;
    let mut out = Vec::with_capacity(s.len() / 2);
    let bytes = s.as_bytes();
    let mut i = 0;
    while i + 1 < bytes.len() {
        let hi = hex_nibble(bytes[i])?;
        let lo = hex_nibble(bytes[i + 1])?;
        out.push((hi << 4) | lo);
        i += 2;
    }
    if i < bytes.len() {
        return None;
    }
    Some(out)
}

fn hex_nibble(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

/// base64_decode decodes a standard base64 string.
fn base64_decode(b: &[u8]) -> Option<Vec<u8>> {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = Vec::with_capacity(b.len() / 4 * 3);
    let mut buf: u32 = 0;
    let mut bits = 0u32;
    for &c in b {
        if c == b'=' {
            break;
        }
        let v = ALPHABET.iter().position(|&a| a == c)? as u32;
        buf = (buf << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    Some(out)
}

/// ensureKeyCase ensures that the key's text is in the correct case based on
/// the control key state.
fn ensure_key_case(key: Key, cks: u32) -> Key {
    if key.text.is_empty() {
        return key;
    }

    let mut key = key;
    let has_shift = cks & SHIFT_PRESSED != 0;
    let has_caps = cks & CAPSLOCK_ON != 0;
    if has_shift || has_caps {
        if let Some(c) = char::from_u32(key.code) {
            if c.is_lowercase() {
                key.shifted_code = c
                    .to_uppercase()
                    .collect::<String>()
                    .chars()
                    .next()
                    .unwrap_or(c) as u32;
                key.text = char::from_u32(key.shifted_code)
                    .map(|s| s.to_string())
                    .unwrap_or_default();
            }
        }
    } else if let Some(c) = char::from_u32(key.code) {
        if c.is_uppercase() {
            key.shifted_code = c
                .to_lowercase()
                .collect::<String>()
                .chars()
                .next()
                .unwrap_or(c) as u32;
            key.text = char::from_u32(key.shifted_code)
                .map(|s| s.to_string())
                .unwrap_or_default();
        }
    }

    key
}

/// translateControlKeyState translates the control key state from the
/// Windows Console API into a [KeyMod].
fn translate_control_key_state(cks: u32) -> KeyMod {
    let mut m = KeyMod(0);
    if cks & LEFT_CTRL_PRESSED != 0 || cks & RIGHT_CTRL_PRESSED != 0 {
        m.0 |= MOD_CTRL.0;
    }
    if cks & LEFT_ALT_PRESSED != 0 || cks & RIGHT_ALT_PRESSED != 0 {
        m.0 |= MOD_ALT.0;
    }
    if cks & SHIFT_PRESSED != 0 {
        m.0 |= MOD_SHIFT.0;
    }
    if cks & CAPSLOCK_ON != 0 {
        m.0 |= MOD_CAPS_LOCK.0;
    }
    if cks & NUMLOCK_ON != 0 {
        m.0 |= MOD_NUM_LOCK.0;
    }
    if cks & SCROLLLOCK_ON != 0 {
        m.0 |= MOD_SCROLL_LOCK.0;
    }
    m
}

// Windows virtual-key codes and control key state flags.
//
// NOTE: upstream uses `charmbracelet/x/windows`; the constants are defined
// locally until the x/windows module is ported.
const VK_BACK: u16 = 0x08;
const VK_TAB: u16 = 0x09;
const VK_RETURN: u16 = 0x0D;
const VK_SHIFT: u16 = 0x10;
const VK_CONTROL: u16 = 0x11;
const VK_MENU: u16 = 0x12;
const VK_PAUSE: u16 = 0x13;
const VK_CAPITAL: u16 = 0x14;
const VK_ESCAPE: u16 = 0x1B;
const VK_SPACE: u16 = 0x20;
const VK_PRIOR: u16 = 0x21;
const VK_NEXT: u16 = 0x22;
const VK_END: u16 = 0x23;
const VK_HOME: u16 = 0x24;
const VK_LEFT: u16 = 0x25;
const VK_UP: u16 = 0x26;
const VK_RIGHT: u16 = 0x27;
const VK_DOWN: u16 = 0x28;
const VK_SELECT: u16 = 0x29;
const VK_SNAPSHOT: u16 = 0x2C;
const VK_INSERT: u16 = 0x2D;
const VK_DELETE: u16 = 0x2E;
const VK_LWIN: u16 = 0x5B;
const VK_RWIN: u16 = 0x5C;
const VK_APPS: u16 = 0x5D;
const VK_MULTIPLY: u16 = 0x6A;
const VK_ADD: u16 = 0x6B;
const VK_SEPARATOR: u16 = 0x6C;
const VK_SUBTRACT: u16 = 0x6D;
const VK_DECIMAL: u16 = 0x6E;
const VK_DIVIDE: u16 = 0x6F;
const VK_NUMLOCK: u16 = 0x90;
const VK_SCROLL: u16 = 0x91;
const VK_LSHIFT: u16 = 0xA0;
const VK_RSHIFT: u16 = 0xA1;
const VK_LCONTROL: u16 = 0xA2;
const VK_RCONTROL: u16 = 0xA3;
const VK_LMENU: u16 = 0xA4;
const VK_RMENU: u16 = 0xA5;
const VK_VOLUME_MUTE: u16 = 0xAD;
const VK_VOLUME_DOWN: u16 = 0xAE;
const VK_VOLUME_UP: u16 = 0xAF;
const VK_MEDIA_NEXT_TRACK: u16 = 0xB0;
const VK_MEDIA_PREV_TRACK: u16 = 0xB1;
const VK_MEDIA_STOP: u16 = 0xB2;
const VK_MEDIA_PLAY_PAUSE: u16 = 0xB3;
const VK_OEM_1: u16 = 0xBA;
const VK_OEM_PLUS: u16 = 0xBB;
const VK_OEM_COMMA: u16 = 0xBC;
const VK_OEM_MINUS: u16 = 0xBD;
const VK_OEM_PERIOD: u16 = 0xBE;
const VK_OEM_2: u16 = 0xBF;
const VK_OEM_3: u16 = 0xC0;
const VK_OEM_4: u16 = 0xDB;
const VK_OEM_5: u16 = 0xDC;
const VK_OEM_6: u16 = 0xDD;
const VK_OEM_7: u16 = 0xDE;

const SHIFT_PRESSED: u32 = 0x0001;
const LEFT_CTRL_PRESSED: u32 = 0x0002;
const LEFT_ALT_PRESSED: u32 = 0x0004;
const RIGHT_CTRL_PRESSED: u32 = 0x0008;
const RIGHT_ALT_PRESSED: u32 = 0x0010;
const SCROLLLOCK_ON: u32 = 0x0040;
const NUMLOCK_ON: u32 = 0x0020;
const CAPSLOCK_ON: u32 = 0x0080;
const ENHANCED_KEY: u32 = 0x0100;

// Kitty protocol modifier bits.
const KITTY_SHIFT: i32 = 1 << 0;
const KITTY_ALT: i32 = 1 << 1;
const KITTY_CTRL: i32 = 1 << 2;
const KITTY_SUPER: i32 = 1 << 3;
const KITTY_HYPER: i32 = 1 << 4;
const KITTY_META: i32 = 1 << 5;
const KITTY_CAPS_LOCK: i32 = 1 << 6;
const KITTY_NUM_LOCK: i32 = 1 << 7;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::key::{
        KEY_BACKSPACE, KEY_CAPS_LOCK, KEY_DELETE, KEY_DOWN, KEY_END, KEY_ENTER, KEY_ESCAPE, KEY_F1,
        KEY_F11, KEY_F12, KEY_F13, KEY_F15, KEY_F17, KEY_F2, KEY_F20, KEY_F21, KEY_F3, KEY_F35,
        KEY_F5, KEY_HOME, KEY_INSERT, KEY_ISO_LEVEL3_SHIFT, KEY_ISO_LEVEL5_SHIFT, KEY_KP_0,
        KEY_KP_1, KEY_KP_2, KEY_KP_3, KEY_KP_4, KEY_KP_5, KEY_KP_6, KEY_KP_7, KEY_KP_8, KEY_KP_9,
        KEY_KP_BEGIN, KEY_KP_DECIMAL, KEY_KP_DELETE, KEY_KP_DIVIDE, KEY_KP_DOWN, KEY_KP_END,
        KEY_KP_ENTER, KEY_KP_EQUAL, KEY_KP_HOME, KEY_KP_INSERT, KEY_KP_LEFT, KEY_KP_MINUS,
        KEY_KP_MULTIPLY, KEY_KP_PG_DOWN, KEY_KP_PG_UP, KEY_KP_PLUS, KEY_KP_RIGHT, KEY_KP_SEP,
        KEY_KP_UP, KEY_LEFT, KEY_LEFT_ALT, KEY_LEFT_CTRL, KEY_LEFT_HYPER, KEY_LEFT_META,
        KEY_LEFT_SHIFT, KEY_LEFT_SUPER, KEY_MEDIA_NEXT, KEY_MEDIA_PAUSE, KEY_MEDIA_PLAY,
        KEY_MEDIA_PLAY_PAUSE, KEY_MEDIA_PREV, KEY_MEDIA_RECORD, KEY_MEDIA_REVERSE, KEY_MEDIA_STOP,
        KEY_MENU, KEY_MUTE, KEY_NUM_LOCK, KEY_PAUSE, KEY_PG_DOWN, KEY_PG_UP, KEY_PRINT_SCREEN,
        KEY_RIGHT, KEY_RIGHT_ALT, KEY_RIGHT_CTRL, KEY_RIGHT_HYPER, KEY_RIGHT_META, KEY_RIGHT_SHIFT,
        KEY_RIGHT_SUPER, KEY_SCROLL_LOCK, KEY_SPACE, KEY_TAB, KEY_UP,
    };
    use crate::mouse::MOUSE_NONE;

    fn decode_all(input: &[u8]) -> Vec<DecodedEvent> {
        let mut d = EventDecoder::default();
        let mut out = Vec::new();
        let mut rest = input;
        while !rest.is_empty() {
            let (n, ev) = d.decode(rest);
            if let Some(ev) = ev {
                out.push(ev);
            }
            if n == 0 {
                break;
            }
            rest = &rest[n..];
        }
        out
    }

    #[test]
    fn test_decode_ascii() {
        let events = decode_all(b"abc");
        assert_eq!(events.len(), 3);
        assert_eq!(
            events[0],
            DecodedEvent::KeyPress(Key {
                code: b'a' as u32,
                text: "a".to_string(),
                ..Key::default()
            })
        );
    }

    #[test]
    fn test_decode_uppercase() {
        let events = decode_all(b"A");
        assert_eq!(events.len(), 1);
        match &events[0] {
            DecodedEvent::KeyPress(k) => {
                assert_eq!(k.code, b'a' as u32);
                assert_eq!(k.text, "A");
                assert_eq!(k.mod_, MOD_SHIFT);
                assert_eq!(k.shifted_code, b'A' as u32);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn test_decode_arrow_keys() {
        let events = decode_all(b"\x1b[A\x1b[B\x1b[C\x1b[D");
        let codes: Vec<u32> = events
            .iter()
            .filter_map(|e| match e {
                DecodedEvent::KeyPress(k) => Some(k.code),
                _ => None,
            })
            .collect();
        assert_eq!(codes, vec![KEY_UP, KEY_DOWN, KEY_RIGHT, KEY_LEFT]);
    }

    #[test]
    fn test_decode_escape() {
        let events = decode_all(b"\x1b");
        assert_eq!(events.len(), 1);
        match &events[0] {
            DecodedEvent::KeyPress(k) => assert_eq!(k.code, KEY_ESCAPE),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn test_decode_alt_key() {
        let events = decode_all(b"\x1bx");
        assert_eq!(events.len(), 1);
        match &events[0] {
            DecodedEvent::KeyPress(k) => {
                assert_eq!(k.code, b'x' as u32);
                assert_eq!(k.mod_, MOD_ALT);
                assert!(k.text.is_empty());
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn test_decode_control() {
        let events = decode_all(b"\x05");
        match &events[0] {
            DecodedEvent::KeyPress(k) => {
                assert_eq!(k.code, b'e' as u32);
                assert_eq!(k.mod_, MOD_CTRL);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn test_decode_ctrl_space_and_tab() {
        let events = decode_all(b"\x00\x09\x0d");
        assert_eq!(
            events[0],
            DecodedEvent::KeyPress(Key {
                code: KEY_SPACE,
                mod_: MOD_CTRL,
                ..Key::default()
            })
        );
        assert_eq!(
            events[1],
            DecodedEvent::KeyPress(Key {
                code: KEY_TAB,
                ..Key::default()
            })
        );
        assert_eq!(
            events[2],
            DecodedEvent::KeyPress(Key {
                code: KEY_ENTER,
                ..Key::default()
            })
        );
    }

    #[test]
    fn test_decode_legacy_flags() {
        let mut d = EventDecoder {
            legacy: LegacyKeyEncoding(0).ctrl_i(true).ctrl_m(true).ctrl_at(true),
            ..Default::default()
        };
        let (_, ev) = d.decode(b"\x00");
        assert_eq!(
            ev,
            Some(DecodedEvent::KeyPress(Key {
                code: b'@' as u32,
                mod_: MOD_CTRL,
                ..Key::default()
            }))
        );
        let (_, ev) = d.decode(b"\x09");
        assert_eq!(
            ev,
            Some(DecodedEvent::KeyPress(Key {
                code: b'i' as u32,
                mod_: MOD_CTRL,
                ..Key::default()
            }))
        );
        let (_, ev) = d.decode(b"\x0d");
        assert_eq!(
            ev,
            Some(DecodedEvent::KeyPress(Key {
                code: b'm' as u32,
                mod_: MOD_CTRL,
                ..Key::default()
            }))
        );
    }

    #[test]
    fn test_decode_csi_keys() {
        let events = decode_all(b"\x1b[5~\x1b[6~\x1b[3~\x1b[2~\x1b[1;5D");
        let codes: Vec<u32> = events
            .iter()
            .filter_map(|e| match e {
                DecodedEvent::KeyPress(k) => Some(k.code),
                _ => None,
            })
            .collect();
        assert_eq!(
            codes,
            vec![KEY_PG_UP, KEY_PG_DOWN, KEY_DELETE, KEY_INSERT, KEY_LEFT]
        );
        // Last one has ctrl modifier (modifier 5 - 1 = 4 = ctrl).
        if let Some(DecodedEvent::KeyPress(k)) = events.last() {
            assert_eq!(k.mod_, MOD_CTRL);
        }
    }

    #[test]
    fn test_decode_sgr_mouse() {
        let events = decode_all(b"\x1b[<0;10;20M\x1b[<0;10;20m\x1b[<64;10;20M");
        assert_eq!(
            events[0],
            DecodedEvent::MouseClick(Mouse {
                x: 9,
                y: 19,
                button: MOUSE_LEFT,
                mod_: KeyMod(0),
            })
        );
        assert_eq!(
            events[1],
            DecodedEvent::MouseRelease(Mouse {
                x: 9,
                y: 19,
                button: MOUSE_LEFT,
                mod_: KeyMod(0),
            })
        );
        assert_eq!(
            events[2],
            DecodedEvent::MouseWheel(Mouse {
                x: 9,
                y: 19,
                button: MOUSE_WHEEL_UP,
                mod_: KeyMod(0),
            })
        );
    }

    #[test]
    fn test_decode_x10_mouse() {
        let events = decode_all(b"\x1b[M \x00\x00\x1b[M#\x00\x00");
        // Button 0 (space) click; button 3 (#) release. Coordinates are
        // 0x00 - 32 - 1 = -33, matching the upstream formula.
        assert_eq!(
            events[0],
            DecodedEvent::MouseClick(Mouse {
                x: -33,
                y: -33,
                button: MOUSE_LEFT,
                mod_: KeyMod(0),
            })
        );
        assert_eq!(
            events[1],
            DecodedEvent::MouseRelease(Mouse {
                x: -33,
                y: -33,
                button: MOUSE_NONE,
                mod_: KeyMod(0),
            })
        );
    }

    #[test]
    fn test_decode_focus() {
        let events = decode_all(b"\x1b[I\x1b[O");
        assert_eq!(events[0], DecodedEvent::Focus);
        assert_eq!(events[1], DecodedEvent::Blur);
    }

    #[test]
    fn test_decode_osc_paste() {
        let events = decode_all(b"\x1b]52;c;aGVsbG8=\x07");
        match &events[0] {
            DecodedEvent::Clipboard { content, selection } => {
                assert_eq!(content, "hello");
                assert_eq!(*selection, b'c');
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn test_decode_bracketed_paste() {
        let events = decode_all(b"\x1b[200~hello\x1b[201~");
        assert_eq!(events[0], DecodedEvent::PasteStart);
        let text: String = events[1..6]
            .iter()
            .filter_map(|e| match e {
                DecodedEvent::KeyPress(k) => Some(k.text.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(text, "hello");
        assert_eq!(events[6], DecodedEvent::PasteEnd);
    }

    #[test]
    fn test_decode_kitty_keyboard() {
        let events = decode_all(b"\x1b[97u");
        match &events[0] {
            DecodedEvent::KeyPress(k) => {
                assert_eq!(k.code, b'a' as u32);
                assert_eq!(k.text, "a");
            }
            other => panic!("unexpected: {other:?}"),
        }
        // With modifiers: 97;5u = ctrl+a.
        let events = decode_all(b"\x1b[97;5u");
        match &events[0] {
            DecodedEvent::KeyPress(k) => {
                assert_eq!(k.code, b'a' as u32);
                assert_eq!(k.mod_, MOD_CTRL);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn test_decode_kitty_release() {
        // Event type 3 is the second sub-parameter of the modifier parameter.
        let events = decode_all(b"\x1b[97;3:3u");
        match &events[0] {
            DecodedEvent::KeyRelease(k) => assert_eq!(k.code, b'a' as u32),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn test_decode_window_size() {
        let events = decode_all(b"\x1b[8;24;80t");
        assert_eq!(
            events[0],
            DecodedEvent::WindowSize(Size {
                width: 80,
                height: 24,
            })
        );
    }

    #[test]
    fn test_decode_cursor_position() {
        let events = decode_all(b"\x1b[5;10R");
        assert_eq!(events[0], DecodedEvent::CursorPosition { x: 9, y: 4 });
    }

    #[test]
    fn test_decode_mode_report() {
        let events = decode_all(b"\x1b[?25;1$y");
        assert_eq!(events[0], DecodedEvent::ModeReport { mode: 25, value: 1 });
    }

    #[test]
    fn test_decode_color_report() {
        let events = decode_all(b"\x1b]11;rgb:ffff/0000/0000\x07");
        match &events[0] {
            DecodedEvent::BackgroundColor(Some(c)) => {
                assert_eq!(c.r, 255);
                assert_eq!(c.g, 0);
                assert_eq!(c.b, 0);
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn test_decode_terminfo_version() {
        let events = decode_all(b"\x1bP>|Alacritty 0.14.0\x1b\\");
        match &events[0] {
            DecodedEvent::TerminalVersion(name) => assert_eq!(name, "Alacritty 0.14.0"),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn test_decode_ss3() {
        let events = decode_all(b"\x1bOP\x1bOQ\x1bOM");
        let codes: Vec<u32> = events
            .iter()
            .filter_map(|e| match e {
                DecodedEvent::KeyPress(k) => Some(k.code),
                _ => None,
            })
            .collect();
        assert_eq!(codes, vec![KEY_F1, KEY_F2, KEY_KP_ENTER]);
    }

    #[test]
    fn test_decode_utf8() {
        let events = decode_all("界".as_bytes());
        match &events[0] {
            DecodedEvent::KeyPress(k) => {
                assert_eq!(k.code, '界' as u32);
                assert_eq!(k.text, "界");
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn test_hex_decode() {
        assert_eq!(hex_decode(b"5463"), Some(vec![0x54, 0x63]));
        assert_eq!(hex_decode(b"zz"), None);
    }

    #[test]
    fn test_base64_decode() {
        assert_eq!(base64_decode(b"aGVsbG8="), Some(b"hello".to_vec()));
        assert_eq!(base64_decode(b"aGk="), Some(b"hi".to_vec()));
    }

    /// Ported from upstream `TestParseControl`.
    #[test]
    fn test_parse_control_table() {
        let cases: Vec<(u8, LegacyKeyEncoding, DecodedEvent)> = vec![
            // NUL with/without CtrlAt.
            (
                0x00,
                LegacyKeyEncoding(FLAG_CTRL_AT),
                DecodedEvent::KeyPress(Key {
                    code: b'@' as u32,
                    mod_: MOD_CTRL,
                    ..Key::default()
                }),
            ),
            (
                0x00,
                LegacyKeyEncoding(0),
                DecodedEvent::KeyPress(Key {
                    code: KEY_SPACE,
                    mod_: MOD_CTRL,
                    ..Key::default()
                }),
            ),
            // BS.
            (
                0x08,
                LegacyKeyEncoding(0),
                DecodedEvent::KeyPress(Key {
                    code: b'h' as u32,
                    mod_: MOD_CTRL,
                    ..Key::default()
                }),
            ),
            // HT with/without CtrlI.
            (
                0x09,
                LegacyKeyEncoding(FLAG_CTRL_I),
                DecodedEvent::KeyPress(Key {
                    code: b'i' as u32,
                    mod_: MOD_CTRL,
                    ..Key::default()
                }),
            ),
            (
                0x09,
                LegacyKeyEncoding(0),
                DecodedEvent::KeyPress(Key {
                    code: KEY_TAB,
                    ..Key::default()
                }),
            ),
            // CR with/without CtrlM.
            (
                0x0D,
                LegacyKeyEncoding(FLAG_CTRL_M),
                DecodedEvent::KeyPress(Key {
                    code: b'm' as u32,
                    mod_: MOD_CTRL,
                    ..Key::default()
                }),
            ),
            (
                0x0D,
                LegacyKeyEncoding(0),
                DecodedEvent::KeyPress(Key {
                    code: KEY_ENTER,
                    ..Key::default()
                }),
            ),
            // ESC with/without CtrlOpenBracket.
            (
                0x1B,
                LegacyKeyEncoding(FLAG_CTRL_OPEN_BRACKET),
                DecodedEvent::KeyPress(Key {
                    code: b'[' as u32,
                    mod_: MOD_CTRL,
                    ..Key::default()
                }),
            ),
            (
                0x1B,
                LegacyKeyEncoding(0),
                DecodedEvent::KeyPress(Key {
                    code: KEY_ESCAPE,
                    ..Key::default()
                }),
            ),
            // DEL with/without Backspace.
            (
                0x7F,
                LegacyKeyEncoding(FLAG_BACKSPACE),
                DecodedEvent::KeyPress(Key {
                    code: KEY_DELETE,
                    ..Key::default()
                }),
            ),
            (
                0x7F,
                LegacyKeyEncoding(0),
                DecodedEvent::KeyPress(Key {
                    code: KEY_BACKSPACE,
                    ..Key::default()
                }),
            ),
            // Space.
            (
                0x20,
                LegacyKeyEncoding(0),
                DecodedEvent::KeyPress(Key {
                    code: KEY_SPACE,
                    text: " ".to_string(),
                    ..Key::default()
                }),
            ),
            // Control letters.
            (
                0x01,
                LegacyKeyEncoding(0),
                DecodedEvent::KeyPress(Key {
                    code: b'a' as u32,
                    mod_: MOD_CTRL,
                    ..Key::default()
                }),
            ),
            (
                0x1A,
                LegacyKeyEncoding(0),
                DecodedEvent::KeyPress(Key {
                    code: b'z' as u32,
                    mod_: MOD_CTRL,
                    ..Key::default()
                }),
            ),
            // FS, US.
            (
                0x1C,
                LegacyKeyEncoding(0),
                DecodedEvent::KeyPress(Key {
                    code: b'\\' as u32,
                    mod_: MOD_CTRL,
                    ..Key::default()
                }),
            ),
            (
                0x1F,
                LegacyKeyEncoding(0),
                DecodedEvent::KeyPress(Key {
                    code: b'_' as u32,
                    mod_: MOD_CTRL,
                    ..Key::default()
                }),
            ),
            // Unknown control.
            (
                0x80,
                LegacyKeyEncoding(0),
                DecodedEvent::Unknown("\u{80}".to_string()),
            ),
        ];
        for (input, legacy, want) in cases {
            let p = EventDecoder {
                legacy,
                ..EventDecoder::default()
            };
            let got = p.parse_control(input);
            assert_eq!(got, want, "parse_control(0x{input:02x}) with {legacy:?}");
        }
    }

    /// LegacyKeyEncoding flag helpers (ported from upstream `TestLegacyKeyEncodingMethods`).
    #[test]
    fn test_legacy_key_encoding_methods() {
        let base = LegacyKeyEncoding(0);
        assert_eq!(base.ctrl_at(true), LegacyKeyEncoding(FLAG_CTRL_AT));
        assert_eq!(base.ctrl_at(false), LegacyKeyEncoding(0));
        assert_eq!(base.ctrl_i(true), LegacyKeyEncoding(FLAG_CTRL_I));
        assert_eq!(base.ctrl_m(true), LegacyKeyEncoding(FLAG_CTRL_M));
        assert_eq!(
            base.ctrl_open_bracket(true),
            LegacyKeyEncoding(FLAG_CTRL_OPEN_BRACKET)
        );
        assert_eq!(base.backspace(true), LegacyKeyEncoding(FLAG_BACKSPACE));
        assert_eq!(base.find(true), LegacyKeyEncoding(FLAG_FIND));
        assert_eq!(base.select(true), LegacyKeyEncoding(FLAG_SELECT));
        assert_eq!(base.f_keys(true), LegacyKeyEncoding(FLAG_F_KEYS));
        // Turning off an already-set flag.
        assert_eq!(base.backspace(true).backspace(false), LegacyKeyEncoding(0));
    }

    /// Ported from upstream `TestDeviceAttributesParsing`.
    #[test]
    fn test_device_attributes() {
        let ev = parse_primary_dev_attrs(&[62, 1, 2, 6, 9]);
        assert_eq!(
            ev,
            DecodedEvent::PrimaryDeviceAttributes(vec![62, 1, 2, 6, 9])
        );
        let ev = parse_secondary_dev_attrs(&[1, 2, 3]);
        assert_eq!(ev, DecodedEvent::SecondaryDeviceAttributes(vec![1, 2, 3]));
        let ev = parse_tertiary_dev_attrs(b"4368726d");
        assert_eq!(
            ev,
            DecodedEvent::TertiaryDeviceAttributes("Chrm".to_string())
        );
    }

    /// Ported from upstream `TestParseTermcap`.
    #[test]
    fn test_parse_termcap() {
        assert_eq!(parse_termcap(b"524742"), "RGB");
        assert_eq!(parse_termcap(b"436F=323536"), "Co=256");
        assert_eq!(parse_termcap(b""), "");
        assert_eq!(parse_termcap(b"GGGG"), "");
        assert_eq!(parse_termcap(b"52474"), "");
    }

    /// Ported from upstream `TestParseUtf8`.
    #[test]
    fn test_parse_utf8_table() {
        let mut p = EventDecoder::default();
        // Empty input.
        assert_eq!(p.parse_utf8(b""), (0, None));
        // Control character (SOH) -> ctrl+a.
        let (n, ev) = p.parse_utf8(b"\x01");
        assert_eq!(n, 1);
        assert_eq!(
            ev,
            Some(DecodedEvent::KeyPress(Key {
                code: b'a' as u32,
                mod_: MOD_CTRL,
                ..Key::default()
            }))
        );
        // ASCII printable.
        let (n, ev) = p.parse_utf8(b"a");
        assert_eq!(n, 1);
        assert_eq!(
            ev,
            Some(DecodedEvent::KeyPress(Key {
                code: b'a' as u32,
                text: "a".to_string(),
                ..Key::default()
            }))
        );
        // Uppercase.
        let (n, ev) = p.parse_utf8(b"A");
        assert_eq!(n, 1);
        assert_eq!(
            ev,
            Some(DecodedEvent::KeyPress(Key {
                code: b'a' as u32,
                shifted_code: b'A' as u32,
                text: "A".to_string(),
                mod_: MOD_SHIFT,
                ..Key::default()
            }))
        );
        // DEL.
        let (n, ev) = p.parse_utf8(b"\x7f");
        assert_eq!(n, 1);
        assert_eq!(
            ev,
            Some(DecodedEvent::KeyPress(Key {
                code: KEY_BACKSPACE,
                ..Key::default()
            }))
        );
        // Multi-byte UTF-8.
        let (n, ev) = p.parse_utf8("€".as_bytes());
        assert_eq!(n, 3);
        assert_eq!(
            ev,
            Some(DecodedEvent::KeyPress(Key {
                code: '€' as u32,
                text: "€".to_string(),
                ..Key::default()
            }))
        );
        // Invalid UTF-8: the port substitutes U+FFFD (a 3-byte cluster).
        let (n, ev) = p.parse_utf8(b"\xff");
        assert_eq!(n, 3);
        assert_eq!(
            ev,
            Some(DecodedEvent::KeyPress(Key {
                code: 0xFFFD,
                text: "\u{FFFD}".to_string(),
                ..Key::default()
            }))
        );
    }

    /// The `parse_apc_data` helpers for xparse-color and hex/base64 decode.
    #[test]
    fn test_parse_apc_helpers() {
        // Hex decoding.
        assert_eq!(hex_decode(b"616263"), Some(b"abc".to_vec()));
        assert_eq!(hex_decode(b"a"), None);
        assert_eq!(hex_decode(b"zz"), None);
        assert_eq!(hex_nibble(b'f'), Some(15));
        assert_eq!(hex_nibble(b'F'), Some(15));
        assert_eq!(hex_nibble(b'g'), None);
        assert_eq!(hex_nibble(b'0'), Some(0));
    }

    /// CSI report branches: device attributes, kitty flags, modifyOtherKeys,
    /// color scheme, focus/blur, and mode reports without a prefix.
    #[test]
    fn test_decode_csi_reports() {
        // Primary Device Attributes.
        let events = decode_all(b"\x1b[?62;1;2;6;9c");
        assert_eq!(
            events[0],
            DecodedEvent::PrimaryDeviceAttributes(vec![62, 1, 2, 6, 9])
        );
        // Secondary Device Attributes.
        let events = decode_all(b"\x1b[>1;2;3c");
        assert_eq!(
            events[0],
            DecodedEvent::SecondaryDeviceAttributes(vec![1, 2, 3])
        );
        // Kitty keyboard flags.
        let events = decode_all(b"\x1b[?1u");
        assert_eq!(events[0], DecodedEvent::KeyboardEnhancements(1));
        // XTerm modifyOtherKeys report.
        let events = decode_all(b"\x1b[>4;2m");
        assert_eq!(events[0], DecodedEvent::ModifyOtherKeys(2));
        // Dark/light color scheme.
        let events = decode_all(b"\x1b[?997;1n");
        assert_eq!(events[0], DecodedEvent::DarkColorScheme);
        let events = decode_all(b"\x1b[?997;2n");
        assert_eq!(events[0], DecodedEvent::LightColorScheme);
        // Focus and blur.
        let events = decode_all(b"\x1b[I");
        assert_eq!(events[0], DecodedEvent::Focus);
        let events = decode_all(b"\x1b[O");
        assert_eq!(events[0], DecodedEvent::Blur);
        // DECRPM without '?' prefix.
        let events = decode_all(b"\x1b[25;2$y");
        assert_eq!(events[0], DecodedEvent::ModeReport { mode: 25, value: 2 });
    }

    /// CSI key sequences: home/end/insert/delete/PgUp/PgDn/F-keys with
    /// modifiers, and URxvt/kitty extensions.
    #[test]
    fn test_decode_csi_keys_extended() {
        // Home/End via ~ params.
        let events = decode_all(b"\x1b[1~");
        assert_eq!(
            events[0],
            DecodedEvent::KeyPress(Key {
                code: KEY_HOME,
                ..Key::default()
            })
        );
        let events = decode_all(b"\x1b[4~");
        assert_eq!(
            events[0],
            DecodedEvent::KeyPress(Key {
                code: KEY_END,
                ..Key::default()
            })
        );
        // Insert/Delete.
        let events = decode_all(b"\x1b[2~");
        assert_eq!(
            events[0],
            DecodedEvent::KeyPress(Key {
                code: KEY_INSERT,
                ..Key::default()
            })
        );
        let events = decode_all(b"\x1b[3~");
        assert_eq!(
            events[0],
            DecodedEvent::KeyPress(Key {
                code: KEY_DELETE,
                ..Key::default()
            })
        );
        // PageUp/PageDown.
        let events = decode_all(b"\x1b[5~");
        assert_eq!(
            events[0],
            DecodedEvent::KeyPress(Key {
                code: KEY_PG_UP,
                ..Key::default()
            })
        );
        let events = decode_all(b"\x1b[6~");
        assert_eq!(
            events[0],
            DecodedEvent::KeyPress(Key {
                code: KEY_PG_DOWN,
                ..Key::default()
            })
        );
        // F-keys via ~ params.
        let events = decode_all(b"\x1b[11~");
        assert_eq!(
            events[0],
            DecodedEvent::KeyPress(Key {
                code: KEY_F1,
                ..Key::default()
            })
        );
        let events = decode_all(b"\x1b[15~");
        assert_eq!(
            events[0],
            DecodedEvent::KeyPress(Key {
                code: KEY_F5,
                ..Key::default()
            })
        );
        let events = decode_all(b"\x1b[23~");
        assert_eq!(
            events[0],
            DecodedEvent::KeyPress(Key {
                code: KEY_F11,
                ..Key::default()
            })
        );
        let events = decode_all(b"\x1b[28~");
        assert_eq!(
            events[0],
            DecodedEvent::KeyPress(Key {
                code: KEY_F15,
                ..Key::default()
            })
        );
        let events = decode_all(b"\x1b[31~");
        assert_eq!(
            events[0],
            DecodedEvent::KeyPress(Key {
                code: KEY_F17,
                ..Key::default()
            })
        );
        // URxvt ^/@ shifted variants set CTRL and CTRL+SHIFT.
        let events = decode_all(b"\x1b[5^");
        assert!(
            matches!(&events[0], DecodedEvent::KeyPress(k) if k.code == KEY_PG_UP && k.mod_ == KeyMod(MOD_CTRL.0))
        );
        let events = decode_all(b"\x1b[5@");
        assert!(
            matches!(&events[0], DecodedEvent::KeyPress(k) if k.code == KEY_PG_UP && k.mod_ == KeyMod(MOD_CTRL.0 | MOD_SHIFT.0))
        );
        // Modified F3 (CSI 1 ; <mod> R) also emits a cursor position report.
        let events = decode_all(b"\x1b[1;2R");
        assert!(matches!(&events[0], DecodedEvent::Multi(ref m)
            if matches!(&m[0], DecodedEvent::KeyPress(k) if k.code == KEY_F3 && k.mod_ == KeyMod(1))));
    }

    /// Window operation reports: pixel, cell, and multi-size reports.
    #[test]
    fn test_decode_window_op_reports() {
        // Pixel size.
        let events = decode_all(b"\x1b[4;24;80t");
        assert_eq!(
            events[0],
            DecodedEvent::PixelSize(Size {
                width: 80,
                height: 24,
            })
        );
        // Cell size.
        let events = decode_all(b"\x1b[6;24;80t");
        assert_eq!(
            events[0],
            DecodedEvent::CellSize(Size {
                width: 80,
                height: 24,
            })
        );
        // Window + pixel report (48 params).
        let events = decode_all(b"\x1b[48;24;80;1440;900t");
        assert_eq!(
            events[0],
            DecodedEvent::Multi(vec![
                DecodedEvent::WindowSize(Size {
                    width: 80,
                    height: 24,
                }),
                DecodedEvent::PixelSize(Size {
                    width: 900,
                    height: 1440,
                }),
            ])
        );
        // Other window ops (e.g. report position) produce WindowOp.
        let events = decode_all(b"\x1b[3;0;0t");
        assert_eq!(
            events[0],
            DecodedEvent::WindowOp {
                op: 3,
                args: vec![0, 0]
            }
        );
    }

    /// Error/invalid CSI reports.
    #[test]
    fn test_decode_csi_invalid() {
        // Invalid DECRPM (no mode).
        let events = decode_all(b"\x1b[?$y");
        assert!(matches!(events[0], DecodedEvent::UnknownCsi(_)));
        // Invalid cursor position (missing col).
        let events = decode_all(b"\x1b[?5R");
        assert!(matches!(events[0], DecodedEvent::UnknownCsi(_)));
        // modifyOtherKeys with wrong mode.
        let events = decode_all(b"\x1b[>3;2m");
        assert!(matches!(events[0], DecodedEvent::UnknownCsi(_)));
        // Empty CSI u is unknown.
        let events = decode_all(b"\x1b[u");
        assert!(matches!(events[0], DecodedEvent::UnknownCsi(_)));
    }

    /// Direct `kitty_key_map` coverage for every mapped code.
    #[test]
    fn test_kitty_key_map() {
        // Faulty C0 mappings.
        assert_eq!(
            kitty_key_map(0x00),
            Some(Key {
                code: KEY_SPACE,
                mod_: MOD_CTRL,
                ..Key::default()
            })
        );
        for c in 0x01..=0x1A {
            assert_eq!(
                kitty_key_map(c),
                Some(Key {
                    code: (c + 0x60) as u32,
                    mod_: MOD_CTRL,
                    ..Key::default()
                })
            );
        }
        for c in 0x1C..=0x1F {
            assert_eq!(
                kitty_key_map(c),
                Some(Key {
                    code: (c + 0x40) as u32,
                    mod_: MOD_CTRL,
                    ..Key::default()
                })
            );
        }
        // 0x08 is within the ctrl-letter range (0x01..=0x1A) and maps first.
        assert_eq!(
            kitty_key_map(0x08),
            Some(Key {
                code: b'h' as u32,
                mod_: MOD_CTRL,
                ..Key::default()
            })
        );
        assert_eq!(kitty_key_map(0x7F).unwrap().code, KEY_BACKSPACE);
        // Functional key codes (57344+).
        let pairs: &[(i32, u32)] = &[
            (57344, KEY_ESCAPE),
            (57345, KEY_ENTER),
            (57346, KEY_TAB),
            (57347, KEY_BACKSPACE),
            (57348, KEY_INSERT),
            (57349, KEY_DELETE),
            (57350, KEY_LEFT),
            (57351, KEY_RIGHT),
            (57352, KEY_UP),
            (57353, KEY_DOWN),
            (57354, KEY_PG_UP),
            (57355, KEY_PG_DOWN),
            (57356, KEY_HOME),
            (57357, KEY_END),
            (57358, KEY_CAPS_LOCK),
            (57359, KEY_SCROLL_LOCK),
            (57360, KEY_NUM_LOCK),
            (57361, KEY_PRINT_SCREEN),
            (57362, KEY_PAUSE),
            (57363, KEY_MENU),
            (57364, KEY_F1),
            (57375, KEY_F12),
            (57376, KEY_F13),
            (57383, KEY_F20),
            (57384, KEY_F21),
            (57398, KEY_F35),
            (57399, KEY_KP_0),
            (57400, KEY_KP_1),
            (57401, KEY_KP_2),
            (57402, KEY_KP_3),
            (57403, KEY_KP_4),
            (57404, KEY_KP_5),
            (57405, KEY_KP_6),
            (57406, KEY_KP_7),
            (57407, KEY_KP_8),
            (57408, KEY_KP_9),
            (57409, KEY_KP_DECIMAL),
            (57410, KEY_KP_DIVIDE),
            (57411, KEY_KP_MULTIPLY),
            (57412, KEY_KP_MINUS),
            (57413, KEY_KP_PLUS),
            (57414, KEY_KP_ENTER),
            (57415, KEY_KP_EQUAL),
            (57416, KEY_KP_SEP),
            (57417, KEY_KP_LEFT),
            (57418, KEY_KP_RIGHT),
            (57419, KEY_KP_UP),
            (57420, KEY_KP_DOWN),
            (57421, KEY_KP_PG_UP),
            (57422, KEY_KP_PG_DOWN),
            (57423, KEY_KP_HOME),
            (57424, KEY_KP_END),
            (57425, KEY_KP_INSERT),
            (57426, KEY_KP_DELETE),
            (57427, KEY_KP_BEGIN),
            (57428, KEY_MEDIA_PLAY),
            (57429, KEY_MEDIA_PAUSE),
            (57430, KEY_MEDIA_PLAY_PAUSE),
            (57431, KEY_MEDIA_REVERSE),
            (57432, KEY_MEDIA_STOP),
            (57433, KEY_MEDIA_FAST_FORWARD),
            (57434, KEY_MEDIA_REWIND),
            (57435, KEY_MEDIA_NEXT),
            (57436, KEY_MEDIA_PREV),
            (57437, KEY_MEDIA_RECORD),
            (57438, KEY_LOWER_VOL),
            (57439, KEY_RAISE_VOL),
            (57440, KEY_MUTE),
            (57441, KEY_LEFT_SHIFT),
            (57442, KEY_LEFT_CTRL),
            (57443, KEY_LEFT_ALT),
            (57444, KEY_LEFT_SUPER),
            (57445, KEY_LEFT_HYPER),
            (57446, KEY_LEFT_META),
            (57447, KEY_RIGHT_SHIFT),
            (57448, KEY_RIGHT_CTRL),
            (57449, KEY_RIGHT_ALT),
            (57450, KEY_RIGHT_SUPER),
            (57451, KEY_RIGHT_HYPER),
            (57452, KEY_RIGHT_META),
            (57453, KEY_ISO_LEVEL3_SHIFT),
            (57454, KEY_ISO_LEVEL5_SHIFT),
        ];
        for &(code, want) in pairs {
            let k = kitty_key_map(code);
            assert!(k.is_some(), "code {code}");
            assert_eq!(k.unwrap().code, want, "code {code}");
        }
        // Unmapped codes.
        assert_eq!(kitty_key_map(99999), None);
        assert_eq!(kitty_key_map(0x1B), kitty_key_map(0x1B));
    }
}
