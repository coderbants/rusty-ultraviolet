//! Cleanroom Rust port of upstream Go source files: `console.go`, `console_unix.go`, `console_windows.go`
//! Upstream Target Tag / Version: `v0.0.0-20260703014108-f5a850f9c2b7`
//!
//! <public-docs>
//! A cross-platform console I/O abstraction: a pair of input/output files
//! with environment access, raw-mode control, and size queries.
//!
//! The upstream build-tag split (`console_unix.go` -> `TTY`, `console_windows.go`
//! -> `WinCon`) is represented with the `TTY` and `WinCon` type aliases.
//! The raw-mode state type comes from `charmbracelet/x/term` upstream; here it
//! is implemented directly on `libc` termios (Unix) since `charming-x-term` is
//! not a dependency of this crate.
//! </public-docs>

use std::io::{Read, Write};

/// File is an interface that represents a file with a file descriptor.
///
/// This is typically an OS file like stdin and stdout.
pub trait File: Read + Write + std::fmt::Debug {
    /// Returns the file descriptor of the file.
    fn fd(&self) -> usize;
    /// Returns the name of the file.
    fn name(&self) -> &str;
}

/// Winsize represents the size of a terminal in cells and pixels.
///
/// This is the same as the Unix `winsize` struct, but defined here for
/// cross-platform compatibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Winsize {
    /// The number of rows (height) in cells.
    pub row: u16,
    /// The number of columns (width) in cells.
    pub col: u16,
    /// The width of the terminal in pixels.
    pub xpixel: u16,
    /// The height of the terminal in pixels.
    pub ypixel: u16,
}

/// An error returned by console operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConsoleError {
    /// The file is not a terminal.
    NotTerminal,
    /// The platform is not supported.
    PlatformNotSupported,
    /// An I/O error occurred.
    Io(String),
}

