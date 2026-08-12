//! Cleanroom Rust port of upstream Go source file: `screen/screen.go`
//! Upstream Target Tag / Version: `v0.0.0-20260703014108-f5a850f9c2b7`
//!
//! <public-docs>
//! Screen manipulation helpers: clearing and filling areas of a screen.
//! </public-docs>

use crate::buffer::{Buffer, RenderBuffer, Screen, ScreenBuffer};
use crate::cell::Cell;
use crate::new_buffer;
use charming_x_ansi::method::WidthMethod;
use std::any::Any;

/// A rectangle with a minimum and maximum corner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rectangle {
    /// The minimum corner (x, y).
    pub min: (usize, usize),
    /// The maximum corner (x, y).
    pub max: (usize, usize),
}

/// Rect returns a new rectangle with the given origin and size.
pub fn rect(x: usize, y: usize, w: usize, h: usize) -> Rectangle {
    Rectangle {
        min: (x, y),
        max: (x + w, y + h),
    }
}

impl Rectangle {
    /// Dx returns the width of the rectangle.
    pub fn dx(&self) -> usize {
        self.max.0.saturating_sub(self.min.0)
    }
    /// Dy returns the height of the rectangle.
    pub fn dy(&self) -> usize {
        self.max.1.saturating_sub(self.min.1)
    }
    /// Contains returns whether the point is inside the rectangle.
    pub fn contains(&self, x: usize, y: usize) -> bool {
        x >= self.min.0 && x < self.max.0 && y >= self.min.1 && y < self.max.1
    }
    /// Overlaps reports whether the rectangle overlaps the given rectangle.
    pub fn overlaps(&self, o: &Rectangle) -> bool {
        self.min.0 < o.max.0 && o.min.0 < self.max.0 && self.min.1 < o.max.1 && o.min.1 < self.max.1
    }
    /// Union returns the smallest rectangle that contains both rectangles.
    pub fn union(&self, o: &Rectangle) -> Rectangle {
        Rectangle {
            min: (self.min.0.min(o.min.0), self.min.1.min(o.min.1)),
            max: (self.max.0.max(o.max.0), self.max.1.max(o.max.1)),
        }
    }
}

/// Clear clears the screen with empty cells. This is equivalent to filling
/// the screen with empty cells.
///
/// If the screen implements a Clear method, it will be called instead of
/// filling the screen with empty cells.
pub fn clear(scr: &mut dyn Screen) {
    if let Some(any) = scr.as_any_mut() {
        if let Some(b) = any.downcast_mut::<Buffer>() {
            b.clear();
            return;
        }
    }
    if let Some(any) = scr.as_any_mut() {
        if let Some(rb) = any.downcast_mut::<RenderBuffer>() {
            rb.clear();
            return;
        }
    }
    if let Some(any) = scr.as_any_mut() {
        if let Some(sb) = any.downcast_mut::<ScreenBuffer>() {
            sb.render_buffer.clear();
            return;
        }
    }
    fill(scr, None);
}

/// ClearArea clears the given area of the screen with empty cells. This is
/// equivalent to filling the area with empty cells.
///
/// If the screen implements a ClearArea method, it will be called instead of
/// filling the area with empty cells.
pub fn clear_area(scr: &mut dyn Screen, area: Rectangle) {
    if let Some(any) = scr.as_any_mut() {
        if let Some(b) = any.downcast_mut::<Buffer>() {
            b.clear_area(area);
            return;
        }
    }
    if let Some(any) = scr.as_any_mut() {
        if let Some(rb) = any.downcast_mut::<RenderBuffer>() {
            rb.clear_area(area);
            return;
        }
    }
    if let Some(any) = scr.as_any_mut() {
        if let Some(sb) = any.downcast_mut::<ScreenBuffer>() {
            sb.render_buffer.clear_area(area);
            return;
        }
    }
    fill_area(scr, None, area);
}

