//! Cleanroom Rust port of upstream Go source file: `logger.go`
//! Upstream Target Tag / Version: `v0.0.0-20260703014108-f5a850f9c2b7`
//!
//! <public-docs>
//! The simple logger interface used by the renderer and screen for debugging.
//! </public-docs>

use std::io::Write;

/// Logger is a simple logger interface.
///
/// NOTE: upstream `Logger.Printf(format, v ...interface{})` is variadic; the
/// arguments are modeled as a slice of already-formatted strings.
pub trait Logger {
    /// Printf logs a formatted message.
    fn printf(&mut self, format: &str, args: &[String]);
}

/// A logger writing to a file, used for `UV_DEBUG` output.
///
/// NOTE: upstream uses `log.SetOutput(f)` with the Go standard logger, which
/// prefixes messages with a date/time. The port writes the raw message.
pub struct FileLogger(pub std::fs::File);

impl Logger for FileLogger {
    fn printf(&mut self, format: &str, args: &[String]) {
        let mut msg = format.to_string();
        for a in args {
            msg.push(' ');
            msg.push_str(a);
        }
        let _ = writeln!(self.0, "{msg}");
    }
}