impl std::fmt::Display for ConsoleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConsoleError::NotTerminal => write!(f, "not a terminal"),
            ConsoleError::PlatformNotSupported => write!(f, "platform not supported"),
            ConsoleError::Io(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for ConsoleError {}

/// The raw-mode state of a console's input side. On Unix this is the raw
/// `termios` struct saved before entering raw mode.
#[derive(Debug, Clone)]
#[cfg(unix)]
pub struct RawState(libc::termios);

/// The raw-mode state of a console's input side (non-Unix stub).
#[derive(Debug, Clone)]
#[cfg(not(unix))]
pub struct RawState(());

/// A Unix TTY device. It implements the [Console] interface.
#[cfg(not(windows))]
pub type TTY = Console;

/// A Windows Console. It implements the [Console] interface.
#[cfg(windows)]
pub type WinCon = Console;

/// Console is a cross-platform console I/O.
#[derive(Debug)]
pub struct Console {
    input: Box<dyn File>,
    input_state: Option<RawState>,
    output: Box<dyn File>,
    output_state: Option<RawState>,
    environ: Vec<String>,
}

/// DefaultConsole returns a new default console instance that uses standard
/// I/O (stdin, stdout) and the process environment.
///
/// To use stderr as the output, you can create a new console with
/// [new_console] and pass stderr as the output parameter.
pub fn default_console() -> Console {
    new_console(None, None, None)
}

/// ControllingConsole returns a new console instance that uses the current
/// controlling terminal's input and output file descriptors.
pub fn controlling_console() -> Result<Console, ConsoleError> {
    let tty = open_tty()?;
    Ok(new_console(Some(Box::new(tty)), None, None))
}

/// NewConsole creates a new [Console] with the given input, output, and
/// environment variables.
///
/// You can use [open_tty] to open the current controlling console files and
/// pass them to this function. Use [controlling_console] for a convenience
/// function that does this for you.
///
/// Use this to create a new terminal for PTY processes by passing the PTY
/// slave file as the input and output and any environment variables the
/// process needs.
pub fn new_console(
    input: Option<Box<dyn File>>,
    output: Option<Box<dyn File>>,
    environ: Option<Vec<String>>,
) -> Console {
    let input = input.unwrap_or_else(|| Box::new(FdFile::stdin_file()));
    let output = output.unwrap_or_else(|| Box::new(FdFile::stdout_file()));
    let environ = environ.unwrap_or_else(vars_env);
    new_console_impl(input, output, environ)
}

/// Returns the process environment as a `KEY=VALUE` list.
fn vars_env() -> Vec<String> {
    std::env::vars()
        .map(|(k, v)| format!("{k}={v}"))
        .collect()
}

/// The platform-split console constructor: `console_unix.go` wraps the
/// console in a `TTY`, `console_windows.go` in a `WinCon`.
#[cfg(not(windows))]
fn new_console_impl(input: Box<dyn File>, output: Box<dyn File>, environ: Vec<String>) -> Console {
    Console {
        input,
        input_state: None,
        output,
        output_state: None,
        environ,
    }
}

#[cfg(windows)]
fn new_console_impl(input: Box<dyn File>, output: Box<dyn File>, environ: Vec<String>) -> Console {
    Console {
        input,
        input_state: None,
        output,
        output_state: None,
        environ,
    }
}

impl Console {
    /// Environ returns the console's environment variables.
    pub fn environ(&self) -> &[String] {
        &self.environ
    }

    /// Writer returns the output writer of the console.
    pub fn writer(&mut self) -> &mut dyn Write {
        &mut self.output
    }

    /// Reader returns the input reader of the console.
    pub fn reader(&mut self) -> &mut dyn Read {
        &mut self.input
    }

    /// InputFd returns the file descriptor of the console's input.
    pub fn input_fd(&self) -> usize {
        self.input.fd()
    }

    /// Write writes data to the console's output.
    pub fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.output.write(buf)
    }

    /// Read reads data from the console's input.
    pub fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.input.read(buf)
    }

    /// Getenv retrieves the value of the environment variable named by the
    /// key.
    pub fn getenv(&self, key: &str) -> String {
        self.lookup_env(key).unwrap_or_default().to_string()
    }

    /// LookupEnv retrieves the value of the environment variable named by the
    /// key. Returns None if the variable is not present.
    pub fn lookup_env(&self, key: &str) -> Option<&str> {
        let prefix = format!("{key}=");
        for entry in self.environ.iter().rev() {
            if let Some(v) = entry.strip_prefix(&prefix) {
                return Some(v);
            }
        }
        None
    }

    /// MakeRaw puts the console input side into raw mode.
    pub fn make_raw(&mut self) -> Result<(), ConsoleError> {
        let in_state = make_raw_side(Some(self.input.as_ref()), Some(self.output.as_ref()))?;
        self.input_state = in_state;
        Ok(())
    }

    /// Restore restores the console to its previous state.
    pub fn restore(&mut self) -> Result<(), ConsoleError> {
        if let Some(state) = self.input_state.take() {
            restore_side(self.input.fd(), &state)?;
        }
        if let Some(state) = self.output_state.take() {
            restore_side(self.output.fd(), &state)?;
        }
        Ok(())
    }

    /// Close restores the console to its previous state and releases
    /// resources.
    pub fn close(&mut self) -> Result<(), ConsoleError> {
        self.restore()
    }

    /// GetSize returns the current size of the console.
    pub fn get_size(&mut self) -> Result<(usize, usize), ConsoleError> {
        let ws = self.get_winsize()?;
        Ok((ws.col as usize, ws.row as usize))
    }

    /// GetWinsize returns the current size of the console in cells and
    /// pixels.
    pub fn get_winsize(&mut self) -> Result<Winsize, ConsoleError> {
        get_winsize(Some(self.input.as_ref()), Some(self.output.as_ref()))
    }
}

