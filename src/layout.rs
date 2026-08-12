//! Cleanroom Rust port of upstream Go source files: `layout/cache.go`, `layout/constraint.go`, `layout/flex.go`, `layout/layout.go`, `layout/padding.go`
//! Upstream Target Tag / Version: `v0.0.0-20260703014108-f5a850f9c2b7`
//!
//! <public-docs>
//! The layout engine: partitions terminal screen space into rectangular
//! regions using a constraint-based solver (a Cassowary solver, ported in
//! [`crate::casso`]).
//!
//! A [`Layout`] takes the available area and a list of constraints ([Constraint::Len],
//! [Constraint::Ratio], [Constraint::Percent], [Constraint::Fill], [Constraint::Min], [Constraint::Max]) and produces a set of
//! non-overlapping rectangles. The solver tries to honour every constraint;
//! when that is impossible it relaxes lower-priority ones first.
//!
//! Note: the old root package `layout.go` was deleted upstream; the layout
//! engine now lives in the `layout/` subpackage.
//! </public-docs>

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use crate::casso::{new_solver, Constraint as CConstraint, Op, Solver, Symbol};
use crate::lru::{new as new_lru, Lru};
use crate::screen::Rectangle;

/// The multiplier that scales cell positions into a higher-precision
/// floating-point domain before handing them to the constraint solver.
/// The number of trailing zeros determines the decimal precision kept
/// during rounding.
pub const FLOAT_PRECISION_MULTIPLIER: f64 = 100.0;

const REQUIRED: f64 = 1_001_001_000.0;
const STRONG: f64 = 1_000_000.0;
const MEDIUM: f64 = 1_000.0;
const WEAK: f64 = 1.0;

/// spacerSizeEq enforces equal sizing across spacers.
///
/// ```text
/// ┌     ┐┌───┐┌     ┐┌───┐┌     ┐
///   ==x  │   │  ==x  │   │  ==x
/// └     ┘└───┘└     ┘└───┘└     ┘
/// ```
const SPACER_SIZE_EQ: f64 = REQUIRED / 10.0;

/// minSizeGTE enforces the lower-bound inequality for [Constraint::Min] constraints.
const MIN_SIZE_GTE: f64 = STRONG * 100.0;

/// maxSizeLTE enforces the upper-bound inequality for [Constraint::Max] constraints.
const MAX_SIZE_LTE: f64 = STRONG * 100.0;

/// lengthSizeEq pins the segment to the exact size requested by a [Constraint::Len]
/// constraint.
const LENGTH_SIZE_EQ: f64 = STRONG * 10.0;

/// percentSizeEq tries to make the segment match its [Constraint::Percent] target.
const PERCENT_SIZE_EQ: f64 = STRONG;

/// ratioSizeEq tries to make the segment match its [Constraint::Ratio] target.
const RATIO_SIZE_EQ: f64 = STRONG / 10.0;

/// minSizeEq is an equality companion for the [Constraint::Min] lower-bound; it nudges
/// the segment toward the minimum value when room is tight.
const MIN_SIZE_EQ: f64 = MEDIUM * 10.0;

/// maxSizeEq is an equality companion for the [Constraint::Max] upper-bound; it nudges
/// the segment toward the maximum value.
const MAX_SIZE_EQ: f64 = MEDIUM * 10.0;

/// fillGrow lets [Constraint::Fill] segments expand into available space.
const FILL_GROW: f64 = MEDIUM;

/// grow is a general expansion priority (used by [Constraint::Min] in non-legacy flex).
const GROW: f64 = 100.0;

/// spaceGrow allows spacers to expand and absorb remaining room.
const SPACE_GROW: f64 = WEAK * 10.0;

/// allSegmentGrow encourages all segments to share the same size.
const ALL_SEGMENT_GROW: f64 = WEAK;

/// globalCacheSize is chosen to comfortably hold one entry per row and column
/// of a typical terminal, with headroom to spare. A 171-column x 51-row
/// display yields 222 unique keys; doubling and rounding up gives 500.
const GLOBAL_CACHE_SIZE: i64 = 500;

/// Splitted holds the rectangles produced by a [Layout::split] call,
/// mirroring Go's `[]uv.Rectangle` slice type.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Splitted(Vec<Rectangle>);

impl Splitted {
    /// Returns the number of rectangles.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns whether there are no rectangles.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Returns the rectangle at the given index.
    pub fn get(&self, index: usize) -> Option<&Rectangle> {
        self.0.get(index)
    }

    /// Returns an iterator over the rectangles.
    pub fn iter(&self) -> std::slice::Iter<'_, Rectangle> {
        self.0.iter()
    }

    /// Returns the underlying vector of rectangles.
    pub fn into_vec(self) -> Vec<Rectangle> {
        self.0
    }

    /// Assign stores each resulting rectangle into the corresponding mutable
    /// slot. `None` slots are silently skipped.
    ///
    /// Panics when `len(areas)` exceeds the number of rectangles in
    /// [Splitted].
    pub fn assign(&self, areas: &mut [Option<&mut Rectangle>]) {
        if areas.len() > self.0.len() {
            panic!("layout: assign: too many areas");
        }
        for (i, area) in areas.iter_mut().enumerate() {
            if let Some(area) = area {
                **area = self.0[i];
            }
        }
    }
}

impl std::ops::Index<usize> for Splitted {
    type Output = Rectangle;

    fn index(&self, index: usize) -> &Rectangle {
        &self.0[index]
    }
}

impl From<Splitted> for Vec<Rectangle> {
    fn from(s: Splitted) -> Vec<Rectangle> {
        s.0
    }
}

impl From<Vec<Rectangle>> for Splitted {
    fn from(v: Vec<Rectangle>) -> Splitted {
        Splitted(v)
    }
}

/// Direction controls whether a [Layout] arranges its segments horizontally
/// (left to right) or vertically (top to bottom).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Direction {
    /// DirectionVertical - layout segments are arranged top to bottom
    /// (default).
    #[default]
    DirectionVertical,
    /// DirectionHorizontal - layout segments are arranged side by side (left
    /// to right).
    DirectionHorizontal,
}

/// Constraint describes how a single segment of a [Layout] should be sized.
///
/// Each constraint type expresses a different kind of sizing rule: fixed
/// ([Constraint::Len]), proportional ([Constraint::Percent], [Constraint::Ratio]), bounded ([Constraint::Min],
/// [Constraint::Max]), or greedy ([Constraint::Fill]). Proportional constraints are evaluated
/// against the full area being split rather than the remaining space after
/// fixed constraints have been applied.
///
/// When the solver cannot satisfy every constraint, it resolves conflicts
/// according to the following priority order (highest first):
///
/// - [Constraint::Min]
/// - [Constraint::Max]
/// - [Constraint::Len]
/// - [Constraint::Percent]
/// - [Constraint::Ratio]
/// - [Constraint::Fill]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Constraint {
    /// Min ensures the segment is no smaller than the given number of cells.
    ///
    /// ```text
    /// [Percent(100), Min(20)]
    /// ┌────────────────────────────┐┌──────────────────┐
    /// │            30 px           ││       20 px      │
    /// └────────────────────────────┘└──────────────────┘
    /// ```
    Min(i64),
    /// Max caps the segment at the given number of cells.
    ///
    /// ```text
    /// [Percent(0), Max(20)]
    /// ┌────────────────────────────┐┌──────────────────┐
    /// │            30 px           ││       20 px      │
    /// └────────────────────────────┘└──────────────────┘
    /// ```
    Max(i64),
    /// Len fixes the segment to exactly the given number of cells.
    ///
    /// ```text
    /// [Len(20), Len(30)]
    /// ┌──────────────────┐┌────────────────────────────┐
    /// │       20 px      ││            30 px           │
    /// └──────────────────┘└────────────────────────────┘
    /// ```
    Len(i64),
    /// Percent sizes the segment as a fraction of the total area.
    ///
    /// The integer value is treated as a percentage (0-100+) and multiplied
    /// by the total area; the result is rounded to the nearest cell.
    ///
    /// ```text
    /// [Percent(75), Fill(1)]
    /// ┌────────────────────────────────────┐┌──────────┐
    /// │                38 px               ││   12 px  │
    /// └────────────────────────────────────┘└──────────┘
    /// ```
    Percent(i64),
    /// Ratio sizes the segment as a numerator/denominator fraction of the
    /// total area.
    ///
    /// ```text
    /// [Ratio(1, 4) ; 4]
    /// ┌───────────┐┌──────────┐┌───────────┐┌──────────┐
    /// │   13 px   ││   12 px  ││   13 px   ││   12 px  │
    /// └───────────┘└──────────┘└───────────┘└──────────┘
    /// ```
    Ratio {
        /// The numerator of the fraction.
        num: i64,
        /// The denominator of the fraction.
        den: i64,
    },
    /// Fill distributes remaining space proportionally among all Fill
    /// segments according to their respective weights.
    ///
    /// ```text
    /// [Fill(1), Fill(2), Fill(3)]
    /// ┌──────┐┌───────────────┐┌───────────────────────┐
    /// │ 8 px ││     17 px     ││         25 px         │
    /// └──────┘└───────────────┘└───────────────────────┘
    /// ```
    Fill(i64),
}

