//! Cleanroom Rust port of upstream Go source file: `terminal_renderer_output_test.go`
//! Upstream Target Tag / Version: `v0.0.0-20260703014108-f5a850f9c2b7`
//!
//! Golden tests for the terminal renderer's diff output, matching the
//! upstream ultraviolet test cases byte-for-byte.

use rusty_ultraviolet::buffer::new_screen_buffer;
use rusty_ultraviolet::environ::Environ;
use rusty_ultraviolet::styled::new_styled_string;
use rusty_ultraviolet::terminal_renderer::TerminalRenderer;

const LOREM_IPSUM: [&str; 5] = [
    "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Vivamus at ornare risus, quis lacinia magna. Suspendisse egestas purus risus, id rutrum diam porta non. Duis luctus tempus dictum. Maecenas luctus metus vitae nulla consectetur egestas. Curabitur faucibus nunc vel eros semper scelerisque. Proin dictum aliquam lacus dignissim fringilla. Praesent ut quam id dui aliquam vehicula in vitae orci. Fusce imperdiet aliquam quam. Nullam euismod magna tincidunt nisl ullamcorper, dignissim rutrum arcu rutrum. Nulla ac fringilla velit. Duis non pellentesque erat.",
    "In egestas ex et sem vulputate, congue bibendum diam ultrices. Nam auctor dictum enim, in rutrum nulla vestibulum sit amet. Vestibulum vel velit ac sem pellentesque accumsan. Vivamus pharetra mi non arcu tristique gravida. Interdum et malesuada fames ac ante ipsum primis in faucibus. Sed molestie lectus nunc, sit amet rhoncus orci laoreet vel. Nulla eget mattis massa. Nunc porta eros sollicitudin lorem dapibus luctus. Vestibulum ut turpis ut nibh tincidunt feugiat. Integer eget augue nunc. Morbi vitae ultrices neque. Nulla et convallis libero. Cras nec faucibus odio. Maecenas lacinia sed odio sit amet ultrices.",
    "Nunc at molestie massa. Phasellus commodo dui odio, quis pulvinar orci eleifend a. In et erat nec nisl auctor facilisis at at orci. Curabitur ut ligula in ipsum consequat consectetur. Suspendisse pulvinar arcu metus, et faucibus risus interdum pharetra. Vestibulum vulputate, arcu at malesuada varius, nisl turpis molestie risus, ut lobortis dolor neque vitae diam. Donec lectus libero, iaculis non diam sit amet, sagittis mattis lectus. Vestibulum a magna molestie neque molestie faucibus sagittis et ante. Etiam porta tincidunt nisi sit amet blandit. Vivamus et tellus diam. Vivamus id dolor placerat, tristique magna non, congue est. Nulla a condimentum nulla. Fusce maximus semper nunc, at bibendum mi. Nam malesuada vitae mi molestie tincidunt. Pellentesque sed vestibulum lectus, eu ultrices ligula. Phasellus id nibh tristique, ultricies diam vel, cursus odio.",
    "Integer sed mi viverra, convallis urna congue, efficitur libero. Duis non eros commodo, ultricies quam hendrerit, molestie velit. Nunc non eros vitae lectus hendrerit gravida. Nunc lacinia neque sapien, et accumsan orci elementum vel. Praesent vel interdum nisl. Duis eget diam turpis. Nunc gravida, lacus dictum congue pharetra, dui est laoreet massa, ac convallis elit est sed dui. Morbi luctus convallis dui id tristique.",
    "Praesent vitae laoreet risus. Sed ac facilisis justo. Morbi fringilla in est vel volutpat. Aliquam erat tortor, posuere ac libero sit amet, vehicula blandit sapien. Nullam feugiat purus eget sapien bibendum, id posuere risus finibus. Aliquam erat volutpat. Pellentesque ac purus accumsan, accumsan mi vel, viverra lectus. Ut sed porta erat, vitae mollis nibh. Nunc dignissim quis tellus sed blandit. Mauris id velit in odio commodo aliquet.",
];

struct Case {
    name: String,
    inputs: Vec<String>,
    wraps: Vec<bool>,
    relative: bool,
    altscreen: bool,
    expected: Vec<String>,
}

fn run_case(c: &Case) -> Vec<String> {
    let mut buf: Vec<u8> = Vec::new();
    let env = Environ(vec![
        "TERM=xterm-256color".to_string(),
        "COLORTERM=truecolor".to_string(),
    ]);
    let mut s = TerminalRenderer::new_without_writer(&env);
    s.set_scroll_optim_public(true);
    s.set_fullscreen_public(c.altscreen);
    s.set_relative_cursor_public(c.relative);
    if c.altscreen {
        s.save_cursor_public();
        s.erase_public();
    }

    let mut scr = new_screen_buffer(10, 5);
    let mut outputs: Vec<String> = Vec::new();
    for (i, input) in c.inputs.iter().enumerate() {
        buf.clear();

        let comp = new_styled_string(input);
        let mut comp = comp;
        if i < c.wraps.len() {
            comp.wrap = c.wraps[i];
        }
        let area = scr.bounds();
        comp.draw(&mut scr, area);
        s.render_public(&mut scr.render_buffer);
        s.flush_into(&mut buf);

        outputs.push(String::from_utf8_lossy(&buf).to_string());
    }
    outputs
}

