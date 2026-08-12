//! Cleanroom Rust port of upstream Go source file: `border.go`
//! Upstream Target Tag / Version: `v0.0.0-20260703014108-f5a850f9c2b7`
//!
//! <public-docs>
//! Border models for drawing frames around components: predefined border
//! styles (normal, rounded, block, half-block, thick, double, hidden,
//! markdown, ASCII) and the `Border::draw` renderer.
//! </public-docs>

use crate::buffer::Screen;
use crate::cell::{Cell, Link};
use crate::screen::Rectangle;
use crate::style::Style;

/// Side represents a single border side with its properties.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Side {
    /// The content of the side.
    pub content: String,
    /// The style of the side.
    pub style: Style,
    /// The link of the side.
    pub link: Link,
}

impl Side {
    /// Creates a new side with the given content.
    pub fn new(content: &str) -> Side {
        Side {
            content: content.to_string(),
            ..Side::default()
        }
    }
}

/// Border represents a border with its properties.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Border {
    /// The top side.
    pub top: Side,
    /// The bottom side.
    pub bottom: Side,
    /// The left side.
    pub left: Side,
    /// The right side.
    pub right: Side,
    /// The top-left corner.
    pub top_left: Side,
    /// The top-right corner.
    pub top_right: Side,
    /// The bottom-left corner.
    pub bottom_left: Side,
    /// The bottom-right corner.
    pub bottom_right: Side,
}

/// NormalBorder returns a standard-type border with a normal weight and 90
/// degree corners.
pub fn normal_border() -> Border {
    Border {
        top: Side::new("─"),
        bottom: Side::new("─"),
        left: Side::new("│"),
        right: Side::new("│"),
        top_left: Side::new("┌"),
        top_right: Side::new("┐"),
        bottom_left: Side::new("└"),
        bottom_right: Side::new("┘"),
    }
}

/// RoundedBorder returns a border with rounded corners.
pub fn rounded_border() -> Border {
    Border {
        top: Side::new("─"),
        bottom: Side::new("─"),
        left: Side::new("│"),
        right: Side::new("│"),
        top_left: Side::new("╭"),
        top_right: Side::new("╮"),
        bottom_left: Side::new("╰"),
        bottom_right: Side::new("╯"),
    }
}

/// BlockBorder returns a border that takes the whole block.
pub fn block_border() -> Border {
    Border {
        top: Side::new("█"),
        bottom: Side::new("█"),
        left: Side::new("█"),
        right: Side::new("█"),
        top_left: Side::new("█"),
        top_right: Side::new("█"),
        bottom_left: Side::new("█"),
        bottom_right: Side::new("█"),
    }
}

/// OuterHalfBlockBorder returns a half-block border that sits outside the
/// frame.
pub fn outer_half_block_border() -> Border {
    Border {
        top: Side::new("▀"),
        bottom: Side::new("▄"),
        left: Side::new("▌"),
        right: Side::new("▐"),
        top_left: Side::new("▛"),
        top_right: Side::new("▜"),
        bottom_left: Side::new("▙"),
        bottom_right: Side::new("▟"),
    }
}

/// InnerHalfBlockBorder returns a half-block border that sits inside the
/// frame.
pub fn inner_half_block_border() -> Border {
    Border {
        top: Side::new("▄"),
        bottom: Side::new("▀"),
        left: Side::new("▐"),
        right: Side::new("▌"),
        top_left: Side::new("▗"),
        top_right: Side::new("▖"),
        bottom_left: Side::new("▝"),
        bottom_right: Side::new("▘"),
    }
}

/// ThickBorder returns a border that's thicker than the one returned by
/// [normal_border].
pub fn thick_border() -> Border {
    Border {
        top: Side::new("━"),
        bottom: Side::new("━"),
        left: Side::new("┃"),
        right: Side::new("┃"),
        top_left: Side::new("┏"),
        top_right: Side::new("┓"),
        bottom_left: Side::new("┗"),
        bottom_right: Side::new("┛"),
    }
}

/// DoubleBorder returns a border comprised of two thin strokes.
pub fn double_border() -> Border {
    Border {
        top: Side::new("═"),
        bottom: Side::new("═"),
        left: Side::new("║"),
        right: Side::new("║"),
        top_left: Side::new("╔"),
        top_right: Side::new("╗"),
        bottom_left: Side::new("╚"),
        bottom_right: Side::new("╝"),
    }
}

/// HiddenBorder returns a border that renders as a series of single-cell
/// spaces. It's useful for cases when you want to remove a standard border
/// but maintain layout positioning.
pub fn hidden_border() -> Border {
    Border {
        top: Side::new(" "),
        bottom: Side::new(" "),
        left: Side::new(" "),
        right: Side::new(" "),
        top_left: Side::new(" "),
        top_right: Side::new(" "),
        bottom_left: Side::new(" "),
        bottom_right: Side::new(" "),
    }
}

/// MarkdownBorder returns a table border in markdown style.
pub fn markdown_border() -> Border {
    Border {
        left: Side::new("|"),
        right: Side::new("|"),
        top_left: Side::new("|"),
        top_right: Side::new("|"),
        bottom_left: Side::new("|"),
        bottom_right: Side::new("|"),
        ..Border::default()
    }
}