impl Constraint {
    /// Returns the scaling factor used when sharing leftover space.
    fn scaling_factor(&self) -> f64 {
        match self {
            Constraint::Fill(scale) => {
                let scale = *scale as f64;
                scale.max(1e-6)
            }
            Constraint::Min(_) => 1.0,
            _ => 0.0,
        }
    }
}

impl std::fmt::Display for Constraint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Constraint::Min(m) => write!(f, "Min({m})"),
            Constraint::Max(m) => write!(f, "Max({m})"),
            Constraint::Len(l) => write!(f, "Len({l})"),
            Constraint::Percent(p) => write!(f, "Percent({p})"),
            Constraint::Ratio { num, den } => write!(f, "Ratio({num} / {den})"),
            Constraint::Fill(fill) => write!(f, "Fill({fill})"),
        }
    }
}

impl Constraint {
    /// Hashes the constraint into the given FNV-1a 64-bit hasher, mirroring
    /// the upstream `hash(w io.Writer)` methods.
    fn hash(&self, h: &mut Fnv64a) {
        match self {
            Constraint::Min(m) => {
                h.write("min".as_bytes());
                h.write(m.to_string().as_bytes());
            }
            Constraint::Max(m) => {
                h.write("max".as_bytes());
                h.write(m.to_string().as_bytes());
            }
            Constraint::Len(l) => {
                h.write("len".as_bytes());
                h.write(l.to_string().as_bytes());
            }
            Constraint::Percent(p) => {
                h.write("percent".as_bytes());
                h.write(p.to_string().as_bytes());
            }
            Constraint::Ratio { num, den } => {
                h.write("ratio".as_bytes());
                h.write(num.to_string().as_bytes());
                h.write(den.to_string().as_bytes());
            }
            Constraint::Fill(fill) => {
                h.write("fill".as_bytes());
                h.write(fill.to_string().as_bytes());
            }
        }
    }
}

/// Flex controls how leftover space is distributed once every segment's
/// constraint has been resolved. It is analogous to the CSS
/// justify-content property and is used together with [Layout].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Flex {
    /// FlexStart pushes segments to the leading edge of the area, leaving any
    /// surplus space at the trailing edge.
    #[default]
    FlexStart,
    /// FlexLegacy fills the entire area by assigning surplus space to the
    /// lowest-priority trailing segment. This reproduces the original
    /// Ratatui/tui-rs layout behaviour.
    FlexLegacy,
    /// FlexEnd pushes segments to the trailing edge of the area, leaving
    /// surplus space at the leading edge.
    FlexEnd,
    /// FlexCenter places segments in the middle of the area, distributing
    /// surplus space equally before the first and after the last segment.
    FlexCenter,
    /// FlexSpaceBetween distributes surplus space equally between adjacent
    /// segments, with no space before the first or after the last.
    FlexSpaceBetween,
    /// FlexSpaceEvenly distributes surplus space so that every gap (including
    /// before the first and after the last segment) is the same width.
    FlexSpaceEvenly,
    /// FlexSpaceAround places equal space on both sides of each segment.
    /// Adjacent segments therefore have twice the gap of the outer edges.
    FlexSpaceAround,
}

impl std::fmt::Display for Flex {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Flex::FlexCenter => write!(f, "Center"),
            Flex::FlexEnd => write!(f, "End"),
            Flex::FlexLegacy => write!(f, "Legacy"),
            Flex::FlexSpaceAround => write!(f, "Space Around"),
            Flex::FlexSpaceBetween => write!(f, "Space Between"),
            Flex::FlexSpaceEvenly => write!(f, "Space Evenly"),
            Flex::FlexStart => write!(f, "Start"),
        }
    }
}

/// Padding defines the inset applied to a [Layout]'s outer area before
/// solving.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Padding {
    /// Top inset.
    pub top: i64,
    /// Right inset.
    pub right: i64,
    /// Bottom inset.
    pub bottom: i64,
    /// Left inset.
    pub left: i64,
}

impl Padding {
    fn apply(&self, area: Rectangle) -> Rectangle {
        let horizontal = self.right + self.left;
        let vertical = self.top + self.bottom;

        if area.dx() < horizontal.max(0) as usize || area.dy() < vertical.max(0) as usize {
            return Rectangle {
                min: (0, 0),
                max: (0, 0),
            };
        }

        crate::window::rect(
            area.min.0 as i64 + self.left,
            area.min.1 as i64 + self.top,
            (area.dx() as i64 - horizontal).max(0),
            (area.dy() as i64 - vertical).max(0),
        )
    }
}

/// Pad builds a [Padding] value from a variable number of sides, following
/// the same shorthand convention as CSS:
/// - 0 args: all sides zero.
/// - 1 arg: uniform on every side.
/// - 2 args: first is top/bottom, second is left/right.
/// - 4 args: top, right, bottom, left.
///
/// Any other count causes a panic.
pub fn pad(sides: &[i64]) -> Padding {
    match sides.len() {
        0 => Padding::default(),
        1 => {
            let side = sides[0];
            Padding {
                top: side,
                right: side,
                bottom: side,
                left: side,
            }
        }
        2 => Padding {
            top: sides[0],
            right: sides[1],
            bottom: sides[0],
            left: sides[1],
        },
        4 => Padding {
            top: sides[0],
            right: sides[1],
            bottom: sides[2],
            left: sides[3],
        },
        _ => panic!("layout.Pad: unexpected sides count"),
    }
}

/// New returns a [Layout] configured with the given direction and constraints.
pub fn new(direction: Direction, constraints: &[Constraint]) -> Layout {
    Layout {
        direction,
        constraints: constraints.to_vec(),
        ..Layout::default()
    }
}

/// Vertical is shorthand for `new(DirectionVertical, constraints...)`.
pub fn vertical(constraints: &[Constraint]) -> Layout {
    new(Direction::DirectionVertical, constraints)
}

/// Horizontal is shorthand for `new(DirectionHorizontal, constraints...)`.
pub fn horizontal(constraints: &[Constraint]) -> Layout {
    new(Direction::DirectionHorizontal, constraints)
}

/// Layout splits a rectangular area into smaller rectangles using a set of
/// constraints. It is the primary building block for structuring terminal
/// user interfaces.
///
/// Fields:
/// - `direction`: whether segments flow vertically or horizontally.
/// - `constraints`: the sizing rules ([Constraint::Len], [Constraint::Ratio], [Constraint::Percent],
///   [Constraint::Fill], [Constraint::Min], [Constraint::Max]).
/// - `padding`: inset applied to the outer area before solving.
/// - `flex`: strategy for distributing leftover space among segments.
/// - `spacing`: gap (or overlap, if negative) between adjacent segments.
///
/// Internally, sizes are resolved by a Cassowary linear-constraint solver
/// that satisfies as many rules as it can, preferring higher-priority
/// constraints when trade-offs are necessary.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Layout {
    /// The direction of the layout.
    pub direction: Direction,
    /// The constraints applied to the layout's segments.
    pub constraints: Vec<Constraint>,
    /// The padding applied to the outer area before solving.
    pub padding: Padding,
    /// The gap between adjacent segments, measured in cells. A negative
    /// value causes segments to overlap by that many cells.
    pub spacing: i64,
    /// The flex strategy for distributing leftover space.
    pub flex: Flex,
}

impl Layout {
    /// WithDirection returns a shallow copy of the layout using the specified
    /// direction.
    pub fn with_direction(mut self, direction: Direction) -> Layout {
        self.direction = direction;
        self
    }

    /// WithPadding returns a shallow copy of the layout using the specified
    /// padding.
    pub fn with_padding(mut self, padding: Padding) -> Layout {
        self.padding = padding;
        self
    }

    /// WithFlex returns a shallow copy of the layout using the specified flex
    /// strategy.
    pub fn with_flex(mut self, flex: Flex) -> Layout {
        self.flex = flex;
        self
    }

    /// WithSpacing returns a shallow copy of the layout using the specified
    /// spacing value.
    pub fn with_spacing(mut self, spacing: i64) -> Layout {
        self.spacing = spacing;
        self
    }

    /// WithConstraints returns a shallow copy of the layout with the given
    /// constraints appended to its existing list.
    pub fn with_constraints(mut self, constraints: &[Constraint]) -> Layout {
        self.constraints.extend_from_slice(constraints);
        self
    }

    /// SplitWithSpacers divides the given area into content segments and the
    /// gaps (spacers) between them. It returns both slices; use
    /// [Layout::split] if you only need the content rectangles.
    ///
    /// Panics when the solver cannot satisfy the constraints.
    pub fn split_with_spacers(&self, area: Rectangle) -> (Splitted, Splitted) {
        match self.split_cached(area) {
            Ok((segments, spacers)) => (Splitted::from(segments), Splitted::from(spacers)),
            Err(err) => panic!("{err}"),
        }
    }

