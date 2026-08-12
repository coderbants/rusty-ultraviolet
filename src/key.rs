//! Cleanroom Rust port of upstream Go source file: `key.go`
//! Upstream Target Tag / Version: `v0.0.0-20260703014108-f5a850f9c2b7`
//!
//! <public-docs>
//! The key model: modifier flags, special key codes, and the `Key` type with
//! its string/keystroke representations used for matching key events.
//! </public-docs>

/// KeyMod represents modifier keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct KeyMod(pub i32);

/// Modifier keys.
pub const MOD_SHIFT: KeyMod = KeyMod(1 << 0);
/// Alt modifier.
pub const MOD_ALT: KeyMod = KeyMod(1 << 1);
/// Ctrl modifier.
pub const MOD_CTRL: KeyMod = KeyMod(1 << 2);
/// Meta modifier.
pub const MOD_META: KeyMod = KeyMod(1 << 3);

// These modifiers are used with the Kitty protocol.
// XXX: Meta and Super are swapped in the Kitty protocol, this is to preserve
// compatibility with XTerm modifiers.

/// Hyper modifier.
pub const MOD_HYPER: KeyMod = KeyMod(1 << 4);
/// Super (Windows/Command) modifier.
pub const MOD_SUPER: KeyMod = KeyMod(1 << 5);

// These are key lock states.

/// Caps lock state.
pub const MOD_CAPS_LOCK: KeyMod = KeyMod(1 << 6);
/// Num lock state.
pub const MOD_NUM_LOCK: KeyMod = KeyMod(1 << 7);
/// Scroll lock state (defined in Windows API only).
pub const MOD_SCROLL_LOCK: KeyMod = KeyMod(1 << 8);

impl KeyMod {
    /// Contains reports whether m contains the given modifiers.
    pub fn contains(&self, mods: KeyMod) -> bool {
        self.0 & mods.0 == mods.0
    }
}

/// KeyExtended is a special key code used to signify that a key event
/// contains multiple runes.
pub const KEY_EXTENDED: u32 = 0x10FFFF + 1;

/// Special key symbols.
///
/// These use `char::MAX` + offsets so they never collide with real code
/// points (upstream: `unicode.MaxRune + iota + 1`).
pub const KEY_UP: u32 = KEY_EXTENDED + 1;
/// KeyDown special key.
pub const KEY_DOWN: u32 = KEY_EXTENDED + 2;
/// KeyRight special key.
pub const KEY_RIGHT: u32 = KEY_EXTENDED + 3;
/// KeyLeft special key.
pub const KEY_LEFT: u32 = KEY_EXTENDED + 4;
/// KeyBegin special key.
pub const KEY_BEGIN: u32 = KEY_EXTENDED + 5;
/// KeyFind special key.
pub const KEY_FIND: u32 = KEY_EXTENDED + 6;
/// KeyInsert special key.
pub const KEY_INSERT: u32 = KEY_EXTENDED + 7;
/// KeyDelete special key.
pub const KEY_DELETE: u32 = KEY_EXTENDED + 8;
/// KeySelect special key.
pub const KEY_SELECT: u32 = KEY_EXTENDED + 9;
/// KeyPgUp special key.
pub const KEY_PG_UP: u32 = KEY_EXTENDED + 10;
/// KeyPgDown special key.
pub const KEY_PG_DOWN: u32 = KEY_EXTENDED + 11;
/// KeyHome special key.
pub const KEY_HOME: u32 = KEY_EXTENDED + 12;
/// KeyEnd special key.
pub const KEY_END: u32 = KEY_EXTENDED + 13;

/// Keypad keys.
pub const KEY_KP_ENTER: u32 = KEY_EXTENDED + 14;
/// Keypad equal key.
pub const KEY_KP_EQUAL: u32 = KEY_EXTENDED + 15;
/// Keypad multiply key.
pub const KEY_KP_MULTIPLY: u32 = KEY_EXTENDED + 16;
/// Keypad plus key.
pub const KEY_KP_PLUS: u32 = KEY_EXTENDED + 17;
/// Keypad comma key.
pub const KEY_KP_COMMA: u32 = KEY_EXTENDED + 18;
/// Keypad minus key.
pub const KEY_KP_MINUS: u32 = KEY_EXTENDED + 19;
/// Keypad decimal key.
pub const KEY_KP_DECIMAL: u32 = KEY_EXTENDED + 20;
/// Keypad divide key.
pub const KEY_KP_DIVIDE: u32 = KEY_EXTENDED + 21;
/// Keypad 0 key.
pub const KEY_KP_0: u32 = KEY_EXTENDED + 22;
/// Keypad 1 key.
pub const KEY_KP_1: u32 = KEY_EXTENDED + 23;
/// Keypad 2 key.
pub const KEY_KP_2: u32 = KEY_EXTENDED + 24;
/// Keypad 3 key.
pub const KEY_KP_3: u32 = KEY_EXTENDED + 25;
/// Keypad 4 key.
pub const KEY_KP_4: u32 = KEY_EXTENDED + 26;
/// Keypad 5 key.
pub const KEY_KP_5: u32 = KEY_EXTENDED + 27;
/// Keypad 6 key.
pub const KEY_KP_6: u32 = KEY_EXTENDED + 28;
/// Keypad 7 key.
pub const KEY_KP_7: u32 = KEY_EXTENDED + 29;
/// Keypad 8 key.
pub const KEY_KP_8: u32 = KEY_EXTENDED + 30;
/// Keypad 9 key.
pub const KEY_KP_9: u32 = KEY_EXTENDED + 31;

