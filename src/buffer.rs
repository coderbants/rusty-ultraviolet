//! Cleanroom Rust port of upstream Go source file: `buffer.go`
//! Upstream Target Tag / Version: `v0.0.0-20260703014108-f5a850f9c2b7`
//!
//! <public-docs>
//! The cell buffer: a grid of cells with render support, the `Screen`
//! interface, the `RenderBuffer` change tracker, and the `ScreenBuffer`
//! width-aware screen.
//! </public-docs>

use crate::cell::{empty_cell, Cell};
use crate::screen::Rectangle;
use crate::style::Style;

use rusty_x_ansi::hyperlink::{reset_hyperlink, set_hyperlink};
use rusty_x_ansi::method::WidthMethod;
use std::any::Any;

/// A row of cells.
#[derive(Debug, Clone, Default)]
pub struct Line(pub Vec<Cell>);

impl std::ops::Deref for Line {
    type Target = [Cell];
    fn deref(&self) -> &[Cell] {
        &self.0
    }
}

impl std::ops::DerefMut for Line {
    fn deref_mut(&mut self) -> &mut [Cell] {
        &mut self.0
    }
}

/// Lines represents a slice of lines.
#[derive(Debug, Clone, Default)]
pub struct Lines(pub Vec<Line>);

impl Lines {
    /// Height returns the height of the lines.
    pub fn height(&self) -> usize {
        self.0.len()
    }

    /// Width returns the width of the widest line.
    pub fn width(&self) -> usize {
        let mut max_width = 0;
        for l in &self.0 {
            max_width = max(max_width, l.len());
        }
        max_width
    }

    /// String returns the string representation of the lines.
    pub fn string(&self) -> String {
        let mut buf = String::new();
        for (i, l) in self.0.iter().enumerate() {
            buf.push_str(&l.string());
            if i < self.0.len() - 1 {
                buf.push('\n');
            }
        }
        buf
    }

    /// Render renders the lines to a styled string with all the required
    /// attributes and styles.
    pub fn render(&self) -> String {
        let mut buf = String::new();
        for (i, l) in self.0.iter().enumerate() {
            render_line(&mut buf, l);
            if i < self.0.len() - 1 {
                buf.push('\n');
            }
        }
        buf
    }
}

impl Line {
    /// Set sets the cell at the given x position.
    pub fn set(&mut self, x: usize, c: Cell) {
        let line_width = self.len();
        if x >= line_width {
            return;
        }

        // When a wide cell is partially overwritten, we need to fill the rest
        // of the cell with space cells to avoid rendering issues.
        let prev = self.get(x).cloned();
        if let Some(prev) = prev {
            let pw = prev.width;
            if pw > 1 {
                // Writing to the first wide cell
                for j in 0..pw {
                    let idx = x + j;
                    if idx >= line_width {
                        break;
                    }
                    if let Some(cell) = self.get_mut(idx) {
                        *cell = prev.clone();
                        cell.empty();
                    }
                }
            } else if pw == 0 {
                // Writing to wide cell placeholders
                for j in 1.. {
                    if x < j {
                        break;
                    }
                    if let Some(wide) = self.get(x - j).cloned() {
                        let ww = wide.width;
                        if ww > 1 && j < ww {
                            for k in 0..ww {
                                if let Some(cell) = self.get_mut(x - j + k) {
                                    *cell = wide.clone();
                                    cell.empty();
                                }
                            }
                            break;
                        }
                    }
                }
            }
        }

        let cw = c.width;
        self[x] = c.clone();
        if x + cw > line_width {
            // If the cell is too wide, we write blanks with the same style.
            for i in 0..cw {
                let idx = x + i;
                if idx >= line_width {
                    break;
                }
                if let Some(cell) = self.get_mut(idx) {
                    *cell = c.clone();
                    cell.empty();
                }
            }
            return;
        }

        if cw > 1 {
            // Mark wide cells with zero-width placeholder cells.
            for j in 1..cw {
                let idx = x + j;
                if idx >= line_width {
                    break;
                }
                self[idx] = Cell::default();
            }
        }
    }

    /// String returns the string representation of the line.
    pub fn string(&self) -> String {
        let mut buf = String::new();
        let mut pending = String::new();
        for c in &self.0 {
            if c.is_zero() {
                continue;
            }
            if c.equal(&empty_cell()) {
                pending.push(' ');
                continue;
            }
            if !pending.is_empty() {
                buf.push_str(&pending);
                pending.clear();
            }
            buf.push_str(&c.content);
        }
        buf
    }
}

/// Screen is the interface that wraps the basic methods of a screen buffer.
pub trait Screen {
    /// Bounds returns the bounds of the screen. This is the rectangle that
    /// includes the start and end points of the screen.
    fn bounds(&self) -> Rectangle;

    /// CellAt returns the cell at the given position. If the position is out
    /// of bounds, it returns None. Otherwise, it always returns a cell, even
    /// if it is empty (i.e., a cell with a space character and a width of 1).
    fn cell_at(&self, x: usize, y: usize) -> Option<&Cell>;

    /// SetCell sets the cell at the given position. A None cell is treated as
    /// an empty cell with a space character and a width of 1.
    fn set_cell(&mut self, x: usize, y: usize, c: Option<&Cell>);

    /// WidthMethod returns the width method used by the screen.
    fn width_method(&self) -> WidthMethod;

    /// Provides access to the underlying concrete type for downcasting.
    fn as_any_mut(&mut self) -> Option<&mut dyn Any> {
        None
    }
}

/// LineData represents the metadata for a line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LineData {
    /// First and last changed cell indices.
    pub first_cell: usize,
    /// First and last changed cell indices.
    pub last_cell: usize,
}

/// Buffer represents a cell buffer that contains the contents of a screen.
#[derive(Debug, Clone, Default)]
pub struct Buffer {
    /// Lines is a slice of lines that make up the cells of the buffer.
    pub lines: Vec<Line>,
}

/// NewBuffer creates a new buffer with the given width and height.
/// This is a convenience function that initializes a new buffer and resizes
/// it.
pub fn new_buffer(width: usize, height: usize) -> Buffer {
    let mut b = Buffer { lines: Vec::new() };
    b.lines = vec![Line(vec![empty_cell(); width]); height];
    b.resize(width, height);
    b
}

impl Buffer {
    /// Resizes the buffer, keeping the current contents where they fit.
    pub fn resize(&mut self, width: usize, height: usize) {
        if self.lines.is_empty() {
            self.lines = vec![Line(vec![empty_cell(); width]); height];
            return;
        }
        let mut lines: Vec<Line> = self
            .lines
            .iter()
            .take(height)
            .map(|l| {
                let mut l = l.clone();
                l.0.resize(width, empty_cell());
                l
            })
            .collect();
        lines.resize(height, Line(vec![empty_cell(); width]));
        self.lines = lines;
    }