    /// Split partitions the area into content rectangles according to the
    /// layout's direction and constraints.
    ///
    /// Because every constraint is evaluated against the total area, mixing
    /// relative constraints (Percent, Ratio) with absolute ones (Min, Max,
    /// Len) can produce ambiguous results. For example, splitting 100 cells
    /// as [Min(20), Percent(50), Percent(50)] will not necessarily yield
    /// [20, 40, 40].
    ///
    /// Panics when the solver cannot satisfy the constraints.
    pub fn split(&self, area: Rectangle) -> Splitted {
        let (segments, _) = self.split_with_spacers(area);
        segments
    }

    fn split_cached(&self, area: Rectangle) -> Result<(Vec<Rectangle>, Vec<Rectangle>), String> {
        let mut cache = global_cache().lock().unwrap();

        let key = self.cache_key(area);

        if let Some(v) = cache.get(&key) {
            return Ok((v.segments.clone(), v.spacers.clone()));
        }

        let (segments, spacers) = self.split_inner(area)?;

        cache.add(
            key,
            CacheValue {
                segments: segments.clone(),
                spacers: spacers.clone(),
            },
        );

        Ok((segments, spacers))
    }

    fn split_inner(&self, area: Rectangle) -> Result<(Vec<Rectangle>, Vec<Rectangle>), String> {
        let mut s = new_solver();

        let inner_area = self.padding.apply(area);

        let (area_start, area_end) = match self.direction {
            Direction::DirectionHorizontal => (
                inner_area.min.0 as f64 * FLOAT_PRECISION_MULTIPLIER,
                inner_area.max.0 as f64 * FLOAT_PRECISION_MULTIPLIER,
            ),
            Direction::DirectionVertical => (
                inner_area.min.1 as f64 * FLOAT_PRECISION_MULTIPLIER,
                inner_area.max.1 as f64 * FLOAT_PRECISION_MULTIPLIER,
            ),
        };

        let variable_count = self.constraints.len() * 2 + 2;

        let variables: Vec<Symbol> = (0..variable_count).map(|_| Symbol::new()).collect();

        let spacer_elements = new_elements(&variables);
        let segment_elements = new_elements(&variables[1..]);

        let spacing = self.spacing;

        let area_el = Element {
            start: variables[0],
            end: variables[variables.len() - 1],
        };

        configure_area(&mut s, area_el, area_start, area_end)
            .map_err(|e| format!("configure area: {e}"))?;
        configure_variable_in_area_constraints(&mut s, &variables, area_el)
            .map_err(|e| format!("configure variable in area constraints: {e}"))?;
        configure_variable_constraints(&mut s, &variables)
            .map_err(|e| format!("configure variable constraints: {e}"))?;
        configure_flex_constraints(&mut s, area_el, &spacer_elements, self.flex, spacing)
            .map_err(|e| format!("configure flex constraints: {e}"))?;
        configure_constraints(
            &mut s,
            area_el,
            &segment_elements,
            &self.constraints,
            self.flex,
        )
        .map_err(|e| format!("configure constraints: {e}"))?;
        configure_fill_constraints(&mut s, &segment_elements, &self.constraints, self.flex)
            .map_err(|e| format!("configure fill constraints: {e}"))?;

        if self.flex != Flex::FlexLegacy {
            for i in 0..segment_elements.len().saturating_sub(1) {
                let left = segment_elements[i];
                let right = segment_elements[i + 1];
                s.add(ALL_SEGMENT_GROW, left.size_eq_size(right))
                    .map_err(|e| format!("add has size constraint: {e}"))?;
            }
        }

        let mut changes: HashMap<Symbol, f64> = HashMap::with_capacity(variable_count);
        for v in &variables {
            changes.insert(*v, s.val(*v));
        }

        let segments = changes_to_rects(&changes, &segment_elements, inner_area, self.direction);
        let spacers = changes_to_rects(&changes, &spacer_elements, inner_area, self.direction);

        Ok((segments, spacers))
    }

    fn cache_key(&self, area: Rectangle) -> CacheKey {
        let mut h = Fnv64a::new();
        for c in &self.constraints {
            c.hash(&mut h);
        }
        CacheKey {
            area_min: area.min,
            area_max: area.max,
            direction: self.direction,
            constraints_hash: h.sum(),
            padding: self.padding,
            spacing: self.spacing,
            flex: self.flex,
        }
    }
}

/// The FNV-1a 64-bit hash function used for cache keys, mirroring Go's
/// `hash/fnv` (64a) used by the upstream cache key.
#[derive(Debug, Clone, Copy)]
struct Fnv64a(u64);

impl Fnv64a {
    const OFFSET_BASIS: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;

    fn new() -> Self {
        Fnv64a(Self::OFFSET_BASIS)
    }

    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.0 ^= b as u64;
            self.0 = self.0.wrapping_mul(Self::PRIME);
        }
    }

    fn sum(&self) -> u64 {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CacheKey {
    area_min: (usize, usize),
    area_max: (usize, usize),
    direction: Direction,
    constraints_hash: u64,
    padding: Padding,
    spacing: i64,
    flex: Flex,
}

#[derive(Debug, Clone)]
struct CacheValue {
    segments: Vec<Rectangle>,
    spacers: Vec<Rectangle>,
}

static GLOBAL_CACHE: OnceLock<Mutex<Lru<CacheKey, CacheValue>>> = OnceLock::new();

fn global_cache() -> &'static Mutex<Lru<CacheKey, CacheValue>> {
    GLOBAL_CACHE.get_or_init(|| Mutex::new(new_lru(GLOBAL_CACHE_SIZE)))
}

fn changes_to_rects(
    changes: &HashMap<Symbol, f64>,
    elements: &[Element],
    area: Rectangle,
    direction: Direction,
) -> Vec<Rectangle> {
    let mut rects = Vec::with_capacity(elements.len());

    for e in elements {
        let start_val = changes[&e.start];
        let end_val = changes[&e.end];

        let start_rounded = (start_val.round() / FLOAT_PRECISION_MULTIPLIER)
            .round()
            .max(0.0) as usize;
        let end_rounded = (end_val.round() / FLOAT_PRECISION_MULTIPLIER)
            .round()
            .max(0.0) as usize;

        let size = end_rounded.saturating_sub(start_rounded);

        match direction {
            Direction::DirectionHorizontal => {
                rects.push(Rectangle {
                    min: (start_rounded, area.min.1),
                    max: (start_rounded + size, area.max.1),
                });
            }
            Direction::DirectionVertical => {
                rects.push(Rectangle {
                    min: (area.min.0, start_rounded),
                    max: (area.max.0, start_rounded + size),
                });
            }
        }
    }

    rects
}

/// configureFillConstraints ensures that every [Constraint::Fill] (and, outside legacy
/// mode, every [Constraint::Min]) segment grows proportionally to its scaling factor,
/// so that remaining space is shared according to the declared weights.
///
/// ```text
/// [Fill(1), Fill(2)]
/// ┌──────┐┌────────────┐
/// │abcdef││abcdefabcdef│
/// └──────┘└────────────┘
/// ```
fn configure_fill_constraints(
    s: &mut Solver,
    segments: &[Element],
    constraints: &[Constraint],
    flex: Flex,
) -> Result<(), String> {
    let mut valid_constraints: Vec<Constraint> = Vec::new();
    let mut valid_segments: Vec<Element> = Vec::new();

    for i in 0..constraints.len().min(segments.len()) {
        let c = constraints[i];
        let seg = segments[i];

        match c {
            Constraint::Fill(_) => {
                valid_constraints.push(c);
                valid_segments.push(seg);
            }
            Constraint::Min(_) if flex != Flex::FlexLegacy => {
                valid_constraints.push(c);
                valid_segments.push(seg);
            }
            _ => {}
        }
    }

    for indices in combinations(valid_constraints.len(), 2) {
        let (i, j) = (indices[0], indices[1]);

        let left_constraint = valid_constraints[i];
        let left_segment = valid_segments[i];
        let right_constraint = valid_constraints[j];
        let right_segment = valid_segments[j];

        let left_scaling_factor = left_constraint.scaling_factor();
        let right_scaling_factor = right_constraint.scaling_factor();

        let c = CConstraint::new_constraint(
            Op::EQ,
            0.0,
            &[
                left_segment.end.t(right_scaling_factor),
                left_segment.start.t(-right_scaling_factor),
                right_segment.end.t(-left_scaling_factor),
                right_segment.start.t(left_scaling_factor),
            ],
        );

        s.add(GROW, c).map_err(|e| format!("add constraint: {e}"))?;
    }

    Ok(())
}

