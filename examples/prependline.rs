//! Cleanroom Rust port of upstream Go example: `examples/prependline/main.go`
//! Upstream Target Tag / Version: `v0.0.0-20260703014108-f5a850f9c2b7`

use charming_ultraviolet::cell::{empty_cell, Cell};
use charming_ultraviolet::decoder::DecodedEvent;
use charming_ultraviolet::screen::{fill_area, rect};
use charming_ultraviolet::style::Style;
use charming_ultraviolet::terminal::default_terminal;
use charming_ultraviolet::terminal_screen::TerminalScreen;
use charming_x_ansi::style::Color;

fn main() {
    let mut t = default_terminal();

    if let Err(e) = t.start() {
        eprintln!("failed to start program: {e}");
        std::process::exit(1);
    }

    t.screen()
        .write_string(&charming_x_ansi::screen::set_window_title("Hello, World!"));

    let mut st = Style::default();
    let mut bg: u8 = 1;
    st.bg = Some(Color::Basic(bg));
    st.fg = Some(Color::Basic(0)); // ansi.Black

    const HW: &str = "Hello, World!";

    let display = |scr: &mut TerminalScreen, st: &Style, bg: u8| {
        let mut cell = empty_cell();
        cell.style = st.clone();
        let w = scr.bounds().dx();
        fill_area(scr, Some(&cell), rect(0, 0, w, 1));
        for (i, r) in HW.chars().enumerate() {
            scr.set_cell(
                i,
                0,
                Some(&Cell {
                    content: r.to_string(),
                    style: st.clone(),
                    width: 1,
                    ..Cell::default()
                }),
            );
        }
        scr.render();
        let _ = scr.flush();
        let _ = bg;
    };

    // initial render
    display(t.screen(), &st, bg);

    'events: loop {
        let ev = t.events().recv();
        let Ok(ev) = ev else { break };
        let ev_desc = go_event_desc(&ev);
        match &ev {
            DecodedEvent::WindowSize(s) => {
                let scr = t.screen();
                scr.resize(s.width, 1);
                display(scr, &st, bg);
            }
            DecodedEvent::KeyPress(k) => {
                if k.match_string(&["q", "ctrl+c"]) {
                    break 'events;
                }
                st.bg = Some(Color::Basic(rand16()));
            }
            _ => {}
        }

        // Log event (this will appear above when we exit altscreen)
        let scr = t.screen();
        let _ = scr.insert_above(&ev_desc);

        bg = rand8();
        st.bg = Some(Color::Basic(bg));
        display(scr, &st, bg);
    }

    t.screen()
        .write_string(&charming_x_ansi::screen::set_window_title(""));

    let _ = t.stop();
}

/// Formats an event the way the upstream example does with
/// `fmt.Sprintf("%T %v", ev, ev)`.
fn go_event_desc(ev: &DecodedEvent) -> String {
    match ev {
        DecodedEvent::WindowSize(s) => {
            format!("uv.WindowSizeEvent {{{} {}}}", s.width, s.height)
        }
        DecodedEvent::KeyPress(k) => format!("uv.KeyPressEvent {}", k.string()),
        _ => format!("{ev:?}"),
    }
}

/// Mirrors `math/rand.Intn(16)` in the upstream example: a fixed low bits of
/// a cheap pseudo-random source. This only affects the pen background color
/// chosen per non-quit key.
fn rand16() -> u8 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    (t.subsec_nanos() % 16) as u8
}

fn rand8() -> u8 {
    rand16() % 8
}
