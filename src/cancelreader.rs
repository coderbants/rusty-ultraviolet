//! Cleanroom Rust port of upstream Go source file: `cancelreader_other.go`
//! Upstream Target Tag / Version: `v0.0.0-20260703014108-f5a850f9c2b7`
//!
//! Also covers (same upstream module): `cancelreader_windows.go`
//!
//! <user-docs>
//! The cancelable reader: wraps a reader with the ability to cancel a
//! blocking read (the port reuses the poll reader, which cancels via the
//! self-pipe).
//! On non-Unix targets the unsupported native operation returns a stable
//! platform error.
//! </user-docs>
//!
//! Internal maintainer note: the Unix implementation consumes the file-backed
//! poller; the non-Unix stub must remain independent of Unix descriptor types.

#[cfg(unix)]
use crate::console::File;
#[cfg(unix)]
use crate::poll::new_poll_reader;
use crate::poll::{PollError, PollReader};

/// NewCancelReader creates a new cancelable reader that provides a cancelable
/// reader interface that can be used to cancel reads.
///
/// NOTE: upstream wraps `muesli/cancelreader`; the port reuses the poll
/// reader which provides the same cancel-on-close semantics.
#[cfg(unix)]
pub fn new_cancel_reader<R: File + 'static>(r: R) -> Result<Box<dyn PollReader>, PollError> {
    new_poll_reader(r)
}

/// NewCancelReader creates a new cancelable reader.
#[cfg(not(unix))]
pub fn new_cancel_reader<R: std::io::Read + Send + 'static>(
    r: R,
) -> Result<Box<dyn PollReader>, PollError> {
    let _ = r;
    Err(PollError::Io(
        std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "platform not supported".to_string(),
        )
        .to_string(),
    ))
}

#[cfg(all(test, not(unix)))]
mod tests {
    use super::*;

    #[test]
    fn non_unix_cancel_reader_reports_stable_platform_error() {
        let result = new_cancel_reader(std::io::Cursor::new(Vec::new()));
        assert!(matches!(
            result,
            Err(PollError::Io(message)) if message == "platform not supported"
        ));
    }
}