// The following are keys defined in the Kitty keyboard protocol.

/// Keypad separator key.
pub const KEY_KP_SEP: u32 = KEY_EXTENDED + 32;
/// Keypad up key.
pub const KEY_KP_UP: u32 = KEY_EXTENDED + 33;
/// Keypad down key.
pub const KEY_KP_DOWN: u32 = KEY_EXTENDED + 34;
/// Keypad left key.
pub const KEY_KP_LEFT: u32 = KEY_EXTENDED + 35;
/// Keypad right key.
pub const KEY_KP_RIGHT: u32 = KEY_EXTENDED + 36;
/// Keypad page up key.
pub const KEY_KP_PG_UP: u32 = KEY_EXTENDED + 37;
/// Keypad page down key.
pub const KEY_KP_PG_DOWN: u32 = KEY_EXTENDED + 38;
/// Keypad home key.
pub const KEY_KP_HOME: u32 = KEY_EXTENDED + 39;
/// Keypad end key.
pub const KEY_KP_END: u32 = KEY_EXTENDED + 40;
/// Keypad insert key.
pub const KEY_KP_INSERT: u32 = KEY_EXTENDED + 41;
/// Keypad delete key.
pub const KEY_KP_DELETE: u32 = KEY_EXTENDED + 42;
/// Keypad begin key.
pub const KEY_KP_BEGIN: u32 = KEY_EXTENDED + 43;

/// Function keys.
pub const KEY_F1: u32 = KEY_EXTENDED + 44;
/// Function key 2.
pub const KEY_F2: u32 = KEY_EXTENDED + 45;
/// Function key 3.
pub const KEY_F3: u32 = KEY_EXTENDED + 46;
/// Function key 4.
pub const KEY_F4: u32 = KEY_EXTENDED + 47;
/// Function key 5.
pub const KEY_F5: u32 = KEY_EXTENDED + 48;
/// Function key 6.
pub const KEY_F6: u32 = KEY_EXTENDED + 49;
/// Function key 7.
pub const KEY_F7: u32 = KEY_EXTENDED + 50;
/// Function key 8.
pub const KEY_F8: u32 = KEY_EXTENDED + 51;
/// Function key 9.
pub const KEY_F9: u32 = KEY_EXTENDED + 52;
/// Function key 10.
pub const KEY_F10: u32 = KEY_EXTENDED + 53;
/// Function key 11.
pub const KEY_F11: u32 = KEY_EXTENDED + 54;
/// Function key 12.
pub const KEY_F12: u32 = KEY_EXTENDED + 55;
/// Function key 13.
pub const KEY_F13: u32 = KEY_EXTENDED + 56;
/// Function key 14.
pub const KEY_F14: u32 = KEY_EXTENDED + 57;
/// Function key 15.
pub const KEY_F15: u32 = KEY_EXTENDED + 58;
/// Function key 16.
pub const KEY_F16: u32 = KEY_EXTENDED + 59;
/// Function key 17.
pub const KEY_F17: u32 = KEY_EXTENDED + 60;
/// Function key 18.
pub const KEY_F18: u32 = KEY_EXTENDED + 61;
/// Function key 19.
pub const KEY_F19: u32 = KEY_EXTENDED + 62;
/// Function key 20.
pub const KEY_F20: u32 = KEY_EXTENDED + 63;
/// Function key 21.
pub const KEY_F21: u32 = KEY_EXTENDED + 64;
/// Function key 22.
pub const KEY_F22: u32 = KEY_EXTENDED + 65;
/// Function key 23.
pub const KEY_F23: u32 = KEY_EXTENDED + 66;
/// Function key 24.
pub const KEY_F24: u32 = KEY_EXTENDED + 67;
/// Function key 25.
pub const KEY_F25: u32 = KEY_EXTENDED + 68;
/// Function key 26.
pub const KEY_F26: u32 = KEY_EXTENDED + 69;
/// Function key 27.
pub const KEY_F27: u32 = KEY_EXTENDED + 70;
/// Function key 28.
pub const KEY_F28: u32 = KEY_EXTENDED + 71;
/// Function key 29.
pub const KEY_F29: u32 = KEY_EXTENDED + 72;
/// Function key 30.
pub const KEY_F30: u32 = KEY_EXTENDED + 73;
/// Function key 31.
pub const KEY_F31: u32 = KEY_EXTENDED + 74;
/// Function key 32.
pub const KEY_F32: u32 = KEY_EXTENDED + 75;
/// Function key 33.
pub const KEY_F33: u32 = KEY_EXTENDED + 76;
/// Function key 34.
pub const KEY_F34: u32 = KEY_EXTENDED + 77;
/// Function key 35.
pub const KEY_F35: u32 = KEY_EXTENDED + 78;
/// Function key 36.
pub const KEY_F36: u32 = KEY_EXTENDED + 79;
/// Function key 37.
pub const KEY_F37: u32 = KEY_EXTENDED + 80;
/// Function key 38.
pub const KEY_F38: u32 = KEY_EXTENDED + 81;
/// Function key 39.
pub const KEY_F39: u32 = KEY_EXTENDED + 82;
/// Function key 40.
pub const KEY_F40: u32 = KEY_EXTENDED + 83;
/// Function key 41.
pub const KEY_F41: u32 = KEY_EXTENDED + 84;
/// Function key 42.
pub const KEY_F42: u32 = KEY_EXTENDED + 85;
/// Function key 43.
pub const KEY_F43: u32 = KEY_EXTENDED + 86;
/// Function key 44.
pub const KEY_F44: u32 = KEY_EXTENDED + 87;
/// Function key 45.
pub const KEY_F45: u32 = KEY_EXTENDED + 88;
/// Function key 46.
pub const KEY_F46: u32 = KEY_EXTENDED + 89;
/// Function key 47.
pub const KEY_F47: u32 = KEY_EXTENDED + 90;
/// Function key 48.
pub const KEY_F48: u32 = KEY_EXTENDED + 91;
/// Function key 49.
pub const KEY_F49: u32 = KEY_EXTENDED + 92;
/// Function key 50.
pub const KEY_F50: u32 = KEY_EXTENDED + 93;
/// Function key 51.
pub const KEY_F51: u32 = KEY_EXTENDED + 94;
/// Function key 52.
pub const KEY_F52: u32 = KEY_EXTENDED + 95;
/// Function key 53.
pub const KEY_F53: u32 = KEY_EXTENDED + 96;
/// Function key 54.
pub const KEY_F54: u32 = KEY_EXTENDED + 97;
/// Function key 55.
pub const KEY_F55: u32 = KEY_EXTENDED + 98;
/// Function key 56.
pub const KEY_F56: u32 = KEY_EXTENDED + 99;
/// Function key 57.
pub const KEY_F57: u32 = KEY_EXTENDED + 100;
/// Function key 58.
pub const KEY_F58: u32 = KEY_EXTENDED + 101;
/// Function key 59.
pub const KEY_F59: u32 = KEY_EXTENDED + 102;
/// Function key 60.
pub const KEY_F60: u32 = KEY_EXTENDED + 103;
/// Function key 61.
pub const KEY_F61: u32 = KEY_EXTENDED + 104;
/// Function key 62.
pub const KEY_F62: u32 = KEY_EXTENDED + 105;
/// Function key 63.
pub const KEY_F63: u32 = KEY_EXTENDED + 106;

