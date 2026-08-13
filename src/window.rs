//! Cleanroom Rust port of upstream Go source file: `window.go`
//! Upstream Target Tag / Version: `v0.0.0-20260703014108-f5a850f9c2b7`
//!
//! <public-docs>
//! A rectangular area on the screen. A window can be a root window with no
//! parent, or a sub-window with a parent window; it can own its own buffer or
//! share the buffer of its parent (a "view").
//!
//! This module also hosts the root-package geometry helpers (`Position`,
//! `Pos`, `Rect`) declared in upstream `buffer.go`, pending their final
//! wiring by the integrator.
//! </public-docs>

use std::cell::RefCell;
use std::rc::Rc;

use rusty_x_ansi::method::WidthMethod;

use crate::buffer::{new_buffer, Buffer};
use crate::cell::{empty_cell, Cell};
use crate::screen::Rectangle;

/// Position represents a position in a coordinate system.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    /// The x coordinate.
    pub x: i64,
    /// The y coordinate.
    pub y: i64,
}

/// Pos is a shorthand for creating a new [Position].
pub fn pos(x: i64, y: i64) -> Position {
    Position { x, y }
}

impl Position {
    /// Returns whether the position is inside the given rectangle.
    pub fn in_rect(&self, r: Rectangle) -> bool {
        self.x >= r.min.0 as i64
            && self.x < r.max.0 as i64
            && self.y >= r.min.1 as i64
            && self.y < r.max.1 as i64
    }
}

/// Rect is a shorthand for creating a new [Rectangle] from a point and a size.
///
/// Width and height are clamped to zero when negative.
pub fn rect(x: i64, y: i64, w: i64, h: i64) -> Rectangle {
    Rectangle {
        min: (x.max(0) as usize, y.max(0) as usize),
        max: ((x + w).max(0) as usize, (y + h).max(0) as usize),
    }
}

/// Window represents a rectangular area on the screen. It can be a root
/// window with no parent, or a sub-window with a parent window. A window can
/// have its own buffer or share the buffer of its parent window (view).
pub struct Window {
    buffer: Rc<RefCell<Buffer>>,
    method: WidthMethod,
    parent: Option<Rc<Window>>,
    bounds: Rectangle,
}

impl std::fmt::Debug for Window {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Window")
            .field("bounds", &self.bounds)
            .field("method", &self.method)
            .field("has_parent", &self.parent.is_some())
            .finish()
    }
}

impl Window {
    /// HasParent returns whether the window has a parent window. This can be
    /// used to determine if the window is a root window or a sub-window.
    pub fn has_parent(&self) -> bool {
        self.parent.is_some()
    }

    /// Parent returns the parent window of the current window.
    ///
    /// If the window does not have a parent, it returns None.
    pub fn parent(&self) -> Option<&Rc<Window>> {
        self.parent.as_ref()
    }

    /// MoveTo moves the window to the specified x and y coordinates.
    ///
    /// Coordinates are clamped to zero because the ported [Rectangle] is
    /// unsigned.
    pub fn move_to(&mut self, x: i64, y: i64) {
        let size_x = self.bounds.dx();
        let size_y = self.bounds.dy();
        self.bounds.min.0 = x.max(0) as usize;
        self.bounds.min.1 = y.max(0) as usize;
        self.bounds.max.0 = self.bounds.min.0 + size_x;
        self.bounds.max.1 = self.bounds.min.1 + size_y;
    }

    /// MoveBy moves the window by the specified delta x and delta y.
    pub fn move_by(&mut self, dx: i64, dy: i64) {
        self.move_to(self.bounds.min.0 as i64 + dx, self.bounds.min.1 as i64 + dy);
    }

    /// Clone creates an exact copy of the window, including its buffer and
    /// values. The cloned window will have the same parent and method as the
    /// original window.
    pub fn clone_window(&self) -> Window {
        self.clone_area(self.bounds)
            .expect("bounds are always in bounds")
    }

    /// CloneArea creates an exact copy of the window, including its buffer
    /// and values, but only within the specified area. The cloned window will
    /// have the same parent and method as the original window, but its bounds
    /// will be limited to the specified area.
    ///
    /// Returns None if the area is outside the window's bounds.
    pub fn clone_area(&self, area: Rectangle) -> Option<Window> {
        let buffer = clone_buffer(&self.buffer.borrow(), area)?;
        Some(Window {
            buffer: Rc::new(RefCell::new(buffer)),
            method: self.method,
            parent: self.parent.clone(),
            bounds: area,
        })
    }

