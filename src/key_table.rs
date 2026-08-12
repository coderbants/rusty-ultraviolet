//! Cleanroom Rust port of upstream Go source file: `key_table.go`
//! Upstream Target Tag / Version: `v0.0.0-20260703014108-f5a850f9c2b7`
//!
//! <public-docs>
//! The lookup table of key sequences to key events (VT100/VT200, XTerm,
//! URxvt, and Terminfo-derived sequences).
//! </public-docs>

use crate::decoder::LegacyKeyEncoding;
use crate::key::{
    Key, KeyMod, KEY_BACKSPACE, KEY_BEGIN, KEY_DELETE, KEY_DOWN, KEY_END, KEY_ENTER, KEY_ESCAPE,
    KEY_F1, KEY_F10, KEY_F11, KEY_F12, KEY_F13, KEY_F14, KEY_F15, KEY_F16, KEY_F17, KEY_F18,
    KEY_F19, KEY_F2, KEY_F20, KEY_F3, KEY_F4, KEY_F5, KEY_F6, KEY_F7, KEY_F8, KEY_F9, KEY_FIND,
    KEY_HOME, KEY_INSERT, KEY_KP_0, KEY_KP_1, KEY_KP_2, KEY_KP_3, KEY_KP_4, KEY_KP_5, KEY_KP_6,
    KEY_KP_7, KEY_KP_8, KEY_KP_9, KEY_KP_COMMA, KEY_KP_DECIMAL, KEY_KP_DIVIDE, KEY_KP_ENTER,
    KEY_KP_EQUAL, KEY_KP_MINUS, KEY_KP_MULTIPLY, KEY_KP_PLUS, KEY_LEFT, KEY_PG_DOWN, KEY_PG_UP,
    KEY_RIGHT, KEY_SELECT, KEY_SPACE, KEY_TAB, KEY_UP, MOD_ALT, MOD_CTRL, MOD_META, MOD_SHIFT,
};
use std::collections::HashMap;

const FLAG_CTRL_AT: u32 = 1 << 0;
const FLAG_CTRL_I: u32 = 1 << 1;
const FLAG_CTRL_M: u32 = 1 << 2;
const FLAG_CTRL_OPEN_BRACKET: u32 = 1 << 3;
const FLAG_BACKSPACE: u32 = 1 << 4;
const FLAG_FIND: u32 = 1 << 5;
const FLAG_SELECT: u32 = 1 << 6;