// The following are keys defined in the Kitty keyboard protocol.

/// Caps lock key.
pub const KEY_CAPS_LOCK: u32 = KEY_EXTENDED + 107;
/// Scroll lock key.
pub const KEY_SCROLL_LOCK: u32 = KEY_EXTENDED + 108;
/// Num lock key.
pub const KEY_NUM_LOCK: u32 = KEY_EXTENDED + 109;
/// Print screen key.
pub const KEY_PRINT_SCREEN: u32 = KEY_EXTENDED + 110;
/// Pause key.
pub const KEY_PAUSE: u32 = KEY_EXTENDED + 111;
/// Menu key.
pub const KEY_MENU: u32 = KEY_EXTENDED + 112;

/// Media play key.
pub const KEY_MEDIA_PLAY: u32 = KEY_EXTENDED + 113;
/// Media pause key.
pub const KEY_MEDIA_PAUSE: u32 = KEY_EXTENDED + 114;
/// Media play/pause key.
pub const KEY_MEDIA_PLAY_PAUSE: u32 = KEY_EXTENDED + 115;
/// Media reverse key.
pub const KEY_MEDIA_REVERSE: u32 = KEY_EXTENDED + 116;
/// Media stop key.
pub const KEY_MEDIA_STOP: u32 = KEY_EXTENDED + 117;
/// Media fast forward key.
pub const KEY_MEDIA_FAST_FORWARD: u32 = KEY_EXTENDED + 118;
/// Media rewind key.
pub const KEY_MEDIA_REWIND: u32 = KEY_EXTENDED + 119;
/// Media next key.
pub const KEY_MEDIA_NEXT: u32 = KEY_EXTENDED + 120;
/// Media previous key.
pub const KEY_MEDIA_PREV: u32 = KEY_EXTENDED + 121;
/// Media record key.
pub const KEY_MEDIA_RECORD: u32 = KEY_EXTENDED + 122;

/// Lower volume key.
pub const KEY_LOWER_VOL: u32 = KEY_EXTENDED + 123;
/// Raise volume key.
pub const KEY_RAISE_VOL: u32 = KEY_EXTENDED + 124;
/// Mute key.
pub const KEY_MUTE: u32 = KEY_EXTENDED + 125;

