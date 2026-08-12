//! Cleanroom Rust port of upstream Go example: `examples/advanced/layout/main.go`
//! Upstream Target Tag / Version: `v0.0.0-20260703014108-f5a850f9c2b7`
//!
//! The Lip Gloss layout showcase rendered through the ultraviolet terminal.

use charming_lipgloss::color::Color;
use charming_lipgloss::position::place;
use charming_lipgloss::style::Style;
use charming_lipgloss::whitespace::{with_whitespace_chars, with_whitespace_style};
use charming_lipgloss::{border, join, size, BOTTOM, CENTER, LEFT, RIGHT, TOP};
use charming_ultraviolet::decoder::DecodedEvent;
use charming_ultraviolet::styled::new_styled_string;
use charming_ultraviolet::terminal::default_terminal;

const WIDTH: usize = 96;
const COLUMN_WIDTH: usize = 30;

/// Minimal port of the `lucasb-eyer/go-colorful` Luv conversions used by the
/// example (sRGB <-> XYZ <-> CIE L*u*v*, D65 reference white).
mod colorful {
    const REF_WHITE_X: f64 = 0.95047;
    const REF_WHITE_Y: f64 = 1.0;
    const REF_WHITE_Z: f64 = 1.08883;

    #[derive(Clone, Copy)]
    pub struct Color {
        pub r: u8,
        pub g: u8,
        pub b: u8,
    }

    #[derive(Clone, Copy)]
    struct Luv {
        l: f64,
        u: f64,
        v: f64,
    }

    pub fn hex(s: &str) -> Color {
        let s = s.trim_start_matches('#');
        let v = u32::from_str_radix(s, 16).unwrap_or(0);
        Color {
            r: ((v >> 16) & 0xff) as u8,
            g: ((v >> 8) & 0xff) as u8,
            b: (v & 0xff) as u8,
        }
    }

    fn linearize(v: u8) -> f64 {
        let c = v as f64 / 255.0;
        if c <= 0.04045 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    }

    fn de_linearize(v: f64) -> u8 {
        if v <= 0.0031308 {
            (v * 12.92 * 255.0) as u8
        } else {
            (1.055 * v.powf(1.0 / 2.4) * 255.0 - 55.0) as u8
        }
    }

    fn linear_rgb_to_xyz(r: f64, g: f64, b: f64) -> (f64, f64, f64) {
        (
            r * 0.4124564 + g * 0.3575761 + b * 0.1804375,
            r * 0.2126729 + g * 0.7151522 + b * 0.0721750,
            r * 0.0193339 + g * 0.1191920 + b * 0.9503041,
        )
    }

    fn xyz_to_linear_rgb(x: f64, y: f64, z: f64) -> (f64, f64, f64) {
        (
            x * 3.2404542 + y * -1.5371385 + z * -0.4985314,
            x * -0.9692660 + y * 1.8760108 + z * 0.0415560,
            x * 0.0556434 + y * -0.2040259 + z * 1.0572252,
        )
    }

    fn xyz_to_luv(x: f64, y: f64, z: f64) -> Luv {
        let (mut up, mut vp) = (0.0, 0.0);
        if y != 0.0 && x != 0.0 && z != 0.0 {
            up = (4.0 * x) / (x + 15.0 * y + 3.0 * z);
            vp = (9.0 * y) / (x + 15.0 * y + 3.0 * z);
        }
        let l = if y != 0.0 {
            116.0 * (y / REF_WHITE_Y).cbrt() - 16.0
        } else {
            0.0
        };
        let uw = (4.0 * REF_WHITE_X) / (REF_WHITE_X + 15.0 * REF_WHITE_Y + 3.0 * REF_WHITE_Z);
        let vw = (9.0 * REF_WHITE_Y) / (REF_WHITE_X + 15.0 * REF_WHITE_Y + 3.0 * REF_WHITE_Z);
        Luv {
            l,
            u: 13.0 * l * (up - uw),
            v: 13.0 * l * (vp - vw),
        }
    }

    fn luv_to_xyz(luv: Luv) -> (f64, f64, f64) {
        if luv.l == 0.0 {
            // Black corner case.
            return (0.0, 0.0, 0.0);
        }
        let uw = (4.0 * REF_WHITE_X) / (REF_WHITE_X + 15.0 * REF_WHITE_Y + 3.0 * REF_WHITE_Z);
        let vw = (9.0 * REF_WHITE_Y) / (REF_WHITE_X + 15.0 * REF_WHITE_Y + 3.0 * REF_WHITE_Z);
        let up = luv.u / (13.0 * luv.l) + uw;
        let vp = luv.v / (13.0 * luv.l) + vw;
        let mut y = REF_WHITE_Y * ((luv.l + 16.0) / 116.0).powi(3);
        if y <= 0.008856 {
            y *= 903.3;
        }
        let x = -9.0 * y * up / ((up - 4.0) * vp - up * vp);
        let z = (9.0 * y - 15.0 * vp * y - vp * x) / (3.0 * vp);
        (x, y, z)
    }

