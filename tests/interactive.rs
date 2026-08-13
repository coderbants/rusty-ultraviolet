//! Interactive integration tests for the ultraviolet examples, driven
//! through a real pseudo-terminal: mouse clicks and drags, keyboard input,
//! and assertions on the reconstructed on-screen state.

use charming_testkit::PtySession;

/// The package's Cargo target directory. `cargo metadata` is authoritative:
/// it honours `CARGO_TARGET_DIR` from the shared machine-wide Cargo cache
/// (scripts/cargo-env.sh) as well as any workspace config, falling back to
/// the checkout-local `target` only when neither is set.
fn target_dir() -> std::path::PathBuf {
    let out = std::process::Command::new("cargo")
        .args(["metadata", "--format-version=1", "--no-deps"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo metadata");
    if !out.status.success() {
        return std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target");
    }
    let json = String::from_utf8_lossy(&out.stdout);
    let key = "\"target_directory\":";
    json.find(key)
        .and_then(|i| {
            let rest = &json[i + key.len()..].trim_start();
            let q0 = rest.find('"')? + 1;
            let tail = &rest[q0..];
            let q1 = tail.find('"')?;
            Some(std::path::PathBuf::from(&tail[..q1]))
        })
        .unwrap_or_else(|| std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target"))
}

fn ex(name: &str) -> String {
    // Always build this package's example first: with a shared machine-wide
    // target dir, a same-named binary may belong to another project and must
    // never be treated as this package's identity (scripts/cargo-env.sh).
    let build = std::process::Command::new("cargo")
        .args(["build", "--quiet", "--example", name])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .status()
        .expect("cargo build --example");
    assert!(build.success(), "failed to build example {name}");
    let dir = target_dir().join("debug/examples");
    let plain = dir.join(name);
    if plain.exists() {
        return plain.to_string_lossy().into_owned();
    }
    // Newer cargo versions disambiguate example binaries that collide with
    // other targets by appending a metadata hash to the file name; fall back
    // to the first `name-*` match.
    let mut candidates: Vec<_> = std::fs::read_dir(&dir)
        .expect("examples dir")
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with(&format!("{name}-")))
        })
        .collect();
    candidates.sort();
    candidates
        .first()
        .unwrap_or_else(|| panic!("example binary for {name} not built"))
        .to_string_lossy()
        .into_owned()
}

/// The uv runtime's startup (winch self-pipe, terminal queries, raw-mode
/// setup) is racy when many PTY sessions start concurrently, so the
/// interactive tests serialize their spawns with a process-wide lock. This
/// keeps `cargo test` (parallel by default) deterministic.
static PTY_LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();

fn pty_lock() -> &'static std::sync::Mutex<()> {
    PTY_LOCK.get_or_init(|| std::sync::Mutex::new(()))
}

#[test]
fn helloworld_quits_on_any_key() {
    let _pty_guard = pty_lock().lock().unwrap();
    let pty = PtySession::spawn(&ex("helloworld"), &[]).expect("spawn");
    pty.wait_for_text("Hello, World!", 30000).expect("shown");
    pty.press("x").expect("x");
    pty.wait_for_exit(5000).expect("exit");
}

#[test]
fn altscreen_toggles_with_space() {
    let _pty_guard = pty_lock().lock().unwrap();
    let pty = PtySession::spawn(&ex("altscreen"), &[]).expect("spawn");
    pty.wait_for_text("inline mode", 30000).expect("inline");
    pty.press("space").expect("space");
    pty.wait_for_text("alternate screen mode", 30000)
        .expect("alt screen");
    pty.press("space").expect("space");
    pty.wait_for_text("inline mode", 30000)
        .expect("back to inline");
    pty.press("q").expect("q");
    pty.wait_for_exit(5000).expect("exit");
}

#[test]
fn mouse_click_reports_position() {
    let _pty_guard = pty_lock().lock().unwrap();
    let pty = PtySession::spawn(&ex("mouse"), &[]).expect("spawn");
    pty.wait_for_text("Button:", 30000).expect("shown");
    std::thread::sleep(std::time::Duration::from_millis(300));
    // Click at cell (10, 5): the example logs the pixel-adjusted position.
    pty.send(&charming_testkit::keys::mouse_click(10, 5))
        .expect("click");
    pty.wait_for_raw("Position:", 30000).expect("event logged");
    pty.press("q").expect("quit");
    pty.wait_for_exit(5000).expect("exit");
}

#[test]
fn boxes_create_window_on_click() {
    let _pty_guard = pty_lock().lock().unwrap();
    let pty = PtySession::spawn(&ex("advanced_boxes"), &[]).expect("spawn");
    pty.wait_for_raw("?1002", 30000).expect("mouse mode");
    std::thread::sleep(std::time::Duration::from_millis(300));
    // Click in the root window: a new window is created.
    pty.send(&charming_testkit::keys::mouse_click(40, 12))
        .expect("click");
    pty.wait_for_raw("clicked root window", 30000)
        .expect("window created");
    // Click again: now the new window is hit and focused.
    pty.send(&charming_testkit::keys::mouse_click(40, 12))
        .expect("click2");
    pty.wait_for_raw("clicked window", 30000)
        .expect("window focused");
    // Right-click destroys it.
    pty.send(&charming_testkit::keys::mouse_right_click(40, 12))
        .expect("right click");
    pty.wait_for_raw("destroying", 30000)
        .expect("window destroyed");
    pty.press("ctrl+c").expect("quit");
    pty.wait_for_exit(5000).expect("exit");
}

