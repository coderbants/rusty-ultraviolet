//! Focused regression coverage for the non-Unix terminal-operation stubs.
//!
//! The Windows implementation is intentionally deferred. These tests keep
//! the non-Unix API compile-safe and verify that unavailable operations expose
//! stable, explicit platform errors without starting an uninterruptible
//! fallback reader.

#![cfg(not(unix))]

use std::io::Cursor;

use rusty_ultraviolet::tty::{open_tty, suspend};
use rusty_ultraviolet::{new_cancel_reader, new_poll_reader, FdFile, PollError};

#[test]
fn tty_stubs_report_unsupported_errors() {
    assert_eq!(
        open_tty().unwrap_err().kind(),
        std::io::ErrorKind::Unsupported
    );
    assert_eq!(
        suspend().unwrap_err().kind(),
        std::io::ErrorKind::Unsupported
    );
}

#[test]
fn window_notification_stub_reports_unsupported_error() {
    let result = rusty_ultraviolet::tty::notify_winch();
    assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::Unsupported);
}

#[test]
fn cancel_reader_stub_reports_stable_error_text() {
    let result = new_cancel_reader(Cursor::new(Vec::<u8>::new()));
    assert!(matches!(
        result,
        Err(PollError::Io(message)) if message == "platform not supported"
    ));
}

#[test]
fn file_backed_poll_reader_reports_unsupported_until_native_impl() {
    let result = new_poll_reader(FdFile::stdin_file());
    assert!(matches!(
        result,
        Err(PollError::Io(message)) if message == "platform not supported"
    ));
}