/// BuildKeysTable builds a table of key sequences and their corresponding key
/// events based on the VT100/VT200, XTerm, and Urxvt terminal specs.
pub fn build_keys_table(
    flags: LegacyKeyEncoding,
    term: &str,
    use_terminfo: bool,
) -> HashMap<String, Key> {
    let nul = if flags.0 & FLAG_CTRL_AT != 0 {
        Key {
            code: b'@' as u32,
            mod_: MOD_CTRL,
            ..Key::default()
        }
    } else {
        Key {
            code: KEY_SPACE,
            mod_: MOD_CTRL,
            ..Key::default()
        }
    };

    let tab = if flags.0 & FLAG_CTRL_I != 0 {
        Key {
            code: b'i' as u32,
            mod_: MOD_CTRL,
            ..Key::default()
        }
    } else {
        Key {
            code: KEY_TAB,
            ..Key::default()
        }
    };

    let enter = if flags.0 & FLAG_CTRL_M != 0 {
        Key {
            code: b'm' as u32,
            mod_: MOD_CTRL,
            ..Key::default()
        }
    } else {
        Key {
            code: KEY_ENTER,
            ..Key::default()
        }
    };

    let esc = if flags.0 & FLAG_CTRL_OPEN_BRACKET != 0 {
        Key {
            code: b'[' as u32,
            mod_: MOD_CTRL,
            ..Key::default()
        }
    } else {
        Key {
            code: KEY_ESCAPE,
            ..Key::default()
        }
    };

    let mut del = Key {
        code: KEY_BACKSPACE,
        ..Key::default()
    };
    if flags.0 & FLAG_BACKSPACE != 0 {
        del.code = KEY_DELETE;
    }

    let mut find = Key {
        code: KEY_HOME,
        ..Key::default()
    };
    if flags.0 & FLAG_FIND != 0 {
        find.code = KEY_FIND;
    }

    let mut sel = Key {
        code: KEY_END,
        ..Key::default()
    };
    if flags.0 & FLAG_SELECT != 0 {
        sel.code = KEY_SELECT;
    }

    let mut table: HashMap<String, Key> = HashMap::new();
    let ctrl = |c: char| Key {
        code: c as u32,
        mod_: MOD_CTRL,
        ..Key::default()
    };

    // C0 control characters
    table.insert(String::from("\x00"), nul.clone());
    table.insert(String::from("\x01"), ctrl('a'));
    table.insert(String::from("\x02"), ctrl('b'));
    table.insert(String::from("\x03"), ctrl('c'));
    table.insert(String::from("\x04"), ctrl('d'));
    table.insert(String::from("\x05"), ctrl('e'));
    table.insert(String::from("\x06"), ctrl('f'));
    table.insert(String::from("\x07"), ctrl('g'));
    table.insert(String::from("\x08"), ctrl('h'));
    table.insert(String::from("\x09"), tab.clone());
    table.insert(String::from("\x0a"), ctrl('j'));
    table.insert(String::from("\x0b"), ctrl('k'));
    table.insert(String::from("\x0c"), ctrl('l'));
    table.insert(String::from("\x0d"), enter.clone());
    table.insert(String::from("\x0e"), ctrl('n'));
    table.insert(String::from("\x0f"), ctrl('o'));
    table.insert(String::from("\x10"), ctrl('p'));
    table.insert(String::from("\x11"), ctrl('q'));
    table.insert(String::from("\x12"), ctrl('r'));
    table.insert(String::from("\x13"), ctrl('s'));
    table.insert(String::from("\x14"), ctrl('t'));
    table.insert(String::from("\x15"), ctrl('u'));
    table.insert(String::from("\x16"), ctrl('v'));
    table.insert(String::from("\x17"), ctrl('w'));
    table.insert(String::from("\x18"), ctrl('x'));
    table.insert(String::from("\x19"), ctrl('y'));
    table.insert(String::from("\x1a"), ctrl('z'));
    table.insert(String::from("\x1b"), esc.clone());
    table.insert(String::from("\x1c"), ctrl('\\'));
    table.insert(String::from("\x1d"), ctrl(']'));
    table.insert(String::from("\x1e"), ctrl('^'));
    table.insert(String::from("\x1f"), ctrl('_'));

    // Special keys in G0
    table.insert(
        String::from("\x20"),
        Key {
            code: KEY_SPACE,
            text: " ".to_string(),
            ..Key::default()
        },
    );
    table.insert(String::from("\x7f"), del.clone());

    // Special keys
    table.insert(
        String::from("\x1b[Z"),
        Key {
            code: KEY_TAB,
            mod_: MOD_SHIFT,
            ..Key::default()
        },
    );

    table.insert(String::from("\x1b[1~"), find.clone());
    table.insert(
        String::from("\x1b[2~"),
        Key {
            code: KEY_INSERT,
            ..Key::default()
        },
    );
    table.insert(
        String::from("\x1b[3~"),
        Key {
            code: KEY_DELETE,
            ..Key::default()
        },
    );
    table.insert(String::from("\x1b[4~"), sel.clone());
    table.insert(
        String::from("\x1b[5~"),
        Key {
            code: KEY_PG_UP,
            ..Key::default()
        },
    );
    table.insert(
        String::from("\x1b[6~"),
        Key {
            code: KEY_PG_DOWN,
            ..Key::default()
        },
    );
    table.insert(
        String::from("\x1b[7~"),
        Key {
            code: KEY_HOME,
            ..Key::default()
        },
    );
    table.insert(
        String::from("\x1b[8~"),
        Key {
            code: KEY_END,
            ..Key::default()
        },
    );

    // Normal mode
    let csi_key = |code: u32| Key {
        code,
        ..Key::default()
    };
    table.insert(String::from("\x1b[A"), csi_key(KEY_UP));
    table.insert(String::from("\x1b[B"), csi_key(KEY_DOWN));
    table.insert(String::from("\x1b[C"), csi_key(KEY_RIGHT));
    table.insert(String::from("\x1b[D"), csi_key(KEY_LEFT));
    table.insert(String::from("\x1b[E"), csi_key(KEY_BEGIN));
    table.insert(String::from("\x1b[F"), csi_key(KEY_END));
    table.insert(String::from("\x1b[H"), csi_key(KEY_HOME));
    table.insert(String::from("\x1b[P"), csi_key(KEY_F1));
    table.insert(String::from("\x1b[Q"), csi_key(KEY_F2));
    table.insert(String::from("\x1b[R"), csi_key(KEY_F3));
    table.insert(String::from("\x1b[S"), csi_key(KEY_F4));

    // Application Cursor Key Mode (DECCKM)
    table.insert(String::from("\x1bOA"), csi_key(KEY_UP));
    table.insert(String::from("\x1bOB"), csi_key(KEY_DOWN));
    table.insert(String::from("\x1bOC"), csi_key(KEY_RIGHT));
    table.insert(String::from("\x1bOD"), csi_key(KEY_LEFT));
    table.insert(String::from("\x1bOE"), csi_key(KEY_BEGIN));
    table.insert(String::from("\x1bOF"), csi_key(KEY_END));
    table.insert(String::from("\x1bOH"), csi_key(KEY_HOME));
    table.insert(String::from("\x1bOP"), csi_key(KEY_F1));
    table.insert(String::from("\x1bOQ"), csi_key(KEY_F2));
    table.insert(String::from("\x1bOR"), csi_key(KEY_F3));
    table.insert(String::from("\x1bOS"), csi_key(KEY_F4));

    // Keypad Application Mode (DECKPAM)
    table.insert(String::from("\x1bOM"), csi_key(KEY_KP_ENTER));
    table.insert(String::from("\x1bOX"), csi_key(KEY_KP_EQUAL));
    table.insert(String::from("\x1bOj"), csi_key(KEY_KP_MULTIPLY));
    table.insert(String::from("\x1bOk"), csi_key(KEY_KP_PLUS));
    table.insert(String::from("\x1bOl"), csi_key(KEY_KP_COMMA));
    table.insert(String::from("\x1bOm"), csi_key(KEY_KP_MINUS));
    table.insert(String::from("\x1bOn"), csi_key(KEY_KP_DECIMAL));
    table.insert(String::from("\x1bOo"), csi_key(KEY_KP_DIVIDE));
    table.insert(String::from("\x1bOp"), csi_key(KEY_KP_0));
    table.insert(String::from("\x1bOq"), csi_key(KEY_KP_1));
    table.insert(String::from("\x1bOr"), csi_key(KEY_KP_2));
    table.insert(String::from("\x1bOs"), csi_key(KEY_KP_3));
    table.insert(String::from("\x1bOt"), csi_key(KEY_KP_4));
    table.insert(String::from("\x1bOu"), csi_key(KEY_KP_5));
    table.insert(String::from("\x1bOv"), csi_key(KEY_KP_6));
    table.insert(String::from("\x1bOw"), csi_key(KEY_KP_7));
    table.insert(String::from("\x1bOx"), csi_key(KEY_KP_8));
    table.insert(String::from("\x1bOy"), csi_key(KEY_KP_9));

    // Function keys
    let fkey = |n: u32| Key {
        code: n,
        ..Key::default()
    };
    table.insert(String::from("\x1b[11~"), fkey(KEY_F1));
    table.insert(String::from("\x1b[12~"), fkey(KEY_F2));
    table.insert(String::from("\x1b[13~"), fkey(KEY_F3));
    table.insert(String::from("\x1b[14~"), fkey(KEY_F4));
    table.insert(String::from("\x1b[15~"), fkey(KEY_F5));
    table.insert(String::from("\x1b[17~"), fkey(KEY_F6));
    table.insert(String::from("\x1b[18~"), fkey(KEY_F7));
    table.insert(String::from("\x1b[19~"), fkey(KEY_F8));
    table.insert(String::from("\x1b[20~"), fkey(KEY_F9));
    table.insert(String::from("\x1b[21~"), fkey(KEY_F10));
    table.insert(String::from("\x1b[23~"), fkey(KEY_F11));
    table.insert(String::from("\x1b[24~"), fkey(KEY_F12));
    table.insert(String::from("\x1b[25~"), fkey(KEY_F13));
    table.insert(String::from("\x1b[26~"), fkey(KEY_F14));
    table.insert(String::from("\x1b[28~"), fkey(KEY_F15));
    table.insert(String::from("\x1b[29~"), fkey(KEY_F16));
    table.insert(String::from("\x1b[31~"), fkey(KEY_F17));
    table.insert(String::from("\x1b[32~"), fkey(KEY_F18));
    table.insert(String::from("\x1b[33~"), fkey(KEY_F19));
    table.insert(String::from("\x1b[34~"), fkey(KEY_F20));

    // CSI ~ sequence keys
    let mut csi_tilde_keys: HashMap<&str, Key> = HashMap::new();
    csi_tilde_keys.insert("1", find.clone());
    csi_tilde_keys.insert("2", fkey(KEY_INSERT));
    csi_tilde_keys.insert("3", fkey(KEY_DELETE));
    csi_tilde_keys.insert("4", sel.clone());
    csi_tilde_keys.insert("5", fkey(KEY_PG_UP));
    csi_tilde_keys.insert("6", fkey(KEY_PG_DOWN));
    csi_tilde_keys.insert("7", fkey(KEY_HOME));
    csi_tilde_keys.insert("8", fkey(KEY_END));
    csi_tilde_keys.insert("11", fkey(KEY_F1));
    csi_tilde_keys.insert("12", fkey(KEY_F2));
    csi_tilde_keys.insert("13", fkey(KEY_F3));
    csi_tilde_keys.insert("14", fkey(KEY_F4));
    csi_tilde_keys.insert("15", fkey(KEY_F5));
    csi_tilde_keys.insert("17", fkey(KEY_F6));
    csi_tilde_keys.insert("18", fkey(KEY_F7));
    csi_tilde_keys.insert("19", fkey(KEY_F8));
    csi_tilde_keys.insert("20", fkey(KEY_F9));
    csi_tilde_keys.insert("21", fkey(KEY_F10));
    csi_tilde_keys.insert("23", fkey(KEY_F11));
    csi_tilde_keys.insert("24", fkey(KEY_F12));
    csi_tilde_keys.insert("25", fkey(KEY_F13));
    csi_tilde_keys.insert("26", fkey(KEY_F14));
    csi_tilde_keys.insert("28", fkey(KEY_F15));
    csi_tilde_keys.insert("29", fkey(KEY_F16));
    csi_tilde_keys.insert("31", fkey(KEY_F17));
    csi_tilde_keys.insert("32", fkey(KEY_F18));
    csi_tilde_keys.insert("33", fkey(KEY_F19));
    csi_tilde_keys.insert("34", fkey(KEY_F20));

    // URxvt keys
    table.insert(
        String::from("\x1b[a"),
        Key {
            code: KEY_UP,
            mod_: MOD_SHIFT,
            ..Key::default()
        },
    );
    table.insert(
        String::from("\x1b[b"),
        Key {
            code: KEY_DOWN,
            mod_: MOD_SHIFT,
            ..Key::default()
        },
    );
    table.insert(
        String::from("\x1b[c"),
        Key {
            code: KEY_RIGHT,
            mod_: MOD_SHIFT,
            ..Key::default()
        },
    );
    table.insert(
        String::from("\x1b[d"),
        Key {
            code: KEY_LEFT,
            mod_: MOD_SHIFT,
            ..Key::default()
        },
    );
    table.insert(
        String::from("\x1bOa"),
        Key {
            code: KEY_UP,
            mod_: MOD_CTRL,
            ..Key::default()
        },
    );
    table.insert(
        String::from("\x1bOb"),
        Key {
            code: KEY_DOWN,
            mod_: MOD_CTRL,
            ..Key::default()
        },
    );
    table.insert(
        String::from("\x1bOc"),
        Key {
            code: KEY_RIGHT,
            mod_: MOD_CTRL,
            ..Key::default()
        },
    );
    table.insert(
        String::from("\x1bOd"),
        Key {
            code: KEY_LEFT,
            mod_: MOD_CTRL,
            ..Key::default()
        },
    );

    // URxvt modifier CSI ~ keys
    for (k, v) in &csi_tilde_keys {
        let mut key = v.clone();
        // Shift modifier
        key.mod_ = MOD_SHIFT;
        table.insert(format!("\x1b[{k}$"), key.clone());
        // Ctrl modifier
        key.mod_ = MOD_CTRL;
        table.insert(format!("\x1b[{k}^"), key.clone());
        // Shift-Ctrl modifier
        key.mod_ = KeyMod(MOD_SHIFT.0 | MOD_CTRL.0);
        table.insert(format!("\x1b[{k}@"), key);
    }

    // URxvt F keys
    let urxvt_fkey = |n: u32, mod_: KeyMod| Key {
        code: n,
        mod_,
        ..Key::default()
    };
    table.insert(String::from("\x1b[23$"), urxvt_fkey(KEY_F11, MOD_SHIFT));
    table.insert(String::from("\x1b[24$"), urxvt_fkey(KEY_F12, MOD_SHIFT));
    table.insert(String::from("\x1b[25$"), urxvt_fkey(KEY_F13, MOD_SHIFT));
    table.insert(String::from("\x1b[26$"), urxvt_fkey(KEY_F14, MOD_SHIFT));
    table.insert(String::from("\x1b[28$"), urxvt_fkey(KEY_F15, MOD_SHIFT));
    table.insert(String::from("\x1b[29$"), urxvt_fkey(KEY_F16, MOD_SHIFT));
    table.insert(String::from("\x1b[31$"), urxvt_fkey(KEY_F17, MOD_SHIFT));
    table.insert(String::from("\x1b[32$"), urxvt_fkey(KEY_F18, MOD_SHIFT));
    table.insert(String::from("\x1b[33$"), urxvt_fkey(KEY_F19, MOD_SHIFT));
    table.insert(String::from("\x1b[34$"), urxvt_fkey(KEY_F20, MOD_SHIFT));
    table.insert(String::from("\x1b[11^"), urxvt_fkey(KEY_F1, MOD_CTRL));
    table.insert(String::from("\x1b[12^"), urxvt_fkey(KEY_F2, MOD_CTRL));
    table.insert(String::from("\x1b[13^"), urxvt_fkey(KEY_F3, MOD_CTRL));
    table.insert(String::from("\x1b[14^"), urxvt_fkey(KEY_F4, MOD_CTRL));
    table.insert(String::from("\x1b[15^"), urxvt_fkey(KEY_F5, MOD_CTRL));
    table.insert(String::from("\x1b[17^"), urxvt_fkey(KEY_F6, MOD_CTRL));
    table.insert(String::from("\x1b[18^"), urxvt_fkey(KEY_F7, MOD_CTRL));
    table.insert(String::from("\x1b[19^"), urxvt_fkey(KEY_F8, MOD_CTRL));
    table.insert(String::from("\x1b[20^"), urxvt_fkey(KEY_F9, MOD_CTRL));
    table.insert(String::from("\x1b[21^"), urxvt_fkey(KEY_F10, MOD_CTRL));
    table.insert(String::from("\x1b[23^"), urxvt_fkey(KEY_F11, MOD_CTRL));
    table.insert(String::from("\x1b[24^"), urxvt_fkey(KEY_F12, MOD_CTRL));
    table.insert(String::from("\x1b[25^"), urxvt_fkey(KEY_F13, MOD_CTRL));
    table.insert(String::from("\x1b[26^"), urxvt_fkey(KEY_F14, MOD_CTRL));
    table.insert(String::from("\x1b[28^"), urxvt_fkey(KEY_F15, MOD_CTRL));
    table.insert(String::from("\x1b[29^"), urxvt_fkey(KEY_F16, MOD_CTRL));
    table.insert(String::from("\x1b[31^"), urxvt_fkey(KEY_F17, MOD_CTRL));
    table.insert(String::from("\x1b[32^"), urxvt_fkey(KEY_F18, MOD_CTRL));
    table.insert(String::from("\x1b[33^"), urxvt_fkey(KEY_F19, MOD_CTRL));
    table.insert(String::from("\x1b[34^"), urxvt_fkey(KEY_F20, MOD_CTRL));
    table.insert(
        String::from("\x1b[23@"),
        urxvt_fkey(KEY_F11, KeyMod(MOD_SHIFT.0 | MOD_CTRL.0)),
    );
    table.insert(
        String::from("\x1b[24@"),
        urxvt_fkey(KEY_F12, KeyMod(MOD_SHIFT.0 | MOD_CTRL.0)),
    );
    table.insert(
        String::from("\x1b[25@"),
        urxvt_fkey(KEY_F13, KeyMod(MOD_SHIFT.0 | MOD_CTRL.0)),
    );
    table.insert(
        String::from("\x1b[26@"),
        urxvt_fkey(KEY_F14, KeyMod(MOD_SHIFT.0 | MOD_CTRL.0)),
    );
    table.insert(
        String::from("\x1b[28@"),
        urxvt_fkey(KEY_F15, KeyMod(MOD_SHIFT.0 | MOD_CTRL.0)),
    );
    table.insert(
        String::from("\x1b[29@"),
        urxvt_fkey(KEY_F16, KeyMod(MOD_SHIFT.0 | MOD_CTRL.0)),
    );
    table.insert(
        String::from("\x1b[31@"),
        urxvt_fkey(KEY_F17, KeyMod(MOD_SHIFT.0 | MOD_CTRL.0)),
    );
    table.insert(
        String::from("\x1b[32@"),
        urxvt_fkey(KEY_F18, KeyMod(MOD_SHIFT.0 | MOD_CTRL.0)),
    );
    table.insert(
        String::from("\x1b[33@"),
        urxvt_fkey(KEY_F19, KeyMod(MOD_SHIFT.0 | MOD_CTRL.0)),
    );
    table.insert(
        String::from("\x1b[34@"),
        urxvt_fkey(KEY_F20, KeyMod(MOD_SHIFT.0 | MOD_CTRL.0)),
    );

    // Register Alt + <key> combinations
    // XXX: this must come after URxvt but before XTerm keys to register
    // URxvt keys with alt modifier.
    let mut alt_table: HashMap<String, Key> = HashMap::new();
    for (seq, key) in &table {
        let mut key = key.clone();
        key.mod_.0 |= MOD_ALT.0;
        key.text = String::new(); // Clear runes
        alt_table.insert(format!("\x1b{seq}"), key);
    }
    for (seq, key) in alt_table {
        table.insert(seq, key);
    }

    // XTerm modifiers
    let modifiers: Vec<KeyMod> = vec![
        MOD_SHIFT,                                                 // 1
        MOD_ALT,                                                   // 2
        KeyMod(MOD_SHIFT.0 | MOD_ALT.0),                           // 3
        MOD_CTRL,                                                  // 4
        KeyMod(MOD_SHIFT.0 | MOD_CTRL.0),                          // 5
        KeyMod(MOD_ALT.0 | MOD_CTRL.0),                            // 6
        KeyMod(MOD_SHIFT.0 | MOD_ALT.0 | MOD_CTRL.0),              // 7
        MOD_META,                                                  // 8
        KeyMod(MOD_META.0 | MOD_SHIFT.0),                          // 9
        KeyMod(MOD_META.0 | MOD_ALT.0),                            // 10
        KeyMod(MOD_META.0 | MOD_SHIFT.0 | MOD_ALT.0),              // 11
        KeyMod(MOD_META.0 | MOD_CTRL.0),                           // 12
        KeyMod(MOD_META.0 | MOD_SHIFT.0 | MOD_CTRL.0),             // 13
        KeyMod(MOD_META.0 | MOD_ALT.0 | MOD_CTRL.0),               // 14
        KeyMod(MOD_META.0 | MOD_SHIFT.0 | MOD_ALT.0 | MOD_CTRL.0), // 15
    ];

    // SS3 keypad function keys
    let ss3_func_keys: HashMap<&str, Key> = [
        ("M", KEY_KP_ENTER),
        ("X", KEY_KP_EQUAL),
        ("j", KEY_KP_MULTIPLY),
        ("k", KEY_KP_PLUS),
        ("l", KEY_KP_COMMA),
        ("m", KEY_KP_MINUS),
        ("n", KEY_KP_DECIMAL),
        ("o", KEY_KP_DIVIDE),
        ("p", KEY_KP_0),
        ("q", KEY_KP_1),
        ("r", KEY_KP_2),
        ("s", KEY_KP_3),
        ("t", KEY_KP_4),
        ("u", KEY_KP_5),
        ("v", KEY_KP_6),
        ("w", KEY_KP_7),
        ("x", KEY_KP_8),
        ("y", KEY_KP_9),
    ]
    .iter()
    .map(|(k, v)| (*k, fkey(*v)))
    .collect();

    // XTerm keys
    let csi_func_keys: HashMap<&str, Key> = [
        ("A", KEY_UP),
        ("B", KEY_DOWN),
        ("C", KEY_RIGHT),
        ("D", KEY_LEFT),
        ("E", KEY_BEGIN),
        ("F", KEY_END),
        ("H", KEY_HOME),
        ("P", KEY_F1),
        ("Q", KEY_F2),
        ("R", KEY_F3),
        ("S", KEY_F4),
    ]
    .iter()
    .map(|(k, v)| (*k, fkey(*v)))
    .collect();

    // CSI 27 ; <modifier> ; <code> ~ keys defined in XTerm modifyOtherKeys
    let modify_other_keys: HashMap<i32, Key> = [
        (0x08, KEY_BACKSPACE),
        (0x09, KEY_TAB),
        (0x0D, KEY_ENTER),
        (0x1B, KEY_ESCAPE),
        (0x7F, KEY_BACKSPACE),
    ]
    .iter()
    .map(|(k, v)| (*k, fkey(*v)))
    .collect();

    for m in &modifiers {
        // XTerm modifier offset +1
        let xterm_mod = (m.0 + 1).to_string();

        // CSI 1 ; <modifier> <func>
        for (k, v) in &csi_func_keys {
            let seq = format!("\x1b[1;{xterm_mod}{k}");
            let mut key = v.clone();
            key.mod_ = *m;
            table.insert(seq, key);
        }
        // SS3 <modifier> <func>
        for (k, v) in &ss3_func_keys {
            let seq = format!("\x1bO{xterm_mod}{k}");
            let mut key = v.clone();
            key.mod_ = *m;
            table.insert(seq, key);
        }
        // CSI <number> ; <modifier> ~
        for (k, v) in &csi_tilde_keys {
            let seq = format!("\x1b[{k};{xterm_mod}~");
            let mut key = v.clone();
            key.mod_ = *m;
            table.insert(seq, key);
        }
        // CSI 27 ; <modifier> ; <code> ~
        for (k, v) in &modify_other_keys {
            let seq = format!("\x1b[27;{xterm_mod};{k}~");
            let mut key = v.clone();
            key.mod_ = *m;
            table.insert(seq, key);
        }
    }

    // Register terminfo keys
    // XXX: this might override keys already registered in table
    if use_terminfo {
        let ti_table = build_terminfo_keys(flags, term);
        for (seq, key) in ti_table {
            table.insert(seq, key);
        }
    }

    table
}

