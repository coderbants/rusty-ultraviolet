//! Cleanroom Rust port of upstream Go source file: `terminal.go`
//! Upstream Target Tag / Version: `v0.0.0-20260703014108-f5a850f9c2b7`
//!
//! <public-docs>
//! The interactive terminal application: wires the console, screen, poll
//! reader, and event loop together with raw-mode management and window-size
//! notifications.
//!
//! NOTE: upstream uses goroutines, `errgroup`, and `os/signal`; the port uses
//! threads, `mpsc` channels, and an atomic cancellation flag. The screen is
//! constructed over the console's stdout fd (the same underlying file, like
//! Go's shared `*os.File`).
//! </public-docs>

use crate::console::{Console, ConsoleError, FdFile};
use crate::decoder::DecodedEvent;
use crate::environ::Environ;
use crate::event::Size;
use crate::logger::Logger;
use crate::terminal_reader::TerminalReader;
use crate::terminal_screen::{new_terminal_screen, TerminalScreen};
use std::io::{self, Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::Arc;
use std::time::Duration;

/// DefaultBufferSize is the default size of the input buffer used for reading
/// terminal events.
pub const DEFAULT_BUFFER_SIZE: usize = 4096;

/// DefaultEventTimeout is the default duration to wait for input events
/// before timing out.
pub const DEFAULT_EVENT_TIMEOUT: Duration = Duration::from_millis(100);

/// Options represents options for creating a new [Terminal].
#[derive(Debug, Clone)]
pub struct Options {
    /// BufferSize is the size of the input buffer used for reading terminal
    /// events. If zero, [DEFAULT_BUFFER_SIZE] is used.
    pub buffer_size: usize,

    /// EventTimeout is the duration to wait for input events before timing
    /// out. If zero, a default of 100 milliseconds is used.
    pub event_timeout: Duration,

    /// LegacyKeyEncoding represents any legacy key encoding ambiguities.
    pub legacy_key_encoding: crate::decoder::LegacyKeyEncoding,

    /// LookupKeys whether to use a lookup table for common key sequences.
    /// This is enabled by default.
    pub lookup_keys: bool,

    /// UseTerminfoKeys whether to use terminfo databases key definitions.
    /// This is disabled by default.
    pub use_terminfo_keys: bool,

    /// Logger is an optional logger for tracing terminal I/O operations.
    pub logger: Option<Box<dyn Logger>>,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            buffer_size: DEFAULT_BUFFER_SIZE,
            event_timeout: DEFAULT_EVENT_TIMEOUT,
            legacy_key_encoding: crate::decoder::LegacyKeyEncoding::default(),
            lookup_keys: true,
            use_terminfo_keys: false,
            logger: None,
        }
    }
}

/// Terminal represents an interactive terminal application.
pub struct Terminal {
    con: Console,
    opts: Options,
    scr: TerminalScreen,
    evc: Sender<DecodedEvent>,
    done: Arc<AtomicBool>,
    input_thread: Option<std::thread::JoinHandle<()>>,
    winch_thread: Option<std::thread::JoinHandle<()>>,
}

/// DefaultTerminal creates a new [Terminal] instance using the default
/// standard console and the given options.
pub fn default_terminal() -> Terminal {
    new_terminal(None, None)
}

/// ControllingTerminal creates a new [Terminal] instance using the
/// controlling terminal's input and output file descriptors.
pub fn controlling_terminal() -> Result<Terminal, ConsoleError> {
    let con = crate::console::controlling_console()?;
    Ok(new_terminal(Some(con), None))
}

/// NewTerminal creates a new [Terminal] instance with the given console and
/// options.
pub fn new_terminal(con: Option<Console>, opts: Option<Options>) -> Terminal {
    let mut opts = opts.unwrap_or_default();
    if opts.buffer_size == 0 {
        opts.buffer_size = DEFAULT_BUFFER_SIZE;
    }
    if opts.event_timeout.is_zero() {
        opts.event_timeout = DEFAULT_EVENT_TIMEOUT;
    }

    let con = match con {
        Some(c) => c,
        None => crate::console::default_console(),
    };
    let env = Environ(con.environ().to_vec());
    // The screen shares the console's stdout fd (Go: the renderer writes to
    // the same underlying file).
    let scr = new_terminal_screen(Box::new(FdFile::stdout_file()), env);
    let (evc, _evr) = channel();
    Terminal {
        con,
        opts,
        scr,
        evc,
        done: Arc::new(AtomicBool::new(false)),
        input_thread: None,
        winch_thread: None,
    }
}

impl Terminal {
    /// GetSize returns the current size of the terminal in characters.
    pub fn get_size(&mut self) -> io::Result<(usize, usize)> {
        self.con
            .get_size()
            .map_err(|e| io::Error::other(e.to_string()))
    }

    /// GetWinsize returns the current size of the terminal as a
    /// [crate::mouse::Winsize] struct.
    pub fn get_winsize(&mut self) -> io::Result<crate::mouse::Winsize> {
        self.con
            .get_winsize()
            .map(|ws| crate::mouse::Winsize {
                row: ws.row,
                col: ws.col,
                xpixel: ws.xpixel,
                ypixel: ws.ypixel,
            })
            .map_err(|e| io::Error::other(e.to_string()))
    }

    /// Screen returns the terminal's screen.
    pub fn screen(&mut self) -> &mut TerminalScreen {
        &mut self.scr
    }

    /// Events returns the terminal's event channel.
    pub fn events(&self) -> Sender<DecodedEvent> {
        self.evc.clone()
    }

