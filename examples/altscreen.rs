//! Cleanroom Rust port of upstream Go example: `examples/altscreen/main.go`
//! Upstream Target Tag / Version: `v0.0.0-20260703014108-f5a850f9c2b7`

use charming_ultraviolet::decoder::DecodedEvent;
use charming_ultraviolet::screen::clear;
use charming_ultraviolet::screen_context::new_context;
use charming_ultraviolet::terminal::default_terminal;
use charming_ultraviolet::terminal_screen::TerminalScreen;

const HELP: &str = "Press space to toggle screen mode or any other key to exit.";

fn display(scr: &mut TerminalScreen, alt_screen: bool) {
    let str = if alt_screen {
        "This is using alternate screen mode."
    } else {
        "This is using inline mode."
    };
    let str = format!("{str}\n{HELP}");

    clear(scr);
    let _ = new_context(Box::new(&mut *scr)).print(format_args!("{str}"));
    scr.render();
    let _ = scr.flush();
}

fn main() {
    let mut t = default_terminal();

    if let Err(e) = t.start() {
        eprintln!("failed to start program: {e}");
        std::process::exit(1);
    }

    let mut alt_screen = false;

    // initial render
    display(t.screen(), alt_screen);

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
                if alt_screen {
                    scr.resize(physical_width, physical_height);
                } else {
                    scr.resize(physical_width, 2);
                }
                display(scr, alt_screen);
            }
            DecodedEvent::KeyPress(k) => {
                let scr = t.screen();
                if k.match_string(&["space"]) {
                    if alt_screen {
                        scr.exit_alt_screen();
                        scr.resize(physical_width, 2);
                    } else {
                        scr.enter_alt_screen();
                        scr.resize(physical_width, physical_height);
                    }
                    alt_screen = !alt_screen;
                    display(scr, alt_screen);
                } else {
                    break 'events;
                }
            }
            _ => {}
        }
    }

    let _ = t.stop();
}
