//! Cleanroom Rust port of upstream Go source file: `tabstop.go`
//! Upstream Target Tag / Version: `v0.0.0-20260703014108-f5a850f9c2b7`
//!
//! <public-docs>
//! Horizontal tab stops, used by the renderer for hard-tab cursor movement
//! optimizations.
//! </public-docs>

/// DefaultTabInterval is the default tab interval.
pub const DEFAULT_TAB_INTERVAL: i32 = 8;

/// TabStops represents horizontal line tab stops.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabStops {
    stops: Vec<u8>,
    interval: i32,
    width: i32,
}

/// NewTabStops creates a new set of tab stops from a number of columns and an
/// interval.
pub fn new_tab_stops(width: i32, interval: i32) -> TabStops {
    let mut ts = TabStops {
        stops: vec![0; ((width + (interval - 1)) / interval) as usize],
        interval,
        width,
    };
    ts.init(0, width);
    ts
}

/// DefaultTabStops creates a new set of tab stops with the default interval.
pub fn default_tab_stops(cols: i32) -> TabStops {
    new_tab_stops(cols, DEFAULT_TAB_INTERVAL)
}

impl TabStops {
    /// Resize resizes the tab stops to the given width.
    pub fn resize(&mut self, width: i32) {
        if width == self.width {
            return;
        }

        if width < self.width {
            let size = (width + (self.interval - 1)) / self.interval;
            self.stops.truncate(size as usize);
        } else {
            let size = (width - self.width + (self.interval - 1)) / self.interval;
            self.stops.resize(self.stops.len() + size as usize, 0);
        }

        self.init(self.width, width);
        self.width = width;
    }

    /// Width returns the width of the screen that the tab stops are set for.
    pub fn width(&self) -> i32 {
        self.width
    }

    /// IsStop returns true if the given column is a tab stop.
    pub fn is_stop(&self, col: i32) -> bool {
        let mask = self.mask(col);
        let i = col >> 3;
        if i < 0 || i as usize >= self.stops.len() {
            return false;
        }
        self.stops[i as usize] & mask != 0
    }

    /// Next returns the next tab stop after the given column.
    pub fn next(&self, col: i32) -> i32 {
        self.find(col, 1)
    }

    /// Prev returns the previous tab stop before the given column.
    pub fn prev(&self, col: i32) -> i32 {
        self.find(col, -1)
    }

    /// Find returns the prev/next tab stop before/after the given column and
    /// delta. If delta is positive, it returns the next tab stop after the
    /// given column. If delta is negative, it returns the previous tab stop
    /// before the given column. If delta is zero, it returns the given
    /// column.
    pub fn find(&self, col: i32, delta: i32) -> i32 {
        if delta == 0 {
            return col;
        }

        let mut prev = false;
        let mut count = delta;
        if count < 0 {
            count = -count;
            prev = true;
        }

        let mut col = col;
        while count > 0 {
            if !prev {
                if col >= self.width - 1 {
                    return col;
                }
                col += 1;
            } else {
                if col < 1 {
                    return col;
                }
                col -= 1;
            }

            if self.is_stop(col) {
                count -= 1;
            }
        }

        col
    }

    /// Set adds a tab stop at the given column.
    pub fn set(&mut self, col: i32) {
        let mask = self.mask(col);
        self.stops[(col >> 3) as usize] |= mask;
    }

    /// Reset removes the tab stop at the given column.
    pub fn reset(&mut self, col: i32) {
        let mask = self.mask(col);
        self.stops[(col >> 3) as usize] &= !mask;
    }

    /// Clear removes all tab stops.
    pub fn clear(&mut self) {
        self.stops = vec![0; self.stops.len()];
    }

    /// mask returns the mask for the given column.
    fn mask(&self, col: i32) -> u8 {
        1 << (col & (self.interval - 1))
    }

    /// init initializes the tab stops starting from col until width.
    fn init(&mut self, col: i32, width: i32) {
        for x in col..width {
            if x % self.interval == 0 {
                self.set(x);
            } else {
                self.reset(x);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tab_stops() {
        let ts = default_tab_stops(80);
        assert!(ts.is_stop(0));
        assert!(!ts.is_stop(1));
        assert!(ts.is_stop(8));
        assert!(ts.is_stop(16));
        assert_eq!(ts.next(0), 8);
        assert_eq!(ts.next(7), 8);
        assert_eq!(ts.next(8), 16);
        assert_eq!(ts.prev(8), 0);
        assert_eq!(ts.prev(9), 8);
    }

    #[test]
    fn test_tab_stops_resize() {
        let mut ts = default_tab_stops(80);
        ts.resize(100);
        assert_eq!(ts.width(), 100);
        assert!(ts.is_stop(96));
        ts.resize(40);
        assert_eq!(ts.width(), 40);
        assert!(!ts.is_stop(48));
    }

    #[test]
    fn test_tab_stops_clear() {
        let mut ts = default_tab_stops(80);
        ts.clear();
        assert!(!ts.is_stop(0));
        assert!(!ts.is_stop(8));
    }

    /// Tab stop find boundary conditions.
    #[test]
    fn test_tab_stops_find_edges() {
        let ts = default_tab_stops(80);
        // delta 0 returns the column unchanged.
        assert_eq!(ts.find(5, 0), 5);
        // next at the last column stays put.
        assert_eq!(ts.next(79), 79);
        // prev at column 0 stays put.
        assert_eq!(ts.prev(0), 0);
        // is_stop out of bounds is false.
        assert!(!ts.is_stop(80));
        assert!(!ts.is_stop(-1));
    }
}
