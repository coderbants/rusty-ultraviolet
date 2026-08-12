//! Cleanroom Rust port of upstream Go example: `examples/advanced/boxes/main.go`
//! Upstream Target Tag / Version: `v0.0.0-20260703014108-f5a850f9c2b7`

use charming_ultraviolet::cell::Cell;
use charming_ultraviolet::decoder::DecodedEvent;
use charming_ultraviolet::mouse::MouseMode;
use charming_ultraviolet::style::Style;
use charming_ultraviolet::terminal::{new_terminal, Options};
use charming_ultraviolet::window::{new_window, pos, Window};
use charming_ultraviolet::{console::new_console, screen::Rectangle};
use charming_x_ansi::style::Color;
use std::rc::Rc;

const ROOT_ID: &str = "root";

struct AppWindow {
    id: String,
    win: Rc<Window>,
    z: i64,
    st: Style,
    ctx: charming_ultraviolet::screen_context::Context<'static>,
}

impl AppWindow {
    fn bounds(&self) -> Rectangle {
        self.win.bounds()
    }

    fn draw(&self, scr: &mut dyn charming_ultraviolet::buffer::Screen, rect: Rectangle) {
        self.win.draw(scr, rect);
    }
}

struct App {
    root: AppWindow,
    wins: std::collections::HashMap<String, AppWindow>,
    zwins: Vec<AppWindow>,
    active: String,
    quit: bool,
    last_clicked: String,
    mouse_down: bool,
}

impl App {
    fn new_app(width: usize, height: usize) -> App {
        let _scr = new_window(
            width,
            height,
            Some(charming_x_ansi::method::WidthMethod::GraphemeWidth),
        );
        let root_win = new_window(
            0,
            0,
            Some(charming_x_ansi::method::WidthMethod::GraphemeWidth),
        );
        let mut root_win = Rc::try_unwrap(root_win).expect("fresh window");
        root_win.resize(width, height);
        let root_rc = Rc::new(root_win);
        let ctx = charming_ultraviolet::screen_context::new_context(Box::new(WindowDrawProxy(
            root_rc.clone(),
        )));
        let root = AppWindow {
            id: ROOT_ID.to_string(),
            win: root_rc,
            z: 0,
            st: Style::default(),
            ctx,
        };
        let zwins = vec![AppWindow {
            id: ROOT_ID.to_string(),
            win: root.win.clone(),
            z: 0,
            st: Style::default(),
            ctx: charming_ultraviolet::screen_context::new_context(Box::new(WindowDrawProxy(
                root.win.clone(),
            ))),
        }];
        App {
            root,
            wins: std::collections::HashMap::new(),
            zwins,
            active: ROOT_ID.to_string(),
            quit: false,
            last_clicked: String::new(),
            mouse_down: false,
        }
    }

    fn bring_to_front(&mut self, id: &str) {
        if !self.wins.contains_key(id) {
            return;
        }
        self.zwins.retain(|zw| zw.id != id);
        if let Some(aw) = self.wins.get(id) {
            let mut aw = AppWindow {
                id: aw.id.clone(),
                win: aw.win.clone(),
                z: aw.z,
                st: aw.st.clone(),
                ctx: charming_ultraviolet::screen_context::new_context(Box::new(WindowDrawProxy(
                    aw.win.clone(),
                ))),
            };
            aw.z = self.zwins.len() as i64;
            self.zwins.push(aw);
        }
    }

    fn create_window(
        &mut self,
        id: &str,
        x: usize,
        y: usize,
        width: usize,
        height: usize,
    ) -> Option<AppWindow> {
        let style = Style {
            bg: Some(Color::Indexed((rand_range(256)) as u8)),
            ..Default::default()
        };

        let win = self
            .root
            .win
            .new_window(x as i64, y as i64, width as i64, height as i64);
        let mut win = Rc::try_unwrap(win).expect("fresh window");
        win.resize(width, height);
        let cell = Cell {
            content: " ".to_string(),
            width: 1,
            style: style.clone(),
            ..Cell::default()
        };
        win.fill(Some(&cell));
        let win = Rc::new(win);

        let aw = AppWindow {
            id: id.to_string(),
            win: win.clone(),
            z: self.zwins.len() as i64,
            st: style,
            ctx: charming_ultraviolet::screen_context::new_context(Box::new(WindowDrawProxy(
                win.clone(),
            ))),
        };
        self.wins.insert(
            id.to_string(),
            AppWindow {
                id: aw.id.clone(),
                win: aw.win.clone(),
                z: aw.z,
                st: aw.st.clone(),
                ctx: charming_ultraviolet::screen_context::new_context(Box::new(WindowDrawProxy(
                    aw.win.clone(),
                ))),
            },
        );
        self.zwins.push(AppWindow {
            id: aw.id.clone(),
            win: aw.win.clone(),
            z: aw.z,
            st: aw.st.clone(),
            ctx: charming_ultraviolet::screen_context::new_context(Box::new(WindowDrawProxy(
                aw.win.clone(),
            ))),
        });
        Some(aw)
    }

