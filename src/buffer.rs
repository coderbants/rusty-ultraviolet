//! Cleanroom Rust port of upstream Go source file: `buffer.go`
//! Upstream Target Tag / Version: `v0.0.0-20251205161215-1948445e3318`
//!
//! <public-docs>
//! The cell buffer: a grid of cells with render support, plus the `Screen`
//! interface.
//! </public-docs>

use crate::cell::{empty_cell, Cell};
use crate::screen::Rectangle;
use crate::style::Style;
use charming_x_ansi::hyperlink::{reset_hyperlink, set_hyperlink};

/// A row of cells.
pub type Line = Vec<Cell>;

/// Screen is the interface that wraps the basic methods of a screen buffer.
pub trait Screen {
    /// Bounds returns the bounds of the screen.
    fn bounds(&self) -> Rectangle;
    /// Width returns the width of the screen.
    fn width(&self) -> usize;
    /// Height returns the height of the screen.
    fn height(&self) -> usize;
    /// CellAt returns the cell at the given position, or None if out of bounds.
    fn cell_at(&self, x: usize, y: usize) -> Option<&Cell>;
    /// CellAtMut returns a mutable reference to the cell at the given position.
    fn cell_at_mut(&mut self, x: usize, y: usize) -> Option<&mut Cell>;
    /// Provides access to the underlying concrete type for downcasting.
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

/// Buffer represents a cell buffer that contains the contents of a screen.
#[derive(Debug, Clone)]
pub struct Buffer {
    /// Lines is a slice of lines that make up the cells of the buffer.
    pub lines: Vec<Line>,
    /// The width of the buffer.
    pub width: usize,
    /// The height of the buffer.
    pub height: usize,
}

/// NewBuffer creates a new buffer with the given width and height.
pub fn new_buffer(width: usize, height: usize) -> Buffer {
    let mut b = Buffer {
        lines: Vec::new(),
        width,
        height,
    };
    b.resize(width, height);
    b
}

impl Buffer {
    /// Resizes the buffer, keeping the current contents where they fit.
    pub fn resize(&mut self, width: usize, height: usize) {
        self.width = width;
        self.height = height;
        self.lines = vec![vec![empty_cell(); width]; height];
    }

    /// Returns the width of the buffer.
    pub fn width(&self) -> usize {
        self.width
    }

    /// Returns the height of the buffer.
    pub fn height(&self) -> usize {
        self.height
    }

    /// Returns the bounds of the buffer.
    pub fn bounds(&self) -> Rectangle {
        Rectangle {
            min: (0, 0),
            max: (self.width, self.height),
        }
    }

    /// Returns the line at the given y position.
    pub fn line(&self, y: usize) -> Option<&Line> {
        self.lines.get(y)
    }

    /// Returns the cell at the given position.
    pub fn cell_at(&self, x: usize, y: usize) -> Option<&Cell> {
        self.lines.get(y)?.get(x)
    }

    /// Returns a mutable reference to the cell at the given position.
    pub fn cell_at_mut(&mut self, x: usize, y: usize) -> Option<&mut Cell> {
        self.lines.get_mut(y)?.get_mut(x)
    }

    /// Sets the cell at the given position.
    pub fn set_cell(&mut self, x: usize, y: usize, c: Cell) {
        if let Some(cell) = self.cell_at_mut(x, y) {
            *cell = c;
        }
    }

    /// Fills the buffer with the given cell (or empty cells if None).
    pub fn fill(&mut self, c: Option<Cell>) {
        for line in &mut self.lines {
            for cell in line {
                *cell = match &c {
                    Some(c) => c.clone(),
                    None => empty_cell(),
                };
            }
        }
    }

    /// Fills the given area with the given cell.
    pub fn fill_area(&mut self, c: Option<Cell>, area: Rectangle) {
        for y in area.min.1..area.max.1.min(self.height) {
            for x in area.min.0..area.max.0.min(self.width) {
                if let Some(cell) = self.cell_at_mut(x, y) {
                    *cell = match &c {
                        Some(c) => c.clone(),
                        None => empty_cell(),
                    };
                }
            }
        }
    }

