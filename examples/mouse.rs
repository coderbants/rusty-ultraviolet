//! Cleanroom Rust port of upstream Go example: `examples/mouse/main.go`
//! Upstream Target Tag / Version: `v0.0.0-20260703014108-f5a850f9c2b7`

use charming_ultraviolet::cell::{empty_cell, Cell};
use charming_ultraviolet::decoder::DecodedEvent;
use charming_ultraviolet::mouse::mouse_pixel_to_cell;
use charming_ultraviolet::screen::{fill_area, rect};
use charming_ultraviolet::style::Style;
use charming_ultraviolet::terminal::default_terminal;
use charming_ultraviolet::terminal_screen::TerminalScreen;
use charming_x_ansi::style::Color;

fn display(scr: &mut TerminalScreen, width: usize, last_btn: &charming_x_ansi::mouse::MouseButton, last_x: i32, last_y: i32) {
    let label = format!(" Button: {:<12} Position: ({last_x}, {last_y})", last_btn.as_str());
    let mut st = Style::default();
    st.bg = Some(Color::Basic(4));
    st.fg = Some(Color::Basic(0)); // ansi.Black
    let mut bg = empty_cell();
    bg.style = st.clone();
    fill_area(scr, Some(&bg), rect(0, 0, width, 1));
    for (i, r) in label.chars().enumerate() {
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
}

fn main() {
    let mut t = default_terminal();
    let ws = match t.get_winsize() {
        Ok(ws) => ws,
        Err(e) => {
            eprintln!("failed to get window size: {e}");
            std::process::exit(1);
        }
    };
    let mut ws = ws;

    if let Err(e) = t.start() {
        eprintln!("failed to start program: {e}");
        std::process::exit(1);
    }

    t.screen().set_mouse_mode(charming_ultraviolet::mouse::MouseMode::MouseModeMotion);
    t.screen().set_mouse_encoding(charming_ultraviolet::mouse::MouseEncoding::MouseEncodingSGRPixel);

    let mut last_btn: charming_x_ansi::mouse::MouseButton = charming_x_ansi::mouse::MOUSE_NONE;
    let mut last_x: i32 = 0;
    let mut last_y: i32 = 0;
    let mut width = 0usize;

    // initial render
    display(t.screen(), width, &last_btn, last_x, last_y);

    'events: loop {
        let ev = t.events().recv();
        let Ok(ev) = ev else { break };
        match ev {
            DecodedEvent::WindowSize(s) => {
                width = s.width;
                ws.col = s.width as u16;
                ws.row = s.height as u16;
                let scr = t.screen();
                scr.resize(width, 1);

                // Query the pixel dimensions of the window in-case this platform
                // doesn't report them via the terminal's get_winsize.
                let _ = scr.write_string(&charming_x_ansi::winop::window_op(charming_x_ansi::winop::RESIZE_WINDOW_WIN_OP, &[]));

                display(scr, width, &last_btn, last_x, last_y);
            }
            DecodedEvent::PixelSize(s) => {
                ws.xpixel = s.width as u16;
                ws.ypixel = s.height as u16;
            }
            DecodedEvent::KeyPress(k) => {
                if k.match_string(&["q", "ctrl+c"]) {
                    break 'events;
                }
            }
            DecodedEvent::MouseClick(m) | DecodedEvent::MouseMotion(m) => {
                let m = mouse_pixel_to_cell(m, &ws);
                last_x = m.x;
                last_y = m.y;
                if m.button != charming_ultraviolet::mouse::MOUSE_NONE {
                    last_btn = m.button;
                }
                let ev_desc = m.string();
                let scr = t.screen();
                let _ = scr.insert_above(&format!(
                    "{ev_desc:<20} ({}, {}) {}",
                    m.x, m.y, m.button.as_str()
                ));
                display(scr, width, &last_btn, last_x, last_y);
            }
            _ => {}
        }
    }

    let _ = t.stop();
}