    impl Color {
        fn luv(&self) -> Luv {
            let r = linearize(self.r);
            let g = linearize(self.g);
            let b = linearize(self.b);
            let (x, y, z) = linear_rgb_to_xyz(r, g, b);
            xyz_to_luv(x, y, z)
        }

        fn from_luv(luv: Luv) -> Color {
            let (x, y, z) = luv_to_xyz(luv);
            let (r, g, b) = xyz_to_linear_rgb(x, y, z);
            Color {
                r: de_linearize(r.clamp(0.0, 1.0)),
                g: de_linearize(g.clamp(0.0, 1.0)),
                b: de_linearize(b.clamp(0.0, 1.0)),
            }
        }

        /// BlendLuv blends two colors in the Luv color space by t.
        pub fn blend_luv(&self, other: &Color, t: f64) -> Color {
            let l1 = self.luv();
            let l2 = other.luv();
            let blended = Luv {
                l: l1.l + t * (l2.l - l1.l),
                u: l1.u + t * (l2.u - l1.u),
                v: l1.v + t * (l2.v - l1.v),
            };
            Color::from_luv(blended)
        }

        /// Hex returns the hex string representation of the color.
        pub fn hex(&self) -> String {
            format!("#{:02X}{:02X}{:02X}", self.r, self.g, self.b)
        }
    }
}

fn color_grid(x_steps: usize, y_steps: usize) -> Vec<Vec<String>> {
    let x0y0 = colorful::hex("#F25D94");
    let x1y0 = colorful::hex("#EDFF82");
    let x0y1 = colorful::hex("#643AFF");
    let x1y1 = colorful::hex("#14F9D5");

    let mut x0: Vec<colorful::Color> = Vec::new();
    for i in 0..y_steps {
        x0.push(x0y0.blend_luv(&x0y1, i as f64 / y_steps as f64));
    }
    let mut x1: Vec<colorful::Color> = Vec::new();
    for i in 0..y_steps {
        x1.push(x1y0.blend_luv(&x1y1, i as f64 / y_steps as f64));
    }

    let mut grid: Vec<Vec<String>> = Vec::new();
    for x in 0..y_steps {
        let y0 = x0[x];
        let mut row: Vec<String> = Vec::new();
        for y in 0..x_steps {
            row.push(y0.blend_luv(&x1[x], y as f64 / x_steps as f64).hex());
        }
        grid.push(row);
    }
    grid
}