    /// Start starts the terminal application event loop. This is a
    /// non-blocking call. Use [Terminal::wait] to wait for the terminal to
    /// exit.
    pub fn start(&mut self) -> io::Result<()> {
        self.con
            .make_raw()
            .map_err(|e| io::Error::other(format!("failed to set terminal to raw mode: {e}")))?;

        let term = self.con.getenv("TERM");
        let flags = self.opts.legacy_key_encoding;
        let lookup = self.opts.lookup_keys;

        // Input loop: read + decode + forward events. The reader's own
        // esc-timeout handles incomplete sequences.
        let reader: Box<dyn Read + Send> = Box::new(crate::console::FdFile::stdin_file());
        let mut tr = TerminalReader::new(reader, &term);
        tr.set_legacy(flags);
        let _ = lookup;

        let (evc, evr): (Sender<DecodedEvent>, Receiver<DecodedEvent>) = channel();
        self.evc = evc.clone();
        let done_input = self.done.clone();
        let reader_sender = evc.clone();
        self.input_thread = Some(std::thread::spawn(move || {
            let _ = tr.stream_events(&reader_sender);
            done_input.store(true, Ordering::SeqCst);
        }));

        // Event forwarding: deliver decoded events to subscribers while
        // negotiating grapheme width (DEC mode 2027).
        let done_fwd = self.done.clone();
        let mut scr = std::mem::replace(
            &mut self.scr,
            new_terminal_screen(Box::new(FdFile::stdout_file()), Environ(vec![])),
        );
        let out_sender = evc.clone();
        self.winch_thread = Some(std::thread::spawn(move || {
            loop {
                if done_fwd.load(Ordering::SeqCst) {
                    break;
                }
                match evr.recv_timeout(Duration::from_millis(50)) {
                    Ok(ev) => {
                        // handleEvent: negotiate Unicode core mode.
                        if let DecodedEvent::ModeReport { mode, value } = ev {
                            if mode == 2027 && (value == 1 || value == 3) {
                                scr.enable_grapheme_width();
                            }
                        }
                        let _ = out_sender.send(ev);
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                }
            }
        }));
        self.scr = scr;

        // Window-size notifications (SIGWINCH).
        self.start_winch_loop()?;

        // Initial window size.
        self.send_winsize()?;

        // Restore any previous screen state.
        self.scr.restore();
        // Query whether the terminal supports Unicode core mode (DEC mode
        // 2027).
        self.scr.request_grapheme_width();
        self.scr
            .flush()
            .map_err(|e| io::Error::other(e.to_string()))?;

        Ok(())
    }

    fn start_winch_loop(&mut self) -> io::Result<()> {
        let rx = crate::tty::notify_winch()?;
        let done = self.done.clone();
        let evc = self.evc.clone();
        let input_fd = self.con.input_fd();
        self.winch_thread = Some(std::thread::spawn(move || {
            while rx.recv().is_ok() {
                if done.load(Ordering::SeqCst) {
                    break;
                }
                if let Ok(ws) = crate::console::get_winsize_for_fd(input_fd) {
                    if ws.col > 0 && ws.row > 0 {
                        let _ = evc.send(DecodedEvent::WindowSize(Size {
                            width: ws.col as usize,
                            height: ws.row as usize,
                        }));
                    }
                    if ws.xpixel > 0 && ws.ypixel > 0 {
                        let _ = evc.send(DecodedEvent::PixelSize(Size {
                            width: ws.xpixel as usize,
                            height: ws.ypixel as usize,
                        }));
                    }
                }
            }
        }));
        Ok(())
    }

    fn send_winsize(&mut self) -> io::Result<()> {
        let ws = self
            .con
            .get_winsize()
            .map_err(|e| io::Error::other(format!("getting terminal size: {e}")))?;
        if ws.col > 0 && ws.row > 0 {
            let _ = self.evc.send(DecodedEvent::WindowSize(Size {
                width: ws.col as usize,
                height: ws.row as usize,
            }));
        }
        if ws.xpixel > 0 && ws.ypixel > 0 {
            let _ = self.evc.send(DecodedEvent::PixelSize(Size {
                width: ws.xpixel as usize,
                height: ws.ypixel as usize,
            }));
        }
        Ok(())
    }

    /// Wait waits for the terminal event loop to exit.
    pub fn wait(&mut self) -> io::Result<()> {
        while !self.done.load(Ordering::SeqCst) {
            std::thread::sleep(Duration::from_millis(50));
        }
        Ok(())
    }

    /// Stop stops the terminal event loop.
    pub fn stop(&mut self) -> io::Result<()> {
        self.done.store(true, Ordering::SeqCst);
        if let Some(h) = self.input_thread.take() {
            let _ = h.join();
        }
        if let Some(h) = self.winch_thread.take() {
            let _ = h.join();
        }
        self.scr.reset();
        self.scr
            .flush()
            .map_err(|e| io::Error::other(format!("failed to flush terminal screen: {e}")))?;
        self.con
            .restore()
            .map_err(|e| io::Error::other(format!("failed to restore terminal state: {e}")))?;
        Ok(())
    }

    /// SendEvent sends an event to the terminal's event channel.
    pub fn send_event(&self, e: DecodedEvent) {
        let _ = self.evc.send(e);
    }

    /// Write writes data directly to the terminal's console output.
    pub fn write(&mut self, p: &[u8]) -> io::Result<usize> {
        self.con.write(p)
    }

    /// Read reads data from the terminal's console input.
    pub fn read(&mut self, p: &mut [u8]) -> io::Result<usize> {
        self.con.read(p)
    }
}