    /// Clears the buffer with empty cells.
    pub fn clear(&mut self) {
        self.fill(None);
    }

    /// Clears the given area with empty cells.
    pub fn clear_area(&mut self, area: Rectangle) {
        self.fill_area(None, area);
    }

    /// Renders the buffer to a styled string with all the required
    /// attributes and styles.
    pub fn render(&self) -> String {
        let mut buf = String::new();
        for (i, l) in self.lines.iter().enumerate() {
            render_line(&mut buf, l);
            if i < self.lines.len() - 1 {
                buf.push('\n');
            }
        }
        buf
    }

    /// Draws the buffer onto the given screen within the specified area.
    pub fn draw(&self, scr: &mut dyn Screen, area: Rectangle) {
        for y in area.min.1..area.max.1 {
            for x in area.min.0..area.max.0 {
                if let Some(cell) = self.cell_at(x - area.min.0, y - area.min.1) {
                    if let Some(target) = scr.cell_at_mut(x, y) {
                        *target = cell.clone();
                    }
                }
            }
        }
    }
}

impl Screen for Buffer {
    fn bounds(&self) -> Rectangle {
        self.bounds()
    }
    fn width(&self) -> usize {
        self.width
    }
    fn height(&self) -> usize {
        self.height
    }
    fn cell_at(&self, x: usize, y: usize) -> Option<&Cell> {
        self.cell_at(x, y)
    }
    fn cell_at_mut(&mut self, x: usize, y: usize) -> Option<&mut Cell> {
        self.cell_at_mut(x, y)
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

/// Renders a line to the buffer, mirroring `renderLine` upstream: empty cells
/// are collapsed into pending spaces that are only emitted before non-empty
/// cells (so trailing blank cells are trimmed).
fn render_line(buf: &mut String, l: &Line) {
    let mut pen = Style::default();
    let mut link: Option<crate::cell::Link> = None;
    let mut pending = String::new();

    for c in l {
        if c.is_zero() {
            pending.push(' ');
            continue;
        }

        if !pending.is_empty() {
            buf.push_str(&pending);
            pending.clear();
        }

        if c.style.is_zero() && !pen.is_zero() {
            buf.push_str(charming_x_ansi::style::RESET_STYLE);
            pen = Style::default();
        }
        if !c.style.equal(&pen) {
            let seq = c.style.diff(&pen);
            buf.push_str(&seq);
            pen = c.style.clone();
        }

        // Write the URL escape sequence.
        if c.link != link {
            if let Some(link) = &link {
                if !link.is_zero() {
                    buf.push_str(reset_hyperlink());
                }
            }
            if let Some(l) = &c.link {
                buf.push_str(&set_hyperlink(&l.url, &l.params));
            }
            link = c.link.clone();
        }

        buf.push_str(&c.content);
    }

    if let Some(link) = &link {
        if !link.is_zero() {
            buf.push_str(reset_hyperlink());
        }
    }
    if !pen.is_zero() {
        buf.push_str(charming_x_ansi::style::RESET_STYLE);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_basic() {
        let mut b = new_buffer(3, 2);
        assert_eq!(b.width(), 3);
        assert_eq!(b.height(), 2);
        b.set_cell(0, 0, Cell::new("a"));
        b.set_cell(1, 0, Cell::new("b"));
        b.set_cell(2, 0, Cell::new("c"));
        let out = b.render();
        assert_eq!(out, "abc\n   ");
    }

    #[test]
    fn test_buffer_clear() {
        let mut b = new_buffer(2, 1);
        b.set_cell(0, 0, Cell::new("x"));
        b.clear();
        assert_eq!(b.cell_at(0, 0).unwrap().content, " ");
    }
}