    /// Width returns the width of the buffer.
    pub fn width(&self) -> usize {
        match self.lines.first() {
            Some(l) => l.len(),
            None => 0,
        }
    }

    /// Height returns the height of the buffer.
    pub fn height(&self) -> usize {
        self.lines.len()
    }

    /// Bounds returns the bounds of the buffer.
    ///
    /// The origin is always at (0, 0) and the maximum coordinates are
    /// determined by the width and height of the buffer.
    pub fn bounds(&self) -> Rectangle {
        Rectangle {
            min: (0, 0),
            max: (self.width(), self.height()),
        }
    }

    /// Line returns a pointer to the line at the given y position.
    pub fn line(&self, y: usize) -> Option<&Line> {
        self.lines.get(y)
    }

    /// CellAt returns the cell at the given position. If the position is out
    /// of bounds, it returns None.
    pub fn cell_at(&self, x: usize, y: usize) -> Option<&Cell> {
        self.lines.get(y)?.get(x)
    }

    /// CellAtMut returns a mutable reference to the cell at the given
    /// position.
    pub fn cell_at_mut(&mut self, x: usize, y: usize) -> Option<&mut Cell> {
        self.lines.get_mut(y)?.get_mut(x)
    }

    /// SetCell sets the cell at the given position. A None cell is treated as
    /// an empty cell with a space character and a width of 1.
    ///
    /// This goes through [Line::set] so the wide-cell placeholder handling
    /// (clearing partially overwritten wide cells) is applied, mirroring
    /// upstream `Buffer.SetCell` calling `Line.Set`.
    pub fn set_cell(&mut self, x: usize, y: usize, c: Option<&Cell>) {
        if x >= self.width() || y >= self.height() {
            return;
        }
        if let Some(line) = self.lines.get_mut(y) {
            line.set(
                x,
                match c {
                    Some(c) => c.clone(),
                    None => empty_cell(),
                },
            );
        }
    }

    /// Fill fills the buffer with the given cell. If the cell is None, it
    /// fills the buffer with empty cells.
    pub fn fill(&mut self, c: Option<&Cell>) {
        self.fill_area(c, self.bounds());
    }

    /// FillArea fills the given area of the buffer with the given cell.
    pub fn fill_area(&mut self, c: Option<&Cell>, area: Rectangle) {
        let cell_width = match c {
            Some(c) if c.width > 1 => c.width,
            _ => 1,
        };
        for y in area.min.1..area.max.1 {
            let mut x = area.min.0;
            while x < area.max.0 {
                self.set_cell(x, y, c);
                x += cell_width;
            }
        }
    }

    /// Clear clears the buffer with space cells.
    pub fn clear(&mut self) {
        let area = self.bounds();
        for y in area.min.1..area.max.1 {
            for x in area.min.0..area.max.0 {
                if let Some(cell) = self.cell_at_mut(x, y) {
                    *cell = empty_cell();
                }
            }
        }
    }

    /// ClearArea clears the buffer with space cells within the specified
    /// rectangle.
    pub fn clear_area(&mut self, area: Rectangle) {
        self.fill_area(None, area);
    }

    /// CloneArea clones the given area of the buffer and returns a new buffer
    /// with the same size as the area. The new buffer will contain the same
    /// cells as the area in the buffer.
    pub fn clone_area(&self, area: Rectangle) -> Option<Buffer> {
        if area.min.0 > area.max.0 || area.min.1 > area.max.1 {
            return None;
        }
        if area.min.0 >= self.width() || area.min.1 >= self.height() {
            return Some(new_buffer(area.dx(), area.dy()));
        }
        let area = Rectangle {
            min: area.min,
            max: (area.max.0.min(self.width()), area.max.1.min(self.height())),
        };
        let mut n = new_buffer(area.dx(), area.dy());
        for y in area.min.1..area.max.1 {
            let mut x = area.min.0;
            while x < area.max.0 {
                let c = self.cell_at(x, y);
                match c {
                    None => {
                        x += 1;
                        continue;
                    }
                    Some(c) if c.is_zero() => {
                        x += 1;
                        continue;
                    }
                    Some(c) => {
                        n.set_cell(x - area.min.0, y - area.min.1, Some(c));
                        x += max(c.width, 1);
                    }
                }
            }
        }
        Some(n)
    }

    /// String returns the string representation of the buffer.
    pub fn string(&self) -> String {
        Lines(self.lines.clone()).string()
    }

    /// Render renders the buffer to a styled string with all the required
    /// attributes and styles.
    pub fn render(&self) -> String {
        Lines(self.lines.clone()).render()
    }