#[test]
fn boxes_keyboard_types_into_window() {
    let _pty_guard = pty_lock().lock().unwrap();
    let pty = PtySession::spawn(&ex("advanced_boxes"), &[]).expect("spawn");
    // The example prints window-mouse events to its stderr; nothing is
    // printed until the first click, so give it a moment to enable mouse
    // tracking, then click repeatedly until a click lands on a window.
    std::thread::sleep(std::time::Duration::from_millis(500));
    let click = charming_testkit::keys::mouse_click(40, 12);
    let mut clicked = false;
    for _ in 0..6 {
        pty.send(&click).expect("click");
        if pty.wait_for_raw("clicked window", 2000).is_ok() {
            clicked = true;
            break;
        }
    }
    assert!(clicked, "clicking should eventually focus a window");
    // Type one character at a time: the window echoes each keystroke, and a
    // per-character wait keeps the assertion robust under CI load (a single
    // multi-char packet can otherwise be split across decode iterations).
    for ch in ["a", "b", "c"] {
        pty.press(ch).expect("type");
        pty.wait_for_text(ch, 30000).expect("typed into window");
    }
    pty.press("esc").expect("esc");
    pty.wait_for_exit(5000).expect("exit");
}

#[test]
fn layout_keys_move_dialog() {
    let _pty_guard = pty_lock().lock().unwrap();
    let pty = PtySession::spawn(&ex("advanced_layout"), &[]).expect("spawn");
    pty.wait_for_text("marmalade", 30000).expect("dialog shown");
    // The dialog box's left half is off-screen (dialogX is negative); the
    // 'j'/'k' keys move it down/up. Capture the raw diff deltas.
    let before = pty.raw_output().len();
    pty.press("j").expect("j");
    pty.wait_until(5000, |s| s.contains("marmalade"))
        .expect("still shown");
    std::thread::sleep(std::time::Duration::from_millis(300));
    let after = pty.raw_output().len();
    assert!(after > before, "moving the dialog should re-render");
    pty.press("q").expect("quit");
    pty.wait_for_exit(5000).expect("exit");
}

#[test]
fn draw_example_supports_typing() {
    let _pty_guard = pty_lock().lock().unwrap();
    let pty = PtySession::spawn(&ex("draw"), &[]).expect("spawn");
    pty.wait_for_text("Draw Example", 30000)
        .expect("help shown");
    // Press any key to dismiss the help; a second press covers the case
    // where the help overlay was already dismissed by the initial resize.
    pty.press("space").expect("space");
    std::thread::sleep(std::time::Duration::from_millis(400));
    pty.press("space").expect("space2");
    pty.wait_until(5000, |s| !s.contains("Welcome to Draw"))
        .expect("help dismissed");
    pty.press("ctrl+c").expect("quit");
    pty.wait_for_exit(5000).expect("exit");
}

#[test]
fn panic_example_quits() {
    let _pty_guard = pty_lock().lock().unwrap();
    let pty = PtySession::spawn(&ex("panic"), &[]).expect("spawn");
    pty.wait_for_text("Panicing after 5 seconds", 30000)
        .expect("shown");
    pty.press("q").expect("q");
    pty.wait_for_exit(5000).expect("exit");
}

#[test]
fn prependline_logs_events() {
    let _pty_guard = pty_lock().lock().unwrap();
    let pty = PtySession::spawn(&ex("prependline"), &[]).expect("spawn");
    pty.wait_for_text("Hello, World!", 30000).expect("title bar");
    pty.wait_for_raw("WindowSizeEvent", 30000)
        .expect("event logged");
    pty.press("q").expect("q");
    pty.wait_for_exit(5000).expect("exit");
}

#[test]
fn splits_renders_and_quits() {
    let _pty_guard = pty_lock().lock().unwrap();
    let pty = PtySession::spawn(&ex("advanced_splits"), &[]).expect("spawn");
    pty.wait_for_text("Horizontal Layout Example", 30000)
        .expect("shown");
    pty.wait_for_text("Len | Len", 30000).expect("combos shown");
    pty.press("q").expect("q");
    pty.wait_for_exit(5000).expect("exit");
}

#[test]
fn tv_renders_and_quits() {
    let _pty_guard = pty_lock().lock().unwrap();
    let pty = PtySession::spawn(&ex("advanced_tv"), &[]).expect("spawn");
    std::thread::sleep(std::time::Duration::from_millis(500));
    pty.wait_for_text("", 30000).expect("running");
    pty.press("q").expect("q");
    pty.wait_for_exit(5000).expect("exit");
}

#[test]
fn draw_mouse_drawing() {
    let _pty_guard = pty_lock().lock().unwrap();
    let pty = PtySession::spawn(&ex("draw"), &[]).expect("spawn");
    pty.wait_for_text("Draw Example", 30000).expect("help shown");
    pty.press("space").expect("space");
    pty.wait_until(5000, |s| !s.contains("Welcome to Draw"))
        .expect("help dismissed");
    // Drag with the left button (SGR motion while pressed).
    pty.send(&charming_testkit::keys::mouse_drag(
        charming_testkit::keys::MOUSE_LEFT,
        20,
        5,
    ))
    .expect("drag");
    pty.send(&charming_testkit::keys::mouse_drag(
        charming_testkit::keys::MOUSE_LEFT,
        30,
        5,
    ))
    .expect("drag2");
    std::thread::sleep(std::time::Duration::from_millis(300));
    pty.wait_for_raw("█", 30000).expect("drawn");
    pty.press("ctrl+c").expect("quit");
    pty.wait_for_exit(5000).expect("exit");
}
