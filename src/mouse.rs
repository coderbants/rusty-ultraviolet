//! Cleanroom Rust port of upstream Go source file: `mouse.go`
//! Upstream Target Tag / Version: `v0.0.0-20260703014108-f5a850f9c2b7`
//!
//! <public-docs>
//! The mouse event model: tracking modes, encodings, buttons, and the `Mouse`
//! message with its string representation.
//! </public-docs>

use crate::console::Winsize;
use crate::key::{KeyMod, MOD_ALT, MOD_CTRL, MOD_SHIFT};
use rusty_x_ansi::mouse::MouseButton;

/// MouseMode represents the mouse tracking mode for the terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseMode {
    /// MouseModeNone disables mouse tracking.
    MouseModeNone,
    /// MouseModePress is press only (DEC mode 9).
    MouseModePress,
    /// MouseModeClick is click tracking (DEC mode 1000).
    MouseModeClick,
    /// MouseModeDrag is drag tracking (DEC mode 1002).
    MouseModeDrag,
    /// MouseModeMotion is motion tracking (DEC mode 1003).
    MouseModeMotion,
}

/// MouseEncoding represents the encoding used for mouse events.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseEncoding {
    /// MouseEncodingLegacy is the legacy X10-compatible encoding.
    MouseEncodingLegacy,
    /// MouseEncodingSGR is the SGR encoding (DEC mode 1006).
    MouseEncodingSGR,
    /// MouseEncodingSGRPixel is the SGR-pixel encoding (DEC mode 1016).
    MouseEncodingSGRPixel,
}

/// Mouse event buttons.
///
/// This is based on X11 mouse button codes.
pub const MOUSE_NONE: MouseButton = rusty_x_ansi::mouse::MOUSE_NONE;
/// Left button.
pub const MOUSE_LEFT: MouseButton = rusty_x_ansi::mouse::MOUSE_BUTTON_1;
/// Middle button (pressing the scroll wheel).
pub const MOUSE_MIDDLE: MouseButton = rusty_x_ansi::mouse::MOUSE_BUTTON_2;
/// Right button.
pub const MOUSE_RIGHT: MouseButton = rusty_x_ansi::mouse::MOUSE_BUTTON_3;
/// Turn scroll wheel up.
pub const MOUSE_WHEEL_UP: MouseButton = rusty_x_ansi::mouse::MOUSE_BUTTON_4;
/// Turn scroll wheel down.
pub const MOUSE_WHEEL_DOWN: MouseButton = rusty_x_ansi::mouse::MOUSE_BUTTON_5;
/// Push scroll wheel left.
pub const MOUSE_WHEEL_LEFT: MouseButton = rusty_x_ansi::mouse::MOUSE_BUTTON_6;
/// Push scroll wheel right.
pub const MOUSE_WHEEL_RIGHT: MouseButton = rusty_x_ansi::mouse::MOUSE_BUTTON_7;
/// 4th button (aka browser backward button).
pub const MOUSE_BACKWARD: MouseButton = rusty_x_ansi::mouse::MOUSE_BUTTON_8;
/// 5th button (aka browser forward button).
pub const MOUSE_FORWARD: MouseButton = rusty_x_ansi::mouse::MOUSE_BUTTON_9;
/// Button 10.
pub const MOUSE_BUTTON_10: MouseButton = rusty_x_ansi::mouse::MOUSE_BUTTON_10;
/// Button 11.
pub const MOUSE_BUTTON_11: MouseButton = rusty_x_ansi::mouse::MOUSE_BUTTON_11;

/// Mouse represents a Mouse message.
///
/// The X and Y coordinates are zero-based, with (0,0) being the upper left
/// corner of the terminal. When using [MouseEncoding::MouseEncodingSGRPixel]
/// (DEC mode 1016), X and Y are in pixel coordinates; otherwise they are in
/// cell coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mouse {
    /// The X coordinate.
    pub x: i32,
    /// The Y coordinate.
    pub y: i32,
    /// The button that was pressed.
    pub button: MouseButton,
    /// The modifier keys pressed.
    pub mod_: KeyMod,
}

impl Mouse {
    /// String returns a string representation of the mouse message.
    pub fn string(&self) -> String {
        let mut s = String::new();
        if self.mod_.contains(MOD_CTRL) {
            s.push_str("ctrl+");
        }
        if self.mod_.contains(MOD_ALT) {
            s.push_str("alt+");
        }
        if self.mod_.contains(MOD_SHIFT) {
            s.push_str("shift+");
        }

        let str = self.button.as_str();
        if str.is_empty() {
            s.push_str("unknown");
        } else if str != "none" {
            // motion events don't have a button
            s.push_str(str);
        }

        s
    }
}

/// MousePixelToCell converts a mouse event with pixel coordinates to cell
/// coordinates.
///
/// This is only meaningful when using [MouseEncoding::MouseEncodingSGRPixel]
/// encoding, which reports mouse coordinates in pixels rather than cell units.
pub fn mouse_pixel_to_cell(m: Mouse, ws: &Winsize) -> Mouse {
    let mut col = 0;
    let mut row = 0;
    if ws.xpixel > 0 {
        col = m.x * i32::from(ws.col) / i32::from(ws.xpixel);
    }
    if ws.ypixel > 0 {
        row = m.y * i32::from(ws.row) / i32::from(ws.ypixel);
    }

    Mouse {
        x: col,
        y: row,
        button: m.button,
        mod_: m.mod_,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mouse_string() {
        let m = Mouse {
            x: 1,
            y: 2,
            button: MOUSE_LEFT,
            mod_: KeyMod::default(),
        };
        assert_eq!(m.string(), "left");
        let m = Mouse {
            x: 1,
            y: 2,
            button: MOUSE_NONE,
            mod_: KeyMod::default(),
        };
        assert_eq!(m.string(), "");
        let m = Mouse {
            x: 1,
            y: 2,
            button: MOUSE_RIGHT,
            mod_: KeyMod(MOD_CTRL.0 | MOD_ALT.0 | MOD_SHIFT.0),
        };
        assert_eq!(m.string(), "ctrl+alt+shift+right");
    }

    #[test]
    fn test_mouse_pixel_to_cell() {
        let ws = Winsize {
            col: 80,
            row: 24,
            xpixel: 800,
            ypixel: 480,
        };
        let m = Mouse {
            x: 400,
            y: 240,
            button: MOUSE_LEFT,
            mod_: KeyMod::default(),
        };
        let c = mouse_pixel_to_cell(m, &ws);
        assert_eq!(c.x, 40);
        assert_eq!(c.y, 12);
    }
}
