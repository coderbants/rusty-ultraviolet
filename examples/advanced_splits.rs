//! Cleanroom Rust port of upstream Go example: `examples/advanced/splits/main.go`
//! Upstream Target Tag / Version: `v0.0.0-20260703014108-f5a850f9c2b7`

use rusty_ultraviolet::cell::Cell;
use rusty_ultraviolet::decoder::DecodedEvent;
use rusty_ultraviolet::layout::Constraint;
use rusty_ultraviolet::screen::fill_area;
use rusty_ultraviolet::screen_context::new_context;
use rusty_ultraviolet::terminal::default_terminal;
use rusty_ultraviolet::terminal_screen::TerminalScreen;
use rusty_x_ansi::style::Color;

const LINES: &str = "Horizontal Layout Example. Press q to quit
Each line has 2 constraints, plus Min(0) to fill the remaining space.
E.g. the second line of the Len/Min box is [Len(2), Min(2), Min(0)]
Note: constraint labels that don't fit are truncated";

fn zero_rect() -> rusty_ultraviolet::screen::Rectangle {
    rusty_ultraviolet::screen::Rectangle {
        min: (0, 0),
        max: (0, 0),
    }
}

fn cell(bg: Color) -> Cell {
    Cell {
        content: " ".to_string(),
        width: 1,
        style: rusty_ultraviolet::style::Style {
            bg: Some(bg),
            ..Default::default()
        },
        ..Cell::default()
    }
}

fn constraint_label(c: &Constraint) -> String {
    match c {
        Constraint::Len(n) => n.to_string(),
        Constraint::Max(n) => n.to_string(),
        Constraint::Min(n) => n.to_string(),
        Constraint::Percent(n) => n.to_string(),
        Constraint::Ratio { num, den } => format!("{num}/{den}"),
        Constraint::Fill(n) => n.to_string(),
    }
}

fn render_example(
    scr: &mut TerminalScreen,
    area: rusty_ultraviolet::screen::Rectangle,
    constraints: &[Constraint],
) {
    let l = rusty_ultraviolet::layout::horizontal(constraints);
    let splits = l.split(area);
    let zero = rusty_ultraviolet::screen::Rectangle {
        min: (0, 0),
        max: (0, 0),
    };
    let mut r = splits.get(0).copied().unwrap_or(zero);
    let mut b = splits.get(1).copied().unwrap_or(zero);
    let mut g = splits.get(2).copied().unwrap_or(zero);
    let mut areas = [Some(&mut r), Some(&mut b), Some(&mut g)];
    // The split order is r, b, g below (matching the upstream Assign).
    let _ = &mut areas;

    fill_area(scr, Some(&cell(Color::Basic(1))), r);
    fill_area(scr, Some(&cell(Color::Basic(2))), g);
    fill_area(scr, Some(&cell(Color::Basic(4))), b);

    let rw = r.dx();
    let gw = g.dx();
    let bw = b.dx();
    let label_r = constraint_label(&constraints[0]);
    let label_b = constraint_label(&constraints[1]);
    let dots = ".".repeat(gw);
    {
        let mut ctx = new_context(Box::new(&mut *scr));
        ctx.set_background(Some(Color::Basic(1)));
        let n = label_r.len().min(rw);
        ctx.draw_string(&label_r[..n], r.min.0 as i64, r.min.1 as i64);
    }
    {
        let mut ctx = new_context(Box::new(&mut *scr));
        ctx.set_background(Some(Color::Basic(2)));
        ctx.draw_string(&dots, g.min.0 as i64, g.min.1 as i64);
    }
    {
        let mut ctx = new_context(Box::new(&mut *scr));
        ctx.set_background(Some(Color::Basic(4)));
        let n = label_b.len().min(bw);
        ctx.draw_string(&label_b[..n], b.min.0 as i64, b.min.1 as i64);
    }
}

fn render_example_combinations(
    scr: &mut TerminalScreen,
    area: rusty_ultraviolet::screen::Rectangle,
    title: &str,
    pairs: &[([Constraint; 2], rusty_ultraviolet::screen::Rectangle)],
) {
    let rows = rusty_ultraviolet::layout::vertical(&vec![Constraint::Len(1); pairs.len() + 1])
        .with_padding(rusty_ultraviolet::layout::pad(&[1]))
        .split(area);

    if let Some(first) = rows.get(0) {
        let y = first.min.1.saturating_sub(1) as i64;
        let x = first.min.0 as i64;
        let mut ctx = new_context(Box::new(&mut *scr));
        ctx.draw_string(title, x, y);
    }

    for (i, (constraints, _row)) in pairs.iter().enumerate() {
        let row = rows.get(i).copied().unwrap_or(zero_rect());
        render_example(
            scr,
            row,
            &[constraints[0], constraints[1], Constraint::Min(0)],
        );
    }

    let nums = "123456789012";
    let row = rows.get(pairs.len()).copied().unwrap_or(zero_rect());
    let n = nums.len().min(row.dx());
    let mut ctx = new_context(Box::new(&mut *scr));
    ctx.draw_string(&nums[..n], row.min.0 as i64, row.min.1 as i64);
}