    /// Resize resizes the window to the specified width and height.
    ///
    /// The buffer is only resized if this window owns its buffer (i.e. it is
    /// a root window or does not share the parent's buffer).
    pub fn resize(&mut self, width: usize, height: usize) {
        let owns_buffer = match &self.parent {
            None => true,
            Some(parent) => !Rc::ptr_eq(&self.buffer, &parent.buffer),
        };
        if owns_buffer {
            self.buffer.borrow_mut().resize(width, height);
        }
        self.bounds.max.0 = self.bounds.min.0 + width;
        self.bounds.max.1 = self.bounds.min.1 + height;
    }

    /// WidthMethod returns the method used to calculate the width of
    /// characters in the window.
    pub fn width_method(&self) -> WidthMethod {
        self.method
    }

    /// SetWidthMethod sets the width method for the window.
    pub fn set_width_method(&mut self, method: WidthMethod) {
        self.method = method;
    }

    /// Bounds returns the bounds of the window as a rectangle.
    pub fn bounds(&self) -> Rectangle {
        self.bounds
    }

    /// Returns a clone of the cell at the given position, or None if out of
    /// bounds. This mirrors the embedded [Buffer]'s cell accessor.
    pub fn cell_at(&self, x: usize, y: usize) -> Option<Cell> {
        self.buffer.borrow().cell_at(x, y).cloned()
    }

    /// Sets the cell at the given position in the window's buffer. This
    /// mirrors the embedded [Buffer]'s cell setter.
    pub fn set_cell(&self, x: usize, y: usize, c: Cell) {
        self.buffer.borrow_mut().set_cell(x, y, Some(&c));
    }

    /// NewWindow creates a new window with its own buffer relative to the
    /// parent window at the specified position and size.
    ///
    /// This will panic if width or height is negative.
    pub fn new_window(self: &Rc<Window>, x: i64, y: i64, width: i64, height: i64) -> Rc<Window> {
        new_window_internal(Some(self), x, y, width, height, Some(self.method), false)
    }

    /// NewView creates a new view into the parent window at the specified
    /// position and size. Unlike [Window::new_window], this view shares the
    /// same buffer as the parent window.
    ///
    /// This will panic if width or height is negative.
    pub fn new_view(self: &Rc<Window>, x: i64, y: i64, width: i64, height: i64) -> Rc<Window> {
        new_window_internal(Some(self), x, y, width, height, Some(self.method), true)
    }
}

impl crate::buffer::Screen for Window {
    /// Bounds returns the bounds of the window.
    fn bounds(&self) -> crate::screen::Rectangle {
        self.bounds
    }

    /// CellAt returns the cell at the given position.
    fn cell_at(&self, _x: usize, _y: usize) -> Option<&Cell> {
        // The window's cells live behind a RefCell; a borrowed cell cannot
        // be returned from this interface (same as TerminalScreen).
        None
    }

    /// SetCell sets the cell at the given position.
    fn set_cell(&mut self, x: usize, y: usize, c: Option<&Cell>) {
        Window::set_cell(self, x, y, c.cloned().unwrap_or_else(empty_cell));
    }

    /// WidthMethod returns the width method used by the window.
    fn width_method(&self) -> WidthMethod {
        self.method
    }
}

impl Window {
    /// Fill fills the entire window with the given cell.
    pub fn fill(&self, c: Option<&Cell>) {
        self.buffer.borrow_mut().fill(c);
    }

    /// Clear clears the window, filling it with empty cells.
    pub fn clear(&self) {
        self.fill(None);
    }

    /// Draw draws the window's buffer onto the given screen at the given
    /// area (upstream `Window.Draw`, which delegates to the embedded
    /// buffer's [crate::Buffer::draw]).
    pub fn draw(&self, scr: &mut dyn crate::buffer::Screen, area: crate::screen::Rectangle) {
        let buf = self.buffer.borrow();
        buf.draw(scr, area);
    }
}