    /// Draw draws the buffer onto the given screen within the specified area.
    pub fn draw(&self, scr: &mut dyn Screen, area: Rectangle) {
        // No need to draw if the buffer is empty.
        if self.lines.is_empty() {
            return;
        }

        // Ensure the area is within the bounds of the screen.
        let bounds = scr.bounds();
        if !area.overlaps(&bounds) {
            return;
        }

        for y in area.min.1..area.max.1 {
            let mut x = area.min.0;
            while x < area.max.0 {
                let c = self.cell_at(x - area.min.0, y - area.min.1);
                match c {
                    None => {
                        x += 1;
                        continue;
                    }
                    Some(c) if c.is_zero() => {
                        x += 1;
                        continue;
                    }
                    Some(c) => {
                        scr.set_cell(x, y, Some(c));
                        let mut width = c.width;
                        if width == 0 {
                            width = 1;
                        }
                        x += width;
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
    fn cell_at(&self, x: usize, y: usize) -> Option<&Cell> {
        self.cell_at(x, y)
    }
    fn set_cell(&mut self, x: usize, y: usize, c: Option<&Cell>) {
        self.set_cell(x, y, c)
    }
    fn width_method(&self) -> WidthMethod {
        WidthMethod::WcWidth
    }
    fn as_any_mut(&mut self) -> Option<&mut dyn Any> {
        Some(self)
    }
}

/// RenderBuffer represents a buffer that keeps track of the current and new
/// state of the screen, allowing for efficient rendering by only updating the
/// parts of the screen that have changed.
#[derive(Debug, Clone, Default)]
pub struct RenderBuffer {
    /// Buffer is the underlying buffer.
    pub buffer: Buffer,
    /// Touched represents the lines that have been modified or touched.
    pub touched: Vec<Option<LineData>>,
}

/// NewRenderBuffer creates a new [RenderBuffer] with the given width and
/// height.
pub fn new_render_buffer(width: usize, height: usize) -> RenderBuffer {
    RenderBuffer {
        buffer: new_buffer(width, height),
        touched: vec![None; height],
    }
}

impl RenderBuffer {
    /// Width returns the width of the buffer.
    pub fn width(&self) -> usize {
        self.buffer.width()
    }

    /// Height returns the height of the buffer.
    pub fn height(&self) -> usize {
        self.buffer.height()
    }

    /// Bounds returns the bounds of the buffer.
    pub fn bounds(&self) -> Rectangle {
        self.buffer.bounds()
    }

    /// Line returns a pointer to the line at the given y position.
    pub fn line(&self, y: usize) -> Option<&Line> {
        self.buffer.line(y)
    }

    /// SetLine replaces the line at the given y position.
    pub fn set_line(&mut self, y: usize, line: Line) {
        if let Some(l) = self.buffer.lines.get_mut(y) {
            *l = line;
        }
    }

    /// CopyLine copies the line at src to the line at dst.
    pub fn copy_line(&mut self, dst: usize, src: usize) {
        let src_line = self.buffer.lines.get(src).cloned();
        if let Some(l) = self.buffer.lines.get_mut(dst) {
            if let Some(s) = src_line {
                *l = s;
            }
        }
    }

    /// CellAt returns the cell at the given position.
    pub fn cell_at(&self, x: usize, y: usize) -> Option<&Cell> {
        self.buffer.cell_at(x, y)
    }

    /// TouchLine marks a line n times starting at the given x position as
    /// touched.
    pub fn touch_line(&mut self, x: usize, y: usize, n: usize) {
        if y >= self.buffer.lines.len() {
            return;
        }

        if y >= self.touched.len() {
            self.touched
                .extend(std::iter::repeat_with(|| None).take(y - self.touched.len() + 1));
        }

        // Re-check bounds: a concurrent resize may have cleared Touched
        if y >= self.touched.len() {
            return;
        }

        match &mut self.touched[y] {
            None => {
                self.touched[y] = Some(LineData {
                    first_cell: x,
                    last_cell: x + n,
                });
            }
            Some(ch) => {
                ch.first_cell = min(ch.first_cell, x);
                ch.last_cell = max(ch.last_cell, x + n);
            }
        }
    }

    /// Touch marks the cell at the given x, y position as touched.
    pub fn touch(&mut self, x: usize, y: usize) {
        self.touch_line(x, y, 0);
    }

    /// TouchedLines returns the number of touched lines in the buffer.
    pub fn touched_lines(&self) -> usize {
        if self.touched.is_empty() {
            return 0;
        }
        self.touched.iter().filter(|t| t.is_some()).count()
    }

    /// SetCell sets the cell at the given x, y position and marks the line as
    /// touched.
    pub fn set_cell(&mut self, x: usize, y: usize, c: Option<&Cell>) {
        if let Some(p) = self.cell_at(x, y) {
            if !cell_equal(Some(p), c) {
                let mut width = 1;
                if let Some(c) = c {
                    if c.width > 0 {
                        width = c.width;
                    }
                }
                if p.width > 0 {
                    width = max(width, p.width);
                }
                self.touch_line(x, y, width);
            }
        }
        self.buffer.set_cell(x, y, c);
    }

    /// InsertLine inserts n lines at the given line position, with the given
    /// optional cell. This follows terminal [rusty_x_ansi] IL behavior.
    pub fn insert_line(&mut self, y: usize, n: usize, c: Option<&Cell>) {
        self.insert_line_area(y, n, c, self.bounds());
    }

    /// InsertLineArea inserts new lines at the given line position, with the
    /// given optional cell, within the rectangle bounds. This follows
    /// terminal [rusty_x_ansi] IL behavior.
    pub fn insert_line_area(&mut self, y: usize, n: usize, c: Option<&Cell>, area: Rectangle) {
        insert_line_area(&mut self.buffer, y, n, c, area);
        for i in area.min.1..area.max.1 {
            self.touch_line(area.min.0, i, area.max.0 - area.min.0);
            if i >= n {
                self.touch_line(area.min.0, i - n, area.max.0 - area.min.0);
            }
        }
    }

    /// DeleteLine deletes n lines at the given line position, with the given
    /// optional cell. This follows terminal [rusty_x_ansi] DL behavior.
    pub fn delete_line(&mut self, y: usize, n: usize, c: Option<&Cell>) {
        self.delete_line_area(y, n, c, self.bounds());
    }

    /// DeleteLineArea deletes lines at the given line position, with the
    /// given optional cell, within the rectangle bounds.
    pub fn delete_line_area(&mut self, y: usize, n: usize, c: Option<&Cell>, area: Rectangle) {
        delete_line_area(&mut self.buffer, y, n, c, area);
        for i in area.min.1..area.max.1 {
            self.touch_line(area.min.0, i, area.max.0 - area.min.0);
            let next = i + n;
            if next < self.buffer.lines.len() {
                self.touch_line(area.min.0, next, area.max.0 - area.min.0);
            }
        }
    }

    /// InsertCell inserts new cells at the given position, with the given
    /// optional cell. This follows terminal [rusty_x_ansi] ICH behavior.
    pub fn insert_cell(&mut self, x: usize, y: usize, n: usize, c: Option<&Cell>) {
        self.insert_cell_area(x, y, n, c, self.bounds());
    }

    /// InsertCellArea inserts new cells at the given position, with the given
    /// optional cell, within the rectangle bounds.
    pub fn insert_cell_area(
        &mut self,
        x: usize,
        y: usize,
        n: usize,
        c: Option<&Cell>,
        area: Rectangle,
    ) {
        insert_cell_area(&mut self.buffer, x, y, n, c, area);
        let mut n = n;
        if x + n > area.max.0 {
            n = area.max.0.saturating_sub(x);
        }
        self.touch_line(x, y, n);
    }

    /// DeleteCell deletes cells at the given position, with the given
    /// optional cell. This follows terminal [rusty_x_ansi] DCH behavior.
    pub fn delete_cell(&mut self, x: usize, y: usize, n: usize, c: Option<&Cell>) {
        self.delete_cell_area(x, y, n, c, self.bounds());
    }

    /// DeleteCellArea deletes cells at the given position, with the given
    /// optional cell, within the rectangle bounds.
    pub fn delete_cell_area(
        &mut self,
        x: usize,
        y: usize,
        n: usize,
        c: Option<&Cell>,
        area: Rectangle,
    ) {
        delete_cell_area(&mut self.buffer, x, y, n, c, area);
        let mut n = n;
        let remaining_cells = area.max.0.saturating_sub(x);
        if n > remaining_cells {
            n = remaining_cells;
        }
        self.touch_line(x, y, n);
    }

    /// Clear clears the buffer with space cells and marks all lines as
    /// touched.
    pub fn clear(&mut self) {
        self.buffer.clear();
        let w = self.width();
        for y in 0..self.buffer.lines.len() {
            self.touch_line(0, y, w);
        }
    }

    /// ClearArea clears the buffer with space cells within the specified
    /// rectangle and marks the affected lines as touched.
    pub fn clear_area(&mut self, area: Rectangle) {
        self.buffer.clear_area(area);
        let w = area.max.0 - area.min.0;
        for y in area.min.1..area.max.1 {
            self.touch_line(area.min.0, y, w);
        }
    }

    /// Fill fills the buffer with the given cell and marks all lines as
    /// touched.
    pub fn fill(&mut self, c: Option<&Cell>) {
        self.fill_area(c, self.bounds());
    }

    /// FillArea fills the buffer with the given cell within the specified
    /// rectangle and marks the affected lines as touched.
    pub fn fill_area(&mut self, c: Option<&Cell>, area: Rectangle) {
        self.buffer.fill_area(c, area);
        let w = area.max.0 - area.min.0;
        for y in area.min.1..area.max.1 {
            self.touch_line(area.min.0, y, w);
        }
    }
}

impl Screen for RenderBuffer {
    fn bounds(&self) -> Rectangle {
        self.bounds()
    }
    fn cell_at(&self, x: usize, y: usize) -> Option<&Cell> {
        self.cell_at(x, y)
    }
    fn set_cell(&mut self, x: usize, y: usize, c: Option<&Cell>) {
        self.set_cell(x, y, c)
    }
    fn width_method(&self) -> WidthMethod {
        WidthMethod::WcWidth
    }
    fn as_any_mut(&mut self) -> Option<&mut dyn Any> {
        Some(self)
    }
}

/// ScreenBuffer is a buffer that can be used as a [Screen].
#[derive(Debug, Clone)]
pub struct ScreenBuffer {
    /// The render buffer backing this screen.
    pub render_buffer: RenderBuffer,
    /// The width method used by the screen.
    pub method: WidthMethod,
}

/// NewScreenBuffer creates a new ScreenBuffer with the given width and
/// height.
pub fn new_screen_buffer(width: usize, height: usize) -> ScreenBuffer {
    ScreenBuffer {
        render_buffer: new_render_buffer(width, height),
        method: WidthMethod::WcWidth,
    }
}

impl ScreenBuffer {
    /// Bounds returns the bounds of the buffer.
    pub fn bounds(&self) -> Rectangle {
        self.render_buffer.bounds()
    }

    /// Width returns the width of the buffer.
    pub fn width(&self) -> usize {
        self.render_buffer.width()
    }

    /// Height returns the height of the buffer.
    pub fn height(&self) -> usize {
        self.render_buffer.height()
    }

    /// CellAt returns the cell at the given position.
    pub fn cell_at(&self, x: usize, y: usize) -> Option<&Cell> {
        self.render_buffer.cell_at(x, y)
    }

    /// SetCell sets the cell at the given position and marks the line as
    /// touched.
    pub fn set_cell(&mut self, x: usize, y: usize, c: Option<&Cell>) {
        self.render_buffer.set_cell(x, y, c);
    }

    /// WidthMethod returns the width method used by the screen.
    pub fn width_method(&self) -> WidthMethod {
        self.method
    }

    /// Clear clears the buffer with space cells and marks all lines as
    /// touched.
    pub fn clear(&mut self) {
        self.render_buffer.clear();
    }

    /// Fill fills the buffer with the given cell and marks all lines as
    /// touched.
    pub fn fill(&mut self, c: Option<&Cell>) {
        self.render_buffer.fill(c);
    }

    /// Draw draws the buffer onto the given screen within the specified area.
    pub fn draw(&self, scr: &mut dyn Screen, area: Rectangle) {
        self.render_buffer.buffer.draw(scr, area);
    }
}

impl Screen for ScreenBuffer {
    fn bounds(&self) -> Rectangle {
        self.bounds()
    }
    fn cell_at(&self, x: usize, y: usize) -> Option<&Cell> {
        self.cell_at(x, y)
    }
    fn set_cell(&mut self, x: usize, y: usize, c: Option<&Cell>) {
        self.set_cell(x, y, c)
    }
    fn width_method(&self) -> WidthMethod {
        self.width_method()
    }
    fn as_any_mut(&mut self) -> Option<&mut dyn Any> {
        Some(self)
    }
}

/// TrimSpace trims trailing spaces from the end of each line in the given
/// string.
pub fn trim_space(s: &str) -> String {
    let mut lines: Vec<String> = s.split('\n').map(|l| l.to_string()).collect();
    for line in &mut lines {
        // Check if we have a trailing '\r' and preserve it
        let has_cr = line.ends_with('\r');
        if has_cr {
            line.pop();
        }
        *line = line.trim_end_matches(' ').to_string();
        if has_cr {
            line.push('\r');
        }
    }
    lines.join("\n")
}

/// cell_equal reports whether the two cells are equal. A None cell is equal
/// to a zero cell.
pub(crate) fn cell_equal(a: Option<&Cell>, b: Option<&Cell>) -> bool {
    // Upstream `cellEqual`: nil is nil, a nil cell is never equal to a real
    // cell (even an empty one) — the empty-line handling in the scroll
    // optimization depends on this distinction.
    match (a, b) {
        (None, None) => true,
        (None, Some(_)) | (Some(_), None) => false,
        (Some(a), Some(b)) => a.equal(b),
    }
}

/// Renders a line to the buffer, mirroring `renderLine` upstream: empty cells
/// are collapsed into pending spaces that are only emitted before non-empty
/// cells (so trailing blank cells are trimmed).
fn render_line(buf: &mut String, l: &Line) {
    let mut pen = Style::default();
    let mut link: Option<crate::cell::Link> = None;
    let mut pending = String::new();

    for c in &l.0 {
        if c.is_zero() {
            continue;
        }
        if c.equal(&empty_cell()) {
            // Upstream emits the reset here, before the pending space.
            if !pen.is_zero() {
                buf.push_str(rusty_x_ansi::style::RESET_STYLE);
                pen = Style::default();
            }
            if let Some(l) = &link {
                if !l.is_zero() {
                    buf.push_str(reset_hyperlink());
                }
                link = None;
            }
            pending.push(' ');
            continue;
        }

        if !pending.is_empty() {
            buf.push_str(&pending);
            pending.clear();
        }

        if c.style.is_zero() && !pen.is_zero() {
            buf.push_str(rusty_x_ansi::style::RESET_STYLE);
            pen = Style::default();
        }
        if !c.style.equal(&pen) {
            let seq = c.style.diff(&pen);
            buf.push_str(&seq);
            pen = c.style.clone();
        }

        // Write the URL escape sequence.
        // NOTE: upstream's Link is a value type, so an unset link (None)
        // compares equal to an empty link (Some(Link::default())); normalize
        // before comparing.
        let link_changed = match (&c.link, &link) {
            (None, None) => false,
            (Some(a), Some(b)) => a != b,
            (Some(a), None) => !a.is_zero(),
            (None, Some(b)) => !b.is_zero(),
        };
        if link_changed {
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
        buf.push_str(rusty_x_ansi::style::RESET_STYLE);
    }
}

/// InsertLineArea inserts new lines at the given line position, with the
/// given optional cell, within the rectangle bounds. This follows terminal
/// [rusty_x_ansi] IL behavior.
pub(crate) fn insert_line_area(
    b: &mut Buffer,
    y: usize,
    n: usize,
    c: Option<&Cell>,
    area: Rectangle,
) {
    if n == 0 || y < area.min.1 || y >= area.max.1 || y >= b.height() {
        return;
    }

    // Limit number of lines to insert to available space
    let n = if y + n > area.max.1 {
        area.max.1 - y
    } else {
        n
    };

    // Move existing lines down within the bounds
    for i in (y + n..area.max.1).rev() {
        for x in area.min.0..area.max.0 {
            let src = b.cell_at(x, i - n).cloned();
            if let Some(dst) = b.cell_at_mut(x, i) {
                *dst = src.unwrap_or_else(empty_cell);
            }
        }
    }

    // Clear the newly inserted lines within bounds
    for i in y..y + n {
        for x in area.min.0..area.max.0 {
            b.set_cell(x, i, c);
        }
    }
}

/// DeleteLineArea deletes lines at the given line position, with the given
/// optional cell, within the rectangle bounds. This follows terminal
/// [rusty_x_ansi] DL behavior.
pub(crate) fn delete_line_area(
    b: &mut Buffer,
    y: usize,
    n: usize,
    c: Option<&Cell>,
    area: Rectangle,
) {
    if n == 0 || y < area.min.1 || y >= area.max.1 || y >= b.height() {
        return;
    }

    // Limit deletion count to available space in scroll region
    let n = if n > area.max.1 - y {
        area.max.1 - y
    } else {
        n
    };

    // Shift cells up within the bounds
    for dst in y..area.max.1 - n {
        let src = dst + n;
        for x in area.min.0..area.max.0 {
            let s = b.cell_at(x, src).cloned();
            if let Some(d) = b.cell_at_mut(x, dst) {
                *d = s.unwrap_or_else(empty_cell);
            }
        }
    }

    // Fill the bottom n lines with blank cells
    for i in area.max.1 - n..area.max.1 {
        for x in area.min.0..area.max.0 {
            b.set_cell(x, i, c);
        }
    }
}

/// InsertCellArea inserts new cells at the given position, with the given
/// optional cell, within the rectangle bounds. This follows terminal
/// [rusty_x_ansi] ICH behavior.
pub(crate) fn insert_cell_area(
    b: &mut Buffer,
    x: usize,
    y: usize,
    n: usize,
    c: Option<&Cell>,
    area: Rectangle,
) {
    if n == 0
        || y < area.min.1
        || y >= area.max.1
        || y >= b.height()
        || x < area.min.0
        || x >= area.max.0
        || x >= b.width()
    {
        return;
    }

    // Limit number of cells to insert to available space
    let n = if x + n > area.max.0 {
        area.max.0 - x
    } else {
        n
    };

    // Move existing cells within rectangle bounds to the right
    for i in (x + n..area.max.0).rev() {
        if i - n >= area.min.0 {
            let src = b.cell_at(i - n, y).cloned();
            if let Some(dst) = b.cell_at_mut(i, y) {
                *dst = src.unwrap_or_else(empty_cell);
            }
        }
    }

    // Clear the newly inserted cells within rectangle bounds
    let end = (x + n).min(area.max.0);
    for i in x..end {
        b.set_cell(i, y, c);
    }
}

/// DeleteCellArea deletes cells at the given position, with the given
/// optional cell, within the rectangle bounds. This follows terminal
/// [rusty_x_ansi] DCH behavior.
pub(crate) fn delete_cell_area(
    b: &mut Buffer,
    x: usize,
    y: usize,
    n: usize,
    c: Option<&Cell>,
    area: Rectangle,
) {
    if n == 0
        || y < area.min.1
        || y >= area.max.1
        || y >= b.height()
        || x < area.min.0
        || x >= area.max.0
        || x >= b.width()
    {
        return;
    }

    // Calculate how many positions we can actually delete
    let n = n.min(area.max.0 - x);

    // Shift the remaining cells to the left. We use SetCell to ensure we
    // blank out any wide cells we encounter.
    for i in x..area.max.0 - n {
        if i + n < area.max.0 {
            let src = b.cell_at(i + n, y).cloned();
            b.set_cell(i, y, src.as_ref());
        }
    }

    // Fill the vacated positions with the given cell
    for i in area.max.0 - n..area.max.0 {
        b.set_cell(i, y, c);
    }
}

fn max(a: usize, b: usize) -> usize {
    if a > b {
        a
    } else {
        b
    }
}

fn min(a: usize, b: usize) -> usize {
    if a < b {
        a
    } else {
        b
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cell::Link;
    use crate::screen::rect;
    use rusty_x_ansi::style::Color;

    #[test]
    fn test_buffer_basic() {
        let mut b = new_buffer(3, 2);
        assert_eq!(b.width(), 3);
        assert_eq!(b.height(), 2);
        b.set_cell(0, 0, Some(&Cell::new("a")));
        b.set_cell(1, 0, Some(&Cell::new("b")));
        b.set_cell(2, 0, Some(&Cell::new("c")));
        let out = b.render();
        assert_eq!(out, "abc\n");
    }

    #[test]
    fn test_buffer_clear() {
        let mut b = new_buffer(2, 1);
        b.set_cell(0, 0, Some(&Cell::new("x")));
        b.clear();
        assert_eq!(b.cell_at(0, 0).unwrap().content, " ");
    }

    #[test]
    fn test_line_set_wide() {
        let mut l = Line(vec![empty_cell(); 4]);
        let w = Cell {
            content: "界".to_string(),
            width: 2,
            ..Cell::default()
        };
        l.set(0, w);
        assert_eq!(l[1].width, 0);
        l.set(1, empty_cell());
        assert_eq!(l[0].width, 1);
        assert_eq!(l[1].width, 1);
    }

    #[test]
    fn test_render_buffer_touch() {
        let mut rb = new_render_buffer(4, 2);
        rb.set_cell(1, 0, Some(&Cell::new("x")));
        assert_eq!(rb.touched_lines(), 1);
        rb.clear();
        assert_eq!(rb.touched_lines(), 2);
    }

    #[test]
    fn test_trim_space() {
        assert_eq!(trim_space("a  \nb  \n"), "a\nb\n");
        assert_eq!(trim_space("a \r\n"), "a\r\n");
    }

    #[test]
    fn test_buffer_clone_area() {
        let mut b = new_buffer(4, 2);
        b.set_cell(1, 0, Some(&Cell::new("x")));
        let n = b.clone_area(Rectangle {
            min: (0, 0),
            max: (2, 1),
        });
        assert!(n.is_some());
        let n = n.unwrap();
        assert_eq!(n.width(), 2);
        assert_eq!(n.cell_at(1, 0).unwrap().content, "x");
    }

    /// Ported from upstream `TestLineRenderLine`: styled and hyperlinked cells.
    #[test]
    fn test_line_render_line() {
        let mut l = Line(vec![empty_cell(); 5]);
        l[0] = Cell {
            content: "H".to_string(),
            width: 1,
            style: Style {
                fg: Some(Color::Basic(1)),
                ..Style::default()
            },
            ..Cell::default()
        };
        l[1] = Cell {
            content: "i".to_string(),
            width: 1,
            ..Cell::default()
        };
        let mut out = String::new();
        render_line(&mut out, &l);
        assert!(out.contains("H"));
        assert!(out.contains("i"));
        assert!(out.contains("31")); // SGR red for fg Basic(1).

        let mut l = Line(vec![empty_cell(); 5]);
        for (i, ch) in "Link".chars().enumerate() {
            l[i] = Cell {
                content: ch.to_string(),
                width: 1,
                link: Some(Link {
                    url: "http://example.com".to_string(),
                    params: String::new(),
                }),
                ..Cell::default()
            };
        }
        let mut out = String::new();
        render_line(&mut out, &l);
        assert!(out.contains("http://example.com"));
    }

    /// render_line with zero cells (pending spaces) and resets.
    #[test]
    fn test_line_render_line_edges() {
        // Zero cells accumulate pending spaces and are emitted before content.
        let mut l = Line(vec![empty_cell(); 3]);
        l[2] = Cell {
            content: "x".to_string(),
            width: 1,
            ..Cell::default()
        };
        let mut out = String::new();
        render_line(&mut out, &l);
        assert!(out.ends_with("  x"));

        // A styled cell followed by an empty cell resets the pen.
        let mut l = Line(vec![empty_cell(); 3]);
        l[0] = Cell {
            content: "a".to_string(),
            width: 1,
            style: Style {
                fg: Some(Color::Basic(2)),
                ..Style::default()
            },
            ..Cell::default()
        };
        l[1] = empty_cell();
        l[2] = Cell {
            content: "b".to_string(),
            width: 1,
            ..Cell::default()
        };
        let mut out = String::new();
        render_line(&mut out, &l);
        assert!(out.contains("\x1b[m")); // reset

        // A trailing styled cell emits a final reset.
        let mut l = Line(vec![empty_cell(); 1]);
        l[0] = Cell {
            content: "a".to_string(),
            width: 1,
            style: Style {
                fg: Some(Color::Basic(3)),
                ..Style::default()
            },
            ..Cell::default()
        };
        let mut out = String::new();
        render_line(&mut out, &l);
        assert!(out.ends_with("\x1b[m"));
    }

    /// Buffer string rendering with a hyperlink and a style change mid-line.
    #[test]
    fn test_buffer_string_links() {
        let mut b = new_buffer(6, 1);
        let link = Link {
            url: "https://x.dev".to_string(),
            params: String::new(),
        };
        b.set_cell(
            0,
            0,
            Some(&Cell {
                content: "a".to_string(),
                width: 1,
                link: Some(link.clone()),
                ..Cell::default()
            }),
        );
        b.set_cell(
            1,
            0,
            Some(&Cell {
                content: "b".to_string(),
                width: 1,
                link: Some(link.clone()),
                ..Cell::default()
            }),
        );
        let out = b.render();
        assert!(out.contains("https://x.dev"));

        // A link change resets the old link.
        b.set_cell(
            2,
            0,
            Some(&Cell {
                content: "c".to_string(),
                width: 1,
                link: Some(Link {
                    url: "https://y.dev".to_string(),
                    params: String::new(),
                }),
                ..Cell::default()
            }),
        );
        let out = b.render();
        assert!(out.contains("https://y.dev"));
    }

    /// Ported from upstream `TestBufferMethods` (Width/CellAt/SetCell/Resize/
    /// FillArea/Touch/Clear/Clone/CloneArea/Draw/Render).
    #[test]
    fn test_buffer_methods() {
        // Empty buffer reports zero size.
        let mut b = new_buffer(0, 0);
        assert_eq!(b.width(), 0);
        assert_eq!(b.height(), 0);
        // Resize.
        b.resize(10, 4);
        assert_eq!(b.width(), 10);
        assert_eq!(b.height(), 4);

        b.set_cell(
            2,
            1,
            Some(&Cell {
                content: "X".to_string(),
                width: 1,
                ..Cell::default()
            }),
        );
        let c = b.cell_at(2, 1).unwrap();
        assert_eq!(c.content, "X");
        // Out-of-bounds accesses return None.
        assert!(b.cell_at(10, 0).is_none());
        assert!(b.cell_at(0, 4).is_none());

        // SetCell overwrites.
        b.set_cell(
            2,
            1,
            Some(&Cell {
                content: "A".to_string(),
                width: 1,
                ..Cell::default()
            }),
        );
        assert_eq!(b.cell_at(2, 1).unwrap().content, "A");

        // Resize larger then smaller.
        b.resize(15, 10);
        assert_eq!((b.width(), b.height()), (15, 10));
        b.resize(15, 10);
        assert_eq!((b.width(), b.height()), (15, 10));

        // FillArea only touches the rectangle.
        let area = rect(1, 1, 3, 2);
        b.fill_area(
            Some(&Cell {
                content: "X".to_string(),
                width: 1,
                ..Cell::default()
            }),
            area,
        );
        assert_eq!(b.cell_at(1, 1).unwrap().content, "X");
        assert_eq!(b.cell_at(2, 2).unwrap().content, "X");
        assert_ne!(b.cell_at(0, 0).unwrap().content, "X");

        // Touch marks the line as dirty.
        let mut rb = new_render_buffer(15, 10);
        rb.touch(1, 3);
        assert_eq!(rb.touched_lines(), 1);

        // Clear resets all cells to spaces.
        b.clear();
        assert_eq!(b.cell_at(1, 1).unwrap().content, " ");

        // Clone is independent.
        b.set_cell(
            2,
            1,
            Some(&Cell {
                content: "X".to_string(),
                width: 1,
                ..Cell::default()
            }),
        );
        let mut clone = b.clone();
        assert_eq!(clone.cell_at(2, 1).unwrap().content, "X");
        clone.set_cell(
            2,
            1,
            Some(&Cell {
                content: "Z".to_string(),
                width: 1,
                ..Cell::default()
            }),
        );
        assert_eq!(b.cell_at(2, 1).unwrap().content, "X");

        // CloneArea copies a sub-rectangle.
        let mut b2 = new_buffer(4, 4);
        b2.set_cell(
            1,
            1,
            Some(&Cell {
                content: "X".to_string(),
                width: 1,
                ..Cell::default()
            }),
        );
        b2.set_cell(
            2,
            2,
            Some(&Cell {
                content: "Y".to_string(),
                width: 1,
                ..Cell::default()
            }),
        );
        let area = b2.clone_area(rect(1, 1, 2, 2)).unwrap();
        assert_eq!((area.width(), area.height()), (2, 2));
        assert_eq!(area.cell_at(0, 0).unwrap().content, "X");
        assert_eq!(area.cell_at(1, 1).unwrap().content, "Y");

        // Render emits the visible content.
        let mut b3 = new_buffer(3, 1);
        b3.set_cell(
            0,
            0,
            Some(&Cell {
                content: "H".to_string(),
                width: 1,
                ..Cell::default()
            }),
        );
        b3.set_cell(
            1,
            0,
            Some(&Cell {
                content: "i".to_string(),
                width: 1,
                ..Cell::default()
            }),
        );
        assert!(b3.render().contains("Hi"));
    }

    /// Ported from upstream `TestBufferLineOperations` (on ScreenBuffer).
    #[test]
    fn test_buffer_line_operations() {
        // InsertLine moves rows down.
        let mut b = new_render_buffer(5, 3);
        b.set_cell(0, 0, Some(&Cell::new("A")));
        b.set_cell(0, 1, Some(&Cell::new("B")));
        b.set_cell(0, 2, Some(&Cell::new("C")));
        b.insert_line(1, 1, None);
        assert_eq!(b.cell_at(0, 2).unwrap().content, "B");
        assert_eq!(b.cell_at(0, 1).unwrap().content, " ");

        // InsertLineArea within a rectangle.
        let mut b = new_render_buffer(5, 5);
        b.set_cell(0, 1, Some(&Cell::new("A")));
        b.set_cell(0, 2, Some(&Cell::new("B")));
        b.insert_line_area(2, 1, None, rect(0, 1, 5, 4));
        assert_eq!(b.cell_at(0, 3).unwrap().content, "B");

        // DeleteLine moves rows up.
        let mut b = new_render_buffer(5, 3);
        b.set_cell(0, 0, Some(&Cell::new("A")));
        b.set_cell(0, 1, Some(&Cell::new("B")));
        b.set_cell(0, 2, Some(&Cell::new("C")));
        b.delete_line(1, 1, None);
        assert_eq!(b.cell_at(0, 1).unwrap().content, "C");
        assert_eq!(b.cell_at(0, 2).unwrap().content, " ");

        // DeleteLineArea within a rectangle.
        let mut b = new_render_buffer(5, 5);
        b.set_cell(0, 1, Some(&Cell::new("A")));
        b.set_cell(0, 2, Some(&Cell::new("B")));
        b.set_cell(0, 3, Some(&Cell::new("C")));
        b.delete_line_area(2, 1, None, rect(0, 1, 5, 4));
        assert_eq!(b.cell_at(0, 2).unwrap().content, "C");
    }

    /// Ported from upstream `TestBufferCellOperations` (on ScreenBuffer).
    #[test]
    fn test_buffer_cell_operations() {
        // InsertCell shifts cells right.
        let mut b = new_render_buffer(5, 2);
        b.set_cell(0, 0, Some(&Cell::new("A")));
        b.set_cell(1, 0, Some(&Cell::new("B")));
        b.set_cell(2, 0, Some(&Cell::new("C")));
        b.insert_cell(1, 0, 1, None);
        assert_eq!(b.cell_at(2, 0).unwrap().content, "B");

        // InsertCellArea within a rectangle.
        let mut b = new_render_buffer(5, 3);
        b.set_cell(1, 1, Some(&Cell::new("A")));
        b.set_cell(2, 1, Some(&Cell::new("B")));
        b.insert_cell_area(1, 1, 1, None, rect(1, 1, 4, 2));
        assert_eq!(b.cell_at(2, 1).unwrap().content, "A");

        // DeleteCell shifts cells left.
        let mut b = new_render_buffer(5, 2);
        b.set_cell(0, 0, Some(&Cell::new("A")));
        b.set_cell(1, 0, Some(&Cell::new("B")));
        b.set_cell(2, 0, Some(&Cell::new("C")));
        b.delete_cell(1, 0, 1, None);
        assert_eq!(b.cell_at(1, 0).unwrap().content, "C");

        // DeleteCellArea within a rectangle.
        let mut b = new_render_buffer(5, 3);
        b.set_cell(1, 1, Some(&Cell::new("A")));
        b.set_cell(2, 1, Some(&Cell::new("B")));
        b.set_cell(3, 1, Some(&Cell::new("C")));
        b.delete_cell_area(2, 1, 1, None, rect(1, 1, 4, 2));
        assert_eq!(b.cell_at(2, 1).unwrap().content, "C");
    }

    /// Buffer draw onto another screen, including wide cells and non-overlap.
    #[test]
    fn test_buffer_draw() {
        // Draw onto a plain Buffer screen.
        let mut src = new_buffer(3, 1);
        src.set_cell(0, 0, Some(&Cell::new("A")));
        src.set_cell(1, 0, Some(&Cell::new("B")));
        let mut dst = new_buffer(5, 2);
        src.draw(&mut dst, rect(1, 0, 3, 1));
        assert_eq!(dst.cell_at(1, 0).unwrap().content, "A");
        assert_eq!(dst.cell_at(2, 0).unwrap().content, "B");
        // Non-overlapping area draws nothing.
        let mut dst2 = new_buffer(5, 2);
        src.draw(&mut dst2, rect(10, 10, 3, 1));
        assert_eq!(dst2.cell_at(0, 0).unwrap().content, " ");
        // Wide cell draws across.
        let mut src2 = new_buffer(3, 1);
        src2.set_cell(0, 0, Some(&Cell::new("界")));
        let mut dst3 = new_buffer(5, 1);
        src2.draw(&mut dst3, rect(0, 0, 3, 1));
        assert_eq!(dst3.cell_at(0, 0).unwrap().content, "界");
        // Empty buffer draws nothing.
        let empty = new_buffer(0, 0);
        let mut dst4 = new_buffer(3, 1);
        empty.draw(&mut dst4, rect(0, 0, 3, 1));
    }

    /// Line/Lines string and width accessors.
    #[test]
    fn test_line_lines_string_width() {
        let l = Line(vec![Cell::new("a"), Cell::new("b"), empty_cell()]);
        assert_eq!(l.string(), "ab");
        assert_eq!(l.len(), 3);
        // A leading empty cell produces a pending space before content.
        let l2 = Line(vec![empty_cell(), Cell::new("x")]);
        assert_eq!(l2.string(), " x");

        let lines = Lines(vec![
            Line(vec![Cell::new("a"), Cell::new("b")]),
            Line(vec![Cell::new("c")]),
        ]);
        assert_eq!(lines.height(), 2);
        assert_eq!(lines.width(), 2);
        assert_eq!(lines.string(), "ab\nc");
    }

    /// Line::set wide-cell continuation and overflow handling.
    #[test]
    fn test_line_set_wide_edges() {
        // Overwriting a wide cell's leading cell clears its placeholders.
        let mut l = Line(vec![empty_cell(); 4]);
        l.set(
            0,
            Cell {
                content: "界".to_string(),
                width: 2,
                ..Cell::default()
            },
        );
        assert_eq!(l[1].width, 0);
        // Writing into the continuation cell clears the wide cell's tail.
        l.set(
            1,
            Cell {
                content: "x".to_string(),
                width: 1,
                ..Cell::default()
            },
        );
        assert_eq!(l[0].width, 1);
        // Writing a wide cell that overflows the line writes blanks.
        let mut l2 = Line(vec![empty_cell(); 2]);
        l2.set(
            1,
            Cell {
                content: "界".to_string(),
                width: 2,
                ..Cell::default()
            },
        );
        assert_eq!(l2[1].content, " ");
        // Setting out of bounds is a no-op.
        l2.set(5, Cell::new("z"));
        assert_eq!(l2[1].content, " ");
    }

    /// Insert/delete area boundary conditions (out-of-bounds, n==0).
    #[test]
    fn test_area_boundary_conditions() {
        let area = rect(1, 1, 3, 3);
        // n == 0 is a no-op.
        let mut b = new_render_buffer(5, 5);
        b.set_cell(2, 2, Some(&Cell::new("X")));
        b.insert_line_area(2, 0, None, area);
        b.delete_line_area(2, 0, None, area);
        b.insert_cell_area(2, 2, 0, None, area);
        b.delete_cell_area(2, 2, 0, None, area);
        assert_eq!(b.cell_at(2, 2).unwrap().content, "X");
        // y outside the area is a no-op.
        let mut b = new_render_buffer(5, 5);
        b.set_cell(2, 2, Some(&Cell::new("X")));
        b.insert_line_area(5, 1, None, area);
        b.delete_line_area(5, 1, None, area);
        b.insert_cell_area(2, 4, 1, None, area);
        b.delete_cell_area(2, 4, 1, None, area);
        assert_eq!(b.cell_at(2, 2).unwrap().content, "X");
        // x outside the area for cell ops is a no-op.
        let mut b = new_render_buffer(5, 5);
        b.set_cell(2, 2, Some(&Cell::new("X")));
        b.insert_cell_area(4, 2, 1, None, area);
        b.delete_cell_area(4, 2, 1, None, area);
        assert_eq!(b.cell_at(2, 2).unwrap().content, "X");
    }

    /// ScreenBuffer clear/fill/draw wrappers.
    #[test]
    fn test_screen_buffer_wrappers() {
        let mut sb = new_screen_buffer(4, 2);
        sb.set_cell(0, 0, Some(&Cell::new("X")));
        sb.clear();
        assert_eq!(sb.cell_at(0, 0).unwrap().content, " ");
        sb.fill(Some(&Cell::new("Z")));
        assert_eq!(sb.cell_at(3, 1).unwrap().content, "Z");
        // draw onto another buffer via the ScreenBuffer's draw.
        let mut dst = new_buffer(5, 3);
        let mut src = new_screen_buffer(2, 1);
        src.set_cell(0, 0, Some(&Cell::new("A")));
        src.draw(&mut dst, rect(1, 1, 2, 1));
        assert_eq!(dst.cell_at(1, 1).unwrap().content, "A");
    }

    /// Overwriting a wide cell's leading cell clears its placeholders.
    #[test]
    fn test_line_set_overwrite_wide_leading() {
        let mut l = Line(vec![empty_cell(); 4]);
        l.set(
            0,
            Cell {
                content: "界".to_string(),
                width: 2,
                ..Cell::default()
            },
        );
        assert_eq!(l[1].width, 0);
        // Overwrite the leading wide cell: clears the placeholder.
        l.set(
            0,
            Cell {
                content: "x".to_string(),
                width: 1,
                ..Cell::default()
            },
        );
        assert_eq!(l[0].content, "x");
        assert_eq!(l[0].width, 1);
        assert_eq!(l[1].content, " ");
        assert_eq!(l[1].width, 1);
    }

    /// Buffer::string joins lines.
    #[test]
    fn test_buffer_string() {
        let mut b = new_buffer(3, 2);
        b.set_cell(0, 0, Some(&Cell::new("a")));
        b.set_cell(0, 1, Some(&Cell::new("b")));
        assert_eq!(b.string(), "a\nb");
    }

    /// Buffer clone_area edge cases and draw with wide/zero cells.
    #[test]
    fn test_buffer_clone_area_and_draw_edges() {
        // clone_area with area beyond the buffer returns an empty buffer.
        let b = new_buffer(5, 5);
        let c = b.clone_area(rect(3, 3, 5, 5));
        assert!(c.is_some());
        // clone_area with an inverted area returns None.
        let inv = Rectangle {
            min: (5, 5),
            max: (3, 3),
        };
        let c = b.clone_area(inv);
        assert!(c.is_none());
        // clone_area copies content.
        let mut bw = new_buffer(5, 1);
        bw.set_cell(0, 0, Some(&Cell::new("X")));
        let c = bw.clone_area(rect(0, 0, 2, 1)).unwrap();
        assert_eq!(c.cell_at(0, 0).unwrap().content, "X");
        // draw copies non-zero cells.
        let mut src = new_buffer(4, 1);
        src.set_cell(0, 0, Some(&Cell::new("X")));
        src.set_cell(2, 0, Some(&Cell::new("Y")));
        let mut dst = new_buffer(6, 1);
        src.draw(&mut dst, rect(0, 0, 4, 1));
        assert_eq!(dst.cell_at(0, 0).unwrap().content, "X");
        assert_eq!(dst.cell_at(2, 0).unwrap().content, "Y");
    }
}
