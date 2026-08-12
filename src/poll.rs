//! Cleanroom Rust port of upstream Go source files: `poll.go`, `poll_bsd.go`, `poll_linux.go`, `poll_select.go`, `poll_solaris.go`, `poll_windows.go`, `poll_fallback.go`, `poll_default.go`
//! Upstream Target Tag / Version: `v0.0.0-20260703014108-f5a850f9c2b7`
//!
//! <public-docs>
//! A poll reader that reads data from an underlying reader using different
//! native poll APIs depending on the operating system.
//!
//! Upstream platform selection:
//! - Linux uses the epoll API (`poll_linux.go`).
//! - Windows uses the Windows I/O and Console APIs (`poll_windows.go`).
//! - macOS and other BSD-based systems try the kqueue API and fall back to
//!   Unix `select` when kqueue is not available (e.g. on a TTY) — and for
//!   `/dev/tty` they use `select` directly (`poll_bsd.go`, `poll_select.go`).
//! - Solaris uses the select API (`poll_select.go`).
//! - All other systems fall back to a simple read loop with a timeout
//!   (`poll_fallback.go`, `poll_default.go`).
//!
//! This port unifies the Linux (epoll), BSD (kqueue) and select variants into
//! a single POSIX `libc::poll` implementation on Unix — `poll(2)` is
//! available on every platform the upstream split covers and provides the
//! same "wait for readability with a timeout, interruptible by a cancel
//! pipe" contract. The kqueue `/dev/tty` special case is unnecessary because
//! `poll(2)` works on TTYs. On non-Unix platforms the fallback stub
//! (`poll_fallback.go`) is used.
//!
//! The cancellation pipe is the Rust equivalent of the upstream cancel
//! signal pipe (kqueue/epoll/select readers) and of the channel-based cancel
//! (fallback reader).
//! </public-docs>

use std::io::Read;
use std::sync::{Arc, Condvar};
use std::time::Duration;

use crate::console::File;

/// An error returned by a poll or read operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PollError {
    /// The poll or read operation was canceled.
    Canceled,
    /// An I/O error occurred.
    Io(String),
}

impl std::fmt::Display for PollError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PollError::Canceled => write!(f, "poll canceled"),
            PollError::Io(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for PollError {}

/// PollReader reads data from an underlying reader, notifying when data is
/// available to read.
///
/// Use [PollReader::poll] to check for data availability before calling
/// `read` to avoid blocking.
pub trait PollReader: Read {
    /// Poll notifies when data is available to read with the given timeout.
    /// Use None to wait indefinitely.
    fn poll(&mut self, timeout: Option<Duration>) -> Result<bool, PollError>;

    /// Cancel cancels any ongoing poll or read operations. It returns true
    /// if an operation was canceled, false otherwise.
    fn cancel(&mut self) -> bool;

    /// Close closes the reader and releases any resources associated with it.
    fn close(&mut self) -> Result<(), PollError>;
}

/// newPollReader creates a new [PollReader] for the given file-backed reader.
///
/// On Unix this uses the POSIX `poll(2)` API (see the module docs for the
/// mapping from the upstream epoll/kqueue/select split). On other platforms
/// it returns a fallback reader.
pub fn new_poll_reader<R: File + 'static>(reader: R) -> Result<Box<dyn PollReader>, PollError> {
    #[cfg(unix)]
    {
        new_poll_reader_unix(Box::new(reader))
    }
    #[cfg(not(unix))]
    {
        new_fallback_reader(Box::new(reader))
    }
}

/// newFallbackReader creates a new fallback [PollReader] for the given
/// reader. This is the fallback implementation that works on all platforms,
/// including readers that do not expose a file descriptor.
pub fn new_fallback_reader<R: Read + Send + 'static>(
    reader: R,
) -> Result<Box<dyn PollReader>, PollError> {
    Ok(Box::new(FallbackReader::new(Box::new(reader))))
}

/// A file-backed poll reader on Unix, using the POSIX `poll(2)` API and a
/// self-pipe for cancellation.
///
/// This corresponds to the upstream `epollReader`, `kqueueReader`, and
/// `selectReader` implementations, unified on `poll(2)`.
#[cfg(unix)]
struct PollReaderUnix {
    reader: Box<dyn File>,
    cancel_reader: libc::c_int,
    cancel_writer: libc::c_int,
    canceled: bool,
}

#[cfg(unix)]
impl PollReaderUnix {
    fn new(reader: Box<dyn File>) -> Result<PollReaderUnix, PollError> {
        let mut fds = [0 as libc::c_int; 2];
        if unsafe { libc::pipe(fds.as_mut_ptr()) } != 0 {
            return Err(PollError::Io(std::io::Error::last_os_error().to_string()));
        }
        Ok(PollReaderUnix {
            reader,
            cancel_reader: fds[0],
            cancel_writer: fds[1],
            canceled: false,
        })
    }
}

