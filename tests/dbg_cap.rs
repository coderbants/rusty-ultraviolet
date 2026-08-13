use rusty_ultraviolet::decoder::EventDecoder;

#[test]
fn dbg() {
    let mut d = EventDecoder::default();
    for (name, bytes) in [
        ("echoed request", b"\x1bP+q544e\x1b\\".as_slice()),
        ("response", b"\x1bP1+r544e\x1b\\"),
        ("response w/ value", b"\x1bP1+r524742=48;5;16\x1b\\"),
        ("split1", b"\x1bP1+r"),
        ("split2", b"544e\x1b\\"),
    ] {
        let ev = d.decode(bytes);
        println!("{name}: {:?}", ev);
    }
}