    fn destroy_window(&mut self, id: &str) {
        if !self.wins.contains_key(id) {
            return;
        }
        eprintln!("destroying window {id:?}");
        self.wins.remove(id);
        self.zwins.retain(|zw| zw.id != id);
        if self.active == id {
            self.active = ROOT_ID.to_string();
        }
    }

    fn set_active_id(&mut self, id: &str) {
        self.active = id.to_string();
    }

    fn active_id(&self) -> &str {
        &self.active
    }

    fn window(&self, id: &str) -> Option<&Rc<Window>> {
        if id == ROOT_ID {
            return Some(&self.root.win);
        }
        self.wins.get(id).map(|aw| &aw.win)
    }

    fn parent_id(&self, id: &str) -> String {
        let Some(win) = self.window(id) else {
            return String::new();
        };
        let Some(parent) = win.parent() else {
            return String::new();
        };
        for aw in self.zwins.iter().chain(std::iter::once(&self.root)) {
            if Rc::ptr_eq(&aw.win, parent) {
                return aw.id.clone();
            }
        }
        String::new()
    }

    fn draw(&mut self, scr: &mut dyn charming_ultraviolet::buffer::Screen, _area: Rectangle) {
        self.zwins.sort_by_key(|aw| aw.z);
        self.root.win.clear();
        for zw in &self.zwins {
            if zw.id == ROOT_ID {
                continue;
            }
            zw.draw(&mut WindowDrawProxy(self.root.win.clone()), zw.bounds());
        }
        self.root.draw(scr, self.root.bounds());
    }

    fn handle_event(&mut self, id: &str, ev: &DecodedEvent) -> bool {
        match ev {
            DecodedEvent::KeyPress(k) => {
                if k.match_string(&["ctrl+c", "esc"]) {
                    self.quit = true;
                    return true;
                }
                let Some(aw) = self.wins.get_mut(&self.active) else {
                    return false;
                };
                let active_win = aw.win.clone();
                let st = aw.st.clone();
                let ctx = &mut aw.ctx;
                ctx.set_style(st);
                ctx.set_foreground(Some(Color::Basic(0))); // color.Black
                if k.match_string(&["backspace"]) {
                    let (mut x, mut y) = ctx.position();
                    x -= 1;
                    if x < 0 {
                        x = active_win.bounds().dx() as i64 - 1;
                        y -= 1;
                        if y < 0 {
                            y = 0;
                        }
                    }
                    ctx.set_position(x, y);
                    let _ = ctx.print(format_args!(" "));
                    ctx.set_position(x, y);
                    return true;
                }
                if k.match_string(&["enter"]) {
                    let _ = ctx.print(format_args!("\n"));
                    return true;
                }
                if !k.text.is_empty() {
                    let _ = ctx.print(format_args!("{}", k.text));
                    return true;
                }
            }
            DecodedEvent::MouseMotion(m) => {
                if self.mouse_down && self.last_clicked == id {
                    let bounds = self.window(id).map(|w| w.bounds()).unwrap_or(Rectangle {
                        min: (0, 0),
                        max: (0, 0),
                    });
                    let new_x = m.x as i64 - bounds.dx() as i64 / 2;
                    let new_y = m.y as i64 - bounds.dy() as i64 / 2;
                    eprintln!("moving window {id:?} to ({new_x}, {new_y})");
                    if let Some(w) = self.window(id) {
                        let mut w = w.clone_window();
                        w.move_to(new_x, new_y);
                    }
                    return true;
                }
            }
            DecodedEvent::MouseRelease(_) => {
                self.mouse_down = false;
                self.last_clicked.clear();
                return true;
            }
            DecodedEvent::MouseClick(m) => {
                self.mouse_down = true;
                let is_left = m.button == charming_ultraviolet::mouse::MOUSE_LEFT;
                let is_right = m.button == charming_ultraviolet::mouse::MOUSE_RIGHT;
                if is_left {
                    eprintln!("mouse left click for {id:?} at ({}, {})", m.x, m.y);
                    let mut clicked: Option<String> = None;
                    for i in (0..self.zwins.len()).rev() {
                        let zw = &self.zwins[i];
                        if zw.id == ROOT_ID {
                            continue;
                        }
                        let p = pos(m.x as i64, m.y as i64);
                        if p.in_rect(zw.bounds()) {
                            clicked = Some(zw.id.clone());
                            break;
                        }
                    }
                    if let Some(zid) = clicked {
                        eprintln!("clicked window {zid} at ({}, {})", m.x, m.y);
                        self.set_active_id(&zid);
                        self.bring_to_front(&zid);
                        self.last_clicked = zid;
                        return true;
                    }
                    eprintln!("no window clicked on at ({}, {})", m.x, m.y);
                    if id == ROOT_ID {
                        eprintln!("clicked root window at ({}, {})", m.x, m.y);
                        let root_size_x = self.root.bounds().dx();
                        let root_size_y = self.root.bounds().dy();
                        let width = rand_range(20);
                        let height = rand_range(10);
                        if width == 0 || height == 0 {
                            // Try again
                            return self.handle_event(id, ev);
                        }
                        let mut x = (m.x as i64 - width as i64 / 2).max(0);
                        let mut y = (m.y as i64 - height as i64 / 2).max(0);
                        if x + width as i64 > root_size_x as i64 {
                            x = root_size_x as i64 - width as i64;
                        }
                        if y + height as i64 > root_size_y as i64 {
                            y = root_size_y as i64 - height as i64;
                        }
                        let win_id = format!("win-{}", self.wins.len());
                        self.create_window(
                            &win_id,
                            x as usize,
                            y as usize,
                            width as usize,
                            height as usize,
                        );
                        self.set_active_id(&win_id);
                        return true;
                    }
                } else if is_right {
                    let mut to_destroy: Vec<String> = Vec::new();
                    for i in (0..self.zwins.len()).rev() {
                        let zw = &self.zwins[i];
                        let p = pos(m.x as i64, m.y as i64);
                        if p.in_rect(zw.bounds()) && zw.id != ROOT_ID {
                            to_destroy.push(zw.id.clone());
                        }
                    }
                    for zid in to_destroy {
                        eprintln!(
                            "right-clicked window {zid:?} at ({}, {}), destroying",
                            m.x, m.y
                        );
                        self.destroy_window(&zid);
                    }
                    return true;
                }
            }
            _ => {}
        }
        false
    }