/// Left shift key.
pub const KEY_LEFT_SHIFT: u32 = KEY_EXTENDED + 126;
/// Left alt key.
pub const KEY_LEFT_ALT: u32 = KEY_EXTENDED + 127;
/// Left ctrl key.
pub const KEY_LEFT_CTRL: u32 = KEY_EXTENDED + 128;
/// Left super key.
pub const KEY_LEFT_SUPER: u32 = KEY_EXTENDED + 129;
/// Left hyper key.
pub const KEY_LEFT_HYPER: u32 = KEY_EXTENDED + 130;
/// Left meta key.
pub const KEY_LEFT_META: u32 = KEY_EXTENDED + 131;
/// Right shift key.
pub const KEY_RIGHT_SHIFT: u32 = KEY_EXTENDED + 132;
/// Right alt key.
pub const KEY_RIGHT_ALT: u32 = KEY_EXTENDED + 133;
/// Right ctrl key.
pub const KEY_RIGHT_CTRL: u32 = KEY_EXTENDED + 134;
/// Right super key.
pub const KEY_RIGHT_SUPER: u32 = KEY_EXTENDED + 135;
/// Right hyper key.
pub const KEY_RIGHT_HYPER: u32 = KEY_EXTENDED + 136;
/// Right meta key.
pub const KEY_RIGHT_META: u32 = KEY_EXTENDED + 137;
/// ISO level 3 shift key.
pub const KEY_ISO_LEVEL3_SHIFT: u32 = KEY_EXTENDED + 138;
/// ISO level 5 shift key.
pub const KEY_ISO_LEVEL5_SHIFT: u32 = KEY_EXTENDED + 139;

/// KeyBackspace is the backspace control code.
pub const KEY_BACKSPACE: u32 = 0x7F;
/// KeyTab is the horizontal tab control code.
pub const KEY_TAB: u32 = 0x09;
/// KeyEnter is the carriage return control code.
pub const KEY_ENTER: u32 = 0x0D;
/// KeyReturn is an alias for [KEY_ENTER].
pub const KEY_RETURN: u32 = KEY_ENTER;
/// KeyEscape is the escape control code.
pub const KEY_ESCAPE: u32 = 0x1B;
/// KeyEsc is an alias for [KEY_ESCAPE].
pub const KEY_ESC: u32 = KEY_ESCAPE;

/// KeySpace is the space character.
pub const KEY_SPACE: u32 = 0x20;

/// Key represents a Key press or release event. It contains information about
/// the Key pressed, like the runes, the type of Key, and the modifiers
/// pressed.
///
/// Note that [Key::text] will be empty for special keys like [KEY_ENTER],
/// [KEY_TAB], and for keys that don't represent printable characters like key
/// combos with modifier keys.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Key {
    /// Text contains the actual characters received. This is usually the same
    /// as [Key::code]. When [Key::text] is non-empty, it indicates that the
    /// key pressed represents printable character(s).
    pub text: String,

    /// Mod represents modifier keys, like [MOD_CTRL], [MOD_ALT], and so on.
    pub mod_: KeyMod,

    /// Code represents the key pressed. This is usually a special key like
    /// [KEY_TAB], [KEY_ENTER], [KEY_F1], or a printable character like 'a'.
    pub code: u32,

    /// ShiftedCode is the actual, shifted key pressed by the user.
    ///
    /// This is only available with the Kitty Keyboard Protocol or the
    /// Windows Console API.
    pub shifted_code: u32,

    /// BaseCode is the key pressed according to the standard PC-101 key
    /// layout.
    ///
    /// This is only available with the Kitty Keyboard Protocol or the
    /// Windows Console API.
    pub base_code: u32,

    /// IsRepeat indicates whether the key is being held down and sending
    /// events repeatedly.
    ///
    /// This is only available with the Kitty Keyboard Protocol or the
    /// Windows Console API.
    pub is_repeat: bool,
}

impl Key {
    /// NewKey creates a new [Key] from the given code.
    pub fn new_key(code: u32) -> Key {
        Key {
            code,
            ..Key::default()
        }
    }

    /// MatchString returns true if the [Key] matches one of the given
    /// strings.
    ///
    /// A string can be a key name like "enter", "tab", "a", or a printable
    /// character like "1" or " ". It can also have combinations of modifiers
    /// like "ctrl+a", "shift+enter", "alt+tab", "ctrl+shift+enter", etc.
    pub fn match_string(&self, strings: &[&str]) -> bool {
        for s in strings {
            if key_match_string(self, s) {
                return true;
            }
        }
        false
    }

    /// String returns the textual representation of the [Key] if there is
    /// one, otherwise, it falls back to [Key::keystroke].
    pub fn string(&self) -> String {
        if !self.text.is_empty() && self.text != " " {
            return self.text.clone();
        }
        self.keystroke()
    }