#[cfg(unix)]
impl Read for PollReaderUnix {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.canceled {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "poll canceled",
            ));
        }
        self.reader.read(buf)
    }
}

#[cfg(unix)]
impl PollReader for PollReaderUnix {
    fn poll(&mut self, timeout: Option<Duration>) -> Result<bool, PollError> {
        if self.canceled {
            return Err(PollError::Canceled);
        }

        let mut fds = [
            libc::pollfd {
                fd: self.reader.fd() as libc::c_int,
                events: libc::POLLIN,
                revents: 0,
            },
            libc::pollfd {
                fd: self.cancel_reader,
                events: libc::POLLIN,
                revents: 0,
            },
        ];

        let timeout_ms: libc::c_int = match timeout {
            None => -1,
            Some(t) => t.as_millis().min(libc::c_int::MAX as u128) as libc::c_int,
        };

        loop {
            let n = unsafe { libc::poll(fds.as_mut_ptr(), 2, timeout_ms) };
            if n < 0 {
                let err = std::io::Error::last_os_error();
                if err.kind() == std::io::ErrorKind::Interrupted {
                    continue; // try again if the syscall was interrupted
                }
                return Err(PollError::Io(err.to_string()));
            }
            if n == 0 {
                return Ok(false); // timeout
            }
            break;
        }

        if fds[1].revents & libc::POLLIN != 0 {
            // remove signal from pipe
            let mut b = [0u8; 1];
            let read_err =
                unsafe { libc::read(self.cancel_reader, b.as_mut_ptr() as *mut libc::c_void, 1) };
            if read_err < 0 {
                return Err(PollError::Io(std::io::Error::last_os_error().to_string()));
            }
            return Err(PollError::Canceled);
        }

        Ok(true)
    }

    fn cancel(&mut self) -> bool {
        self.canceled = true;

        // send cancel signal
        let b = [b'c'];
        let n = unsafe { libc::write(self.cancel_writer, b.as_ptr() as *const libc::c_void, 1) };
        n == 1
    }

    fn close(&mut self) -> Result<(), PollError> {
        let mut errs = Vec::new();

        if unsafe { libc::close(self.cancel_writer) } != 0 {
            errs.push(format!(
                "closing cancel signal writer: {}",
                std::io::Error::last_os_error()
            ));
        }
        if unsafe { libc::close(self.cancel_reader) } != 0 {
            errs.push(format!(
                "closing cancel signal reader: {}",
                std::io::Error::last_os_error()
            ));
        }

        if !errs.is_empty() {
            return Err(PollError::Io(errs.join(", ")));
        }
        Ok(())
    }
}

#[cfg(unix)]
impl Drop for PollReaderUnix {
    fn drop(&mut self) {
        if self.cancel_writer >= 0 {
            unsafe {
                libc::close(self.cancel_writer);
            }
        }
        if self.cancel_reader >= 0 {
            unsafe {
                libc::close(self.cancel_reader);
            }
        }
    }
}

#[cfg(unix)]
fn new_poll_reader_unix(reader: Box<dyn File>) -> Result<Box<dyn PollReader>, PollError> {
    Ok(Box::new(PollReaderUnix::new(reader)?))
}

/// fallbackReader implements [PollReader] using a background thread and
/// buffered I/O. This is a fallback implementation that works on all
/// platforms, mirroring `fallbackReader` in `poll_fallback.go`.
///
/// A background thread blocks on the underlying reader and pushes the bytes
/// it receives into a buffer; [PollReader::poll] waits (with a timeout) for a
/// "data available" notification from the thread. Cancellation wakes the
/// waiter through a shared condition variable.
pub struct FallbackReader {
    inner: Arc<FallbackInner>,
}

struct FallbackInner {
    reader: std::sync::Mutex<Box<dyn Read + Send>>,
    buffer: std::sync::Mutex<Vec<u8>>,
    cond: Condvar,
    canceled: std::sync::Mutex<bool>,
    started: std::sync::Mutex<bool>,
    stopped: std::sync::Mutex<bool>,
}

impl FallbackReader {
    fn new(reader: Box<dyn Read + Send>) -> FallbackReader {
        FallbackReader {
            inner: Arc::new(FallbackInner {
                reader: std::sync::Mutex::new(reader),
                buffer: std::sync::Mutex::new(Vec::new()),
                cond: Condvar::new(),
                canceled: std::sync::Mutex::new(false),
                started: std::sync::Mutex::new(false),
                stopped: std::sync::Mutex::new(false),
            }),
        }
    }

