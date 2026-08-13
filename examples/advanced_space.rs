//! Cleanroom Rust port of upstream Go example: `examples/advanced/space/main.go`
//! Upstream Target Tag / Version: `v0.0.0-20260703014108-f5a850f9c2b7`

use rusty_ultraviolet::cell::{empty_cell, Cell};
use rusty_ultraviolet::decoder::DecodedEvent;
use rusty_ultraviolet::screen::clear;
use rusty_ultraviolet::styled::new_styled_string;
use rusty_ultraviolet::terminal::default_terminal;
use rusty_x_ansi::color::RGBColor;
use rusty_x_ansi::style::Color;
use std::time::Instant;

fn setup_colors(width: usize, height: usize) -> Vec<Vec<[u8; 3]>> {
    let height = height * 2; // double height for half blocks
    let mut colors: Vec<Vec<[u8; 3]>> = Vec::with_capacity(height);

    for y in 0..height {
        let mut row: Vec<[u8; 3]> = Vec::with_capacity(width);
        let randomness_factor = (height - y) as f64 / height as f64;

        for _x in 0..width {
            let base_value = randomness_factor * ((height - y) as f64 / height as f64);
            let random_offset = (rand_f64() * 0.2) - 0.1;
            let value = clamp(base_value + random_offset, 0.0, 1.0);

            // Convert value to grayscale color (0-255)
            let gray = (value * 255.0) as u8;
            row.push([gray, gray, gray]);
        }
        colors.push(row);
    }
    colors
}

fn clamp(value: f64, min: f64, max: f64) -> f64 {
    if value < min {
        return min;
    }
    if value > max {
        return max;
    }
    value
}

fn rand_f64() -> f64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let bits = (t.as_nanos() ^ (t.subsec_nanos() as u128) << 32) as u64;
    // 53-bit mantissa like Go's rand.Float64().
    (bits >> 11) as f64 / (1u64 << 53) as f64
}

fn main() {
    let mut t = default_terminal();
    t.screen().enter_alt_screen();

    if let Err(e) = t.start() {
        eprintln!("failed to start terminal: {e}");
        std::process::exit(1);
    }

    let mut frame_count: usize = 0;
    let now = Instant::now();
    let mut fps = 60.0f64;
    let mut fps_frame_count: usize = 0;

    let mut colors: Vec<Vec<[u8; 3]>> = Vec::new();

    // The upstream sends an immediate tick event; a local channel emulates
    // the custom tickEvent type.
    let (tick_tx, tick_rx) = std::sync::mpsc::channel::<()>();
    let _ = tick_tx.send(());

    'events: loop {
        // Merge the tick channel and the terminal events (upstream select
        // over the tick channel and t.Events()). The tick chain is
        // self-sustaining only after a rendered frame: an initial tick that
        // arrives before the first window-size event is skipped without
        // re-sending (upstream behaviour, reproduced exactly).
        let tick_ready = tick_rx.try_recv().is_ok();
        if !tick_ready {
            match t
                .events()
                .recv_timeout(std::time::Duration::from_millis(16))
            {
                Ok(ev) => {
                    let scr = t.screen();
                    match ev {
                        DecodedEvent::KeyPress(k) => match k.string().as_str() {
                            "q" | "ctrl+c" => break 'events,
                            _ => {}
                        },
                        DecodedEvent::WindowSize(s) => {
                            colors = setup_colors(s.width, s.height);
                            scr.resize(s.width, s.height);
                        }
                        _ => {}
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                Err(_) => break 'events,
            }
        }
        if tick_ready {
            if colors.is_empty() {
                continue;
            }

            frame_count += 1;
            fps_frame_count += 1;
            let scr = t.screen();
            clear(scr);

            let bounds = scr.bounds();

            // Title
            let mut title = new_styled_string(&format!("\x1b[1mSpace / FPS: {fps:.1}\x1b[m"));
            let _ = scr.display(Some(&mut title));

            // Color display
            let width = bounds.dx();
            let height = bounds.dy();
            for y in 1..height {
                for x in 0..width {
                    let xi = (x + frame_count) % width;
                    let fg = colors[y * 2][xi];
                    let bg = colors[y * 2 + 1][xi];
                    let mut cell = empty_cell();
                    cell.style.fg = Some(Color::RGB(RGBColor {
                        r: fg[0],
                        g: fg[1],
                        b: fg[2],
                    }));
                    cell.style.bg = Some(Color::RGB(RGBColor {
                        r: bg[0],
                        g: bg[1],
                        b: bg[2],
                    }));
                    scr.set_cell(
                        x,
                        y,
                        Some(&Cell {
                            content: "▀".to_string(),
                            style: cell.style.clone(),
                            width: 1,
                            ..Cell::default()
                        }),
                    );
                }
            }

            scr.render();
            let _ = scr.flush();

            let elapsed = now.elapsed().as_secs_f64();
            if elapsed > 1.0 && fps_frame_count > 2 {
                fps = fps_frame_count as f64 / elapsed;
                fps_frame_count = 0;
            }

            let _ = tick_tx.send(());
        }
    }

    let _ = t.stop();
}