fn apply_gradient(base: Style, input: &str, from: &str, to: &str) -> String {
    let chars: Vec<char> = input.chars().collect();
    let a = colorful::hex(to);
    let b = colorful::hex(from);
    let mut output = String::new();
    for i in 0..chars.len() {
        let t = if chars.len() > 1 {
            i as f64 / (chars.len() - 1) as f64
        } else {
            0.0
        };
        let hex = a.blend_luv(&b, t).hex();
        output.push_str(
            &base
                .clone()
                .foreground_color(Color::parse(&hex))
                .render(&chars[i].to_string()),
        );
    }
    output
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let has_dark_bg = charming_lipgloss::compat::has_dark_background();
    let light_dark = |light: &str, dark: &str| -> String {
        if has_dark_bg {
            dark.to_string()
        } else {
            light.to_string()
        }
    };

    let subtle = light_dark("#D9DCCF", "#383838");
    let highlight = light_dark("#874BFD", "#7D56F4");
    let special = light_dark("#43BF6D", "#73F59F");

    let divider = Style::new()
        .set_string(&["•"])
        .padding(&[0, 1])
        .foreground_color(Color::parse(&subtle))
        .string();
    let url = |s: &str| {
        Style::new()
            .foreground_color(Color::parse(&special))
            .render(s)
    };

    let active_tab_border = charming_lipgloss::border::Border {
        top: "─".to_string(),
        bottom: " ".to_string(),
        left: "│".to_string(),
        right: "│".to_string(),
        top_left: "╭".to_string(),
        top_right: "╮".to_string(),
        bottom_left: "┘".to_string(),
        bottom_right: "└".to_string(),
        ..Default::default()
    };
    let tab_border = charming_lipgloss::border::Border {
        top: "─".to_string(),
        bottom: "─".to_string(),
        left: "│".to_string(),
        right: "│".to_string(),
        top_left: "╭".to_string(),
        top_right: "╮".to_string(),
        bottom_left: "┴".to_string(),
        bottom_right: "┴".to_string(),
        ..Default::default()
    };

    let tab = Style::new()
        .border(tab_border.clone(), &[true])
        .border_foreground(&[&highlight])
        .padding(&[0, 1]);
    let active_tab = tab.clone().border(active_tab_border, &[true]);
    let tab_gap = tab
        .clone()
        .border_top(false)
        .border_left(false)
        .border_right(false);

    let title_style = Style::new()
        .margin_left(1)
        .margin_right(5)
        .padding(&[0, 1])
        .italic(true)
        .foreground_color(Color::parse("#FFF7DB"))
        .set_string(&["Lip Gloss"]);

    let desc_style = Style::new().margin_top(1);
    let info_style = Style::new()
        .border_style(border::normal_border())
        .border_top(true)
        .border_foreground(&[&subtle]);

    let dialog_box_style = Style::new()
        .border(border::rounded_border(), &[true])
        .border_foreground(&["#874BFD"])
        .padding(&[1, 0])
        .border_top(true)
        .border_left(true)
        .border_right(true)
        .border_bottom(true);

    let button_style = Style::new()
        .foreground_color(Color::parse("#FFF7DB"))
        .background_color(Color::parse("#888B7E"))
        .padding(&[0, 3])
        .margin_top(1);
    let active_button_style = button_style
        .clone()
        .foreground_color(Color::parse("#FFF7DB"))
        .background_color(Color::parse("#F25D94"))
        .margin_right(2)
        .underline(true);

    let list = Style::new()
        .border(border::normal_border(), &[false, true, false, false])
        .border_foreground(&[&subtle])
        .margin_right(2)
        .height(8)
        .width(COLUMN_WIDTH + 1);

    let list_header = |s: &str| {
        Style::new()
            .border_style(border::normal_border())
            .border_bottom(true)
            .border_foreground(&[&subtle])
            .margin_right(2)
            .render(s)
    };
    let list_item = |s: &str| Style::new().padding_left(2).render(s);
    let check_mark = Style::new()
        .set_string(&["✓"])
        .foreground_color(Color::parse(&special))
        .padding_right(1)
        .string();
    let list_done = |s: &str| {
        check_mark.clone()
            + &Style::new()
                .strikethrough(true)
                .foreground_color(Color::parse(&light_dark("#969B86", "#696969")))
                .render(s)
    };

    let history_style = Style::new()
        .align(&[LEFT])
        .foreground_color(Color::parse("#FAFAFA"))
        .background_color(Color::parse(&highlight))
        .margin(&[1, 3, 0, 0])
        .padding(&[1, 2])
        .height(19)
        .width(COLUMN_WIDTH);

    let status_nugget = Style::new()
        .foreground_color(Color::parse("#FFFDF5"))
        .padding(&[0, 1]);
    let status_bar_style = Style::new()
        .foreground_color(Color::parse(&light_dark("#343433", "#C1C6B2")))
        .background_color(Color::parse(&light_dark("#D9DCCF", "#353533")));
    let status_style = status_bar_style
        .clone()
        .foreground_color(Color::parse("#FFFDF5"))
        .background_color(Color::parse("#FF5F87"))
        .padding(&[0, 1])
        .margin_right(1);
    let encoding_style = status_nugget
        .clone()
        .background_color(Color::parse("#A550DF"))
        .align(&[RIGHT]);
    let status_text = status_bar_style.clone();
    let fish_cake_style = status_nugget.background_color(Color::parse("#6124DF"));
    let mut doc_style = Style::new().padding(&[1, 2, 1, 2]);

    let mut doc = String::new();

    // Tabs.
    {
        let t1 = active_tab.render("Lip Gloss");
        let t2 = tab.render("Blush");
        let t3 = tab.render("Eye Shadow");
        let t4 = tab.render("Mascara");
        let t5 = tab.render("Foundation");
        let row = join::join_horizontal(
            TOP,
            &[
                t1.as_str(),
                t2.as_str(),
                t3.as_str(),
                t4.as_str(),
                t5.as_str(),
            ],
        );
        let gap = tab_gap
            .render(&" ".repeat((WIDTH as i64 - size::width(&row) as i64 - 2).max(0) as usize));
        let row = join::join_horizontal(BOTTOM, &[row.as_str(), gap.as_str()]);
        doc.push_str(&row);
        doc.push_str("\n\n");
    }

    // Title + color grid.
    {
        let colors = color_grid(1, 5);
        let mut title = String::new();
        for (i, v) in colors.iter().enumerate() {
            const OFFSET: usize = 2;
            title.push_str(
                &title_style
                    .clone()
                    .margin_left(i * OFFSET)
                    .background_color(Color::parse(&v[0]))
                    .string(),
            );
            if i < colors.len() - 1 {
                title.push('\n');
            }
        }
        let d1 = desc_style.render("Style Definitions for Nice Terminal Layouts");
        let d2 = info_style.render(&format!(
            "From Charm{divider}{}",
            url("https://github.com/charmbracelet/lipgloss")
        ));
        let desc = join::join_vertical(LEFT, &[d1.as_str(), d2.as_str()]);
        let row = join::join_horizontal(TOP, &[title.as_str(), desc.as_str()]);
        doc.push_str(&row);
        doc.push_str("\n\n");
    }

    let ok_button = active_button_style.render("Yes");
    let cancel_button = button_style.render("Maybe");

    let grad = apply_gradient(
        Style::new(),
        "Are you sure you want to eat marmalade?",
        "#EDFF82",
        "#F25D94",
    );
    let question = Style::new().width(50).align(&[CENTER]).render(&grad);
    let buttons = join::join_horizontal(TOP, &[ok_button.as_str(), cancel_button.as_str()]);
    let dialog_ui = join::join_vertical(CENTER, &[question.as_str(), buttons.as_str()]);

    let dialog = place(
        WIDTH,
        9,
        CENTER,
        CENTER,
        "",
        &[
            with_whitespace_chars("猫咪"),
            with_whitespace_style(Style::new().foreground_color(Color::parse(&subtle))),
        ],
    );
    doc.push_str(&dialog);
    doc.push_str("\n\n");

    let colors = {
        let colors = color_grid(14, 8);
        let mut b = String::new();
        for row in &colors {
            for y in row {
                let s = Style::new()
                    .set_string(&["  "])
                    .background_color(Color::parse(y))
                    .string();
                b.push_str(&s);
            }
            b.push('\n');
        }
        b
    };

    let lh1 = list_header("Citrus Fruits to Try");
    let ld1 = list_done("Grapefruit");
    let ld2 = list_done("Yuzu");
    let li1 = list_item("Citron");
    let li2 = list_item("Kumquat");
    let li3 = list_item("Pomelo");
    let lv1 = join::join_vertical(
        LEFT,
        &[
            lh1.as_str(),
            ld1.as_str(),
            ld2.as_str(),
            li1.as_str(),
            li2.as_str(),
            li3.as_str(),
        ],
    );
    let lh2 = list_header("Actual Lip Gloss Vendors");
    let li4 = list_item("Glossier");
    let li5 = list_item("Claire's Boutique");
    let ld3 = list_done("Nyx");
    let li6 = list_item("Mac");
    let ld4 = list_done("Milk");
    let lv2 = join::join_vertical(
        LEFT,
        &[
            lh2.as_str(),
            li4.as_str(),
            li5.as_str(),
            ld3.as_str(),
            li6.as_str(),
            ld4.as_str(),
        ],
    );
    let l1 = list.render(&lv1);
    let l2 = list.width(COLUMN_WIDTH).render(&lv2);
    let lists = join::join_horizontal(TOP, &[l1.as_str(), l2.as_str()]);
    doc.push_str(&join::join_horizontal(
        TOP,
        &[lists.as_str(), colors.as_str()],
    ));

    {
        const HISTORY_A: &str = "The Romans learned from the Greeks that quinces slowly cooked with honey would \"set\" when cool. The Apicius gives a recipe for preserving whole quinces, stems and leaves attached, in a bath of honey diluted with defrutum: Roman marmalade. Preserves of quince and lemon appear (along with rose, apple, plum and pear) in the Book of ceremonies of the Byzantine Emperor Constantine VII Porphyrogennetos.";
        const HISTORY_B: &str = "Medieval quince preserves, which went by the French name cotignac, produced in a clear version and a fruit pulp version, began to lose their medieval seasoning of spices in the 16th century. In the 17th century, La Varenne provided recipes for both thick and clear cotignac.";
        const HISTORY_C: &str = "In 1524, Henry VIII, King of England, received a \"box of marmalade\" from Mr. Hull of Exeter. This was probably marmelada, a solid quince paste from Portugal, still made and sold in southern Europe today. It became a favourite treat of Anne Boleyn and her ladies in waiting.";

        let h1 = history_style.clone().align(&[RIGHT]).render(HISTORY_A);
        let h2 = history_style.clone().align(&[CENTER]).render(HISTORY_B);
        let h3 = history_style.margin_right(0).render(HISTORY_C);
        doc.push_str(&join::join_horizontal(
            TOP,
            &[h1.as_str(), h2.as_str(), h3.as_str()],
        ));
        doc.push_str("\n\n");
    }

    {
        let w = size::width;
        let light_dark_state = if has_dark_bg { "Dark" } else { "Light" };
        let status_key = status_style.render("STATUS");
        let encoding = encoding_style.render("UTF-8");
        let fish_cake = fish_cake_style.render("🍥 Fish Cake");
        let status_val = status_text
            .width(WIDTH - w(&status_key) - w(&encoding) - w(&fish_cake))
            .render(&format!("Ravishingly {light_dark_state}!"));
        let bar = join::join_horizontal(
            TOP,
            &[
                status_key.as_str(),
                status_val.as_str(),
                encoding.as_str(),
                fish_cake.as_str(),
            ],
        );
        doc.push_str(&status_bar_style.width(WIDTH).render(&bar));
    }

    let mut t = default_terminal();

    if let Err(e) = t.start() {
        eprintln!("starting program: {e}");
        std::process::exit(1);
    }

    let physical_width = t.screen().bounds().dx();
    if physical_width > 0 {
        doc_style = doc_style.max_width(physical_width);
    }

    {
        let scr = t.screen();
        scr.enter_alt_screen();
        use charming_x_ansi::mode::{Mode, MODE_MOUSE_BUTTON_EVENT, MODE_MOUSE_EXT_SGR};
        scr.write_string(&charming_x_ansi::mode::set_mode(&[
            Mode::Dec(MODE_MOUSE_BUTTON_EVENT),
            Mode::Dec(MODE_MOUSE_EXT_SGR),
        ]));
    }

    let dialog_width = size::width(&dialog_ui) + dialog_box_style.get_horizontal_frame_size();
    let dialog_height = size::height(&dialog_ui) + dialog_box_style.get_vertical_frame_size();
    let mut dialog_x = physical_width as i64 / 2 - dialog_width as i64 / 2
        + doc_style.get_vertical_frame_size() as i64
        - 1;
    let mut dialog_y = 12i64;
    let main_doc = doc_style.render(&doc);

    let display = |scr: &mut charming_ultraviolet::terminal_screen::TerminalScreen,
                   dialog_x: i64,
                   dialog_y: i64| {
        // Mirrors the upstream display(): clear once, draw the main doc,
        // draw the dialog box on top, then render and flush once. The box
        // origin is signed: when the pre-resize screen width is 0 the box
        // sits partially off-screen and the left columns are clipped.
        charming_ultraviolet::screen::clear(scr);
        let bounds = scr.bounds();
        let main_ss = new_styled_string(&main_doc);
        main_ss.draw(scr, bounds);
        let box_ss = new_styled_string(&dialog_box_style.render(&dialog_ui));
        box_ss.draw_at(scr, dialog_x, dialog_y, dialog_width, dialog_height);
        scr.render();
        let _ = scr.flush();
    };

    // initial render
    display(t.screen(), dialog_x, dialog_y);

    'events: loop {
        let ev = t.events().recv();
        let Ok(ev) = ev else { break };
        match ev {
            DecodedEvent::WindowSize(s) => {
                t.screen().resize(s.width, s.height);
            }
            DecodedEvent::MouseClick(m) => {
                dialog_x = m.x as i64 - dialog_width as i64 / 2;
                dialog_y = m.y as i64 - dialog_height as i64 / 2;
            }
            DecodedEvent::KeyPress(k) => {
                if k.match_string(&["ctrl+c", "q"]) {
                    break 'events;
                }
                if k.match_string(&["left", "h"]) {
                    dialog_x -= 1;
                }
                if k.match_string(&["down", "j"]) {
                    dialog_y += 1;
                }
                if k.match_string(&["up", "k"]) {
                    dialog_y -= 1;
                }
                if k.match_string(&["right", "l"]) {
                    dialog_x += 1;
                }
            }
            _ => {}
        }
        display(t.screen(), dialog_x, dialog_y);
    }

    {
        let scr = t.screen();
        use charming_x_ansi::mode::{Mode, MODE_MOUSE_BUTTON_EVENT, MODE_MOUSE_EXT_SGR};
        scr.write_string(&charming_x_ansi::mode::reset_mode(&[
            Mode::Dec(MODE_MOUSE_BUTTON_EVENT),
            Mode::Dec(MODE_MOUSE_EXT_SGR),
        ]));
    }

    let _ = t.stop();
    Ok(())
}