    /// Keystroke returns the keystroke representation of the [Key]. While
    /// less type safe than looking at the individual fields, it will usually
    /// be more convenient and readable to use this method when matching
    /// against keys.
    ///
    /// Note that modifier keys are always printed in the following order:
    /// ctrl, alt, shift, meta, hyper, super.
    pub fn keystroke(&self) -> String {
        let mut sb = String::new();
        if self.mod_.contains(MOD_CTRL) && self.code != KEY_LEFT_CTRL && self.code != KEY_RIGHT_CTRL
        {
            sb.push_str("ctrl+");
        }
        if self.mod_.contains(MOD_ALT) && self.code != KEY_LEFT_ALT && self.code != KEY_RIGHT_ALT {
            sb.push_str("alt+");
        }
        if self.mod_.contains(MOD_SHIFT)
            && self.code != KEY_LEFT_SHIFT
            && self.code != KEY_RIGHT_SHIFT
        {
            sb.push_str("shift+");
        }
        if self.mod_.contains(MOD_META) && self.code != KEY_LEFT_META && self.code != KEY_RIGHT_META
        {
            sb.push_str("meta+");
        }
        if self.mod_.contains(MOD_HYPER)
            && self.code != KEY_LEFT_HYPER
            && self.code != KEY_RIGHT_HYPER
        {
            sb.push_str("hyper+");
        }
        if self.mod_.contains(MOD_SUPER)
            && self.code != KEY_LEFT_SUPER
            && self.code != KEY_RIGHT_SUPER
        {
            sb.push_str("super+");
        }

        if let Some(kt) = key_type_string(self.code) {
            sb.push_str(kt);
        } else {
            let code = if self.base_code != 0 {
                self.base_code
            } else {
                self.code
            };
            match code {
                KEY_SPACE => sb.push_str("space"),
                KEY_EXTENDED => sb.push_str(&self.text),
                _ => {
                    if let Some(c) = char::from_u32(code) {
                        sb.push(c);
                    }
                }
            }
        }

        sb
    }
}

fn key_match_string(k: &Key, s: &str) -> bool {
    let mut mods = KeyMod(0);
    let mut code: u32 = 0;
    let mut text = String::new();

    for part in s.split('+') {
        match part {
            "ctrl" => mods.0 |= MOD_CTRL.0,
            "alt" => mods.0 |= MOD_ALT.0,
            "shift" => mods.0 |= MOD_SHIFT.0,
            "meta" => mods.0 |= MOD_META.0,
            "hyper" => mods.0 |= MOD_HYPER.0,
            "super" => mods.0 |= MOD_SUPER.0,
            "capslock" => mods.0 |= MOD_CAPS_LOCK.0,
            "scrolllock" => mods.0 |= MOD_SCROLL_LOCK.0,
            "numlock" => mods.0 |= MOD_NUM_LOCK.0,
            _ => {
                // Check if the part is a key name.
                if let Some(k) = string_key_type(part) {
                    code = k;
                } else {
                    // Check if the part is a printable character.
                    let mut chars = part.chars();
                    match (chars.next(), chars.next()) {
                        (Some(c), None) => code = c as u32,
                        (Some(_), _) => {
                            // Multi-rune key.
                            code = KEY_EXTENDED;
                            text = part.to_string();
                        }
                        (None, _) => {}
                    }
                }
            }
        }
    }

    // Check if we have a printable character.
    let smod = KeyMod(mods.0 & !(MOD_SHIFT.0 | MOD_CAPS_LOCK.0));
    if smod.0 == 0 && text.is_empty() && code <= 0x10FFFF {
        if let Some(c) = char::from_u32(code) {
            if c.is_alphabetic() || c.is_numeric() || c.is_ascii_punctuation() || c.is_alphabetic()
            {
                if mods.contains(MOD_SHIFT) || mods.contains(MOD_CAPS_LOCK) {
                    // Shifted code we need to use uppercase.
                    text = c.to_uppercase().collect::<String>();
                } else {
                    // Otherwise, use the code as is.
                    text = c.to_string();
                }
            }
        }
    }

    // Check if we have a match.
    (k.mod_ == mods && k.code == code) || (!k.text.is_empty() && k.text == text)
}