fn main() {
    let mut t = default_terminal();
    t.screen().enter_alt_screen();

    if let Err(e) = t.start() {
        eprintln!("failed to start terminal: {e}");
        std::process::exit(1);
    }

    let mut area = rusty_ultraviolet::screen::Rectangle {
        min: (0, 0),
        max: (0, 0),
    };

    let ticker = std::time::Duration::from_secs_f64(1.0 / 60.0);

    let mut next_tick = std::time::Instant::now() + ticker;

    'events: loop {
        if std::time::Instant::now() >= next_tick {
            next_tick += ticker;
            render(t.screen(), area);
            t.screen().render();
            let _ = t.screen().flush();
        }
        let timeout = next_tick.saturating_duration_since(std::time::Instant::now());
        match t
            .events()
            .recv_timeout(timeout.max(std::time::Duration::from_millis(1)))
        {
            Ok(ev) => match ev {
                DecodedEvent::WindowSize(s) => {
                    area = rusty_ultraviolet::screen::Rectangle {
                        min: (0, 0),
                        max: (s.width, s.height),
                    };
                    let scr = t.screen();
                    scr.resize(s.width, s.height);
                    rusty_ultraviolet::screen::clear(scr);
                }
                DecodedEvent::KeyPress(k) if k.match_string(&["ctrl+c", "q"]) => {
                    break 'events;
                }
                _ => {}
            },
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
            Err(_) => break 'events,
        }
    }

    let _ = t.stop();
}

fn render(scr: &mut TerminalScreen, area: rusty_ultraviolet::screen::Rectangle) {
    let mut text_area = area;
    let mut rest = area;

    // Mirrors the upstream `Assign(&textArea, &area)`: the second segment
    // (the remainder) replaces `area`, so the rows below split the
    // remaining space, not the full area.
    let mut assign_areas = [Some(&mut text_area), Some(&mut rest)];
    rusty_ultraviolet::layout::vertical(&[Constraint::Len(7), Constraint::Min(0)])
        .split(area)
        .assign(&mut assign_areas);
    let area = rest;

    {
        let mut ctx = new_context(Box::new(&mut *scr));
        ctx.draw_string_wrapped(LINES, text_area.min.0 as i64, text_area.min.1 as i64);
    }

    let rows = rusty_ultraviolet::layout::vertical(&[
        Constraint::Len(9),
        Constraint::Len(9),
        Constraint::Len(9),
        Constraint::Len(9),
        Constraint::Len(9),
        Constraint::Min(0), // fills remaining space
    ])
    .split(area);

    let mut areas: Vec<rusty_ultraviolet::screen::Rectangle> = Vec::new();

    for row in rows.iter() {
        let cols = rusty_ultraviolet::layout::horizontal(&[
            Constraint::Len(14),
            Constraint::Len(14),
            Constraint::Len(14),
            Constraint::Len(14),
            Constraint::Len(14),
            Constraint::Min(0),
        ])
        .split(*row);
        for c in cols.iter().take(5) {
            // ignore Min(0)
            areas.push(*c);
        }
    }

    struct Named {
        name: &'static str,
        constraints: Vec<Constraint>,
    }

    let examples = vec![
        Named {
            name: "Len",
            constraints: vec![
                Constraint::Len(0),
                Constraint::Len(2),
                Constraint::Len(3),
                Constraint::Len(6),
                Constraint::Len(10),
                Constraint::Len(15),
            ],
        },
        Named {
            name: "Min",
            constraints: vec![
                Constraint::Min(0),
                Constraint::Min(2),
                Constraint::Min(3),
                Constraint::Min(6),
                Constraint::Min(10),
                Constraint::Min(15),
            ],
        },
        Named {
            name: "Max",
            constraints: vec![
                Constraint::Max(0),
                Constraint::Max(2),
                Constraint::Max(3),
                Constraint::Max(6),
                Constraint::Max(10),
                Constraint::Max(15),
            ],
        },
        Named {
            name: "Perc",
            constraints: vec![
                Constraint::Percent(0),
                Constraint::Percent(25),
                Constraint::Percent(50),
                Constraint::Percent(75),
                Constraint::Percent(100),
                Constraint::Percent(150),
            ],
        },
        Named {
            name: "Ratio",
            constraints: vec![
                Constraint::Ratio { num: 0, den: 4 },
                Constraint::Ratio { num: 1, den: 4 },
                Constraint::Ratio { num: 2, den: 4 },
                Constraint::Ratio { num: 3, den: 4 },
                Constraint::Ratio { num: 4, den: 4 },
                Constraint::Ratio { num: 6, den: 4 },
            ],
        },
    ];

    let mut i = 0;
    for a in &examples {
        for b in &examples {
            if i >= areas.len() {
                break;
            }
            let area = areas[i];
            let title = format!("{} | {}", a.name, b.name);
            let mut pairs = Vec::new();
            for (ca, cb) in a.constraints.iter().zip(b.constraints.iter()) {
                pairs.push(([*ca, *cb], area));
            }
            render_example_combinations(scr, area, &title, &pairs);
            i += 1;
        }
    }
}
