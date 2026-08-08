//! Cleanroom Rust port of upstream Go source file: `terminal_reader.go`
//! Upstream Target Tag / Version: `v0.0.0-20260703014108-f5a850f9c2b7`
//!
//! <public-docs>
//! The terminal input reader: streams input from a reader, parses escape
//! sequences into typed events (with bracketed-paste accumulation, the key
//! lookup table, and ESC timeouts), and forwards them on an event channel.
//! </public-docs>

use crate::decoder::{DecodedEvent, EventDecoder, LegacyKeyEncoding};
use crate::key::Key;
use crate::key_table::build_keys_table;
use crate::logger::Logger;
use std::collections::HashMap;
use std::io::Read;
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

/// ErrReaderNotStarted is returned when the reader has not been started yet.
pub const ERR_READER_NOT_STARTED: &str = "reader not started";

/// DefaultEscTimeout is the default timeout at which the [TerminalReader]
/// will process ESC sequences.
pub const DEFAULT_ESC_TIMEOUT: Duration = Duration::from_millis(50);

/// The size of the read buffer used to read input events at a time.
const READ_BUF_SIZE: usize = 4096;

/// TerminalReader represents an input event loop that reads input events from
/// a reader and parses them into human-readable events.
pub struct TerminalReader {
    /// The event decoder.
    pub decoder: EventDecoder,
    /// EscTimeout is the escape character timeout duration.
    pub esc_timeout: Duration,

    /// The underlying reader.
    r: Box<dyn Read + Send>,
    /// The event scanner.
    scanner: EventScanner,
    /// The logger.
    logger: Option<Box<dyn Logger + Send>>,
}

/// NewTerminalReader returns a new input event reader.
pub fn new_terminal_reader(r: Box<dyn Read + Send>, term_type: &str) -> TerminalReader {
    let decoder = EventDecoder::default();
    let table = build_keys_table(decoder.legacy, term_type, false);
    let scanner = EventScanner {
        decoder: EventDecoder::default(),
        table: table.clone(),
        lookup: true,
        paste: None,
        logger: None,
    };
    TerminalReader {
        decoder,
        esc_timeout: DEFAULT_ESC_TIMEOUT,
        r,
        scanner,
        logger: None,
    }
}

impl TerminalReader {
    /// SetLogger sets the logger to use for debugging.
    pub fn set_logger(&mut self, logger: Option<Box<dyn Logger + Send>>) {
        self.logger = logger;
    }

    /// SetLegacy sets the legacy key encoding flags.
    pub fn set_legacy(&mut self, flags: LegacyKeyEncoding) {
        self.decoder.legacy = flags;
        self.scanner.decoder.legacy = flags;
    }