/// A file opened on a tty device.
///
/// This mirrors `os.OpenFile("/dev/tty", os.O_RDWR, 0)` upstream.
#[cfg(unix)]
pub fn open_tty() -> Result<FdFile, ConsoleError> {
    let fd = unsafe { libc::open(b"/dev/tty\0".as_ptr() as *const libc::c_char, libc::O_RDWR) };
    if fd < 0 {
        return Err(ConsoleError::Io(std::io::Error::last_os_error().to_string()));
    }
    Ok(FdFile {
        fd,
        name: "/dev/tty".to_string(),
        close_on_drop: true,
    })
}

/// OpenTTY stub for unsupported platforms.
#[cfg(not(unix))]
pub fn open_tty() -> Result<FdFile, ConsoleError> {
    Err(ConsoleError::PlatformNotSupported)
}

/// A file backed by a raw file descriptor, providing `Read`/`Write` through
/// `libc`. Used for the standard streams and the controlling tty; the
/// standard-stream instances do not close the descriptor on drop.
#[derive(Debug)]
pub struct FdFile {
    fd: libc::c_int,
    name: String,
    close_on_drop: bool,
}

impl FdFile {
    /// Returns the file descriptor.
    pub fn fd(&self) -> usize {
        self.fd as usize
    }
}

impl FdFile {
    /// Creates a file handle for the standard input (fd 0).
    pub fn stdin_file() -> FdFile {
        FdFile {
            fd: 0,
            name: "/dev/stdin".to_string(),
            close_on_drop: false,
        }
    }

    /// Creates a file handle for the standard output (fd 1).
    pub fn stdout_file() -> FdFile {
        FdFile {
            fd: 1,
            name: "/dev/stdout".to_string(),
            close_on_drop: false,
        }
    }

    /// Creates a file handle for the standard error (fd 2).
    pub fn stderr_file() -> FdFile {
        FdFile {
            fd: 2,
            name: "/dev/stderr".to_string(),
            close_on_drop: false,
        }
    }
}

#[cfg(unix)]
impl Read for FdFile {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = unsafe {
            libc::read(
                self.fd,
                buf.as_mut_ptr() as *mut libc::c_void,
                buf.len(),
            )
        };
        if n < 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(n as usize)
    }
}

#[cfg(unix)]
impl Write for FdFile {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let n = unsafe { libc::write(self.fd, buf.as_ptr() as *const libc::c_void, buf.len()) };
        if n < 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok(n as usize)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(unix)]
impl File for FdFile {
    fn fd(&self) -> usize {
        self.fd as usize
    }

    fn name(&self) -> &str {
        &self.name
    }
}

#[cfg(unix)]
impl Drop for FdFile {
    fn drop(&mut self) {
        if self.close_on_drop {
            unsafe {
                libc::close(self.fd);
            }
        }
    }
}

#[cfg(not(unix))]
impl Read for FdFile {
    fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "platform not supported",
        ))
    }
}

#[cfg(not(unix))]
impl Write for FdFile {
    fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "platform not supported",
        ))
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(not(unix))]
impl File for FdFile {
    fn fd(&self) -> usize {
        self.fd as usize
    }

    fn name(&self) -> &str {
        &self.name
    }
}

#[cfg(not(unix))]
impl Drop for FdFile {
    fn drop(&mut self) {}
}

