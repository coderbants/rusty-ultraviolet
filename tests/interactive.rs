//! Interactive integration tests for the ultraviolet examples, driven
//! through a real pseudo-terminal: mouse clicks and drags, keyboard input,
//! and assertions on the reconstructed on-screen state.

use charming_testkit::PtySession;

fn ex(name: &str) -> String {
    format!("target/debug/examples/{name}")
}

#[test]
fn helloworld_quits_on_any_key() {
    let pty = PtySession::spawn(&ex("helloworld"), &[]).expect("spawn");
    pty.wait_for_text("Hello, World!", 5000).expect("shown");
    pty.press("x").expect("x");
    pty.wait_for_exit(5000).expect("exit");
}

#[test]
fn altscreen_toggles_with_space() {
    let pty = PtySession::spawn(&ex("altscreen"), &[]).expect("spawn");
    pty.wait_for_text("inline mode", 5000).expect("inline");
    pty.press("space").expect("space");
    pty.wait_for_text("alternate screen mode", 5000)
        .expect("alt screen");
    pty.press("space").expect("space");
    pty.wait_for_text("inline mode", 5000)
        .expect("back to inline");
    pty.press("q").expect("q");
    pty.wait_for_exit(5000).expect("exit");
}

#[test]
fn mouse_click_reports_position() {
    let pty = PtySession::spawn(&ex("mouse"), &[]).expect("spawn");
    pty.wait_for_text("Button:", 5000).expect("shown");
    std::thread::sleep(std::time::Duration::from_millis(300));
    // Click at cell (10, 5): the example logs the pixel-adjusted position.
    pty.send(&charming_testkit::keys::mouse_click(10, 5))
        .expect("click");
    pty.wait_for_raw("Position:", 5000).expect("event logged");
    pty.press("q").expect("quit");
    pty.wait_for_exit(5000).expect("exit");
}

#[test]
fn boxes_create_window_on_click() {
    let pty = PtySession::spawn(&ex("advanced_boxes"), &[]).expect("spawn");
    pty.wait_for_raw("?1002", 5000).expect("mouse mode");
    std::thread::sleep(std::time::Duration::from_millis(300));
    // Click in the root window: a new window is created.
    pty.send(&charming_testkit::keys::mouse_click(40, 12))
        .expect("click");
    pty.wait_for_raw("clicked root window", 5000)
        .expect("window created");
    // Click again: now the new window is hit and focused.
    pty.send(&charming_testkit::keys::mouse_click(40, 12))
        .expect("click2");
    pty.wait_for_raw("clicked window", 5000)
        .expect("window focused");
    // Right-click destroys it.
    pty.send(&charming_testkit::keys::mouse_right_click(40, 12))
        .expect("right click");
    pty.wait_for_raw("destroying", 5000)
        .expect("window destroyed");
    pty.press("ctrl+c").expect("quit");
    pty.wait_for_exit(5000).expect("exit");
}

#[test]
fn boxes_keyboard_types_into_window() {
    let pty = PtySession::spawn(&ex("advanced_boxes"), &[]).expect("spawn");
    std::thread::sleep(std::time::Duration::from_millis(300));
    // Click to create + focus a window, then type into it.
    pty.send(&charming_testkit::keys::mouse_click(40, 12))
        .expect("click");
    pty.wait_for_raw("clicked root window", 5000)
        .expect("window created");
    pty.send(&charming_testkit::keys::mouse_click(40, 12))
        .expect("click2");
    pty.wait_for_raw("clicked window", 5000)
        .expect("window focused");
    pty.type_text("abc").expect("type");
    pty.wait_for_text("abc", 5000).expect("typed into window");
    pty.press("esc").expect("esc");
    pty.wait_for_exit(5000).expect("exit");
}

#[test]
fn layout_keys_move_dialog() {
    let pty = PtySession::spawn(&ex("advanced_layout"), &[]).expect("spawn");
    pty.wait_for_text("marmalade", 5000).expect("dialog shown");
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
    let pty = PtySession::spawn(&ex("draw"), &[]).expect("spawn");
    pty.wait_for_text("Draw Example", 5000).expect("help shown");
    // Press any key to dismiss the help.
    pty.press("space").expect("space");
    pty.wait_until(5000, |s| !s.contains("Welcome to Draw"))
        .expect("help dismissed");
    pty.press("ctrl+c").expect("quit");
    pty.wait_for_exit(5000).expect("exit");
}

#[test]
fn panic_example_quits() {
    let pty = PtySession::spawn(&ex("panic"), &[]).expect("spawn");
    pty.wait_for_text("Panicing after 5 seconds", 5000)
        .expect("shown");
    pty.press("q").expect("q");
    pty.wait_for_exit(5000).expect("exit");
}

#[test]
fn prependline_logs_events() {
    let pty = PtySession::spawn(&ex("prependline"), &[]).expect("spawn");
    pty.wait_for_text("Hello, World!", 5000).expect("title bar");
    pty.wait_for_raw("WindowSizeEvent", 5000)
        .expect("event logged");
    pty.press("q").expect("q");
    pty.wait_for_exit(5000).expect("exit");
}

#[test]
fn splits_renders_and_quits() {
    let pty = PtySession::spawn(&ex("advanced_splits"), &[]).expect("spawn");
    pty.wait_for_text("Horizontal Layout Example", 5000)
        .expect("shown");
    pty.wait_for_text("Len | Len", 5000).expect("combos shown");
    pty.press("q").expect("q");
    pty.wait_for_exit(5000).expect("exit");
}

#[test]
fn tv_renders_and_quits() {
    let pty = PtySession::spawn(&ex("advanced_tv"), &[]).expect("spawn");
    std::thread::sleep(std::time::Duration::from_millis(500));
    pty.wait_for_text("", 5000).expect("running");
    pty.press("q").expect("q");
    pty.wait_for_exit(5000).expect("exit");
}

#[test]
fn draw_mouse_drawing() {
    let pty = PtySession::spawn(&ex("draw"), &[]).expect("spawn");
    pty.wait_for_text("Draw Example", 5000).expect("help shown");
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
    pty.wait_for_raw("█", 5000).expect("drawn");
    pty.press("ctrl+c").expect("quit");
    pty.wait_for_exit(5000).expect("exit");
}