/// NewWindow creates a new root [Window] with the given size and width
/// method. If the method is None, it defaults to [WidthMethod::WcWidth].
///
/// The width method is used to calculate the width of characters in the
/// window, which is important for correctly rendering text, especially when
/// dealing with wide characters, combining characters, emojis, and other
/// Unicode characters that may have varying widths.
pub fn new_window(width: usize, height: usize, method: Option<WidthMethod>) -> Rc<Window> {
    new_window_internal(None, 0, 0, width as i64, height as i64, method, false)
}

/// newWindow creates a new [Window] with the specified parent, position,
/// method, and size.
fn new_window_internal(
    parent: Option<&Rc<Window>>,
    x: i64,
    y: i64,
    width: i64,
    height: i64,
    method: Option<WidthMethod>,
    view: bool,
) -> Rc<Window> {
    if width < 0 || height < 0 {
        panic!("window: negative size");
    }
    let buffer = match (view, parent) {
        (true, Some(parent)) => Rc::clone(&parent.buffer),
        _ => Rc::new(RefCell::new(new_buffer(width as usize, height as usize))),
    };
    Rc::new(Window {
        buffer,
        method: method.unwrap_or_default(),
        parent: parent.cloned(),
        bounds: rect(x, y, width, height),
    })
}

/// Clones the given area of the buffer into a new buffer, mirroring
/// `Buffer.CloneArea` upstream. Returns None if the area is not inside the
/// buffer's bounds.
fn clone_buffer(buf: &Buffer, area: Rectangle) -> Option<Buffer> {
    let bounds = buf.bounds();
    if area.min.0 < bounds.min.0
        || area.min.1 < bounds.min.1
        || area.max.0 > bounds.max.0
        || area.max.1 > bounds.max.1
    {
        return None;
    }

    let mut n = new_buffer(area.dx(), area.dy());
    for y in area.min.1..area.max.1 {
        let mut x = area.min.0;
        while x < area.max.0 {
            let Some(c) = buf.cell_at(x, y) else {
                x += 1;
                continue;
            };
            if c.is_zero() {
                x += 1;
                continue;
            }
            n.set_cell(x - area.min.0, y - area.min.1, Some(c));
            x += c.width.max(1);
        }
    }
    Some(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window_new_and_bounds() {
        let w = new_window(10, 5, None);
        assert!(!w.has_parent());
        assert!(w.parent().is_none());
        let b = w.bounds();
        assert_eq!(b.dx(), 10);
        assert_eq!(b.dy(), 5);
    }

    #[test]
    fn test_window_move() {
        let mut w = new_window(10, 5, None);
        let wm = Rc::get_mut(&mut w).unwrap();
        wm.move_to(3, 4);
        let b = wm.bounds();
        assert_eq!(b.min, (3, 4));
        assert_eq!(b.max, (13, 9));
        wm.move_by(1, 2);
        let b = wm.bounds();
        assert_eq!(b.min, (4, 6));
    }

    #[test]
    fn test_window_view_shares_buffer() {
        let parent = new_window(20, 10, None);
        let view = parent.new_view(0, 0, 5, 5);
        assert!(view.has_parent());
        assert!(Rc::ptr_eq(view.parent().unwrap(), &parent));
        view.set_cell(0, 0, Cell::new("x"));
        assert_eq!(parent.cell_at(0, 0).unwrap().content, "x");
    }

    #[test]
    fn test_window_clone_area() {
        let parent = new_window(10, 10, None);
        parent.set_cell(1, 1, Cell::new("y"));
        let area = rect(1, 1, 3, 3);
        let clone = parent.clone_area(area).unwrap();
        assert_eq!(clone.bounds(), area);
        assert_eq!(clone.cell_at(0, 0).unwrap().content, "y");
        let out = parent.clone_area(rect(8, 8, 4, 4));
        assert!(out.is_none());
    }

    #[test]
    fn test_rect_and_pos() {
        let r = rect(2, 3, 4, 5);
        assert_eq!(r.min, (2, 3));
        assert_eq!(r.max, (6, 8));
        assert!(pos(2, 3).in_rect(r));
        assert!(!pos(6, 3).in_rect(r));
        assert!(!pos(5, 8).in_rect(r));
        assert_eq!(rect(-2, -3, 4, 5).min, (0, 0));
    }
}