/// makeRaw puts the input side into raw mode, mirroring `makeRaw` in
/// `terminal_unix.go` (which calls `term.MakeRaw`).
///
/// The raw-mode implementation is a direct `libc` port of the essential
/// `charmbracelet/x/term` `MakeRaw` behaviour (tcgetattr/cfmakeraw/
/// tcsetattr).
#[cfg(unix)]
fn make_raw_side(
    in_tty: Option<&dyn File>,
    out_tty: Option<&dyn File>,
) -> Result<Option<RawState>, ConsoleError> {
    if in_tty.is_none() && out_tty.is_none() {
        return Err(ConsoleError::NotTerminal);
    }

    for f in [in_tty, out_tty].into_iter().flatten() {
        let mut termios: libc::termios = unsafe { std::mem::zeroed() };
        if unsafe { libc::tcgetattr(f.fd() as libc::c_int, &mut termios) } == 0 {
            let original = RawState(termios);
            unsafe {
                libc::cfmakeraw(&mut termios);
                if libc::tcsetattr(f.fd() as libc::c_int, libc::TCSANOW, &termios) != 0 {
                    return Err(ConsoleError::Io(std::io::Error::last_os_error().to_string()));
                }
            }
            return Ok(Some(original));
        }
    }

    Err(ConsoleError::Io(std::io::Error::last_os_error().to_string()))
}

#[cfg(not(unix))]
fn make_raw_side(
    _in_tty: Option<&dyn File>,
    _out_tty: Option<&dyn File>,
) -> Result<Option<RawState>, ConsoleError> {
    Err(ConsoleError::PlatformNotSupported)
}

#[cfg(unix)]
fn restore_side(fd: usize, state: &RawState) -> Result<(), ConsoleError> {
    let termios = &state.0;
    if unsafe { libc::tcsetattr(fd as libc::c_int, libc::TCSANOW, termios) } != 0 {
        return Err(ConsoleError::Io(std::io::Error::last_os_error().to_string()));
    }
    Ok(())
}

#[cfg(not(unix))]
fn restore_side(_fd: usize, _state: &RawState) -> Result<(), ConsoleError> {
    Err(ConsoleError::PlatformNotSupported)
}

/// GetWinsizeForFd returns the terminal size of the given file descriptor,
/// mirroring `termios.GetWinsize` in `winch_unix.go`.
#[cfg(unix)]
pub(crate) fn get_winsize_for_fd(fd: usize) -> Result<Winsize, ConsoleError> {
    let mut ws: libc::winsize = unsafe { std::mem::zeroed() };
    let r = unsafe { libc::ioctl(fd as libc::c_int, libc::TIOCGWINSZ, &mut ws) };
    if r == 0 {
        return Ok(Winsize {
            row: ws.ws_row,
            col: ws.ws_col,
            xpixel: ws.ws_xpixel,
            ypixel: ws.ws_ypixel,
        });
    }
    Err(ConsoleError::Io(std::io::Error::last_os_error().to_string()))
}

/// GetWinsizeForFd returns the terminal size of the given file descriptor.
#[cfg(not(unix))]
pub(crate) fn get_winsize_for_fd(_fd: usize) -> Result<Winsize, ConsoleError> {
    Err(ConsoleError::PlatformNotSupported)
}

/// getWinsize returns the terminal size of the given files, trying each in
/// turn, mirroring `getWinsize` in `terminal_unix.go`.
#[cfg(unix)]
fn get_winsize(in_tty: Option<&dyn File>, out_tty: Option<&dyn File>) -> Result<Winsize, ConsoleError> {
    let mut err: Option<ConsoleError> = Some(ConsoleError::NotTerminal);
    for f in [in_tty, out_tty].into_iter().flatten() {
        let mut ws: libc::winsize = unsafe { std::mem::zeroed() };
        let r = unsafe { libc::ioctl(f.fd() as libc::c_int, libc::TIOCGWINSZ, &mut ws) };
        if r == 0 {
            return Ok(Winsize {
                row: ws.ws_row,
                col: ws.ws_col,
                xpixel: ws.ws_xpixel,
                ypixel: ws.ws_ypixel,
            });
        }
        err = Some(ConsoleError::Io(std::io::Error::last_os_error().to_string()));
    }
    Err(err.unwrap())
}

#[cfg(not(unix))]
fn get_winsize(_in_tty: Option<&dyn File>, _out_tty: Option<&dyn File>) -> Result<Winsize, ConsoleError> {
    Err(ConsoleError::PlatformNotSupported)
}
