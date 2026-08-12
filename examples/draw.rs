//! Cleanroom Rust port of upstream Go example: `examples/draw/main.go`
//! Upstream Target Tag / Version: `v0.0.0-20260703014108-f5a850f9c2b7`

use charming_ultraviolet::cell::{empty_cell, Cell};
use charming_ultraviolet::decoder::DecodedEvent;
use charming_ultraviolet::mouse::Mouse;
use charming_ultraviolet::screen::{clear, clone_area, rect};
use charming_ultraviolet::styled::new_styled_string;
use charming_ultraviolet::terminal::default_terminal;
use charming_ultraviolet::terminal_screen::TerminalScreen;
use charming_x_ansi::mode::{Mode, MODE_FOCUS_EVENT, MODE_MOUSE_BUTTON_EVENT, MODE_MOUSE_EXT_SGR};

const HELP: &str = "Welcome to Draw Example!

Use the mouse to draw on the screen.
Press ctrl+c to exit.
Press esc to clear the screen.
Press alt+esc to reset the pen character, color, and the screen.
Press 0-9 to set the foreground color.
Press any other key to set the pen character.
Press ctrl+h for this help message.

Press any key to continue...";

fn display_help(scr: &mut TerminalScreen, show: bool, prev_help_buf: &mut Option<charming_ultraviolet::Buffer>) {
    let help_comp = new_styled_string(HELP);
    let help_area = help_comp.bounds();
    let help_w = help_area.dx();
    let help_h = help_area.dy();

    let bounds = scr.bounds();
    let mid_x = bounds.dx() / 2;
    let mid_y = bounds.dy() / 2;
    let x = mid_x.saturating_sub(help_w / 2);
    let y = mid_y.saturating_sub(help_h / 2);
    let mid_area = rect(x, y, help_w, help_h);
    if show {
        // Save the area under the help to restore it later.
        *prev_help_buf = Some(clone_area(scr, mid_area));
        help_comp.draw(&mut *scr, mid_area);
    } else if let Some(prev) = prev_help_buf {
        // Restore saved area under the help.
        let p = prev.clone();
        p.draw(&mut *scr, mid_area);
    }
    scr.render();
    let _ = scr.flush();
}

fn clear_screen(scr: &mut TerminalScreen) {
    clear(scr);
    scr.render();
    let _ = scr.flush();
}

fn run_draw(scr: &mut TerminalScreen, pen: &Cell, m: &Mouse) {
    let cur = scr.cell_at(m.x as usize, m.y as usize);
    if cur.is_none() {
        // Position out of bounds.
        return;
    }
    let cur = cur.unwrap();
    let cur_zero = cur.is_zero();

    if cur_zero && pen.width == 1 {
        // Find the previous wide cell.
        let mut wide: Option<(usize, usize)> = None;
        for i in 1..5 {
            if m.x - i < 0 {
                break;
            }
            let w = scr.cell_at((m.x - i) as usize, m.y as usize);
            if let Some(w) = w {
                if !w.is_zero() && w.width > 1 {
                    wide = Some(((m.x - i) as usize, m.y as usize));
                    break;
                }
            }
        }

        if let Some((wx, wy)) = wide {
            // Found a wide cell, make all cells blank.
            let mut wc = scr.cell_at(wx, wy).unwrap().clone();
            wc.empty();
            scr.set_cell(wx, wy, Some(&wc));
        }
    }

    // Can we fit the cell?
    let mut fit = true;
    let w = pen.width;
    if w > 1 {
        if cur_zero || cur.width > 1 {
            fit = false;
        } else {
            for i in 1..w {
                let c = scr.cell_at(m.x as usize + i, m.y as usize);
                if let Some(c) = c {
                    if c.is_zero() || c.width > 1 {
                        // Position out of bounds or not empty.
                        fit = false;
                        break;
                    }
                } else {
                    fit = false;
                    break;
                }
            }
        }
    }
    if !fit {
        // Cell is too wide, ignore it.
        return;
    }

    scr.set_cell(m.x as usize, m.y as usize, Some(pen));
    scr.render();
    let _ = scr.flush();
}

fn main() {
    let mut t = default_terminal();

    // Start in altscreen mode
    t.screen().enter_alt_screen();

    if let Err(e) = t.start() {
        eprintln!("failed to start program: {e}");
        std::process::exit(1);
    }

    {
        let scr = t.screen();
        scr.write_string(&charming_x_ansi::mode::set_mode(&[
            Mode::Dec(MODE_MOUSE_BUTTON_EVENT),
            Mode::Dec(MODE_MOUSE_EXT_SGR),
            Mode::Dec(MODE_FOCUS_EVENT),
        ]));
    }

    let mut prev_help_buf: Option<charming_ultraviolet::Buffer> = None;
    let mut showing_help = true;
    display_help(t.screen(), showing_help, &mut prev_help_buf);

    const DEFAULT_CHAR: &str = "█";
    let mut pen = empty_cell();
    pen.content = DEFAULT_CHAR.to_string();


    'events: loop {
        let ev = t.events().recv();
        let Ok(ev) = ev else { break };
        match ev {
            DecodedEvent::WindowSize(s) => {
                let scr = t.screen();
                if showing_help {
                    display_help(scr, false, &mut prev_help_buf);
                }
                scr.resize(s.width, s.height);
                if showing_help {
                    display_help(scr, showing_help, &mut prev_help_buf);
                }
            }
            DecodedEvent::KeyPress(k) => {
                if showing_help {
                    let scr = t.screen();
                    showing_help = false;
                    display_help(scr, false, &mut prev_help_buf);
                    continue;
                }
                if k.match_string(&["ctrl+c"]) {
                    break 'events;
                } else if k.match_string(&["alt+esc"]) {
                    pen.style = Default::default();
                    pen.content = DEFAULT_CHAR.to_string();
                }
                if k.match_string(&["esc"]) {
                    clear_screen(t.screen());
                } else if k.match_string(&["ctrl+h"]) {
                    showing_help = true;
                    display_help(t.screen(), showing_help, &mut prev_help_buf);
                } else {
                    let text = k.text.clone();
                    if text.is_empty() {
                        continue;
                    }
                    let ch = text.chars().next().unwrap_or(' ');
                    if text.len() == 1 && ch.is_ascii_digit() {
                        let fg = (ch as u32) - '0' as u32;
                        pen.style.fg = Some(charming_x_ansi::style::Color::Basic(fg as u8));
                    } else {
                        pen.content = text.clone();
                        pen.width = charming_x_ansi::width::string_width(&text).max(1);
                    }
                }
            }
            DecodedEvent::MouseClick(m) => {
                if showing_help {
                    continue;
                }
                run_draw(t.screen(), &pen, &m);
            }
            DecodedEvent::MouseMotion(m) => {
                if showing_help || m.button == charming_ultraviolet::mouse::MOUSE_NONE {
                    continue;
                }
                run_draw(t.screen(), &pen, &m);
            }
            _ => {}
        }
    }

    {
        let scr = t.screen();
        scr.write_string(&charming_x_ansi::mode::reset_mode(&[
            Mode::Dec(MODE_MOUSE_BUTTON_EVENT),
            Mode::Dec(MODE_MOUSE_EXT_SGR),
            Mode::Dec(MODE_FOCUS_EVENT),
        ]));
    }
}
