//! Cleanroom Rust port of upstream Go source file: `cancelreader_other.go`
//! Upstream Target Tag / Version: `v0.0.0-20260703014108-f5a850f9c2b7`
//!
//! Also covers (same upstream module): `cancelreader_windows.go`
//!
//! <public-docs>
//! The cancelable reader: wraps a reader with the ability to cancel a
//! blocking read (the port reuses the poll reader, which cancels via the
//! self-pipe).
//! </public-docs>

use crate::console::File;
use crate::poll::{new_poll_reader, PollError, PollReader};

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
    Err(PollError::Io(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "platform not supported".to_string(),
    )))
}
