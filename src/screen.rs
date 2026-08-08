//! Cleanroom Rust port of upstream Go source file: `screen/screen.go`
//! Upstream Target Tag / Version: `v0.0.0-20251205161215-1948445e3318`
//!
//! <public-docs>
//! Screen manipulation helpers: clearing and filling areas of a screen.
//! </public-docs>

use crate::buffer::Screen;
use crate::cell::Cell;

/// A rectangle with a minimum and maximum corner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rectangle {
    /// The minimum corner (x, y).
    pub min: (usize, usize),
    /// The maximum corner (x, y).
    pub max: (usize, usize),
}

impl Rectangle {
    /// Returns the width of the rectangle.
    pub fn dx(&self) -> usize {
        self.max.0.saturating_sub(self.min.0)
    }
    /// Returns the height of the rectangle.
    pub fn dy(&self) -> usize {
        self.max.1.saturating_sub(self.min.1)
    }
    /// Returns whether the point is inside the rectangle.
    pub fn contains(&self, x: usize, y: usize) -> bool {
        x >= self.min.0 && x < self.max.0 && y >= self.min.1 && y < self.max.1
    }
}

/// Clears the screen with empty cells. If the screen has a `clear` method it
/// is called instead of filling the screen with empty cells.
pub fn clear(scr: &mut dyn Screen) {
    if let Some(c) = scr
        .as_any_mut()
        .downcast_mut::<crate::buffer::Buffer>()
    {
        c.clear();
        return;
    }
    let area = scr.bounds();
    fill_area(scr, None, area);
}

/// Clears the given area of the screen with empty cells.
pub fn clear_area(scr: &mut dyn Screen, area: Rectangle) {
    fill_area(scr, None, area);
}

/// Fills the screen with the given cell. If the cell is None, it fills the
/// screen with empty cells.
pub fn fill(scr: &mut dyn Screen, cell: Option<Cell>) {
    let area = scr.bounds();
    fill_area(scr, cell, area);
}

/// Fills the given area of the screen with the given cell. If the cell is
/// None, it fills the area with empty cells.
pub fn fill_area(scr: &mut dyn Screen, cell: Option<Cell>, area: Rectangle) {
    for y in area.min.1..area.max.1 {
        for x in area.min.0..area.max.0 {
            if let Some(c) = scr.cell_at_mut(x, y) {
                match &cell {
                    Some(cell) => *c = cell.clone(),
                    None => c.empty(),
                }
            }
        }
    }
}