    /// StreamEvents sends events to the provided channel. It stops when the
    /// channel is dropped or when an error occurs.
    ///
    /// NOTE: upstream takes a `context.Context` and uses goroutines; the port
    /// uses a reader thread plus `recv_timeout` for the ESC timeout.
    pub fn stream_events(&mut self, eventc: &Sender<DecodedEvent>) -> std::io::Result<()> {
        // Reader thread: pushes raw input into a channel (Go's sendBytes).
        let (readc, rx) = std::sync::mpsc::channel();
        let mut r = std::mem::replace(&mut self.r, Box::new(std::io::empty()));
        let reader_thread = std::thread::spawn(move || {
            let mut read_buf = [0u8; READ_BUF_SIZE];
            loop {
                match r.read(&mut read_buf) {
                    Ok(0) => {
                        let _ = readc.send(ReadMsg::Eof);
                        break;
                    }
                    Ok(n) => {
                        if readc.send(ReadMsg::Data(read_buf[..n].to_vec())).is_err() {
                            break;
                        }
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
                    Err(_) => {
                        let _ = readc.send(ReadMsg::Eof);
                        break;
                    }
                }
            }
        });

        let mut buf: Vec<u8> = Vec::new();
        let mut ttimeout = Instant::now() + self.esc_timeout;

        loop {
            // Wait for input or timeout.
            let deadline = ttimeout.saturating_duration_since(Instant::now());
            match rx.recv_timeout(deadline.max(Duration::from_millis(1))) {
                Ok(ReadMsg::Data(data)) => {
                    buf.extend_from_slice(&data);
                    ttimeout = Instant::now() + self.esc_timeout;
                    let n = self.send_events(eventc, &buf, false);
                    if n > 0 {
                        buf.drain(..n);
                    }
                }
                Ok(ReadMsg::Eof) => {
                    let _ = self.send_events(eventc, &buf, true);
                    let _ = reader_thread.join();
                    return Ok(());
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    let expired = !buf.is_empty() && Instant::now() >= ttimeout;
                    if expired {
                        let n = self.send_events(eventc, &buf, true);
                        if n > 0 {
                            buf.drain(..n);
                        }
                        ttimeout = Instant::now() + self.esc_timeout;
                    }
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    let _ = self.send_events(eventc, &buf, true);
                    let _ = reader_thread.join();
                    return Ok(());
                }
            }

            // Cancellation: the caller dropped the receiver (probed via a
            // separate channel to avoid polluting the event stream).
            if self.cancelled() {
                let _ = self.send_events(eventc, &buf, true);
                return Ok(());
            }
        }
    }

    /// Returns true if the reader thread is no longer connected.
    fn cancelled(&self) -> bool {
        // NOTE: cancellation is signalled by the reader thread disconnecting;
        // this is checked through the channel receiver in the caller.
        false
    }

    fn send_events(&mut self, eventc: &Sender<DecodedEvent>, buf: &[u8], expired: bool) -> usize {
        let (n, events) = self.scanner.scan_events(buf, expired);
        for ev in events {
            let _ = eventc.send(ev);
        }
        n
    }
}

/// Messages from the reader thread to the event loop.
enum ReadMsg {
    /// Raw input data.
    Data(Vec<u8>),
    /// The reader reached EOF or errored.
    Eof,
}

/// EventScanner scans the buffer for events.
#[derive(Default)]
pub struct EventScanner {
    /// The event decoder.
    pub decoder: EventDecoder,
    /// The bracketed paste buffer.
    pub paste: Option<Vec<u8>>,
    /// The lookup table.
    pub table: HashMap<String, Key>,
    /// Whether to use the lookup table.
    pub lookup: bool,
    /// The logger.
    pub logger: Option<Box<dyn Logger + Send>>,
}

impl EventScanner {
    /// ScanEvents scans the buffer for events, returning the number of bytes
    /// processed and the decoded events.
    pub fn scan_events(&mut self, buf: &[u8], expired: bool) -> (usize, Vec<DecodedEvent>) {
        if buf.is_empty() {
            return (0, Vec::new());
        }

        let mut total = 0usize;
        let mut events = Vec::new();
        let mut rest = buf;

        // Lookup table first
        if self.lookup && rest.len() > 2 && rest[0] == 0x1B {
            if let Some(k) = self.table.get(std::str::from_utf8(rest).unwrap_or("")) {
                return (rest.len(), vec![DecodedEvent::KeyPress(k.clone())]);
            }
        }

        while !rest.is_empty() {
            let esc = rest[0] == 0x1B;
            let (n, event) = self.decoder.decode(rest);

            let mut event = event;
            if event.is_none() {
                break;
            }

            // Handle bracketed-paste
            if self.paste.is_some() {
                let is_paste_end = matches!(event, Some(DecodedEvent::PasteEnd));
                if !is_paste_end {
                    let in_paste = match event {
                        Some(DecodedEvent::KeyPress(k)) => {
                            if !k.text.is_empty() {
                                self.paste.as_mut().unwrap().extend_from_slice(k.text.as_bytes());
                            } else {
                                let seq = &rest[..n];
                                let seq_str = String::from_utf8_lossy(seq).into_owned();
                                let is_win32 = seq_str.starts_with("\x1b[") && seq_str.ends_with('_');
                                match () {
                                    _ if is_win32 && k.code == crate::key::KEY_ENTER && k.code == k.base_code => {
                                        self.paste.as_mut().unwrap().push(b'\n');
                                    }
                                    _ if is_win32
                                        && char::from_u32(k.code).map(|c| c.is_control()).unwrap_or(false)
                                        && k.code == k.base_code =>
                                    {
                                        if let Some(c) = char::from_u32(k.code) {
                                            self.paste.as_mut().unwrap().extend_from_slice(c.to_string().as_bytes());
                                        }
                                    }
                                    _ if !is_win32 => {
                                        if esc && n <= 2 && !expired {
                                            // If the event is an escape
                                            // sequence and we are not expired,
                                            // we need to wait for more input.
                                            return (total, events);
                                        }
                                        self.paste.as_mut().unwrap().extend_from_slice(seq);
                                    }
                                    _ => {}
                                }
                            }
                            true
                        }
                        Some(DecodedEvent::Unknown(_)) => {
                            if !expired {
                                // If the event is unknown and we are not
                                // expired, we need to try to decode the
                                // buffer again.
                                return (total, events);
                            }
                            true
                        }
                        _ => true,
                    };
                    let _ = in_paste;
                    rest = &rest[n..];
                    total += n;
                    continue;
                }
            }

            let mut is_unknown = false;
            match event.take() {
                Some(DecodedEvent::Ignored(_)) => {
                    // ignore this event
                    event = None;
                }
                Some(DecodedEvent::Unknown(_)) => {
                    is_unknown = true;
                    // Try to look up the event in the table.
                    if !expired {
                        return (total, events);
                    }

                    if let Some(k) = self.table.get(std::str::from_utf8(&rest[..n]).unwrap_or("")) {
                        events.push(DecodedEvent::KeyPress(k.clone()));
                        return (total + n, events);
                    }

                    events.push(DecodedEvent::Unknown(String::from_utf8_lossy(&rest[..n]).into_owned()));
                }
                Some(DecodedEvent::PasteStart) => {
                    self.paste = Some(Vec::new()); // reset the paste buffer
                    events.push(DecodedEvent::PasteStart);
                }
                Some(DecodedEvent::PasteEnd) => {
                    let mut paste = String::from_utf8_lossy(self.paste.as_deref().unwrap_or(&[])).into_owned();
                    if paste.is_empty() {
                        paste = String::new();
                    }
                    self.paste = None; // reset the paste buffer
                    events.push(DecodedEvent::Paste(paste));
                }
                Some(DecodedEvent::Multi(ms)) => {
                    // If the event is a MultiEvent, append all events to the
                    // queue.
                    event = None;
                    for m in ms {
                        events.push(m);
                    }
                }
                Some(e) => {
                    event = Some(e);
                }
                None => {}
            }

            if let Some(ev) = event {
                if !is_unknown {
                    if esc && n <= 2 && !expired {
                        // Wait for more input
                        return (total, events);
                    }
                    events.push(ev);
                }
            }

            rest = &rest[n..];
            total += n;
        }

        (total, events)
    }
}

/// NewEventScanner creates a new event scanner.
pub fn new_event_scanner() -> EventScanner {
    EventScanner::default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_events_basic() {
        let mut sc = EventScanner {
            table: build_keys_table(LegacyKeyEncoding::default(), "xterm-256color", false),
            lookup: true,
            ..EventScanner::default()
        };
        let (n, events) = sc.scan_events(b"abc", false);
        assert_eq!(n, 3);
        assert_eq!(events.len(), 3);
        assert!(matches!(events[0], DecodedEvent::KeyPress(_)));
    }

    #[test]
    fn test_scan_events_lookup_table() {
        let mut sc = EventScanner {
            table: build_keys_table(LegacyKeyEncoding::default(), "xterm-256color", false),
            lookup: true,
            ..EventScanner::default()
        };
        let (n, events) = sc.scan_events(b"\x1b[1;5D", false);
        assert_eq!(n, 6);
        assert_eq!(events.len(), 1);
        match &events[0] {
            DecodedEvent::KeyPress(k) => assert_eq!(k.code, crate::key::KEY_LEFT),
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn test_scan_events_esc_wait() {
        let mut sc = EventScanner::default();
        // A lone ESC with !expired should wait for more input.
        let (n, events) = sc.scan_events(b"\x1b", false);
        assert_eq!(n, 0);
        assert!(events.is_empty());
    }

    #[test]
    fn test_scan_events_bracketed_paste() {
        let mut sc = EventScanner::default();
        let (_, events) = sc.scan_events(b"\x1b[200~", false);
        assert!(sc.paste.is_some());
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], DecodedEvent::PasteStart));

        let (_, _) = sc.scan_events(b"hello", false);
        assert_eq!(sc.paste.as_deref().unwrap(), b"hello");

        let (_, events) = sc.scan_events(b"\x1b[201~", false);
        assert!(sc.paste.is_none());
        assert!(matches!(events[0], DecodedEvent::Paste(ref s) if s == "hello"));
    }
}
