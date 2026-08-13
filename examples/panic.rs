//! Cleanroom Rust port of upstream Go example: `examples/panic/main.go`
//! Upstream Target Tag / Version: `v0.0.0-20260703014108-f5a850f9c2b7`

use rusty_ultraviolet::decoder::DecodedEvent;
use rusty_ultraviolet::screen::clear;
use rusty_ultraviolet::styled::new_styled_string;
use rusty_ultraviolet::terminal::default_terminal;
use rusty_ultraviolet::terminal_screen::TerminalScreen;
use std::time::{Duration, Instant};

fn main() {
    let mut t = default_terminal();

    if let Err(e) = t.start() {
        eprintln!("failed to start terminal: {e}");
        std::process::exit(1);
    }

    let mut counter: i32 = 5;
    let mut next_tick = Instant::now() + Duration::from_secs(1);

    let view = |c: i32| format!("Panicing after {c} seconds...\nPress 'q' or 'Ctrl+C' to exit.");

    let render = |scr: &mut TerminalScreen, c: i32| {
        let mut ss = new_styled_string(&view(c));
        clear(scr);
        let _ = scr.display(Some(&mut ss));
    };

    // initial render
    render(t.screen(), counter);

    'outer: loop {
        // Wait for either a tick or an event, mirroring the upstream
        // select over the ticker and the events channel.
        let timeout = next_tick.saturating_duration_since(Instant::now());
        let ev = t
            .events()
            .recv_timeout(timeout.max(Duration::from_millis(1)));
        let mut ticked = false;
        match ev {
            Ok(ev) => {
                let scr = t.screen();
                match ev {
                    DecodedEvent::WindowSize(s) => {
                        scr.resize(s.width, 2);
                    }
                    DecodedEvent::KeyPress(k) if k.match_string(&["q", "ctrl+c"]) => {
                        break 'outer;
                    }
                    _ => {}
                }
                render(scr, counter);
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => ticked = true,
            Err(_) => break 'outer,
        }
        if ticked && Instant::now() >= next_tick {
            next_tick += Duration::from_secs(1);
            counter -= 1;
            if counter < 0 {
                // Mirror the upstream panic: the deferred recovery prints
                // the message and a stack trace to stderr, then the program
                // exits with a non-zero status.
                let _ = t.stop();
                eprintln!("\r\nrecovered from panic: Time's up!\n");
                let bt = std::backtrace::Backtrace::force_capture();
                eprintln!("{bt}");
                std::process::exit(1);
            }
            render(t.screen(), counter);
        }
    }

    {
        let mut ss = new_styled_string(&format!("{}\n", view(counter)));
        let scr = t.screen();
        clear(scr);
        let _ = scr.display(Some(&mut ss));
    }

    if let Err(e) = t.stop() {
        eprintln!("failed to stop terminal: {e}");
        std::process::exit(1);
    }
}
