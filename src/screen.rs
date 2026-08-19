//! Cleanroom Rust port of upstream Go source file: `screen/screen.go`
//! Upstream Target Tag / Version: `v0.0.0-20260703014108-f5a850f9c2b7`
//!
//! <public-docs>
//! Screen manipulation helpers: clearing and filling areas of a screen.
//! </public-docs>

use crate::buffer::{Buffer, RenderBuffer, Screen, ScreenBuffer};
use crate::cell::Cell;
use crate::new_buffer;
use rusty_x_ansi::method::WidthMethod;
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
    fn as_any_mut(&mut self) -> Option<&mut dyn Any> {
        (**self).as_any_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::buffer::{new_render_buffer, new_screen_buffer};
    use crate::cell::Cell;

    #[test]
    fn test_rectangle_methods() {
        let r = rect(2, 3, 5, 4);
        assert_eq!(r.dx(), 5);
        assert_eq!(r.dy(), 4);
        assert!(r.contains(2, 3));
        assert!(r.contains(6, 6));
        assert!(!r.contains(1, 3));
        assert!(!r.contains(2, 7));
        let o = rect(6, 3, 2, 1);
        assert!(r.overlaps(&o));
        assert!(!r.overlaps(&rect(20, 20, 1, 1)));
        let u = r.union(&rect(0, 0, 1, 1));
        assert_eq!(u.min, (0, 0));
        assert_eq!(u.max, (7, 7));
    }

    #[test]
    fn test_screen_clear_fill_clone() {
        // Buffer downcast paths.
        let mut b = new_buffer(3, 2);
        b.set_cell(0, 0, Some(&Cell::new("X")));
        clear(&mut b);
        assert_eq!(b.cell_at(0, 0).unwrap().content, " ");

        fill(&mut b, Some(&Cell::new("Z")));
        assert_eq!(b.cell_at(2, 1).unwrap().content, "Z");

        fill_area(&mut b, Some(&Cell::new("Q")), rect(1, 0, 1, 1));
        assert_eq!(b.cell_at(1, 0).unwrap().content, "Q");
        assert_eq!(b.cell_at(2, 1).unwrap().content, "Z");

        clear_area(&mut b, rect(1, 0, 1, 1));
        assert_eq!(b.cell_at(1, 0).unwrap().content, " ");
        assert_eq!(b.cell_at(2, 1).unwrap().content, "Z");

        // CloneArea via the free function.
        b.set_cell(1, 1, Some(&Cell::new("Y")));
        let clone = clone_area(&b, rect(1, 1, 1, 1));
        assert_eq!(clone.cell_at(0, 0).unwrap().content, "Y");
    }

    #[test]
    fn test_screen_render_buffer_paths() {
        // RenderBuffer downcast paths.
        let mut rb = new_render_buffer(3, 2);
        rb.set_cell(0, 0, Some(&Cell::new("X")));
        clear(&mut rb);
        assert_eq!(rb.cell_at(0, 0).unwrap().content, " ");

        fill(&mut rb, Some(&Cell::new("Z")));
        assert_eq!(rb.cell_at(1, 1).unwrap().content, "Z");

        fill_area(&mut rb, Some(&Cell::new("Q")), rect(0, 0, 1, 1));
        assert_eq!(rb.cell_at(0, 0).unwrap().content, "Q");

        clear_area(&mut rb, rect(0, 0, 1, 1));
        assert_eq!(rb.cell_at(0, 0).unwrap().content, " ");
    }

    #[test]
    fn test_screen_screen_buffer_paths() {
        // ScreenBuffer downcast paths (through its render_buffer).
        let mut sb = new_screen_buffer(3, 2);
        sb.set_cell(0, 0, Some(&Cell::new("X")));
        clear(&mut sb);
        assert_eq!(sb.cell_at(0, 0).unwrap().content, " ");

        fill(&mut sb, Some(&Cell::new("Z")));
        assert_eq!(sb.cell_at(2, 1).unwrap().content, "Z");

        clear_area(&mut sb, rect(0, 0, 1, 1));
        assert_eq!(sb.cell_at(0, 0).unwrap().content, " ");
    }

    /// The fill/clear fallback paths for a screen that does not downcast.
    #[test]
    fn test_screen_fallback_paths() {
        // A wrapper screen that delegates but returns None from as_any_mut.
        struct Wrapper(Buffer);
        impl Screen for Wrapper {
            fn bounds(&self) -> Rectangle {
                self.0.bounds()
            }
            fn cell_at(&self, x: usize, y: usize) -> Option<&Cell> {
                self.0.cell_at(x, y)
            }
            fn set_cell(&mut self, x: usize, y: usize, c: Option<&Cell>) {
                self.0.set_cell(x, y, c)
            }
            fn width_method(&self) -> WidthMethod {
                WidthMethod::WcWidth
            }
            fn as_any_mut(&mut self) -> Option<&mut dyn Any> {
                None
            }
        }
        let mut w = Wrapper(new_buffer(3, 2));
        // fill falls through to fill_area.
        fill(&mut w, Some(&Cell::new("Z")));
        assert_eq!(w.0.cell_at(2, 1).unwrap().content, "Z");
        // clear falls through to fill(None).
        clear(&mut w);
        assert_eq!(w.0.cell_at(0, 0).unwrap().content, " ");
        // fill_area with a wide cell steps by its width.
        let mut w2 = Wrapper(new_buffer(6, 1));
        let wide = Cell {
            content: "界".to_string(),
            width: 2,
            ..Cell::default()
        };
        fill_area(&mut w2, Some(&wide), rect(0, 0, 6, 1));
        assert_eq!(w2.0.cell_at(0, 0).unwrap().content, "界");
        assert_eq!(w2.0.cell_at(2, 0).unwrap().content, "界");
        // clear_area falls through to fill_area(None).
        let mut w3 = Wrapper(new_buffer(3, 1));
        w3.0.set_cell(1, 0, Some(&Cell::new("X")));
        clear_area(&mut w3, rect(0, 0, 3, 1));
        assert_eq!(w3.0.cell_at(1, 0).unwrap().content, " ");

        // &mut dyn Screen forwarding impl
        let mut b4 = new_buffer(3, 3);
        let scr_ref: &mut dyn Screen = &mut b4;
        let dyn_ref = &mut *scr_ref;
        assert_eq!(dyn_ref.bounds(), rect(0, 0, 3, 3));
        assert_eq!(dyn_ref.width_method(), WidthMethod::WcWidth);
        assert!(dyn_ref.as_any_mut().is_some());
        dyn_ref.set_cell(1, 1, Some(&Cell::new("Q")));
        assert_eq!(dyn_ref.cell_at(1, 1).unwrap().content, "Q");

        // clone_area on a screen with wide cell and zero/empty cells
        let mut b5 = new_buffer(4, 2);
        b5.set_cell(0, 0, Some(&wide));
        b5.set_cell(2, 0, Some(&Cell::default())); // zero cell
        let cloned = clone_area(&b5, rect(0, 0, 4, 2));
        assert_eq!(cloned.cell_at(0, 0).unwrap().content, "界");
    }
}
