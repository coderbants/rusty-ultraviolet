//! Cleanroom Rust port of upstream Go source file: `tty.go`
//! Upstream Target Tag / Version: `v0.0.0-20260703014108-f5a850f9c2b7`
//!
//! Also covers (same upstream module): `tty_unix.go`, `tty_other.go`
//!
//! <public-docs>
//! Terminal helpers: opening the controlling TTY, suspending the process
//! group, and window-size-change signal notification.
//!
//! NOTE: upstream uses `os/signal.Notify`; the port uses a self-pipe fed by
//! the signal handlers and delivers notifications through channels.
//! </public-docs>

#[cfg(not(unix))]
use crate::err_platform_not_supported;
use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::OnceLock;

/// The self-pipe write end used by the signal handlers. Bytes written here
/// wake the reader thread, which forwards them as notifications.
static PIPE_WRITE: OnceLock<std::os::fd::OwnedFd> = OnceLock::new();

/// Opens the self-pipe (and its reader thread) on first use.
fn ensure_signal_pipe() -> io::Result<std::os::fd::OwnedFd> {
    if let Some(fd) = PIPE_WRITE.get() {
        return Ok(unsafe { OwnedFd::from_raw_fd(libc::dup(fd.as_raw_fd())) });
    }

    let mut fds = [0 as libc::c_int; 2];
    unsafe {
        if libc::pipe(fds.as_mut_ptr()) != 0 {
            return Err(io::Error::last_os_error());
        }
    }
    let read_fd = unsafe { OwnedFd::from_raw_fd(fds[0]) };
    let write_fd = unsafe { OwnedFd::from_raw_fd(fds[1]) };
    match PIPE_WRITE.set(unsafe { OwnedFd::from_raw_fd(libc::dup(fds[1])) }) {
        Ok(()) => {
            // Reader thread: forwards signal bytes to the subscribers.
            std::thread::spawn(move || {
                let mut buf = [0u8; 64];
                loop {
                    let n = unsafe {
                        libc::read(
                            read_fd.as_raw_fd(),
                            buf.as_mut_ptr() as *mut libc::c_void,
                            buf.len(),
                        )
                    };
                    if n <= 0 {
                        break;
                    }
                    for &b in &buf[..n as usize] {
                        if b == 1 {
                            // SIGWINCH
                            forward_winch();
                        }
                    }
                }
            });
            Ok(write_fd)
        }
        Err(_) => {
            // Already initialized by a concurrent call; use the stored fd.
            let fd = PIPE_WRITE.get().unwrap();
            Ok(unsafe { OwnedFd::from_raw_fd(libc::dup(fd.as_raw_fd())) })
        }
    }
}

/// The signal handler entry point; dispatch based on the signal number.
#[cfg(unix)]
extern "C" fn signal_handler(sig: libc::c_int) {
    let byte = if sig == libc::SIGWINCH { 1u8 } else { 2u8 };
    if let Some(fd) = PIPE_WRITE.get() {
        unsafe {
            libc::write(fd.as_raw_fd(), &byte as *const u8 as *const libc::c_void, 1);
        }
    }
}

/// OpenTTY opens the terminal's input and output file descriptors.
#[cfg(unix)]
pub fn open_tty() -> io::Result<(OwnedFd, OwnedFd)> {
    let fd = unsafe {
        let fd = libc::open(c"/dev/tty".as_ptr(), libc::O_RDWR);
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        OwnedFd::from_raw_fd(fd)
    };
    // The same fd serves as both input and output.
    let out = unsafe { OwnedFd::from_raw_fd(libc::dup(fd.as_raw_fd())) };
    Ok((fd, out))
}

/// OpenTTY opens the terminal's input and output file descriptors.
#[cfg(not(unix))]
pub fn open_tty() -> io::Result<(OwnedFd, OwnedFd)> {
    Err(err_platform_not_supported())
}

/// Suspend suspends the current process group by sending SIGTSTP, then
/// waits for SIGCONT.
#[cfg(unix)]
pub fn suspend() -> io::Result<()> {
    let _pipe = ensure_signal_pipe()?;
    unsafe {
        let handler = signal_handler as extern "C" fn(libc::c_int);
        libc::signal(libc::SIGCONT, handler as usize);
        if libc::kill(0, libc::SIGTSTP) != 0 {
            return Err(io::Error::last_os_error());
        }
    }
    // Blocks until a SIGCONT arrives (byte 2 in the pipe).
    loop {
        let mut buf = [0u8; 16];
        let n = unsafe {
            libc::read(
                _pipe.as_raw_fd(),
                buf.as_mut_ptr() as *mut libc::c_void,
                buf.len(),
            )
        };
        if n > 0 && buf[..n as usize].contains(&2) {
            break;
        }
        if n <= 0 {
            return Err(io::Error::last_os_error());
        }
    }
    Ok(())
}

/// Suspend suspends the current process group.
#[cfg(not(unix))]
pub fn suspend() -> io::Result<()> {
    Err(err_platform_not_supported())
}

/// NotifyWinch sets up a channel to receive window size change signals.
pub fn notify_winch() -> io::Result<Receiver<()>> {
    let _pipe = ensure_signal_pipe()?;
    #[cfg(unix)]
    unsafe {
        let handler = signal_handler as extern "C" fn(libc::c_int);
        libc::signal(libc::SIGWINCH, handler as usize);
    }
    let (tx, rx) = channel();
    // Subscribe this receiver to future pipe notifications.
    WINCH_SUBSCRIBERS
        .get_or_init(|| std::sync::Mutex::new(Vec::new()))
        .lock()
        .unwrap()
        .push(tx);
    Ok(rx)
}

static WINCH_SUBSCRIBERS: OnceLock<std::sync::Mutex<Vec<Sender<()>>>> = OnceLock::new();

/// Forwards a signal byte to all winch subscribers. Called by the reader
/// thread when byte 1 (SIGWINCH) arrives.
pub(crate) fn forward_winch() {
    if let Some(subs) = WINCH_SUBSCRIBERS.get() {
        for tx in subs.lock().unwrap().iter() {
            let _ = tx.send(());
        }
    }
}