/// ASCIIBorder returns a table border with ASCII characters.
pub fn ascii_border() -> Border {
    Border {
        top: Side::new("-"),
        bottom: Side::new("-"),
        left: Side::new("|"),
        right: Side::new("|"),
        top_left: Side::new("+"),
        top_right: Side::new("+"),
        bottom_left: Side::new("+"),
        bottom_right: Side::new("+"),
    }
}

impl Border {
    /// Style returns a new [Border] with the given style applied to all
    /// [Side]s.
    pub fn style(&self, style: Style) -> Border {
        Border {
            top: Side {
                style: style.clone(),
                ..self.top.clone()
            },
            bottom: Side {
                style: style.clone(),
                ..self.bottom.clone()
            },
            left: Side {
                style: style.clone(),
                ..self.left.clone()
            },
            right: Side {
                style: style.clone(),
                ..self.right.clone()
            },
            top_left: Side {
                style: style.clone(),
                ..self.top_left.clone()
            },
            top_right: Side {
                style: style.clone(),
                ..self.top_right.clone()
            },
            bottom_left: Side {
                style: style.clone(),
                ..self.bottom_left.clone()
            },
            bottom_right: Side {
                style,
                ..self.bottom_right.clone()
            },
        }
    }

    /// Link returns a new [Border] with the given link applied to all
    /// [Side]s.
    pub fn link(&self, link: Link) -> Border {
        Border {
            top: Side {
                link: link.clone(),
                ..self.top.clone()
            },
            bottom: Side {
                link: link.clone(),
                ..self.bottom.clone()
            },
            left: Side {
                link: link.clone(),
                ..self.left.clone()
            },
            right: Side {
                link: link.clone(),
                ..self.right.clone()
            },
            top_left: Side {
                link: link.clone(),
                ..self.top_left.clone()
            },
            top_right: Side {
                link: link.clone(),
                ..self.top_right.clone()
            },
            bottom_left: Side {
                link: link.clone(),
                ..self.bottom_left.clone()
            },
            bottom_right: Side {
                link,
                ..self.bottom_right.clone()
            },
        }
    }

    /// Draw draws the border around the given component.
    pub fn draw(&self, scr: &mut dyn Screen, area: Rectangle) {
        for y in area.min.1..area.max.1 {
            for x in area.min.0..area.max.0 {
                let side: Option<&Side> = if y == area.min.1 && x == area.min.0 {
                    Some(&self.top_left)
                } else if y == area.min.1 && x == area.max.0 - 1 {
                    Some(&self.top_right)
                } else if y == area.max.1 - 1 && x == area.min.0 {
                    Some(&self.bottom_left)
                } else if y == area.max.1 - 1 && x == area.max.0 - 1 {
                    Some(&self.bottom_right)
                } else if y == area.min.1 {
                    Some(&self.top)
                } else if y == area.max.1 - 1 {
                    Some(&self.bottom)
                } else if x == area.min.0 {
                    Some(&self.left)
                } else if x == area.max.0 - 1 {
                    Some(&self.right)
                } else {
                    None
                };
                if let Some(side) = side {
                    let cell = border_cell(scr, side);
                    scr.set_cell(x, y, Some(&cell));
                }
            }
        }
    }
}

fn border_cell(scr: &dyn Screen, b: &Side) -> Cell {
    let mut c = Cell {
        content: b.content.clone(),
        width: scr.width_method().string_width(&b.content),
        ..Cell::default()
    };
    c.style = b.style.clone();
    c.link = Some(b.link.clone());
    c
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normal_border() {
        let b = normal_border();
        assert_eq!(b.top.content, "─");
        assert_eq!(b.top_left.content, "┌");
        assert_eq!(b.bottom_right.content, "┘");
    }

    #[test]
    fn test_border_style() {
        let b = normal_border().style(Style {
            fg: Some(charming_x_ansi::style::Color::Basic(1)),
            ..Style::default()
        });
        assert_eq!(b.top.style, b.bottom.style);
        assert_eq!(b.top.style.fg, Some(charming_x_ansi::style::Color::Basic(1)));
    }

    #[test]
    fn test_border_draw() {
        let mut buf = crate::new_buffer(5, 3);
        let b = normal_border();
        b.draw(&mut buf, Rectangle { min: (0, 0), max: (5, 3) });
        assert_eq!(buf.cell_at(0, 0).unwrap().content, "┌");
        assert_eq!(buf.cell_at(4, 0).unwrap().content, "┐");
        assert_eq!(buf.cell_at(0, 2).unwrap().content, "└");
        assert_eq!(buf.cell_at(4, 2).unwrap().content, "┘");
        assert_eq!(buf.cell_at(2, 0).unwrap().content, "─");
        assert_eq!(buf.cell_at(0, 1).unwrap().content, "│");
        // Interior cells untouched.
        assert_eq!(buf.cell_at(2, 1).unwrap().content, " ");
    }
}