fn configure_constraints(
    s: &mut Solver,
    area: Element,
    segments: &[Element],
    constraints: &[Constraint],
    flex: Flex,
) -> Result<(), String> {
    for i in 0..constraints.len().min(segments.len()) {
        let constraint = constraints[i];
        let segment = segments[i];

        match constraint {
            Constraint::Max(size) => {
                s.add(MAX_SIZE_LTE, segment.size_lte(size))
                    .map_err(|e| format!("add constraints: {e}"))?;
                s.add(MAX_SIZE_EQ, segment.size_eq_const(size))
                    .map_err(|e| format!("add constraints: {e}"))?;
            }
            Constraint::Min(size) => {
                s.add(MIN_SIZE_GTE, segment.size_gte(size))
                    .map_err(|e| format!("add has min size constraint: {e}"))?;
                if flex == Flex::FlexLegacy {
                    s.add(MIN_SIZE_EQ, segment.size_eq_const(size))
                        .map_err(|e| format!("add has size constraint: {e}"))?;
                } else {
                    s.add(FILL_GROW, segment.size_eq_size(area))
                        .map_err(|e| format!("add has size constraint: {e}"))?;
                }
            }
            Constraint::Len(length) => {
                s.add(LENGTH_SIZE_EQ, segment.size_eq_const(length))
                    .map_err(|e| format!("add has int size constraint: {e}"))?;
            }
            Constraint::Percent(p) => {
                let f = p as f64 / 100.0;
                s.add(PERCENT_SIZE_EQ, segment.size_eq_scaled_size(area, f))
                    .map_err(|e| format!("add has size constraint: {e}"))?;
            }
            Constraint::Ratio { num, den } => {
                let f = num as f64 / den.max(1) as f64;
                s.add(RATIO_SIZE_EQ, segment.size_eq_scaled_size(area, f))
                    .map_err(|e| format!("add has size constraint: {e}"))?;
            }
            Constraint::Fill(_) => {
                s.add(FILL_GROW, segment.size_eq_size(area))
                    .map_err(|e| format!("add has size constraint: {e}"))?;
            }
        }
    }

    Ok(())
}

fn configure_flex_constraints(
    s: &mut Solver,
    area: Element,
    spacers: &[Element],
    flex: Flex,
    spacing: i64,
) -> Result<(), String> {
    let spacers_except_first_and_last: Vec<Element> = if spacers.len() > 2 {
        spacers[1..spacers.len() - 1].to_vec()
    } else {
        Vec::new()
    };

    match flex {
        Flex::FlexLegacy => {
            for sp in &spacers_except_first_and_last {
                s.add(SPACER_SIZE_EQ, sp.size_eq_const(spacing))
                    .map_err(|e| format!("add has size constraint: {e}"))?;
            }

            if spacers.len() >= 2 {
                let first = spacers[0];
                let last = spacers[spacers.len() - 1];

                s.add(REQUIRED - WEAK, first.empty())
                    .map_err(|e| format!("add constraints: {e}"))?;
                s.add(REQUIRED - WEAK, last.empty())
                    .map_err(|e| format!("add constraints: {e}"))?;
            }
        }

        Flex::FlexSpaceEvenly => {
            for indices in combinations(spacers.len(), 2) {
                let (i, j) = (indices[0], indices[1]);
                let left = spacers[i];
                let right = spacers[j];
                s.add(SPACER_SIZE_EQ, left.size_eq_size(right))
                    .map_err(|e| format!("add has size constraint: {e}"))?;
            }

            for sp in spacers {
                s.add(SPACER_SIZE_EQ, sp.size_gte(spacing))
                    .map_err(|e| format!("add constraints: {e}"))?;
                s.add(SPACE_GROW, sp.size_eq_size(area))
                    .map_err(|e| format!("add constraints: {e}"))?;
            }
        }

        Flex::FlexSpaceAround => {
            if spacers.len() <= 2 {
                for indices in combinations(spacers.len(), 2) {
                    let (i, j) = (indices[0], indices[1]);
                    let left = spacers[i];
                    let right = spacers[j];
                    s.add(SPACER_SIZE_EQ, left.size_eq_size(right))
                        .map_err(|e| format!("add has size constraint: {e}"))?;
                }

                for sp in spacers {
                    s.add(SPACER_SIZE_EQ, sp.size_gte(spacing))
                        .map_err(|e| format!("add constraints: {e}"))?;
                    s.add(SPACE_GROW, sp.size_eq_size(area))
                        .map_err(|e| format!("add constraints: {e}"))?;
                }
            } else {
                let first = spacers[0];
                let rest = &spacers[1..];
                let last = rest[rest.len() - 1];
                let middle = &rest[..rest.len() - 1];

                for indices in combinations(middle.len(), 2) {
                    let (i, j) = (indices[0], indices[1]);
                    let left = middle[i];
                    let right = middle[j];
                    s.add(SPACER_SIZE_EQ, left.size_eq_size(right))
                        .map_err(|e| format!("add has size constraint: {e}"))?;
                }

                if !middle.is_empty() {
                    let first_middle = middle[0];
                    for e in [first, last] {
                        s.add(SPACER_SIZE_EQ, first_middle.size_eq_double(e))
                            .map_err(|e| format!("add has double size constraint: {e}"))?;
                    }
                }

                for sp in spacers {
                    s.add(SPACER_SIZE_EQ, sp.size_gte(spacing))
                        .map_err(|e| format!("add has min size constraint: {e}"))?;
                    s.add(SPACE_GROW, sp.size_eq_size(area))
                        .map_err(|e| format!("add has size constraint: {e}"))?;
                }
            }
        }

        Flex::FlexSpaceBetween => {
            for indices in combinations(spacers_except_first_and_last.len(), 2) {
                let (i, j) = (indices[0], indices[1]);
                let left = spacers_except_first_and_last[i];
                let right = spacers_except_first_and_last[j];
                s.add(SPACER_SIZE_EQ, left.size_eq_size(right))
                    .map_err(|e| format!("add has size constraint: {e}"))?;
            }

            for sp in &spacers_except_first_and_last {
                s.add(SPACER_SIZE_EQ, sp.size_gte(spacing))
                    .map_err(|e| format!("add constraints: {e}"))?;
                s.add(SPACE_GROW, sp.size_eq_size(area))
                    .map_err(|e| format!("add constraints: {e}"))?;
            }

            if spacers.len() >= 2 {
                let first = spacers[0];
                let last = spacers[spacers.len() - 1];

                s.add(REQUIRED - WEAK, first.empty())
                    .map_err(|e| format!("add constraints: {e}"))?;
                s.add(REQUIRED - WEAK, last.empty())
                    .map_err(|e| format!("add constraints: {e}"))?;
            }
        }

        Flex::FlexStart => {
            for sp in &spacers_except_first_and_last {
                s.add(SPACER_SIZE_EQ, sp.size_eq_const(spacing))
                    .map_err(|e| format!("add has size constraint: {e}"))?;
            }

            if spacers.len() >= 2 {
                let first = spacers[0];
                let last = spacers[spacers.len() - 1];

                s.add(REQUIRED - WEAK, first.empty())
                    .map_err(|e| format!("add constraints: {e}"))?;
                s.add(GROW, last.size_eq_size(area))
                    .map_err(|e| format!("add constraints: {e}"))?;
            }
        }

        Flex::FlexCenter => {
            for sp in &spacers_except_first_and_last {
                s.add(SPACER_SIZE_EQ, sp.size_eq_const(spacing))
                    .map_err(|e| format!("add has size constraint: {e}"))?;
            }

            if spacers.len() >= 2 {
                let first = spacers[0];
                let last = spacers[spacers.len() - 1];

                s.add(GROW, first.size_eq_size(area))
                    .map_err(|e| format!("add constraints: {e}"))?;
                s.add(GROW, last.size_eq_size(area))
                    .map_err(|e| format!("add constraints: {e}"))?;
                s.add(SPACER_SIZE_EQ, first.size_eq_size(last))
                    .map_err(|e| format!("add constraints: {e}"))?;
            }
        }

        Flex::FlexEnd => {
            for sp in &spacers_except_first_and_last {
                s.add(SPACER_SIZE_EQ, sp.size_eq_const(spacing))
                    .map_err(|e| format!("add has size constraint: {e}"))?;
            }

            if spacers.len() >= 2 {
                let first = spacers[0];
                let last = spacers[spacers.len() - 1];

                s.add(REQUIRED - WEAK, last.empty())
                    .map_err(|e| format!("add constraints: {e}"))?;
                s.add(GROW, first.size_eq_size(area))
                    .map_err(|e| format!("add constraints: {e}"))?;
            }
        }
    }

    Ok(())
}

fn configure_variable_constraints(s: &mut Solver, variables: &[Symbol]) -> Result<(), String> {
    let variables = &variables[1..];
    let count = variables.len();

    let mut i = 0;
    while i < count - count % 2 {
        let left = variables[i];
        let right = variables[i + 1];

        s.add(
            REQUIRED,
            CConstraint::new_constraint(Op::LTE, 0.0, &[left.t(1.0), right.t(-1.0)]),
        )
        .map_err(|e| format!("add constraint: {e}"))?;

        i += 2;
    }

    Ok(())
}