/// BuildTerminfoKeys builds a key table from the Terminfo database.
///
/// NOTE: the upstream uses `xo/terminfo` to load the compiled terminfo
/// database. That parser is not ported yet; mirroring Go's behavior when the
/// database is unavailable (`terminfo.Load` returning nil), this returns an
/// empty table.
fn build_terminfo_keys(_flags: LegacyKeyEncoding, _term: &str) -> HashMap<String, Key> {
    HashMap::new()
}

/// DefaultTerminfoKeys returns a map of terminfo keys to key events.
///
/// NOTE: this is only reachable once the terminfo database loader is ported;
/// the table itself is not materialized until then.
#[allow(dead_code)]
fn default_terminfo_keys(_flags: LegacyKeyEncoding) -> HashMap<String, Key> {
    HashMap::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decoder::LegacyKeyEncoding as LKE;

    #[test]
    fn test_build_keys_table_basics() {
        let table = build_keys_table(LKE::default(), "xterm-256color", false);
        // C0
        assert_eq!(table["\x00"].code, KEY_SPACE);
        assert_eq!(table["\x00"].mod_, MOD_CTRL);
        assert_eq!(table["\x09"].code, KEY_TAB);
        assert_eq!(table["\x0d"].code, KEY_ENTER);
        assert_eq!(table["\x1b"].code, KEY_ESCAPE);
        assert_eq!(table["\x7f"].code, KEY_BACKSPACE);
        // Arrows
        assert_eq!(table["\x1b[A"].code, KEY_UP);
        assert_eq!(table["\x1b[B"].code, KEY_DOWN);
        assert_eq!(table["\x1bOA"].code, KEY_UP);
        // F keys
        assert_eq!(table["\x1b[11~"].code, KEY_F1);
        assert_eq!(table["\x1b[24~"].code, KEY_F12);
        // Keypad
        assert_eq!(table["\x1bOM"].code, KEY_KP_ENTER);
        assert_eq!(table["\x1bOp"].code, KEY_KP_0);
        // URxvt
        assert_eq!(table["\x1b[a"].code, KEY_UP);
        assert_eq!(table["\x1b[a"].mod_, MOD_SHIFT);
        // XTerm modifier (Go-verified: "\x1b[1;5D" is ctrl+left; shift+ctrl
        // is modifier 6 in our numbering).
        assert_eq!(table["\x1b[1;5D"].code, KEY_LEFT);
        assert_eq!(table["\x1b[1;5D"].mod_, MOD_CTRL);
        assert_eq!(table["\x1b[1;6D"].mod_, KeyMod(MOD_SHIFT.0 | MOD_CTRL.0));
        assert_eq!(table["\x1b[1;3D"].mod_, MOD_ALT);
        assert_eq!(table["\x1b[1;4D"].mod_, KeyMod(MOD_SHIFT.0 | MOD_ALT.0));
        // Alt combos
        assert_eq!(table["\x1b\x1b[A"].code, KEY_UP);
        assert_eq!(table["\x1b\x1b[A"].mod_, MOD_ALT);
    }

    #[test]
    fn test_build_keys_table_legacy_flags() {
        let flags = LKE::default()
            .ctrl_at(true)
            .ctrl_i(true)
            .ctrl_m(true)
            .backspace(true);
        let table = build_keys_table(flags, "xterm-256color", false);
        assert_eq!(table["\x00"].code, b'@' as u32);
        assert_eq!(table["\x09"].code, b'i' as u32);
        assert_eq!(table["\x0d"].code, b'm' as u32);
        assert_eq!(table["\x7f"].code, KEY_DELETE);
    }
}