#[test]
fn test_renderer_output() {
    let cases: Vec<Case> = vec![
        Case {
            name: "scroll to bottom in inline mode".to_string(),
            inputs: vec!["ABC".to_string(), "XXX".to_string()],
            wraps: vec![],
            relative: true,
            altscreen: false,
            expected: vec!["\rABC".to_string(), "\rXXX".to_string()],
        },
        Case {
            name: "scroll one line".to_string(),
            inputs: vec![
                LOREM_IPSUM[0].to_string(),
                LOREM_IPSUM[0][10..].to_string(),
            ],
            wraps: vec![true, true],
            relative: false,
            altscreen: true,
            expected: vec![
                "\x1b[H\x1b[2JLorem ipsu\r\nm dolor si\r\nt amet, co\r\nnsectetur\r\nadipiscin\x1b[?7lg\x1b[?7h".to_string(),
                "\r\n elit. Vi\x1b[?7lv\x1b[?7h".to_string(),
            ],
        },
        Case {
            name: "scroll two lines".to_string(),
            inputs: vec![
                LOREM_IPSUM[0].to_string(),
                LOREM_IPSUM[0][20..].to_string(),
            ],
            wraps: vec![true, true],
            relative: false,
            altscreen: true,
            expected: vec![
                "\x1b[H\x1b[2JLorem ipsu\r\nm dolor si\r\nt amet, co\r\nnsectetur\r\nadipiscin\x1b[?7lg\x1b[?7h".to_string(),
                "\r\x1b[2S\x1bM elit. Viv\r\namus at o\x1b[?7lr\x1b[?7h".to_string(),
            ],
        },
        Case {
            name: "insert line in the middle".to_string(),
            inputs: vec![
                "ABC\nDEF\nGHI\n".to_string(),
                "ABC\n\nDEF\nGHI".to_string(),
            ],
            wraps: vec![true, true],
            relative: false,
            altscreen: true,
            expected: vec![
                "\x1b[H\x1b[2JABC\r\nDEF\r\nGHI".to_string(),
                "\r\x1bM\x1b[L".to_string(),
            ],
        },
        Case {
            name: "erase until end of line".to_string(),
            inputs: vec![
                "\nABCEFGHIJK".to_string(),
                "\nABCE      ".to_string(),
            ],
            wraps: vec![],
            relative: false,
            altscreen: false,
            expected: vec![
                "\x1b[2;1HABCEFGHIJK".to_string(),
                "\r\x1b[5G\x1b[K".to_string(),
            ],
        },
    ];

    for c in &cases {
        let got = run_case(c);
        for (i, exp) in c.expected.iter().enumerate() {
            assert_eq!(
                &got[i], exp,
                "case {:?} output[{}]:\nExpected: {:?}\nGot:      {:?}",
                c.name, i, exp, got[i]
            );
        }
    }
}

/// Ported from upstream `TestRendererWideCellReanchor`: a wide-cell line is
/// re-anchored once (not per-cell), and grapheme mode skips the re-anchor.
#[test]
fn test_renderer_wide_cell_reanchor() {
    let render = |grapheme: bool| -> String {
        let mut buf: Vec<u8> = Vec::new();
        let env = Environ(vec![
            "TERM=xterm-256color".to_string(),
            "COLORTERM=truecolor".to_string(),
        ]);
        let mut s = TerminalRenderer::new_without_writer(&env);
        s.set_fullscreen_public(true);
        s.set_grapheme_width_public(grapheme);
        s.save_cursor_public();
        s.erase_public();

        let mut scr = new_screen_buffer(10, 1);
        buf.clear();
        let ss = new_styled_string("世界");
        let area = scr.bounds();
        ss.draw(&mut scr, area);
        s.render_public(&mut scr.render_buffer);
        s.flush_into(&mut buf);
        String::from_utf8_lossy(&buf).to_string()
    };

    let out = render(false);
    let reanchors = out.matches("\x1b[5G").count();
    assert_eq!(
        reanchors, 1,
        "non-grapheme line with wide cells: want 1 re-anchor, got {reanchors} in {out:?}"
    );

    let gout = render(true);
    let reanchors = gout.matches("\x1b[5G").count();
    assert_eq!(
        reanchors, 0,
        "grapheme-mode line should not re-anchor, got {reanchors} in {gout:?}"
    );
}
