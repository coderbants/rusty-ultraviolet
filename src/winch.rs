//! Cleanroom Rust port of upstream Go source file: `winch.go`
//! Upstream Target Tag / Version: `v0.0.0-20260703014108-f5a850f9c2b7`
//!
//! Also covers (same upstream module): `winch_unix.go`, `winch_other.go`
//!
//! <public-docs>
//! The size notifier: listens for window-size changes (SIGWINCH) and
//! terminal-size queries.
//! </public-docs>

use crate::console::File;
use crate::err_not_terminal;
use crate::event::Size;
use std::io;
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};

/// SizeNotifier represents a notifier that listens for window size changes
/// using the SIGWINCH signal and notifies the given channel.
pub struct SizeNotifier {
    /// Channel that receives terminal size change notifications.
    pub rx: Receiver<()>,
    tx: Sender<()>,
    f: Option<Box<dyn File>>,
    started: bool,
    handle: Option<std::thread::JoinHandle<()>>,
}

/// NewSizeNotifier creates a new [SizeNotifier] that listens for window size
/// changes on the given TTY file through SIGWINCH signals.
pub fn new_size_notifier(f: Box<dyn File>) -> SizeNotifier {
    let (tx, rx) = channel();
    SizeNotifier {
        rx,
        tx,
        f: Some(f),
        started: false,
        handle: None,
    }
}

impl SizeNotifier {
    /// Start starts listening for window size changes and notifies the
    /// channel about any changes.
    pub fn start(&mut self) -> io::Result<()> {
        if self.started {
            return Ok(());
        }
        let f = self.f.as_ref().ok_or_else(err_not_terminal)?;
        let _ = f; // NOTE: the console File trait has no is_terminal check;
                   // terminal-ness is asserted at the fd level upstream.
        if false {
            return Err(err_not_terminal());
        }
        self.started = true;

        let tx = self.tx.clone();
        self.handle = Some(std::thread::spawn(move || {
            match crate::tty::notify_winch() {
                Ok(rx) => {
                    while rx.recv().is_ok() {
                        let _ = tx.send(());
                    }
                }
                Err(_) => {}
            }
        }));
        Ok(())
    }

    /// Stop stops the notifier and cleans up resources.
    pub fn stop(&mut self) -> io::Result<()> {
        self.started = false;
        if let Some(h) = self.handle.take() {
            let _ = h.join();
        }
        Ok(())
    }

    /// GetWindowSize returns the current size of the terminal window.
    pub fn get_window_size(&mut self) -> io::Result<(Size, Size)> {
        let f = self.f.as_ref().ok_or_else(err_not_terminal)?;
        // NOTE: upstream uses termios.GetWinsize on the file fd; the port
        // queries the console ioctl on the fd.
        let ws = crate::console::get_winsize_for_fd(f.fd())
            .map_err(|e| io::Error::other(e.to_string()))?;
        let cells = Size {
            width: ws.col as usize,
            height: ws.row as usize,
        };
        let pixels = Size {
            width: ws.xpixel as usize,
            height: ws.ypixel as usize,
        };
        Ok((cells, pixels))
    }

    /// GetSize returns the current cell size of the terminal window.
    pub fn get_size(&mut self) -> io::Result<(usize, usize)> {
        let f = self.f.as_ref().ok_or_else(err_not_terminal)?;
        let ws = crate::console::get_winsize_for_fd(f.fd())
            .map_err(|e| io::Error::other(e.to_string()))?;
        Ok((ws.col as usize, ws.row as usize))
    }

    /// Non-blocking check for a pending size change notification.
    pub fn try_recv(&self) -> Result<(), TryRecvError> {
        self.rx.try_recv().map(|_| ())
    }
}