    /// checkBuffered runs in a background thread to signal when data is
    /// available, mirroring `checkBuffered` in `poll_fallback.go`.
    fn check_buffered(inner: Arc<FallbackInner>) {
        let reader = std::mem::replace(
            &mut *inner.reader.lock().unwrap(),
            Box::new(std::io::empty()),
        );
        let mut reader = reader;
        let mut buf = [0u8; 1024];
        loop {
            {
                let canceled = inner.canceled.lock().unwrap();
                if *canceled {
                    break;
                }
            }

            // Block until data arrives.
            let n = match reader.read(&mut buf) {
                Ok(0) => break, // EOF: stop the thread
                Ok(n) => n,
                Err(_) => break, // error (including EOF): stop the thread
            };

            {
                let canceled = inner.canceled.lock().unwrap();
                if *canceled {
                    break;
                }
                inner.buffer.lock().unwrap().extend_from_slice(&buf[..n]);
            }
            inner.cond.notify_all();

            // Wait a bit before checking again to avoid busy loop.
            std::thread::sleep(Duration::from_millis(10));
        }
        *inner.stopped.lock().unwrap() = true;
        inner.cond.notify_all();
    }

    /// Returns the current state: (buffer_empty, canceled, stopped).
    fn state(&self) -> (bool, bool, bool) {
        (
            self.inner.buffer.lock().unwrap().is_empty(),
            *self.inner.canceled.lock().unwrap(),
            *self.inner.stopped.lock().unwrap(),
        )
    }
}

impl Read for FallbackReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut buffer = self.inner.buffer.lock().unwrap();
        loop {
            if !buffer.is_empty() {
                let n = buffer.len().min(buf.len());
                buf[..n].copy_from_slice(&buffer[..n]);
                buffer.drain(..n);
                return Ok(n);
            }
            if *self.inner.canceled.lock().unwrap() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Interrupted,
                    "poll canceled",
                ));
            }
            if *self.inner.stopped.lock().unwrap() {
                return Ok(0); // EOF
            }
            let (guard, _) = self
                .inner
                .cond
                .wait_timeout(buffer, Duration::from_millis(10))
                .unwrap();
            buffer = guard;
        }
    }
}

impl PollReader for FallbackReader {
    fn poll(&mut self, timeout: Option<Duration>) -> Result<bool, PollError> {
        {
            let canceled = self.inner.canceled.lock().unwrap();
            if *canceled {
                return Err(PollError::Canceled);
            }
        }

        // Start the background reader thread if not already started.
        let mut started = self.inner.started.lock().unwrap();
        if !*started {
            *started = true;
            let inner = Arc::clone(&self.inner);
            std::thread::spawn(move || Self::check_buffered(inner));
        }
        drop(started);

        let deadline = timeout.map(|t| std::time::Instant::now() + t);

        loop {
            let (empty, canceled, stopped) = self.state();
            if !empty {
                return Ok(true);
            }
            if canceled {
                return Err(PollError::Canceled);
            }
            if stopped {
                return Ok(false); // no data will ever arrive
            }

            let wait = match deadline {
                Some(d) => {
                    let now = std::time::Instant::now();
                    if now >= d {
                        return Ok(false); // timeout
                    }
                    d - now
                }
                None => Duration::from_millis(100),
            };

            let _ = self
                .inner
                .cond
                .wait_timeout(self.inner.buffer.lock().unwrap(), wait);
        }
    }

    fn cancel(&mut self) -> bool {
        let mut canceled = self.inner.canceled.lock().unwrap();
        if *canceled {
            return false;
        }
        *canceled = true;
        self.inner.cond.notify_all();
        true
    }

    fn close(&mut self) -> Result<(), PollError> {
        self.cancel();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_reader_non_file() {
        let mut pr = new_fallback_reader(std::io::Cursor::new(Vec::new())).unwrap();
        assert!(pr.cancel());
    }

    #[test]
    fn test_fallback_poll_timeout() {
        let mut pr = new_fallback_reader(std::io::Cursor::new(Vec::new())).unwrap();
        let ready = pr.poll(Some(Duration::from_millis(50))).unwrap();
        assert!(!ready);
    }

    #[test]
    fn test_fallback_canceled_after_cancel() {
        let mut pr = new_fallback_reader(std::io::Cursor::new(Vec::new())).unwrap();
        assert!(pr.cancel());
        let err = pr.poll(Some(Duration::from_millis(10))).unwrap_err();
        assert_eq!(err, PollError::Canceled);
    }

    #[test]
    fn test_fallback_reads_data() {
        let data = b"hello".to_vec();
        let mut pr = new_fallback_reader(std::io::Cursor::new(data)).unwrap();
        let ready = pr.poll(Some(Duration::from_millis(100))).unwrap();
        assert!(ready);
        let mut buf = [0u8; 8];
        let n = pr.read(&mut buf).unwrap();
        assert_eq!(&buf[..n], b"hello");
    }
}
