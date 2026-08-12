//! Cleanroom Rust port of upstream Go example: `examples/helloworld/main.go`
//! Upstream Target Tag / Version: `v0.0.0-20260703014108-f5a850f9c2b7`

use charming_ultraviolet::decoder::DecodedEvent;
use charming_ultraviolet::screen::clear;
use charming_ultraviolet::screen_context::new_context;
use charming_ultraviolet::terminal::default_terminal;
use charming_ultraviolet::terminal_screen::TerminalScreen;

const VIEW: [&str; 2] = ["Hello, World!", "Press any key to exit."];

fn display(scr: &mut TerminalScreen) {
    clear(scr);
    let bounds = scr.bounds();
    let mut pos: Vec<(i64, i64)> = Vec::new();
    for (i, line) in VIEW.iter().enumerate() {
        let w = scr.string_width(line);
        pos.push((
            (bounds.dx().saturating_sub(w)) as i64 / 2,
            ((bounds.dy().saturating_sub(VIEW.len())) as i64 / 2) + i as i64,
        ));
    }
    for (i, line) in VIEW.iter().enumerate() {
        let (x, y) = pos[i];
        new_context(Box::new(&mut *scr)).draw_string(line, x, y);
    }
    scr.render();
    let _ = scr.flush();
}

fn main() {
    let mut t = default_terminal();

    // Start in alternate screen mode
    t.screen().enter_alt_screen();

    if let Err(e) = t.start() {
        eprintln!("error: {e}");
        std::process::exit(1);
    }

    // initial render
    display(t.screen());

    let mut physical_width = 0usize;
    let mut physical_height = 0usize;

    'events: loop {
        let ev = t.events().recv();
        let Ok(ev) = ev else { break };
        match ev {
            DecodedEvent::WindowSize(s) => {
                physical_width = s.width;
                physical_height = s.height;
                let scr = t.screen();
                if scr.alt_screen() {
                    scr.resize(physical_width, physical_height);
                } else {
                    scr.resize(physical_width, VIEW.len());
                }
                display(scr);
            }
            DecodedEvent::KeyPress(k) => {
                let scr = t.screen();
                if k.match_string(&["space"]) {
                    if scr.alt_screen() {
                        scr.exit_alt_screen();
                        scr.resize(physical_width, VIEW.len());
                    } else {
                        scr.enter_alt_screen();
                        scr.resize(physical_width, physical_height);
                    }
                    display(scr);
                } else {
                    break 'events;
                }
            }
            _ => {}
        }
    }

    // last render
    display(t.screen());

    let _ = t.stop();
}