fn configure_variable_in_area_constraints(
    s: &mut Solver,
    variables: &[Symbol],
    area: Element,
) -> Result<(), String> {
    for v in variables {
        s.add(
            REQUIRED,
            CConstraint::new_constraint(Op::GTE, 0.0, &[v.t(1.0), area.start.t(-1.0)]),
        )
        .map_err(|e| format!("add start constraint: {e}"))?;
        s.add(
            REQUIRED,
            CConstraint::new_constraint(Op::LTE, 0.0, &[v.t(1.0), area.end.t(-1.0)]),
        )
        .map_err(|e| format!("add end constraint: {e}"))?;
    }

    Ok(())
}

fn configure_area(
    s: &mut Solver,
    area: Element,
    area_start: f64,
    area_end: f64,
) -> Result<(), String> {
    s.add(
        REQUIRED,
        CConstraint::new_constraint(Op::EQ, -area_start, &[area.start.t(1.0)]),
    )
    .map_err(|e| format!("add start constraint: {e}"))?;
    s.add(
        REQUIRED,
        CConstraint::new_constraint(Op::EQ, -area_end, &[area.end.t(1.0)]),
    )
    .map_err(|e| format!("add end constraint: {e}"))?;

    Ok(())
}

fn new_elements(variables: &[Symbol]) -> Vec<Element> {
    let count = variables.len();

    let mut elements = Vec::with_capacity(count / 2 + 1);

    let mut i = 0;
    while i < count - count % 2 {
        let (s, e) = (variables[i], variables[i + 1]);
        elements.push(Element { start: s, end: e });
        i += 2;
    }

    elements
}

/// An element is a span between two solver symbols: a start and an end.
#[derive(Debug, Clone, Copy)]
struct Element {
    start: Symbol,
    end: Symbol,
}

impl Element {
    fn empty(&self) -> CConstraint {
        CConstraint::new_constraint(Op::EQ, 0.0, &[self.end.t(1.0), self.start.t(-1.0)])
    }

    fn size_eq_const(&self, size: i64) -> CConstraint {
        CConstraint::new_constraint(
            Op::EQ,
            -(size as f64) * FLOAT_PRECISION_MULTIPLIER,
            &[self.end.t(1.0), self.start.t(-1.0)],
        )
    }

    fn size_lte(&self, size: i64) -> CConstraint {
        CConstraint::new_constraint(
            Op::LTE,
            -(size as f64) * FLOAT_PRECISION_MULTIPLIER,
            &[self.end.t(1.0), self.start.t(-1.0)],
        )
    }

    fn size_gte(&self, size: i64) -> CConstraint {
        CConstraint::new_constraint(
            Op::GTE,
            -(size as f64) * FLOAT_PRECISION_MULTIPLIER,
            &[self.end.t(1.0), self.start.t(-1.0)],
        )
    }

    fn size_eq_size(&self, other: Element) -> CConstraint {
        CConstraint::new_constraint(
            Op::EQ,
            0.0,
            &[
                self.end.t(1.0),
                self.start.t(-1.0),
                other.end.t(-1.0),
                other.start.t(1.0),
            ],
        )
    }

    fn size_eq_scaled_size(&self, other: Element, f: f64) -> CConstraint {
        CConstraint::new_constraint(
            Op::EQ,
            0.0,
            &[
                self.end.t(1.0),
                self.start.t(-1.0),
                other.end.t(-f),
                other.start.t(f),
            ],
        )
    }

    fn size_eq_double(&self, other: Element) -> CConstraint {
        CConstraint::new_constraint(
            Op::EQ,
            0.0,
            &[
                self.end.t(1.0),
                self.start.t(-1.0),
                other.end.t(-2.0),
                other.start.t(2.0),
            ],
        )
    }
}

/// Returns all combinations of k elements chosen from n, in lexicographic
/// order.
fn combinations(n: usize, k: usize) -> Vec<Vec<usize>> {
    let combs = binomial(n, k);
    let mut data: Vec<Vec<usize>> = Vec::with_capacity(combs);
    if combs == 0 {
        return data;
    }

    data.push((0..k).collect());
    for _ in 1..combs {
        let mut next = data[data.len() - 1].clone();
        next_combination(&mut next, n, k);
        data.push(next);
    }

    data
}

fn next_combination(s: &mut [usize], n: usize, k: usize) {
    for j in (0..k).rev() {
        if s[j] == n + j - k {
            continue;
        }
        s[j] += 1;
        for l in j + 1..k {
            s[l] = s[j] + l - j;
        }
        break;
    }
}

