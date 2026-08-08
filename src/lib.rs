//! Cleanroom Rust port of upstream Go source file: `doc.go` / `uv.go`
//! Upstream Target Tag / Version: `v0.0.0-20251205161215-1948445e3318`
//!
//! <public-docs>
//! A high-performance terminal rendering library for Rust, ported from
//! Charmbracelet's Ultraviolet. Provides cell buffers, screen rendering, and
//! the primitives used by Bubble Tea's terminal renderer.
//! </public-docs>

#![deny(unsafe_code)]

pub mod buffer;
pub mod cell;
pub mod screen;
pub mod style;

pub use buffer::{new_buffer, Buffer, Line, Screen};
pub use cell::{empty_cell, new_link, Cell, Link};
pub use screen::{clear, clear_area, fill, fill_area, Rectangle};
pub use style::{style_diff, Attr, Style};
