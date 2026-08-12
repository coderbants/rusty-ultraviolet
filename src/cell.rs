//! Cleanroom Rust port of upstream Go source file: `cell.go` (Cell, Link)
//! Upstream Target Tag / Version: `v0.0.0-20260703014108-f5a850f9c2b7`
//!
//! <public-docs>
//! The cell model: a single grapheme cluster with a style, optional link, and
//! measured width.
//! </public-docs>

use crate::style::Style;

/// EmptyCell is a cell with a single space, width of 1, and no style or link.
pub fn empty_cell() -> Cell {
    Cell {
        content: " ".to_string(),
        width: 1,
        ..Cell::default()
    }
}

/// Cell represents a single cell in the terminal screen.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Cell {
    /// Content is the cell's content, which consists of a single grapheme
    /// cluster. Most of the time, this will be a single rune as well, but it
    /// can also be a combination of runes that form a grapheme cluster.
    pub content: String,
    /// The style of the cell. Nil style means no style. Zero value prints a
    /// reset sequence.
    pub style: Style,
    /// Link is the hyperlink of the cell.
    pub link: Option<Link>,
    /// Width is the mono-spaced width of the grapheme cluster.
    pub width: usize,
}

impl Cell {
    /// Creates a new cell from the given string grapheme. It will only use
    /// the first grapheme in the string and ignore the rest.
    pub fn new(gr: &str) -> Cell {
        if gr.is_empty() {
            return Cell::default();
        }
        if gr == " " {
            return empty_cell();
        }
        Cell {
            content: gr.to_string(),
            width: charming_x_ansi::util::string_width(gr),
            ..Cell::default()
        }
    }

    /// Returns the string content of the cell excluding any styles, links,
    /// and escape sequences.
    pub fn string(&self) -> &str {
        &self.content
    }

    /// Returns whether the cell is equal to the other cell.
    pub fn equal(&self, o: &Cell) -> bool {
        self.width == o.width
            && self.content == o.content
            && self.style == o.style
            && self.link == o.link
    }

    /// Returns whether the cell is an empty cell.
    pub fn is_zero(&self) -> bool {
        self.content.is_empty() && self.style.is_zero() && self.link.is_none() && self.width == 0
    }

    /// isWidePlaceholder reports whether the cell is the continuation column
    /// of a wide cell, marked with a zero display width.
    pub fn is_wide_placeholder(&self) -> bool {
        self.width == 0
    }

    /// Returns a copy of the cell.
    pub fn clone_cell(&self) -> Cell {
        self.clone()
    }

    /// Makes the cell an empty cell by setting its content to a single space
    /// and width to 1.
    pub fn empty(&mut self) {
        self.content = " ".to_string();
        self.width = 1;
    }
}

/// Creates a new hyperlink with the given URL and parameters.
pub fn new_link(url: &str, params: &[&str]) -> Link {
    Link {
        url: url.to_string(),
        params: params.join(":"),
    }
}

/// Link represents a hyperlink in the terminal screen.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Link {
    /// The URL of the hyperlink.
    pub url: String,
    /// The parameters of the hyperlink.
    pub params: String,
}

impl Link {
    /// Returns a string representation of the hyperlink.
    pub fn to_string(&self) -> &str {
        &self.url
    }

    /// Returns whether the hyperlink is empty.
    pub fn is_zero(&self) -> bool {
        self.url.is_empty() && self.params.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_cell() {
        let c = Cell::new("a");
        assert_eq!(c.content, "a");
        assert_eq!(c.width, 1);
        let e = Cell::new(" ");
        assert_eq!(e.content, " ");
        assert_eq!(e.width, 1);
        assert!(!e.is_zero());
    }
}
