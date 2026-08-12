//! Cleanroom Rust port of upstream Go example: `examples/advanced/tv/main.go`
//! Upstream Target Tag / Version: `v0.0.0-20260703014108-f5a850f9c2b7`

use charming_ultraviolet::cell::empty_cell;
use charming_ultraviolet::decoder::DecodedEvent;
use charming_ultraviolet::screen::{clear, fill_area, rect};
use charming_ultraviolet::terminal::default_terminal;
use charming_ultraviolet::terminal_screen::TerminalScreen;
use charming_x_ansi::color::{RGBColor};
use charming_x_ansi::style::Color;

const BAR_COUNT: usize = 7;
const BOT_BAR_COUNT: usize = 6;

fn rgb(r: u8, g: u8, b: u8) -> Color {
    Color::RGB(RGBColor { r, g, b })
}

fn main() {
    let mut t = default_terminal();

    if let Err(e) = t.start() {
        eprintln!("Error starting terminal: {e}");
        std::process::exit(1);
    }

    t.screen().enter_alt_screen();

    let row_colors: [Vec<[u8; 3]>; 3] = [
        vec![
            [180, 180, 180], // white
            [180, 180, 16],  // yellow
            [16, 180, 180],  // cyan
            [16, 180, 16],   // green
            [180, 16, 180],  // magenta
            [180, 16, 16],   // red
            [16, 16, 180],   // blue
        ],
        vec![
            [16, 16, 180],   // blue
            [16, 16, 16],    // black
            [180, 16, 180],  // magenta
            [16, 16, 16],    // black
            [16, 180, 180],  // cyan
            [16, 16, 16],    // black
            [180, 180, 180], // white
        ],
        vec![
            [16, 70, 106],   // navy
            [235, 235, 235], // fullWhite
            [72, 16, 116],   // purple
            [16, 16, 16],    // black
            [16, 16, 16],    // black
            [16, 16, 16],    // black
        ],
    ];

    let display = |scr: &mut TerminalScreen| {
        clear(scr);

        let area = scr.bounds();
        let top_row = rect(0, 0, area.max.0, (area.max.1 * 66) / 100);
        let mid_row = rect(0, top_row.max.1, area.max.0, (area.max.1 * 8) / 100);
        let bot_row = rect(0, mid_row.max.1, area.max.0, (area.max.1 * 26) / 100);

        let bar_width = if BAR_COUNT > 0 { top_row.max.0 / BAR_COUNT } else { 0 };
        for (i, row) in [top_row, mid_row].iter().enumerate() {
            for j in 0..BAR_COUNT {
                let bar = rect(j * bar_width, row.min.1, (j + 1) * bar_width, row.max.1);
                let mut cell = empty_cell();
                let c = row_colors[i][j % row_colors[i].len()];
                cell.style.bg = Some(rgb(c[0], c[1], c[2]));
                fill_area(scr, Some(&cell), bar);
            }
        }

        let bot_bar_width = if BOT_BAR_COUNT > 0 { bot_row.max.0 / BOT_BAR_COUNT } else { 0 };
        for i in 0..BOT_BAR_COUNT {
            let bar = rect(i * bot_bar_width, bot_row.min.1, (i + 1) * bot_bar_width, bot_row.max.1);
            let mut cell = empty_cell();
            let c = row_colors[2][i % row_colors[2].len()];
            cell.style.bg = Some(rgb(c[0], c[1], c[2]));
            fill_area(scr, Some(&cell), bar);
        }

        // Special case for the before last bar
        const SPECIAL_ROW: usize = 5;
        let sub_bar_width = if bar_width > 0 { bar_width / 3 } else { 0 };
        for i in 0..3usize {
            let bar = rect(
                SPECIAL_ROW * bar_width + i * sub_bar_width,
                bot_row.min.1,
                sub_bar_width,
                bot_row.max.1,
            );
            let mut cell = empty_cell();
            match i {
                0 => cell.style.bg = Some(rgb(0, 0, 0)), // fullBlack
                1 => continue,
                _ => cell.style.bg = Some(rgb(26, 26, 26)), // lightBlack
            }
            fill_area(scr, Some(&cell), bar);
        }

        scr.render();
        let _ = scr.flush();
    };

    // initial render
    display(t.screen());

    'events: loop {
        let ev = t.events().recv();
        let Ok(ev) = ev else { break };
        match ev {
            DecodedEvent::WindowSize(s) => {
                let scr = t.screen();
                scr.resize(s.width, s.height);
                display(scr);
            }
            DecodedEvent::KeyPress(k) => {
                if k.match_string(&["q", "ctrl+c"]) {
                    break 'events;
                }
            }
            _ => {}
        }
    }

    let _ = t.stop();
}