fn binomial(n: usize, k: usize) -> usize {
    if n < k {
        return 0;
    }

    // (n,k) = (n, n-k)
    let mut k = k;
    if k > n / 2 {
        k = n - k;
    }

    let mut b: usize = 1;
    for i in 1..=k {
        b = (n - k + i) * b / i;
    }

    b
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A constraint-position case: (flex, constraints, expected positions).
    type PositionCase = Vec<(Flex, Vec<Constraint>, Vec<(usize, usize)>)>;
    /// A spacing case: (flex, spacing, expected positions).
    type SpacingCase = Vec<(Flex, i64, Vec<(usize, usize)>)>;

    /// Mirrors upstream `layout_test.go` `paintLayout` (an index-based
    /// rasterizer, so the range loop is intentional).
    #[allow(clippy::needless_range_loop)]
    fn letters(flex: Flex, constraints: &[Constraint], width: usize) -> String {
        let area = crate::window::rect(0, 0, width as i64, 1);

        let layout = Layout {
            direction: Direction::DirectionHorizontal,
            constraints: constraints.to_vec(),
            flex,
            ..Layout::default()
        }
        .split(area);

        let mut got = vec![' '; width];
        let latin = "abcdefghijklmnopqrstuvwxyz";

        for i in 0..constraints.len().min(layout.len()) {
            let c = latin.as_bytes()[i] as char;
            let a = layout[i];
            for x in a.min.0..a.max.0 {
                if x < width {
                    got[x] = c;
                }
            }
        }

        got.into_iter().collect()
    }

    #[test]
    fn test_priority_is_valid() {
        // Ported from upstream `layout_test.go` `TestPriorityIsValid`; the
        // ordering is compile-time constant in Rust but kept to mirror the
        // upstream test.
        #[allow(clippy::assertions_on_constants)]
        {
            assert!(SPACER_SIZE_EQ > MAX_SIZE_LTE);
            assert!(MAX_SIZE_LTE > MAX_SIZE_EQ);
            assert!((MIN_SIZE_GTE - MAX_SIZE_LTE).abs() < f64::EPSILON);
            assert!(MAX_SIZE_LTE > LENGTH_SIZE_EQ);
            assert!(LENGTH_SIZE_EQ > PERCENT_SIZE_EQ);
            assert!(PERCENT_SIZE_EQ > RATIO_SIZE_EQ);
            assert!(RATIO_SIZE_EQ > MAX_SIZE_EQ);
            assert!(MIN_SIZE_GTE > FILL_GROW);
            assert!(FILL_GROW > GROW);
            assert!(GROW > SPACE_GROW);
            assert!(SPACE_GROW > ALL_SEGMENT_GROW);
        }
    }

    #[test]
    fn test_length() {
        let cases: Vec<(Flex, Vec<Constraint>, usize, &str)> = vec![
            (Flex::FlexLegacy, vec![Constraint::Len(0)], 1, "a"),
            (Flex::FlexLegacy, vec![Constraint::Len(1)], 1, "a"),
            (Flex::FlexLegacy, vec![Constraint::Len(2)], 1, "a"),
            (Flex::FlexLegacy, vec![Constraint::Len(0)], 2, "aa"),
            (Flex::FlexLegacy, vec![Constraint::Len(3)], 2, "aa"),
            (
                Flex::FlexLegacy,
                vec![Constraint::Len(1), Constraint::Len(0)],
                2,
                "ab",
            ),
            (
                Flex::FlexLegacy,
                vec![Constraint::Len(1), Constraint::Len(1)],
                2,
                "ab",
            ),
            (
                Flex::FlexLegacy,
                vec![Constraint::Len(2), Constraint::Len(2)],
                2,
                "aa",
            ),
            (
                Flex::FlexLegacy,
                vec![Constraint::Len(3), Constraint::Len(3)],
                2,
                "aa",
            ),
            (
                Flex::FlexLegacy,
                vec![Constraint::Len(2), Constraint::Len(2)],
                3,
                "aab",
            ),
        ];
        for (flex, constraints, width, want) in cases {
            let got = letters(flex, &constraints, width);
            assert_eq!(
                got, want,
                "flex={flex} constraints={constraints:?} width={width}"
            );
        }
    }

    #[test]
    fn test_percent() {
        let cases: Vec<(Flex, Vec<Constraint>, usize, &str)> = vec![
            (
                Flex::FlexStart,
                vec![Constraint::Percent(0), Constraint::Percent(0)],
                10,
                "          ",
            ),
            (
                Flex::FlexStart,
                vec![Constraint::Percent(0), Constraint::Percent(25)],
                10,
                "bbb       ",
            ),
            (
                Flex::FlexStart,
                vec![Constraint::Percent(0), Constraint::Percent(50)],
                10,
                "bbbbb     ",
            ),
            (
                Flex::FlexStart,
                vec![Constraint::Percent(0), Constraint::Percent(100)],
                10,
                "bbbbbbbbbb",
            ),
            (
                Flex::FlexStart,
                vec![Constraint::Percent(10), Constraint::Percent(0)],
                10,
                "a         ",
            ),
            (
                Flex::FlexStart,
                vec![Constraint::Percent(10), Constraint::Percent(25)],
                10,
                "abbb      ",
            ),
            (
                Flex::FlexStart,
                vec![Constraint::Percent(10), Constraint::Percent(50)],
                10,
                "abbbbb    ",
            ),
            (
                Flex::FlexStart,
                vec![Constraint::Percent(10), Constraint::Percent(100)],
                10,
                "abbbbbbbbb",
            ),
            (
                Flex::FlexStart,
                vec![Constraint::Percent(25), Constraint::Percent(0)],
                10,
                "aaa       ",
            ),
            (
                Flex::FlexStart,
                vec![Constraint::Percent(25), Constraint::Percent(25)],
                10,
                "aaabb     ",
            ),
            (
                Flex::FlexStart,
                vec![Constraint::Percent(25), Constraint::Percent(50)],
                10,
                "aaabbbbb  ",
            ),
            (
                Flex::FlexStart,
                vec![Constraint::Percent(25), Constraint::Percent(100)],
                10,
                "aaabbbbbbb",
            ),
            (
                Flex::FlexStart,
                vec![Constraint::Percent(50), Constraint::Percent(0)],
                10,
                "aaaaa     ",
            ),
            (
                Flex::FlexStart,
                vec![Constraint::Percent(50), Constraint::Percent(50)],
                10,
                "aaaaabbbbb",
            ),
            (
                Flex::FlexStart,
                vec![Constraint::Percent(100), Constraint::Percent(0)],
                10,
                "aaaaaaaaaa",
            ),
            (
                Flex::FlexStart,
                vec![Constraint::Percent(100), Constraint::Percent(50)],
                10,
                "aaaaabbbbb",
            ),
            (
                Flex::FlexSpaceBetween,
                vec![Constraint::Percent(0), Constraint::Percent(0)],
                10,
                "          ",
            ),
            (
                Flex::FlexSpaceBetween,
                vec![Constraint::Percent(0), Constraint::Percent(25)],
                10,
                "        bb",
            ),
            (
                Flex::FlexSpaceBetween,
                vec![Constraint::Percent(0), Constraint::Percent(50)],
                10,
                "     bbbbb",
            ),
            (
                Flex::FlexSpaceBetween,
                vec![Constraint::Percent(0), Constraint::Percent(100)],
                10,
                "bbbbbbbbbb",
            ),
            (
                Flex::FlexSpaceBetween,
                vec![Constraint::Percent(10), Constraint::Percent(0)],
                10,
                "a         ",
            ),
            (
                Flex::FlexSpaceBetween,
                vec![Constraint::Percent(10), Constraint::Percent(25)],
                10,
                "a       bb",
            ),
            (
                Flex::FlexSpaceBetween,
                vec![Constraint::Percent(10), Constraint::Percent(50)],
                10,
                "a    bbbbb",
            ),
            (
                Flex::FlexSpaceBetween,
                vec![Constraint::Percent(10), Constraint::Percent(100)],
                10,
                "abbbbbbbbb",
            ),
            (
                Flex::FlexSpaceBetween,
                vec![Constraint::Percent(25), Constraint::Percent(0)],
                10,
                "aaa       ",
            ),
            (
                Flex::FlexSpaceBetween,
                vec![Constraint::Percent(25), Constraint::Percent(25)],
                10,
                "aaa     bb",
            ),
            (
                Flex::FlexSpaceBetween,
                vec![Constraint::Percent(25), Constraint::Percent(50)],
                10,
                "aaa  bbbbb",
            ),
            (
                Flex::FlexSpaceBetween,
                vec![Constraint::Percent(25), Constraint::Percent(100)],
                10,
                "aaabbbbbbb",
            ),
            (
                Flex::FlexSpaceBetween,
                vec![Constraint::Percent(50), Constraint::Percent(0)],
                10,
                "aaaaa     ",
            ),
            (
                Flex::FlexSpaceBetween,
                vec![Constraint::Percent(50), Constraint::Percent(50)],
                10,
                "aaaaabbbbb",
            ),
            (
                Flex::FlexSpaceBetween,
                vec![Constraint::Percent(100), Constraint::Percent(0)],
                10,
                "aaaaaaaaaa",
            ),
            (
                Flex::FlexSpaceBetween,
                vec![Constraint::Percent(100), Constraint::Percent(50)],
                10,
                "aaaaabbbbb",
            ),
        ];
        for (flex, constraints, width, want) in cases {
            let got = letters(flex, &constraints, width);
            assert_eq!(
                got, want,
                "flex={flex} constraints={constraints:?} width={width}"
            );
        }
    }

    #[test]
    fn test_ratio() {
        let cases: Vec<(Flex, Vec<Constraint>, usize, &str)> = vec![
            (
                Flex::FlexLegacy,
                vec![Constraint::Ratio { num: 0, den: 1 }],
                1,
                "a",
            ),
            (
                Flex::FlexLegacy,
                vec![Constraint::Ratio { num: 0, den: 1 }],
                2,
                "aa",
            ),
            (
                Flex::FlexLegacy,
                vec![
                    Constraint::Ratio { num: 0, den: 1 },
                    Constraint::Ratio { num: 0, den: 1 },
                ],
                10,
                "bbbbbbbbbb",
            ),
            (
                Flex::FlexLegacy,
                vec![
                    Constraint::Ratio { num: 1, den: 10 },
                    Constraint::Ratio { num: 0, den: 1 },
                ],
                10,
                "abbbbbbbbb",
            ),
            (
                Flex::FlexLegacy,
                vec![
                    Constraint::Ratio { num: 1, den: 4 },
                    Constraint::Ratio { num: 0, den: 1 },
                ],
                10,
                "aaabbbbbbb",
            ),
            (
                Flex::FlexLegacy,
                vec![
                    Constraint::Ratio { num: 1, den: 2 },
                    Constraint::Ratio { num: 0, den: 1 },
                ],
                10,
                "aaaaabbbbb",
            ),
            (
                Flex::FlexLegacy,
                vec![
                    Constraint::Ratio { num: 1, den: 1 },
                    Constraint::Ratio { num: 0, den: 1 },
                ],
                10,
                "aaaaaaaaaa",
            ),
            (
                Flex::FlexStart,
                vec![
                    Constraint::Ratio { num: 0, den: 1 },
                    Constraint::Ratio { num: 0, den: 1 },
                ],
                10,
                "          ",
            ),
            (
                Flex::FlexStart,
                vec![
                    Constraint::Ratio { num: 0, den: 1 },
                    Constraint::Ratio { num: 1, den: 4 },
                ],
                10,
                "bbb       ",
            ),
            (
                Flex::FlexStart,
                vec![
                    Constraint::Ratio { num: 0, den: 1 },
                    Constraint::Ratio { num: 1, den: 2 },
                ],
                10,
                "bbbbb     ",
            ),
            (
                Flex::FlexStart,
                vec![
                    Constraint::Ratio { num: 0, den: 1 },
                    Constraint::Ratio { num: 1, den: 1 },
                ],
                10,
                "bbbbbbbbbb",
            ),
            (
                Flex::FlexStart,
                vec![
                    Constraint::Ratio { num: 1, den: 10 },
                    Constraint::Ratio { num: 0, den: 1 },
                ],
                10,
                "a         ",
            ),
            (
                Flex::FlexStart,
                vec![
                    Constraint::Ratio { num: 1, den: 10 },
                    Constraint::Ratio { num: 1, den: 4 },
                ],
                10,
                "abbb      ",
            ),
            (
                Flex::FlexStart,
                vec![
                    Constraint::Ratio { num: 1, den: 10 },
                    Constraint::Ratio { num: 1, den: 2 },
                ],
                10,
                "abbbbb    ",
            ),
            (
                Flex::FlexStart,
                vec![
                    Constraint::Ratio { num: 1, den: 10 },
                    Constraint::Ratio { num: 1, den: 1 },
                ],
                10,
                "abbbbbbbbb",
            ),
            (
                Flex::FlexStart,
                vec![
                    Constraint::Ratio { num: 1, den: 4 },
                    Constraint::Ratio { num: 0, den: 1 },
                ],
                10,
                "aaa       ",
            ),
            (
                Flex::FlexStart,
                vec![
                    Constraint::Ratio { num: 1, den: 4 },
                    Constraint::Ratio { num: 1, den: 4 },
                ],
                10,
                "aaabb     ",
            ),
            (
                Flex::FlexStart,
                vec![
                    Constraint::Ratio { num: 1, den: 4 },
                    Constraint::Ratio { num: 1, den: 2 },
                ],
                10,
                "aaabbbbb  ",
            ),
            (
                Flex::FlexStart,
                vec![
                    Constraint::Ratio { num: 1, den: 4 },
                    Constraint::Ratio { num: 1, den: 1 },
                ],
                10,
                "aaabbbbbbb",
            ),
            (
                Flex::FlexStart,
                vec![
                    Constraint::Ratio { num: 1, den: 2 },
                    Constraint::Ratio { num: 0, den: 1 },
                ],
                10,
                "aaaaa     ",
            ),
            (
                Flex::FlexStart,
                vec![
                    Constraint::Ratio { num: 1, den: 2 },
                    Constraint::Ratio { num: 1, den: 2 },
                ],
                10,
                "aaaaabbbbb",
            ),
            (
                Flex::FlexStart,
                vec![
                    Constraint::Ratio { num: 1, den: 1 },
                    Constraint::Ratio { num: 0, den: 1 },
                ],
                10,
                "aaaaaaaaaa",
            ),
        ];
        for (flex, constraints, width, want) in cases {
            let got = letters(flex, &constraints, width);
            assert_eq!(
                got, want,
                "flex={flex} constraints={constraints:?} width={width}"
            );
        }
    }

    #[test]
    fn test_min_max_len() {
        let cases: Vec<(Flex, Vec<Constraint>, usize, &str)> = vec![
            (
                Flex::FlexLegacy,
                vec![Constraint::Min(0), Constraint::Min(0)],
                1,
                "b",
            ),
            (
                Flex::FlexLegacy,
                vec![Constraint::Min(0), Constraint::Min(1)],
                1,
                "b",
            ),
            (
                Flex::FlexLegacy,
                vec![Constraint::Min(1), Constraint::Min(0)],
                1,
                "a",
            ),
            (
                Flex::FlexLegacy,
                vec![Constraint::Min(2), Constraint::Min(2)],
                2,
                "aa",
            ),
            (
                Flex::FlexLegacy,
                vec![Constraint::Min(2), Constraint::Min(2)],
                3,
                "aab",
            ),
            (
                Flex::FlexLegacy,
                vec![Constraint::Min(2), Constraint::Min(0)],
                2,
                "aa",
            ),
            (
                Flex::FlexStart,
                vec![Constraint::Fill(1), Constraint::Fill(1)],
                10,
                "aaaaabbbbb",
            ),
            (
                Flex::FlexStart,
                vec![Constraint::Fill(1), Constraint::Fill(2)],
                10,
                "aaabbbbbbb",
            ),
        ];
        for (flex, constraints, width, want) in cases {
            let got = letters(flex, &constraints, width);
            assert_eq!(
                got, want,
                "flex={flex} constraints={constraints:?} width={width}"
            );
        }
    }

    /// Splits a 100x1 area and returns each segment as an (start, end) x-range.
    fn x_ranges(flex: Flex, constraints: &[Constraint]) -> Vec<(usize, usize)> {
        let area = crate::window::rect(0, 0, 100, 1);
        let layout = Layout {
            direction: Direction::DirectionHorizontal,
            constraints: constraints.to_vec(),
            flex,
            ..Layout::default()
        }
        .split(area);
        layout
            .iter()
            .map(|r| (r.min.0, r.max.0))
            .collect::<Vec<_>>()
    }

    #[test]
    fn test_flex_constraint_positions() {
        let cases: PositionCase = vec![
            (Flex::FlexLegacy, vec![Constraint::Len(50)], vec![(0, 100)]),
            (Flex::FlexStart, vec![Constraint::Len(50)], vec![(0, 50)]),
            (Flex::FlexEnd, vec![Constraint::Len(50)], vec![(50, 100)]),
            (Flex::FlexCenter, vec![Constraint::Len(50)], vec![(25, 75)]),
            (
                Flex::FlexLegacy,
                vec![Constraint::Ratio { num: 1, den: 2 }],
                vec![(0, 100)],
            ),
            (
                Flex::FlexStart,
                vec![Constraint::Ratio { num: 1, den: 2 }],
                vec![(0, 50)],
            ),
            (
                Flex::FlexEnd,
                vec![Constraint::Ratio { num: 1, den: 2 }],
                vec![(50, 100)],
            ),
            (
                Flex::FlexCenter,
                vec![Constraint::Ratio { num: 1, den: 2 }],
                vec![(25, 75)],
            ),
            (
                Flex::FlexLegacy,
                vec![Constraint::Percent(50)],
                vec![(0, 100)],
            ),
            (
                Flex::FlexStart,
                vec![Constraint::Percent(50)],
                vec![(0, 50)],
            ),
            (
                Flex::FlexEnd,
                vec![Constraint::Percent(50)],
                vec![(50, 100)],
            ),
            (
                Flex::FlexCenter,
                vec![Constraint::Percent(50)],
                vec![(25, 75)],
            ),
            (Flex::FlexLegacy, vec![Constraint::Min(50)], vec![(0, 100)]),
            (Flex::FlexStart, vec![Constraint::Min(50)], vec![(0, 100)]),
            (Flex::FlexEnd, vec![Constraint::Min(50)], vec![(0, 100)]),
            (Flex::FlexCenter, vec![Constraint::Min(50)], vec![(0, 100)]),
            (Flex::FlexLegacy, vec![Constraint::Max(50)], vec![(0, 100)]),
            (Flex::FlexStart, vec![Constraint::Max(50)], vec![(0, 50)]),
            (Flex::FlexEnd, vec![Constraint::Max(50)], vec![(50, 100)]),
            (Flex::FlexCenter, vec![Constraint::Max(50)], vec![(25, 75)]),
            (
                Flex::FlexSpaceBetween,
                vec![Constraint::Min(1)],
                vec![(0, 100)],
            ),
            (
                Flex::FlexSpaceBetween,
                vec![Constraint::Max(20)],
                vec![(0, 100)],
            ),
            (
                Flex::FlexSpaceBetween,
                vec![Constraint::Len(20)],
                vec![(0, 100)],
            ),
            (
                Flex::FlexLegacy,
                vec![Constraint::Len(25), Constraint::Len(25)],
                vec![(0, 25), (25, 100)],
            ),
            (
                Flex::FlexStart,
                vec![Constraint::Len(25), Constraint::Len(25)],
                vec![(0, 25), (25, 50)],
            ),
            (
                Flex::FlexCenter,
                vec![Constraint::Len(25), Constraint::Len(25)],
                vec![(25, 50), (50, 75)],
            ),
            (
                Flex::FlexEnd,
                vec![Constraint::Len(25), Constraint::Len(25)],
                vec![(50, 75), (75, 100)],
            ),
            (
                Flex::FlexSpaceBetween,
                vec![Constraint::Len(25), Constraint::Len(25)],
                vec![(0, 25), (75, 100)],
            ),
            (
                Flex::FlexSpaceEvenly,
                vec![Constraint::Len(25), Constraint::Len(25)],
                vec![(17, 42), (58, 83)],
            ),
            (
                Flex::FlexSpaceAround,
                vec![Constraint::Len(25), Constraint::Len(25)],
                vec![(13, 38), (63, 88)],
            ),
            (
                Flex::FlexLegacy,
                vec![Constraint::Percent(25), Constraint::Percent(25)],
                vec![(0, 25), (25, 100)],
            ),
            (
                Flex::FlexStart,
                vec![Constraint::Percent(25), Constraint::Percent(25)],
                vec![(0, 25), (25, 50)],
            ),
            (
                Flex::FlexCenter,
                vec![Constraint::Percent(25), Constraint::Percent(25)],
                vec![(25, 50), (50, 75)],
            ),
            (
                Flex::FlexEnd,
                vec![Constraint::Percent(25), Constraint::Percent(25)],
                vec![(50, 75), (75, 100)],
            ),
            (
                Flex::FlexSpaceBetween,
                vec![Constraint::Percent(25), Constraint::Percent(25)],
                vec![(0, 25), (75, 100)],
            ),
            (
                Flex::FlexSpaceEvenly,
                vec![Constraint::Percent(25), Constraint::Percent(25)],
                vec![(17, 42), (58, 83)],
            ),
            (
                Flex::FlexSpaceAround,
                vec![Constraint::Percent(25), Constraint::Percent(25)],
                vec![(13, 38), (63, 88)],
            ),
            (
                Flex::FlexLegacy,
                vec![Constraint::Min(25), Constraint::Min(25)],
                vec![(0, 25), (25, 100)],
            ),
            (
                Flex::FlexStart,
                vec![Constraint::Min(25), Constraint::Min(25)],
                vec![(0, 50), (50, 100)],
            ),
            (
                Flex::FlexCenter,
                vec![Constraint::Min(25), Constraint::Min(25)],
                vec![(0, 50), (50, 100)],
            ),
            (
                Flex::FlexEnd,
                vec![Constraint::Min(25), Constraint::Min(25)],
                vec![(0, 50), (50, 100)],
            ),
            (
                Flex::FlexSpaceBetween,
                vec![Constraint::Min(25), Constraint::Min(25)],
                vec![(0, 50), (50, 100)],
            ),
            (
                Flex::FlexSpaceEvenly,
                vec![Constraint::Min(25), Constraint::Min(25)],
                vec![(0, 50), (50, 100)],
            ),
            (
                Flex::FlexSpaceAround,
                vec![Constraint::Min(25), Constraint::Min(25)],
                vec![(0, 50), (50, 100)],
            ),
            (
                Flex::FlexLegacy,
                vec![Constraint::Max(25), Constraint::Max(25)],
                vec![(0, 25), (25, 100)],
            ),
            (
                Flex::FlexStart,
                vec![Constraint::Max(25), Constraint::Max(25)],
                vec![(0, 25), (25, 50)],
            ),
            (Flex::FlexLegacy, vec![Constraint::Fill(1)], vec![(0, 100)]),
            (Flex::FlexStart, vec![Constraint::Fill(1)], vec![(0, 100)]),
            (Flex::FlexEnd, vec![Constraint::Fill(1)], vec![(0, 100)]),
            (Flex::FlexCenter, vec![Constraint::Fill(1)], vec![(0, 100)]),
            (
                Flex::FlexStart,
                vec![Constraint::Fill(1), Constraint::Fill(2)],
                vec![(0, 33), (33, 100)],
            ),
            (
                Flex::FlexStart,
                vec![Constraint::Fill(1), Constraint::Fill(1)],
                vec![(0, 50), (50, 100)],
            ),
            (
                Flex::FlexStart,
                vec![Constraint::Min(1), Constraint::Fill(1)],
                vec![(0, 50), (50, 100)],
            ),
        ];
        for (flex, constraints, want) in cases {
            let got = x_ranges(flex, &constraints);
            assert_eq!(got, want, "flex={flex} constraints={constraints:?}");
        }
    }

    #[test]
    fn test_edge_cases() {
        let area = crate::window::rect(0, 0, 1, 1);
        let layout = Layout {
            direction: Direction::DirectionVertical,
            constraints: vec![
                Constraint::Percent(50),
                Constraint::Percent(50),
                Constraint::Min(0),
            ],
            ..Layout::default()
        }
        .split(area);
        assert_eq!(
            layout
                .iter()
                .map(|r| (r.min.0, r.min.1, r.dx(), r.dy()))
                .collect::<Vec<_>>(),
            vec![(0, 0, 1, 1), (0, 1, 1, 0), (0, 1, 1, 0)]
        );

        let area = crate::window::rect(0, 0, 7, 1);
        let layout = Layout {
            direction: Direction::DirectionHorizontal,
            constraints: vec![
                Constraint::Len(3),
                Constraint::Min(4),
                Constraint::Len(1),
                Constraint::Min(4),
            ],
            ..Layout::default()
        }
        .split(area);
        assert_eq!(
            layout.iter().map(|r| (r.min.0, r.dx())).collect::<Vec<_>>(),
            vec![(0, 0), (0, 4), (4, 0), (4, 3)]
        );
    }

    /// Splits a 100x1 area with the given spacing and returns each segment as
    /// a (start, dx) pair.
    fn spaced_ranges(flex: Flex, constraints: &[Constraint], spacing: i64) -> Vec<(usize, usize)> {
        let area = crate::window::rect(0, 0, 100, 1);
        let layout = Layout {
            direction: Direction::DirectionHorizontal,
            constraints: constraints.to_vec(),
            flex,
            spacing,
            ..Layout::default()
        }
        .split(area);
        layout.iter().map(|r| (r.min.0, r.dx())).collect::<Vec<_>>()
    }

    #[test]
    fn test_flex_spacing() {
        let len_three = vec![
            Constraint::Len(20),
            Constraint::Len(20),
            Constraint::Len(20),
        ];
        let cases: SpacingCase = vec![
            (Flex::FlexStart, 0, vec![(0, 20), (20, 20), (40, 20)]),
            (Flex::FlexStart, -1, vec![(0, 20), (19, 20), (38, 20)]),
            (Flex::FlexCenter, -1, vec![(21, 20), (40, 20), (59, 20)]),
            (Flex::FlexEnd, -1, vec![(42, 20), (61, 20), (80, 20)]),
            (Flex::FlexLegacy, -1, vec![(0, 20), (19, 20), (38, 62)]),
            (
                Flex::FlexSpaceBetween,
                -1,
                vec![(0, 20), (40, 20), (80, 20)],
            ),
            (
                Flex::FlexSpaceEvenly,
                -1,
                vec![(10, 20), (40, 20), (70, 20)],
            ),
            (Flex::FlexSpaceAround, -1, vec![(7, 20), (40, 20), (73, 20)]),
            (Flex::FlexStart, 2, vec![(0, 20), (22, 20), (44, 20)]),
            (Flex::FlexCenter, 2, vec![(18, 20), (40, 20), (62, 20)]),
            (Flex::FlexEnd, 2, vec![(36, 20), (58, 20), (80, 20)]),
            (Flex::FlexLegacy, 2, vec![(0, 20), (22, 20), (44, 56)]),
            (Flex::FlexSpaceBetween, 2, vec![(0, 20), (40, 20), (80, 20)]),
            (Flex::FlexSpaceEvenly, 2, vec![(10, 20), (40, 20), (70, 20)]),
            (Flex::FlexSpaceAround, 2, vec![(7, 20), (40, 20), (73, 20)]),
        ];
        for (flex, spacing, want) in cases {
            let got = spaced_ranges(flex, &len_three, spacing);
            assert_eq!(got, want, "flex={flex} spacing={spacing}");
        }
    }

    #[test]
    fn test_assign() {
        let area = crate::window::rect(0, 0, 100, 1);
        let layout = Layout {
            direction: Direction::DirectionHorizontal,
            constraints: vec![Constraint::Len(25), Constraint::Len(25)],
            flex: Flex::FlexStart,
            ..Layout::default()
        }
        .split(area);
        let mut top = Rectangle {
            min: (0, 0),
            max: (0, 0),
        };
        let mut bottom = Rectangle {
            min: (0, 0),
            max: (0, 0),
        };
        let mut areas: [Option<&mut Rectangle>; 2] = [Some(&mut top), Some(&mut bottom)];
        layout.assign(&mut areas);
        assert_eq!(top.min, (0, 0));
        assert_eq!(top.max, (25, 1));
        assert_eq!(bottom.min, (25, 0));
        assert_eq!(bottom.max, (50, 1));
    }

    #[test]
    fn test_padding() {
        let p = pad(&[1, 2]);
        assert_eq!(
            p,
            Padding {
                top: 1,
                right: 2,
                bottom: 1,
                left: 2
            }
        );
        let area = crate::window::rect(0, 0, 10, 10);
        let inner = p.apply(area);
        assert_eq!(inner.min, (2, 1));
        assert_eq!(inner.max, (8, 9));
        let p4 = pad(&[1, 2, 3, 4]);
        assert_eq!(
            p4,
            Padding {
                top: 1,
                right: 2,
                bottom: 3,
                left: 4
            }
        );
    }

    #[test]
    fn test_combinations() {
        assert_eq!(
            combinations(4, 2),
            vec![
                vec![0, 1],
                vec![0, 2],
                vec![0, 3],
                vec![1, 2],
                vec![1, 3],
                vec![2, 3]
            ]
        );
        assert!(combinations(1, 2).is_empty());
        assert_eq!(combinations(3, 3), vec![vec![0, 1, 2]]);
    }

    #[test]
    fn test_flex_display() {
        assert_eq!(Flex::FlexStart.to_string(), "Start");
        assert_eq!(Flex::FlexLegacy.to_string(), "Legacy");
        assert_eq!(Flex::FlexEnd.to_string(), "End");
        assert_eq!(Flex::FlexCenter.to_string(), "Center");
        assert_eq!(Flex::FlexSpaceBetween.to_string(), "Space Between");
        assert_eq!(Flex::FlexSpaceEvenly.to_string(), "Space Evenly");
        assert_eq!(Flex::FlexSpaceAround.to_string(), "Space Around");
        assert_eq!(Constraint::Min(20).to_string(), "Min(20)");
        assert_eq!(
            Constraint::Ratio { num: 1, den: 2 }.to_string(),
            "Ratio(1 / 2)"
        );
    }
}