/// keyTypeString maps a special key code to its textual representation.
fn key_type_string(code: u32) -> Option<&'static str> {
    Some(match code {
        KEY_ENTER => "enter",
        KEY_TAB => "tab",
        KEY_BACKSPACE => "backspace",
        KEY_ESCAPE => "esc",
        KEY_SPACE => "space",
        KEY_UP => "up",
        KEY_DOWN => "down",
        KEY_LEFT => "left",
        KEY_RIGHT => "right",
        KEY_BEGIN => "begin",
        KEY_FIND => "find",
        KEY_INSERT => "insert",
        KEY_DELETE => "delete",
        KEY_SELECT => "select",
        KEY_PG_UP => "pgup",
        KEY_PG_DOWN => "pgdown",
        KEY_HOME => "home",
        KEY_END => "end",
        KEY_KP_ENTER => "enter",
        KEY_KP_EQUAL => "equal",
        KEY_KP_MULTIPLY => "mul",
        KEY_KP_PLUS => "plus",
        KEY_KP_COMMA => "comma",
        KEY_KP_MINUS => "minus",
        KEY_KP_DECIMAL => "period",
        KEY_KP_DIVIDE => "div",
        KEY_KP_0 => "0",
        KEY_KP_1 => "1",
        KEY_KP_2 => "2",
        KEY_KP_3 => "3",
        KEY_KP_4 => "4",
        KEY_KP_5 => "5",
        KEY_KP_6 => "6",
        KEY_KP_7 => "7",
        KEY_KP_8 => "8",
        KEY_KP_9 => "9",
        KEY_KP_SEP => "sep",
        KEY_KP_UP => "up",
        KEY_KP_DOWN => "down",
        KEY_KP_LEFT => "left",
        KEY_KP_RIGHT => "right",
        KEY_KP_PG_UP => "pgup",
        KEY_KP_PG_DOWN => "pgdown",
        KEY_KP_HOME => "home",
        KEY_KP_END => "end",
        KEY_KP_INSERT => "insert",
        KEY_KP_DELETE => "delete",
        KEY_KP_BEGIN => "begin",
        KEY_F1 => "f1",
        KEY_F2 => "f2",
        KEY_F3 => "f3",
        KEY_F4 => "f4",
        KEY_F5 => "f5",
        KEY_F6 => "f6",
        KEY_F7 => "f7",
        KEY_F8 => "f8",
        KEY_F9 => "f9",
        KEY_F10 => "f10",
        KEY_F11 => "f11",
        KEY_F12 => "f12",
        KEY_F13 => "f13",
        KEY_F14 => "f14",
        KEY_F15 => "f15",
        KEY_F16 => "f16",
        KEY_F17 => "f17",
        KEY_F18 => "f18",
        KEY_F19 => "f19",
        KEY_F20 => "f20",
        KEY_F21 => "f21",
        KEY_F22 => "f22",
        KEY_F23 => "f23",
        KEY_F24 => "f24",
        KEY_F25 => "f25",
        KEY_F26 => "f26",
        KEY_F27 => "f27",
        KEY_F28 => "f28",
        KEY_F29 => "f29",
        KEY_F30 => "f30",
        KEY_F31 => "f31",
        KEY_F32 => "f32",
        KEY_F33 => "f33",
        KEY_F34 => "f34",
        KEY_F35 => "f35",
        KEY_F36 => "f36",
        KEY_F37 => "f37",
        KEY_F38 => "f38",
        KEY_F39 => "f39",
        KEY_F40 => "f40",
        KEY_F41 => "f41",
        KEY_F42 => "f42",
        KEY_F43 => "f43",
        KEY_F44 => "f44",
        KEY_F45 => "f45",
        KEY_F46 => "f46",
        KEY_F47 => "f47",
        KEY_F48 => "f48",
        KEY_F49 => "f49",
        KEY_F50 => "f50",
        KEY_F51 => "f51",
        KEY_F52 => "f52",
        KEY_F53 => "f53",
        KEY_F54 => "f54",
        KEY_F55 => "f55",
        KEY_F56 => "f56",
        KEY_F57 => "f57",
        KEY_F58 => "f58",
        KEY_F59 => "f59",
        KEY_F60 => "f60",
        KEY_F61 => "f61",
        KEY_F62 => "f62",
        KEY_F63 => "f63",
        KEY_CAPS_LOCK => "capslock",
        KEY_SCROLL_LOCK => "scrolllock",
        KEY_NUM_LOCK => "numlock",
        KEY_PRINT_SCREEN => "printscreen",
        KEY_PAUSE => "pause",
        KEY_MENU => "menu",
        KEY_MEDIA_PLAY => "mediaplay",
        KEY_MEDIA_PAUSE => "mediapause",
        KEY_MEDIA_PLAY_PAUSE => "mediaplaypause",
        KEY_MEDIA_REVERSE => "mediareverse",
        KEY_MEDIA_STOP => "mediastop",
        KEY_MEDIA_FAST_FORWARD => "mediafastforward",
        KEY_MEDIA_REWIND => "mediarewind",
        KEY_MEDIA_NEXT => "medianext",
        KEY_MEDIA_PREV => "mediaprev",
        KEY_MEDIA_RECORD => "mediarecord",
        KEY_LOWER_VOL => "lowervol",
        KEY_RAISE_VOL => "raisevol",
        KEY_MUTE => "mute",
        KEY_LEFT_SHIFT => "leftshift",
        KEY_LEFT_ALT => "leftalt",
        KEY_LEFT_CTRL => "leftctrl",
        KEY_LEFT_SUPER => "leftsuper",
        KEY_LEFT_HYPER => "lefthyper",
        KEY_LEFT_META => "leftmeta",
        KEY_RIGHT_SHIFT => "rightshift",
        KEY_RIGHT_ALT => "rightalt",
        KEY_RIGHT_CTRL => "rightctrl",
        KEY_RIGHT_SUPER => "rightsuper",
        KEY_RIGHT_HYPER => "righthyper",
        KEY_RIGHT_META => "rightmeta",
        KEY_ISO_LEVEL3_SHIFT => "isolevel3shift",
        KEY_ISO_LEVEL5_SHIFT => "isolevel5shift",
        _ => return None,
    })
}