/// Fill fills the screen with the given cell. If the cell is None, it fills
/// the screen with empty cells.
///
/// If the screen implements a Fill method, it will be called instead of
/// filling the screen with empty cells.
pub fn fill(scr: &mut dyn Screen, cell: Option<&Cell>) {
    if let Some(any) = scr.as_any_mut() {
        if let Some(b) = any.downcast_mut::<Buffer>() {
            b.fill(cell);
            return;
        }
    }
    if let Some(any) = scr.as_any_mut() {
        if let Some(rb) = any.downcast_mut::<RenderBuffer>() {
            rb.fill(cell);
            return;
        }
    }
    if let Some(any) = scr.as_any_mut() {
        if let Some(sb) = any.downcast_mut::<ScreenBuffer>() {
            sb.render_buffer.fill(cell);
            return;
        }
    }
    let area = scr.bounds();
    fill_area(scr, cell, area);
}

/// FillArea fills the given area of the screen with the given cell. If the
/// cell is None, it fills the area with empty cells.
///
/// If the screen implements a FillArea method, it will be called instead of
/// filling the area with empty cells.
pub fn fill_area(scr: &mut dyn Screen, cell: Option<&Cell>, area: Rectangle) {
    if let Some(any) = scr.as_any_mut() {
        if let Some(b) = any.downcast_mut::<Buffer>() {
            b.fill_area(cell, area);
            return;
        }
    }
    if let Some(any) = scr.as_any_mut() {
        if let Some(rb) = any.downcast_mut::<RenderBuffer>() {
            rb.fill_area(cell, area);
            return;
        }
    }
    if let Some(any) = scr.as_any_mut() {
        if let Some(sb) = any.downcast_mut::<ScreenBuffer>() {
            sb.render_buffer.fill_area(cell, area);
            return;
        }
    }
    let cell_width = match cell {
        Some(c) if c.width > 1 => c.width,
        _ => 1,
    };
    for y in area.min.1..area.max.1 {
        let mut x = area.min.0;
        while x < area.max.0 {
            scr.set_cell(x, y, cell);
            x += cell_width;
        }
    }
}

/// CloneArea clones the given area of the screen and returns a new buffer
/// with the same size as the area. The new buffer will contain the same cells
/// as the area in the screen.
///
/// Use [crate::Buffer::draw] to draw the cloned buffer to a screen again.
///
/// If the screen implements a CloneArea method, it will be called instead of
/// cloning the area manually.
pub fn clone_area(scr: &dyn Screen, area: Rectangle) -> Buffer {
    let mut buf = new_buffer(area.dx(), area.dy());
    for y in area.min.1..area.max.1 {
        let mut x = area.min.0;
        while x < area.max.0 {
            let cell = scr.cell_at(x, y);
            match cell {
                None => {
                    x += 1;
                    continue;
                }
                Some(c) if c.is_zero() => {
                    x += 1;
                    continue;
                }
                Some(c) => {
                    buf.set_cell(x - area.min.0, y - area.min.1, Some(c));
                    x += std::cmp::max(c.width, 1);
                }
            }
        }
    }
    buf
}

/// Blanket forwarding impl so a mutable reference to any screen can be used
/// where a `dyn Screen` is expected (mirrors Go's pointer-based interfaces).
impl<T: Screen + ?Sized> Screen for &mut T {
    fn bounds(&self) -> Rectangle {
        (**self).bounds()
    }
    fn cell_at(&self, x: usize, y: usize) -> Option<&Cell> {
        (**self).cell_at(x, y)
    }
    fn set_cell(&mut self, x: usize, y: usize, c: Option<&Cell>) {
        (**self).set_cell(x, y, c);
    }
    fn width_method(&self) -> WidthMethod {
        (**self).width_method()
    }
    fn as_any_mut<'b>(&'b mut self) -> Option<&'b mut dyn Any> {
        (**self).as_any_mut()
    }
}