    fn run(&mut self, environ: Vec<String>) -> Result<(), Box<dyn std::error::Error>> {
        let con = new_console(None, None, Some(environ));
        let mut term = new_terminal(Some(con), Some(Options::default()));
        // We're using the alternate screen buffer, we need to ensure we're
        // using the fullscreen and absolute cursor movement flags.
        {
            let scr = term.screen();
            scr.enter_alt_screen();
            scr.hide_cursor();
            scr.set_mouse_mode(MouseMode::MouseModeDrag);
        }

        term.start()?;

        'outer: while !self.quit {
            let ev = term.events().recv();
            let Ok(ev) = ev else { break 'outer };

            if let DecodedEvent::WindowSize(s) = &ev {
                // We need to update our terminal size and root window size.
                let scr = term.screen();
                scr.resize(s.width, s.height);
                let mut root_win =
                    Rc::try_unwrap(self.root.win.clone()).unwrap_or_else(|rc| (*rc).clone_window());
                root_win.resize(s.width, s.height);
                self.root.win = Rc::new(root_win);
            }

            let mut focused_id = self.active_id().to_string();
            if focused_id.is_empty() {
                // Ignore events if no window is focused.
                continue;
            }

            let mut handled = false;
            loop {
                if self.handle_event(&focused_id, &ev) {
                    handled = true;
                    break;
                }
                let parent_id = self.parent_id(&focused_id);
                if !parent_id.is_empty() {
                    eprintln!(
                        "event not handled by {focused_id:?}, passing to parent {parent_id:?}"
                    );
                    focused_id = parent_id;
                } else {
                    break;
                }
            }
            let _ = handled;

            let scr = term.screen();
            let _ = scr.display(Some(&mut DrawableApp(self)));
        }

        let _ = term.stop();
        Ok(())
    }
}

/// A screen adapter that forwards to the window's own Screen impl without
/// requiring an exclusive borrow of the shared `Rc<Window>`.
struct WindowDrawProxy(Rc<Window>);

impl charming_ultraviolet::buffer::Screen for WindowDrawProxy {
    fn bounds(&self) -> Rectangle {
        self.0.bounds()
    }
    fn cell_at(&self, _x: usize, _y: usize) -> Option<&Cell> {
        None
    }
    fn set_cell(&mut self, x: usize, y: usize, c: Option<&Cell>) {
        self.0.set_cell(
            x,
            y,
            c.cloned()
                .unwrap_or_else(charming_ultraviolet::cell::empty_cell),
        );
    }
    fn width_method(&self) -> charming_x_ansi::method::WidthMethod {
        self.0.width_method()
    }
    fn as_any_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        None
    }
}

struct DrawableApp<'a>(&'a mut App);

impl charming_ultraviolet::Drawable for DrawableApp<'_> {
    fn draw(&mut self, scr: &mut dyn charming_ultraviolet::buffer::Screen, area: Rectangle) {
        self.0.draw(scr, area);
    }
}

fn rand_range(n: u64) -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let t = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    (t.subsec_nanos() as u64 ^ (t.as_nanos() as u64)) % n
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let (width, height) = term_size()?;
    let environ = std::env::vars().map(|(k, v)| format!("{k}={v}")).collect();
    let mut app = App::new_app(width, height);
    app.run(environ)
}

fn term_size() -> Result<(usize, usize), Box<dyn std::error::Error>> {
    let mut con = charming_ultraviolet::console::default_console();
    let ws = con.get_winsize()?;
    Ok((ws.col as usize, ws.row as usize))
}