/// stringKeyType maps a key name string to its special key code.
fn string_key_type(s: &str) -> Option<u32> {
    Some(match s {
        "enter" => KEY_ENTER,
        "tab" => KEY_TAB,
        "backspace" => KEY_BACKSPACE,
        "escape" => KEY_ESCAPE,
        "esc" => KEY_ESCAPE,
        "space" => KEY_SPACE,
        "up" => KEY_UP,
        "down" => KEY_DOWN,
        "left" => KEY_LEFT,
        "right" => KEY_RIGHT,
        "begin" => KEY_BEGIN,
        "find" => KEY_FIND,
        "insert" => KEY_INSERT,
        "delete" => KEY_DELETE,
        "select" => KEY_SELECT,
        "pgup" => KEY_PG_UP,
        "pgdown" => KEY_PG_DOWN,
        "home" => KEY_HOME,
        "end" => KEY_END,
        "kpenter" => KEY_KP_ENTER,
        "kpequal" => KEY_KP_EQUAL,
        "kpmul" => KEY_KP_MULTIPLY,
        "kpplus" => KEY_KP_PLUS,
        "kpcomma" => KEY_KP_COMMA,
        "kpminus" => KEY_KP_MINUS,
        "kpperiod" => KEY_KP_DECIMAL,
        "kpdiv" => KEY_KP_DIVIDE,
        "kp0" => KEY_KP_0,
        "kp1" => KEY_KP_1,
        "kp2" => KEY_KP_2,
        "kp3" => KEY_KP_3,
        "kp4" => KEY_KP_4,
        "kp5" => KEY_KP_5,
        "kp6" => KEY_KP_6,
        "kp7" => KEY_KP_7,
        "kp8" => KEY_KP_8,
        "kp9" => KEY_KP_9,
        "kpsep" => KEY_KP_SEP,
        "kpup" => KEY_KP_UP,
        "kpdown" => KEY_KP_DOWN,
        "kpleft" => KEY_KP_LEFT,
        "kpright" => KEY_KP_RIGHT,
        "kppgup" => KEY_KP_PG_UP,
        "kppgdown" => KEY_KP_PG_DOWN,
        "kphome" => KEY_KP_HOME,
        "kpend" => KEY_KP_END,
        "kpinsert" => KEY_KP_INSERT,
        "kpdelete" => KEY_KP_DELETE,
        "kpbegin" => KEY_KP_BEGIN,
        "f1" => KEY_F1,
        "f2" => KEY_F2,
        "f3" => KEY_F3,
        "f4" => KEY_F4,
        "f5" => KEY_F5,
        "f6" => KEY_F6,
        "f7" => KEY_F7,
        "f8" => KEY_F8,
        "f9" => KEY_F9,
        "f10" => KEY_F10,
        "f11" => KEY_F11,
        "f12" => KEY_F12,
        "f13" => KEY_F13,
        "f14" => KEY_F14,
        "f15" => KEY_F15,
        "f16" => KEY_F16,
        "f17" => KEY_F17,
        "f18" => KEY_F18,
        "f19" => KEY_F19,
        "f20" => KEY_F20,
        "f21" => KEY_F21,
        "f22" => KEY_F22,
        "f23" => KEY_F23,
        "f24" => KEY_F24,
        "f25" => KEY_F25,
        "f26" => KEY_F26,
        "f27" => KEY_F27,
        "f28" => KEY_F28,
        "f29" => KEY_F29,
        "f30" => KEY_F30,
        "f31" => KEY_F31,
        "f32" => KEY_F32,
        "f33" => KEY_F33,
        "f34" => KEY_F34,
        "f35" => KEY_F35,
        "f36" => KEY_F36,
        "f37" => KEY_F37,
        "f38" => KEY_F38,
        "f39" => KEY_F39,
        "f40" => KEY_F40,
        "f41" => KEY_F41,
        "f42" => KEY_F42,
        "f43" => KEY_F43,
        "f44" => KEY_F44,
        "f45" => KEY_F45,
        "f46" => KEY_F46,
        "f47" => KEY_F47,
        "f48" => KEY_F48,
        "f49" => KEY_F49,
        "f50" => KEY_F50,
        "f51" => KEY_F51,
        "f52" => KEY_F52,
        "f53" => KEY_F53,
        "f54" => KEY_F54,
        "f55" => KEY_F55,
        "f56" => KEY_F56,
        "f57" => KEY_F57,
        "f58" => KEY_F58,
        "f59" => KEY_F59,
        "f60" => KEY_F60,
        "f61" => KEY_F61,
        "f62" => KEY_F62,
        "f63" => KEY_F63,
        "capslock" => KEY_CAPS_LOCK,
        "scrolllock" => KEY_SCROLL_LOCK,
        "numlock" => KEY_NUM_LOCK,
        "printscreen" => KEY_PRINT_SCREEN,
        "pause" => KEY_PAUSE,
        "menu" => KEY_MENU,
        "mediaplay" => KEY_MEDIA_PLAY,
        "mediapause" => KEY_MEDIA_PAUSE,
        "mediaplaypause" => KEY_MEDIA_PLAY_PAUSE,
        "mediareverse" => KEY_MEDIA_REVERSE,
        "mediastop" => KEY_MEDIA_STOP,
        "mediafastforward" => KEY_MEDIA_FAST_FORWARD,
        "mediarewind" => KEY_MEDIA_REWIND,
        "medianext" => KEY_MEDIA_NEXT,
        "mediaprev" => KEY_MEDIA_PREV,
        "mediarecord" => KEY_MEDIA_RECORD,
        "lowervol" => KEY_LOWER_VOL,
        "raisevol" => KEY_RAISE_VOL,
        "mute" => KEY_MUTE,
        "leftshift" => KEY_LEFT_SHIFT,
        "leftalt" => KEY_LEFT_ALT,
        "leftctrl" => KEY_LEFT_CTRL,
        "leftsuper" => KEY_LEFT_SUPER,
        "lefthyper" => KEY_LEFT_HYPER,
        "leftmeta" => KEY_LEFT_META,
        "rightshift" => KEY_RIGHT_SHIFT,
        "rightalt" => KEY_RIGHT_ALT,
        "rightctrl" => KEY_RIGHT_CTRL,
        "rightsuper" => KEY_RIGHT_SUPER,
        "righthyper" => KEY_RIGHT_HYPER,
        "rightmeta" => KEY_RIGHT_META,
        "isolevel3shift" => KEY_ISO_LEVEL3_SHIFT,
        "isolevel5shift" => KEY_ISO_LEVEL5_SHIFT,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mod_contains() {
        let m = KeyMod(MOD_ALT.0 | MOD_CTRL.0);
        assert!(m.contains(MOD_CTRL));
        assert!(m.contains(KeyMod(MOD_ALT.0 | MOD_CTRL.0)));
        assert!(!m.contains(KeyMod(MOD_ALT.0 | MOD_CTRL.0 | MOD_SHIFT.0)));
    }

    #[test]
    fn test_key_keystroke() {
        let k = Key {
            code: KEY_ENTER,
            ..Key::default()
        };
        assert_eq!(k.keystroke(), "enter");
        let k = Key {
            code: 'a' as u32,
            ..Key::default()
        };
        assert_eq!(k.keystroke(), "a");
        let k = Key {
            code: 'a' as u32,
            mod_: MOD_CTRL,
            ..Key::default()
        };
        assert_eq!(k.keystroke(), "ctrl+a");
        let k = Key {
            code: KEY_SPACE,
            ..Key::default()
        };
        assert_eq!(k.keystroke(), "space");
        let k = Key {
            code: KEY_F5,
            ..Key::default()
        };
        assert_eq!(k.keystroke(), "f5");
    }

    #[test]
    fn test_key_string() {
        let k = Key {
            text: "?".to_string(),
            code: '/' as u32,
            ..Key::default()
        };
        assert_eq!(k.string(), "?");
        let k = Key {
            code: KEY_UP,
            ..Key::default()
        };
        assert_eq!(k.string(), "up");
    }

    #[test]
    fn test_key_match_string() {
        let k = Key {
            code: 'a' as u32,
            ..Key::default()
        };
        assert!(k.match_string(&["a"]));
        assert!(k.match_string(&["ctrl+b", "a"]));
        assert!(!k.match_string(&["b"]));
        let k = Key {
            code: KEY_ENTER,
            ..Key::default()
        };
        assert!(k.match_string(&["enter"]));
        let k = Key {
            code: 'a' as u32,
            mod_: MOD_CTRL,
            ..Key::default()
        };
        assert!(k.match_string(&["ctrl+a"]));
        assert!(!k.match_string(&["a"]));
        // Go TestKeyMatchString vectors:
        let k = Key {
            code: 'a' as u32,
            mod_: KeyMod(MOD_CTRL.0 | MOD_ALT.0 | MOD_SHIFT.0),
            ..Key::default()
        };
        assert!(k.match_string(&["ctrl+alt+shift+a"]));
        let k = Key {
            code: 'H' as u32,
            text: "H".to_string(),
            ..Key::default()
        };
        assert!(k.match_string(&["H"]));
        let k = Key {
            code: 'h' as u32,
            mod_: MOD_SHIFT,
            text: "H".to_string(),
            ..Key::default()
        };
        assert!(k.match_string(&["H"]));
        assert!(k.match_string(&["shift+h"]));
        let k = Key {
            code: '/' as u32,
            mod_: MOD_SHIFT,
            text: "?".to_string(),
            ..Key::default()
        };
        assert!(k.match_string(&["?"]));
        assert!(k.match_string(&["shift+/"]));
        // ctrl+capslock+a does NOT match "ctrl+a".
        let k = Key {
            code: 'a' as u32,
            mod_: KeyMod(MOD_CTRL.0 | MOD_CAPS_LOCK.0),
            ..Key::default()
        };
        assert!(!k.match_string(&["ctrl+a"]));
        let k = Key {
            code: KEY_SPACE,
            mod_: MOD_CTRL,
            ..Key::default()
        };
        assert!(k.match_string(&["ctrl+space"]));
    }
}
