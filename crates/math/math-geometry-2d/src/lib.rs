#![doc = include_str!("../README.md")]

pub mod surface;
use std::ops::{Add, AddAssign, Div, Mul, Sub};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq)]
/// Error type for validated 2D geometry contracts.
pub enum GeometryError {
    /// Invalid dimensions.
    InvalidDimensions {
        /// Width in pixels.
        width: u32,
        /// Height in pixels.
        height: u32,
    },
    /// Invalid argument.
    InvalidArgument(String),
}

impl std::fmt::Display for GeometryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidDimensions { width, height } => {
                write!(f, "invalid dimensions: {width}x{height}")
            }
            Self::InvalidArgument(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for GeometryError {}

impl From<GeometryError> for media_core::DetectError {
    fn from(value: GeometryError) -> Self {
        Self::InvalidArgument(value.to_string())
    }
}

/// Result type for 2D geometry operations.
pub type Result<T> = std::result::Result<T, GeometryError>;

fn invalid_argument(message: impl Into<String>) -> GeometryError {
    GeometryError::InvalidArgument(message.into())
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
/// Data type for point2f.
pub struct Point2f {
    /// The x value.
    pub x: f32,
    /// The y value.
    pub y: f32,
}

impl Point2f {
    /// Creates a new value.
    pub fn new(x: f32, y: f32) -> Result<Self> {
        let point = Self { x, y };
        point.validate()?;
        Ok(point)
    }

    /// Validates this value.
    pub fn validate(self) -> Result<()> {
        if !self.x.is_finite() || !self.y.is_finite() {
            return Err(invalid_argument("2D point coordinates must be finite"));
        }
        Ok(())
    }

    /// Returns translate.
    pub fn translate(self, delta: Vector2f) -> Result<Self> {
        delta.validate()?;
        Self::new(self.x + delta.x, self.y + delta.y)
    }

    /// Converts this value to normalized.
    pub fn to_normalized(self, size: Size2u) -> Result<NormalizedPoint2> {
        size.validate()?;
        NormalizedPoint2::new(self.x / size.width as f32, self.y / size.height as f32)
    }
}

impl Add<Vector2f> for Point2f {
    type Output = Self;

    fn add(self, rhs: Vector2f) -> Self::Output {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl Sub<Vector2f> for Point2f {
    type Output = Self;

    fn sub(self, rhs: Vector2f) -> Self::Output {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl Sub<Point2f> for Point2f {
    type Output = Vector2f;

    fn sub(self, rhs: Point2f) -> Self::Output {
        Vector2f {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
/// Data type for point2i.
pub struct Point2i {
    /// The x value.
    pub x: i32,
    /// The y value.
    pub y: i32,
}

impl Point2i {
    /// Creates a new value.
    pub const fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
/// Data type for vector2f.
pub struct Vector2f {
    /// The x value.
    pub x: f32,
    /// The y value.
    pub y: f32,
}

impl Vector2f {
    /// Creates a new value.
    pub fn new(x: f32, y: f32) -> Result<Self> {
        let vector = Self { x, y };
        vector.validate()?;
        Ok(vector)
    }

    /// Validates this value.
    pub fn validate(self) -> Result<()> {
        if !self.x.is_finite() || !self.y.is_finite() {
            return Err(invalid_argument("2D vector components must be finite"));
        }
        Ok(())
    }

    /// Returns length.
    pub fn length(self) -> Result<f32> {
        self.validate()?;
        Ok((self.x * self.x + self.y * self.y).sqrt())
    }

    /// Returns dot.
    pub fn dot(self, rhs: Self) -> Result<f32> {
        self.validate()?;
        rhs.validate()?;
        Ok(self.x.mul_add(rhs.x, self.y * rhs.y))
    }

    /// Returns cross.
    pub fn cross(self, rhs: Self) -> Result<f32> {
        self.validate()?;
        rhs.validate()?;
        Ok(self.x.mul_add(rhs.y, -(self.y * rhs.x)))
    }

    /// Normalizes this value.
    pub fn normalize(self) -> Result<Self> {
        let length = self.length()?;
        if length <= f32::EPSILON {
            return Err(invalid_argument(
                "2D vector length must be greater than zero",
            ));
        }
        Self::new(self.x / length, self.y / length)
    }
}

impl Add for Vector2f {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x + rhs.x,
            y: self.y + rhs.y,
        }
    }
}

impl AddAssign for Vector2f {
    fn add_assign(&mut self, rhs: Self) {
        self.x += rhs.x;
        self.y += rhs.y;
    }
}

impl Sub for Vector2f {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self {
            x: self.x - rhs.x,
            y: self.y - rhs.y,
        }
    }
}

impl Mul<f32> for Vector2f {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self::Output {
        Self {
            x: self.x * rhs,
            y: self.y * rhs,
        }
    }
}

impl Mul<Vector2f> for f32 {
    type Output = Vector2f;

    fn mul(self, rhs: Vector2f) -> Self::Output {
        rhs * self
    }
}

impl Div<f32> for Vector2f {
    type Output = Self;

    fn div(self, rhs: f32) -> Self::Output {
        Self {
            x: self.x / rhs,
            y: self.y / rhs,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
/// Data type for size2u.
pub struct Size2u {
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

impl Size2u {
    /// Creates a new value.
    pub fn new(width: u32, height: u32) -> Result<Self> {
        let size = Self { width, height };
        size.validate()?;
        Ok(size)
    }

    /// Validates this value.
    pub fn validate(self) -> Result<()> {
        if self.width == 0 || self.height == 0 {
            return Err(GeometryError::InvalidDimensions {
                width: self.width,
                height: self.height,
            });
        }
        Ok(())
    }

    /// Returns area.
    pub fn area(self) -> Result<u64> {
        self.validate()?;
        Ok(self.width as u64 * self.height as u64)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
/// Data type for rect u32.
pub struct RectU32 {
    /// The x value.
    pub x: u32,
    /// The y value.
    pub y: u32,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
}

impl RectU32 {
    /// Creates a new value.
    pub fn new(x: u32, y: u32, width: u32, height: u32) -> Result<Self> {
        let rect = Self {
            x,
            y,
            width,
            height,
        };
        rect.validate()?;
        Ok(rect)
    }

    /// Validates this value.
    pub fn validate(self) -> Result<()> {
        if self.width == 0 || self.height == 0 {
            return Err(GeometryError::InvalidDimensions {
                width: self.width,
                height: self.height,
            });
        }
        let _ = self.max_x()?;
        let _ = self.max_y()?;
        Ok(())
    }

    /// Returns max x.
    pub fn max_x(self) -> Result<u32> {
        self.x
            .checked_add(self.width)
            .ok_or_else(|| invalid_argument("rectangle x range overflows"))
    }

    /// Returns max y.
    pub fn max_y(self) -> Result<u32> {
        self.y
            .checked_add(self.height)
            .ok_or_else(|| invalid_argument("rectangle y range overflows"))
    }

    /// Returns contains.
    pub fn contains(self, other: Self) -> Result<bool> {
        self.validate()?;
        other.validate()?;
        Ok(self.x <= other.x
            && self.y <= other.y
            && self.max_x()? >= other.max_x()?
            && self.max_y()? >= other.max_y()?)
    }

    /// Returns contains point.
    pub fn contains_point(self, x: u32, y: u32) -> Result<bool> {
        self.validate()?;
        Ok(x >= self.x && x < self.max_x()? && y >= self.y && y < self.max_y()?)
    }

    /// Returns intersects.
    pub fn intersects(self, other: Self) -> Result<bool> {
        self.validate()?;
        other.validate()?;
        Ok(self.x < other.max_x()?
            && other.x < self.max_x()?
            && self.y < other.max_y()?
            && other.y < self.max_y()?)
    }

    /// Returns intersection.
    pub fn intersection(self, other: Self) -> Result<Option<Self>> {
        if !self.intersects(other)? {
            return Ok(None);
        }
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let max_x = self.max_x()?.min(other.max_x()?);
        let max_y = self.max_y()?.min(other.max_y()?);
        Self::new(x, y, max_x - x, max_y - y).map(Some)
    }

    /// Returns intersection-over-union for two rectangles.
    pub fn iou(self, other: Self) -> Result<f32> {
        let intersection_area = self
            .intersection(other)?
            .map(|rect| rect.area())
            .transpose()?
            .unwrap_or(0) as f32;
        let union_area = self.area()? as f32 + other.area()? as f32 - intersection_area;
        if union_area <= f32::EPSILON {
            return Err(invalid_argument(
                "rectangle union area must be greater than zero",
            ));
        }
        Ok(intersection_area / union_area)
    }

    /// Returns the intersection area divided by this rectangle area.
    pub fn overlap_ratio(self, other: Self) -> Result<f32> {
        let intersection_area = self
            .intersection(other)?
            .map(|rect| rect.area())
            .transpose()?
            .unwrap_or(0) as f32;
        Ok(intersection_area / self.area()? as f32)
    }

    /// Returns union.
    pub fn union(self, other: Self) -> Result<Self> {
        self.validate()?;
        other.validate()?;
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        let max_x = self.max_x()?.max(other.max_x()?);
        let max_y = self.max_y()?.max(other.max_y()?);
        Self::new(x, y, max_x - x, max_y - y)
    }

    /// Returns clamp to.
    pub fn clamp_to(self, bounds: Self) -> Result<Option<Self>> {
        self.intersection(bounds)
    }

    /// Returns translate.
    pub fn translate(self, dx: i32, dy: i32) -> Result<Self> {
        self.validate()?;
        let x = self.x as i64 + dx as i64;
        let y = self.y as i64 + dy as i64;
        if x < 0 || y < 0 || x > u32::MAX as i64 || y > u32::MAX as i64 {
            return Err(invalid_argument(
                "translated rectangle is out of u32 bounds",
            ));
        }
        Self::new(x as u32, y as u32, self.width, self.height)
    }

    /// Returns scale.
    pub fn scale(self, factor: f32) -> Result<Self> {
        if !factor.is_finite() || factor <= 0.0 {
            return Err(invalid_argument("scale factor must be finite and positive"));
        }
        let x = (self.x as f32 * factor).round();
        let y = (self.y as f32 * factor).round();
        let width = (self.width as f32 * factor).round();
        let height = (self.height as f32 * factor).round();
        if [x, y, width, height]
            .iter()
            .any(|value| *value < 0.0 || *value > u32::MAX as f32)
        {
            return Err(invalid_argument("scaled rectangle is out of u32 bounds"));
        }
        Self::new(x as u32, y as u32, width as u32, height as u32)
    }

    /// Returns inflate.
    pub fn inflate(self, dx: i32, dy: i32) -> Result<Self> {
        self.validate()?;
        let x = self.x as i64 - dx as i64;
        let y = self.y as i64 - dy as i64;
        let width = self.width as i64 + 2 * dx as i64;
        let height = self.height as i64 + 2 * dy as i64;
        if x < 0 || y < 0 || width <= 0 || height <= 0 {
            return Err(invalid_argument(
                "inflated rectangle must stay within positive bounds",
            ));
        }
        if x > u32::MAX as i64
            || y > u32::MAX as i64
            || width > u32::MAX as i64
            || height > u32::MAX as i64
        {
            return Err(invalid_argument("inflated rectangle exceeds u32 bounds"));
        }
        Self::new(x as u32, y as u32, width as u32, height as u32)
    }

    /// Returns center f32.
    pub fn center_f32(self) -> Point2f {
        Point2f {
            x: self.x as f32 + self.width as f32 / 2.0,
            y: self.y as f32 + self.height as f32 / 2.0,
        }
    }

    /// Returns area.
    pub fn area(self) -> Result<u64> {
        self.validate()?;
        Ok(self.width as u64 * self.height as u64)
    }

    /// Returns size.
    pub fn size(self) -> Size2u {
        Size2u {
            width: self.width,
            height: self.height,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
/// Data type for a broad-phase collision pair.
pub struct CollisionPair {
    /// Index of the first item.
    pub left_index: usize,
    /// Index of the second item.
    pub right_index: usize,
}

impl CollisionPair {
    /// Creates a new ordered same-set pair.
    pub fn ordered(left_index: usize, right_index: usize) -> Self {
        if left_index <= right_index {
            Self {
                left_index,
                right_index,
            }
        } else {
            Self {
                left_index: right_index,
                right_index: left_index,
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Strategy used for broad-phase 2D collision detection.
pub enum BroadPhase2Strategy {
    /// Selects an implementation from the input shape.
    Auto,
    /// Checks every pair.
    BruteForce,
    /// Uses a spatial hash grid.
    SpatialHashGrid,
    /// Uses sweep and prune along the x axis.
    SweepAndPrune,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Cell size selection for 2D spatial hashing.
pub enum SpatialCellSize2 {
    /// Uses median item dimensions.
    Auto,
    /// Uses a fixed cell size.
    Fixed {
        /// Cell width in pixels.
        width: u32,
        /// Cell height in pixels.
        height: u32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Options for 2D broad-phase collision detection.
pub struct BroadPhase2Config {
    /// Strategy to use.
    pub strategy: BroadPhase2Strategy,
    /// Maximum item count handled with brute force in auto mode.
    pub brute_force_threshold: usize,
    /// Maximum cells a single item may span before auto mode uses sweep and prune.
    pub max_cells_per_item: usize,
    /// Spatial hash grid cell size.
    pub cell_size: SpatialCellSize2,
}

impl Default for BroadPhase2Config {
    fn default() -> Self {
        Self {
            strategy: BroadPhase2Strategy::Auto,
            brute_force_threshold: 128,
            max_cells_per_item: 1024,
            cell_size: SpatialCellSize2::Auto,
        }
    }
}

impl BroadPhase2Config {
    /// Validates this value.
    pub fn validate(self) -> Result<()> {
        if self.max_cells_per_item == 0 {
            return Err(invalid_argument(
                "max_cells_per_item must be greater than zero",
            ));
        }
        if let SpatialCellSize2::Fixed { width, height } = self.cell_size {
            if width == 0 || height == 0 {
                return Err(invalid_argument("fixed spatial cell size must be non-zero"));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Runtime statistics for broad-phase collision detection.
pub struct BroadPhaseStats {
    /// Number of objects indexed.
    pub object_count: usize,
    /// Number of occupied cells.
    pub cell_count: usize,
    /// Number of object-cell entries.
    pub cell_entry_count: usize,
    /// Number of candidate pairs emitted.
    pub candidate_pair_count: usize,
    /// Strategy selected after auto resolution.
    pub selected_strategy: BroadPhase2Strategy,
}

impl Default for BroadPhaseStats {
    fn default() -> Self {
        Self {
            object_count: 0,
            cell_count: 0,
            cell_entry_count: 0,
            candidate_pair_count: 0,
            selected_strategy: BroadPhase2Strategy::BruteForce,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct GridCell2 {
    x: u32,
    y: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct GridEntry2 {
    cell: GridCell2,
    set: u8,
    index: usize,
}

#[derive(Debug, Clone)]
/// Reusable spatial hash grid for 2D broad-phase collision detection.
pub struct SpatialHashGrid2 {
    config: BroadPhase2Config,
    cell_width: u32,
    cell_height: u32,
    left_bounds: Vec<RectU32>,
    right_bounds: Vec<RectU32>,
    entries: Vec<GridEntry2>,
    pairs: Vec<CollisionPair>,
    stats: BroadPhaseStats,
}

impl SpatialHashGrid2 {
    /// Creates a new value.
    pub fn new(config: BroadPhase2Config) -> Result<Self> {
        config.validate()?;
        Ok(Self {
            config,
            cell_width: 1,
            cell_height: 1,
            left_bounds: Vec::new(),
            right_bounds: Vec::new(),
            entries: Vec::new(),
            pairs: Vec::new(),
            stats: BroadPhaseStats {
                selected_strategy: config.strategy,
                ..BroadPhaseStats::default()
            },
        })
    }

    /// Rebuilds this grid for a single set of bounds.
    pub fn rebuild(&mut self, bounds: &[RectU32]) -> Result<()> {
        validate_rects(bounds)?;
        let (cell_width, cell_height) = resolve_cell_size_2d(bounds, self.config.cell_size)?;
        self.cell_width = cell_width;
        self.cell_height = cell_height;
        self.left_bounds.clear();
        self.left_bounds.extend_from_slice(bounds);
        self.right_bounds.clear();
        self.entries.clear();
        self.pairs.clear();
        self.push_entries(bounds, 0)?;
        self.stats = BroadPhaseStats {
            object_count: bounds.len(),
            cell_count: 0,
            cell_entry_count: self.entries.len(),
            candidate_pair_count: 0,
            selected_strategy: BroadPhase2Strategy::SpatialHashGrid,
        };
        Ok(())
    }

    /// Returns candidate pairs for the most recently rebuilt single set.
    pub fn candidate_pairs(&mut self) -> Result<&[CollisionPair]> {
        self.entries.sort_unstable();
        self.pairs.clear();
        let mut cell_count = 0;
        let mut start = 0;
        while start < self.entries.len() {
            let cell = self.entries[start].cell;
            let mut end = start + 1;
            while end < self.entries.len() && self.entries[end].cell == cell {
                end += 1;
            }
            cell_count += 1;
            for left in start..end {
                for right in (left + 1)..end {
                    let left_index = self.entries[left].index;
                    let right_index = self.entries[right].index;
                    if self.left_bounds[left_index].intersects(self.left_bounds[right_index])? {
                        self.pairs
                            .push(CollisionPair::ordered(left_index, right_index));
                    }
                }
            }
            start = end;
        }
        finish_pairs(&mut self.pairs);
        self.stats.cell_count = cell_count;
        self.stats.candidate_pair_count = self.pairs.len();
        Ok(&self.pairs)
    }

    /// Returns candidate pairs between two independent sets.
    pub fn candidate_pairs_between(
        &mut self,
        left: &[RectU32],
        right: &[RectU32],
    ) -> Result<&[CollisionPair]> {
        validate_rects(left)?;
        validate_rects(right)?;
        let (cell_width, cell_height) =
            resolve_cell_size_2d_for_sets(left, right, self.config.cell_size)?;
        self.cell_width = cell_width;
        self.cell_height = cell_height;
        self.left_bounds.clear();
        self.left_bounds.extend_from_slice(left);
        self.right_bounds.clear();
        self.right_bounds.extend_from_slice(right);
        self.entries.clear();
        self.pairs.clear();
        self.push_entries(left, 0)?;
        self.push_entries(right, 1)?;
        self.entries.sort_unstable();

        let mut cell_count = 0;
        let mut start = 0;
        while start < self.entries.len() {
            let cell = self.entries[start].cell;
            let mut end = start + 1;
            while end < self.entries.len() && self.entries[end].cell == cell {
                end += 1;
            }
            cell_count += 1;
            for left_entry in start..end {
                if self.entries[left_entry].set != 0 {
                    continue;
                }
                for right_entry in start..end {
                    if self.entries[right_entry].set == 1
                        && self.left_bounds[self.entries[left_entry].index]
                            .intersects(self.right_bounds[self.entries[right_entry].index])?
                    {
                        self.pairs.push(CollisionPair {
                            left_index: self.entries[left_entry].index,
                            right_index: self.entries[right_entry].index,
                        });
                    }
                }
            }
            start = end;
        }
        finish_pairs(&mut self.pairs);
        self.stats = BroadPhaseStats {
            object_count: left.len() + right.len(),
            cell_count,
            cell_entry_count: self.entries.len(),
            candidate_pair_count: self.pairs.len(),
            selected_strategy: BroadPhase2Strategy::SpatialHashGrid,
        };
        Ok(&self.pairs)
    }

    /// Returns stats.
    pub fn stats(&self) -> BroadPhaseStats {
        self.stats
    }

    fn push_entries(&mut self, bounds: &[RectU32], set: u8) -> Result<()> {
        for (index, rect) in bounds.iter().copied().enumerate() {
            let (min_cell, max_cell) = rect_cell_range_2d(rect, self.cell_width, self.cell_height)?;
            for y in min_cell.y..=max_cell.y {
                for x in min_cell.x..=max_cell.x {
                    self.entries.push(GridEntry2 {
                        cell: GridCell2 { x, y },
                        set,
                        index,
                    });
                }
            }
        }
        Ok(())
    }
}

/// Returns broad-phase candidate pairs for 2D rectangles.
pub fn broad_phase_pairs_2d(
    bounds: &[RectU32],
    config: BroadPhase2Config,
) -> Result<Vec<CollisionPair>> {
    config.validate()?;
    validate_rects(bounds)?;
    let (strategy, stats) = select_strategy_2d(bounds, config)?;
    match strategy {
        BroadPhase2Strategy::Auto => unreachable!("auto strategy must resolve before execution"),
        BroadPhase2Strategy::BruteForce => Ok(brute_force_pairs_2d(bounds)),
        BroadPhase2Strategy::SweepAndPrune => Ok(sweep_and_prune_pairs_2d(bounds)?),
        BroadPhase2Strategy::SpatialHashGrid => {
            let mut grid = SpatialHashGrid2::new(BroadPhase2Config { strategy, ..config })?;
            grid.rebuild(bounds)?;
            let pairs = grid.candidate_pairs()?.to_vec();
            let _ = stats;
            Ok(pairs)
        }
    }
}

fn select_strategy_2d(
    bounds: &[RectU32],
    config: BroadPhase2Config,
) -> Result<(BroadPhase2Strategy, BroadPhaseStats)> {
    let selected = match config.strategy {
        BroadPhase2Strategy::Auto if bounds.len() <= config.brute_force_threshold => {
            BroadPhase2Strategy::BruteForce
        }
        BroadPhase2Strategy::Auto => {
            let (cell_width, cell_height) = resolve_cell_size_2d(bounds, config.cell_size)?;
            if bounds.iter().copied().any(|rect| {
                rect_cell_count_2d(rect, cell_width, cell_height)
                    .map(|count| count > config.max_cells_per_item)
                    .unwrap_or(true)
            }) {
                BroadPhase2Strategy::SweepAndPrune
            } else {
                BroadPhase2Strategy::SpatialHashGrid
            }
        }
        strategy => strategy,
    };
    Ok((
        selected,
        BroadPhaseStats {
            object_count: bounds.len(),
            selected_strategy: selected,
            ..BroadPhaseStats::default()
        },
    ))
}

fn brute_force_pairs_2d(bounds: &[RectU32]) -> Vec<CollisionPair> {
    let mut pairs = Vec::new();
    for left_index in 0..bounds.len() {
        for right_index in (left_index + 1)..bounds.len() {
            if bounds[left_index]
                .intersects(bounds[right_index])
                .unwrap_or(false)
            {
                pairs.push(CollisionPair {
                    left_index,
                    right_index,
                });
            }
        }
    }
    pairs
}

fn sweep_and_prune_pairs_2d(bounds: &[RectU32]) -> Result<Vec<CollisionPair>> {
    let mut ordered = bounds
        .iter()
        .copied()
        .enumerate()
        .map(|(index, rect)| Ok((index, rect.x, rect.max_x()?, rect)))
        .collect::<Result<Vec<_>>>()?;
    ordered.sort_unstable_by_key(|(index, min_x, _, _)| (*min_x, *index));

    let mut pairs = Vec::new();
    let mut active = Vec::<(usize, u32, RectU32)>::new();
    for (index, min_x, max_x, rect) in ordered {
        active.retain(|(_, active_max_x, _)| *active_max_x > min_x);
        for (active_index, _, active_rect) in &active {
            if active_rect.intersects(rect)? {
                pairs.push(CollisionPair::ordered(*active_index, index));
            }
        }
        active.push((index, max_x, rect));
    }
    finish_pairs(&mut pairs);
    Ok(pairs)
}

fn resolve_cell_size_2d(bounds: &[RectU32], cell_size: SpatialCellSize2) -> Result<(u32, u32)> {
    match cell_size {
        SpatialCellSize2::Fixed { width, height } => {
            if width == 0 || height == 0 {
                return Err(invalid_argument("fixed spatial cell size must be non-zero"));
            }
            Ok((width, height))
        }
        SpatialCellSize2::Auto => {
            if bounds.is_empty() {
                return Ok((1, 1));
            }
            let mut widths = bounds.iter().map(|rect| rect.width).collect::<Vec<_>>();
            let mut heights = bounds.iter().map(|rect| rect.height).collect::<Vec<_>>();
            widths.sort_unstable();
            heights.sort_unstable();
            Ok((
                widths[widths.len() / 2].max(1),
                heights[heights.len() / 2].max(1),
            ))
        }
    }
}

fn resolve_cell_size_2d_for_sets(
    left: &[RectU32],
    right: &[RectU32],
    cell_size: SpatialCellSize2,
) -> Result<(u32, u32)> {
    match cell_size {
        SpatialCellSize2::Fixed { .. } => resolve_cell_size_2d(left, cell_size),
        SpatialCellSize2::Auto => {
            let mut widths = left
                .iter()
                .chain(right.iter())
                .map(|rect| rect.width)
                .collect::<Vec<_>>();
            let mut heights = left
                .iter()
                .chain(right.iter())
                .map(|rect| rect.height)
                .collect::<Vec<_>>();
            if widths.is_empty() {
                return Ok((1, 1));
            }
            widths.sort_unstable();
            heights.sort_unstable();
            Ok((
                widths[widths.len() / 2].max(1),
                heights[heights.len() / 2].max(1),
            ))
        }
    }
}

fn rect_cell_range_2d(
    rect: RectU32,
    cell_width: u32,
    cell_height: u32,
) -> Result<(GridCell2, GridCell2)> {
    let max_x = rect.max_x()?.saturating_sub(1);
    let max_y = rect.max_y()?.saturating_sub(1);
    Ok((
        GridCell2 {
            x: rect.x / cell_width,
            y: rect.y / cell_height,
        },
        GridCell2 {
            x: max_x / cell_width,
            y: max_y / cell_height,
        },
    ))
}

fn rect_cell_count_2d(rect: RectU32, cell_width: u32, cell_height: u32) -> Result<usize> {
    let (min, max) = rect_cell_range_2d(rect, cell_width, cell_height)?;
    let width = (max.x - min.x + 1) as usize;
    let height = (max.y - min.y + 1) as usize;
    Ok(width.saturating_mul(height))
}

fn validate_rects(bounds: &[RectU32]) -> Result<()> {
    for rect in bounds {
        rect.validate()?;
    }
    Ok(())
}

fn finish_pairs(pairs: &mut Vec<CollisionPair>) {
    pairs.sort_unstable();
    pairs.dedup();
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
/// Data type for rect f32.
pub struct RectF32 {
    /// The x value.
    pub x: f32,
    /// The y value.
    pub y: f32,
    /// Width in pixels.
    pub width: f32,
    /// Height in pixels.
    pub height: f32,
}

impl RectF32 {
    /// Creates a new value.
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Result<Self> {
        let rect = Self {
            x,
            y,
            width,
            height,
        };
        rect.validate()?;
        Ok(rect)
    }

    /// Validates this value.
    pub fn validate(self) -> Result<()> {
        if [self.x, self.y, self.width, self.height]
            .iter()
            .any(|value| !value.is_finite())
        {
            return Err(invalid_argument("rectangle values must be finite"));
        }
        if self.width <= 0.0 || self.height <= 0.0 {
            return Err(invalid_argument(
                "rectangle width and height must be greater than zero",
            ));
        }
        Ok(())
    }

    /// Returns max x.
    pub fn max_x(self) -> Result<f32> {
        self.validate()?;
        Ok(self.x + self.width)
    }

    /// Returns max y.
    pub fn max_y(self) -> Result<f32> {
        self.validate()?;
        Ok(self.y + self.height)
    }

    /// Returns contains point.
    pub fn contains_point(self, point: Point2f) -> Result<bool> {
        self.validate()?;
        point.validate()?;
        Ok(point.x >= self.x
            && point.x <= self.max_x()?
            && point.y >= self.y
            && point.y <= self.max_y()?)
    }

    /// Returns intersects.
    pub fn intersects(self, other: Self) -> Result<bool> {
        self.validate()?;
        other.validate()?;
        Ok(self.x < other.max_x()?
            && other.x < self.max_x()?
            && self.y < other.max_y()?
            && other.y < self.max_y()?)
    }

    /// Returns intersection.
    pub fn intersection(self, other: Self) -> Result<Option<Self>> {
        if !self.intersects(other)? {
            return Ok(None);
        }
        let x = self.x.max(other.x);
        let y = self.y.max(other.y);
        let max_x = self.max_x()?.min(other.max_x()?);
        let max_y = self.max_y()?.min(other.max_y()?);
        Self::new(x, y, max_x - x, max_y - y).map(Some)
    }

    /// Returns intersection-over-union for two rectangles.
    pub fn iou(self, other: Self) -> Result<f32> {
        let intersection_area = self
            .intersection(other)?
            .map(|rect| rect.area())
            .transpose()?
            .unwrap_or(0.0);
        let union_area = self.area()? + other.area()? - intersection_area;
        if union_area <= f32::EPSILON {
            return Err(invalid_argument(
                "rectangle union area must be greater than zero",
            ));
        }
        Ok(intersection_area / union_area)
    }

    /// Returns the intersection area divided by this rectangle area.
    pub fn overlap_ratio(self, other: Self) -> Result<f32> {
        let intersection_area = self
            .intersection(other)?
            .map(|rect| rect.area())
            .transpose()?
            .unwrap_or(0.0);
        Ok(intersection_area / self.area()?)
    }

    /// Returns union.
    pub fn union(self, other: Self) -> Result<Self> {
        self.validate()?;
        other.validate()?;
        let x = self.x.min(other.x);
        let y = self.y.min(other.y);
        let max_x = self.max_x()?.max(other.max_x()?);
        let max_y = self.max_y()?.max(other.max_y()?);
        Self::new(x, y, max_x - x, max_y - y)
    }

    /// Returns clamp to.
    pub fn clamp_to(self, bounds: Self) -> Result<Option<Self>> {
        self.intersection(bounds)
    }

    /// Returns translate.
    pub fn translate(self, delta: Vector2f) -> Result<Self> {
        delta.validate()?;
        Self::new(self.x + delta.x, self.y + delta.y, self.width, self.height)
    }

    /// Returns scale.
    pub fn scale(self, factor: f32) -> Result<Self> {
        if !factor.is_finite() || factor <= 0.0 {
            return Err(invalid_argument("scale factor must be finite and positive"));
        }
        Self::new(
            self.x * factor,
            self.y * factor,
            self.width * factor,
            self.height * factor,
        )
    }

    /// Returns inflate.
    pub fn inflate(self, dx: f32, dy: f32) -> Result<Self> {
        if !dx.is_finite() || !dy.is_finite() {
            return Err(invalid_argument("inflate deltas must be finite"));
        }
        Self::new(
            self.x - dx,
            self.y - dy,
            self.width + 2.0 * dx,
            self.height + 2.0 * dy,
        )
    }

    /// Returns center.
    pub fn center(self) -> Result<Point2f> {
        self.validate()?;
        Point2f::new(self.x + self.width / 2.0, self.y + self.height / 2.0)
    }

    /// Returns area.
    pub fn area(self) -> Result<f32> {
        self.validate()?;
        Ok(self.width * self.height)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
/// Data type for normalized point2.
pub struct NormalizedPoint2 {
    /// The x value.
    pub x: f32,
    /// The y value.
    pub y: f32,
}

impl NormalizedPoint2 {
    /// Creates a new value.
    pub fn new(x: f32, y: f32) -> Result<Self> {
        let point = Self { x, y };
        point.validate()?;
        Ok(point)
    }

    /// Validates this value.
    pub fn validate(self) -> Result<()> {
        if !self.x.is_finite() || !self.y.is_finite() {
            return Err(invalid_argument("normalized coordinates must be finite"));
        }
        if !(0.0..=1.0).contains(&self.x) || !(0.0..=1.0).contains(&self.y) {
            return Err(invalid_argument(
                "normalized coordinates must be within the inclusive range [0, 1]",
            ));
        }
        Ok(())
    }

    /// Converts this value to pixel point.
    pub fn to_pixel_point(self, size: Size2u) -> Point2i {
        Point2i {
            x: (self.x * size.width as f32).round() as i32,
            y: (self.y * size.height as f32).round() as i32,
        }
    }

    /// Converts this value to pixel point f32.
    pub fn to_pixel_point_f32(self, size: Size2u) -> Point2f {
        Point2f {
            x: self.x * size.width as f32,
            y: self.y * size.height as f32,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
/// Data type for affine2.
pub struct Affine2 {
    /// The m11 value.
    pub m11: f32,
    /// The m12 value.
    pub m12: f32,
    /// The m21 value.
    pub m21: f32,
    /// The m22 value.
    pub m22: f32,
    /// The tx value.
    pub tx: f32,
    /// The ty value.
    pub ty: f32,
}

impl Affine2 {
    /// Creates a new value.
    pub const fn identity() -> Self {
        Self {
            m11: 1.0,
            m12: 0.0,
            m21: 0.0,
            m22: 1.0,
            tx: 0.0,
            ty: 0.0,
        }
    }

    /// Creates a new value.
    pub const fn translation(tx: f32, ty: f32) -> Self {
        Self {
            tx,
            ty,
            ..Self::identity()
        }
    }

    /// Creates a new value.
    pub const fn scaling(sx: f32, sy: f32) -> Self {
        Self {
            m11: sx,
            m12: 0.0,
            m21: 0.0,
            m22: sy,
            tx: 0.0,
            ty: 0.0,
        }
    }

    /// Validates this value.
    pub fn validate(self) -> Result<()> {
        if [self.m11, self.m12, self.m21, self.m22, self.tx, self.ty]
            .iter()
            .any(|value| !value.is_finite())
        {
            return Err(invalid_argument("affine transform values must be finite"));
        }
        Ok(())
    }

    /// Returns determinant.
    pub fn determinant(self) -> Result<f32> {
        self.validate()?;
        Ok(self.m11 * self.m22 - self.m12 * self.m21)
    }

    /// Returns apply point.
    pub fn apply_point(self, point: Point2f) -> Point2f {
        Point2f {
            x: self.m11 * point.x + self.m12 * point.y + self.tx,
            y: self.m21 * point.x + self.m22 * point.y + self.ty,
        }
    }

    /// Returns invert.
    pub fn invert(self) -> Result<Self> {
        self.validate()?;
        let det = self.determinant()?;
        if det.abs() <= f32::EPSILON {
            return Err(invalid_argument("affine transform is not invertible"));
        }
        let inv_det = 1.0 / det;
        let m11 = self.m22 * inv_det;
        let m12 = -self.m12 * inv_det;
        let m21 = -self.m21 * inv_det;
        let m22 = self.m11 * inv_det;
        let tx = -(m11 * self.tx + m12 * self.ty);
        let ty = -(m21 * self.tx + m22 * self.ty);
        Ok(Self {
            m11,
            m12,
            m21,
            m22,
            tx,
            ty,
        })
    }

    /// Returns compose.
    pub fn compose(self, next: Self) -> Result<Self> {
        self.validate()?;
        next.validate()?;
        Ok(Self {
            m11: next.m11 * self.m11 + next.m12 * self.m21,
            m12: next.m11 * self.m12 + next.m12 * self.m22,
            m21: next.m21 * self.m11 + next.m22 * self.m21,
            m22: next.m21 * self.m12 + next.m22 * self.m22,
            tx: next.m11 * self.tx + next.m12 * self.ty + next.tx,
            ty: next.m21 * self.tx + next.m22 * self.ty + next.ty,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
/// Data type for line segment2.
pub struct LineSegment2 {
    /// The start value.
    pub start: Point2f,
    /// The end value.
    pub end: Point2f,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
/// Intersection details for two finite 2D line segments.
pub struct LineIntersection2 {
    /// Intersection point.
    pub point: Point2f,
    /// Parametric position on the left segment in `[0, 1]`.
    pub left_t: f32,
    /// Parametric position on the right segment in `[0, 1]`.
    pub right_t: f32,
}

impl LineSegment2 {
    /// Creates a new value.
    pub fn new(start: Point2f, end: Point2f) -> Result<Self> {
        start.validate()?;
        end.validate()?;
        if start == end {
            return Err(invalid_argument(
                "line segment start and end must not be identical",
            ));
        }
        Ok(Self { start, end })
    }

    /// Returns length.
    pub fn length(self) -> Result<f32> {
        (self.end - self.start).length()
    }

    /// Returns midpoint.
    pub fn midpoint(self) -> Point2f {
        self.start + ((self.end - self.start) * 0.5)
    }

    /// Returns direction.
    pub fn direction(self) -> Result<Vector2f> {
        (self.end - self.start).normalize()
    }

    /// Returns the finite segment intersection, if present.
    pub fn intersection(self, other: Self) -> Result<Option<LineIntersection2>> {
        let p = self.start;
        let r = self.end - self.start;
        let q = other.start;
        let s = other.end - other.start;
        p.validate()?;
        q.validate()?;
        let denominator = r.cross(s)?;
        let q_minus_p = q - p;
        if denominator.abs() <= f32::EPSILON {
            return Ok(None);
        }
        let left_t = q_minus_p.cross(s)? / denominator;
        let right_t = q_minus_p.cross(r)? / denominator;
        if !(-f32::EPSILON..=1.0 + f32::EPSILON).contains(&left_t)
            || !(-f32::EPSILON..=1.0 + f32::EPSILON).contains(&right_t)
        {
            return Ok(None);
        }
        let point = p + r * left_t.clamp(0.0, 1.0);
        Ok(Some(LineIntersection2 {
            point,
            left_t,
            right_t,
        }))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
/// Data type for circle2.
pub struct Circle2 {
    /// The center value.
    pub center: Point2f,
    /// The radius value.
    pub radius: f32,
}

impl Circle2 {
    /// Creates a new value.
    pub fn new(center: Point2f, radius: f32) -> Result<Self> {
        center.validate()?;
        if !radius.is_finite() || radius <= 0.0 {
            return Err(invalid_argument(
                "circle radius must be finite and greater than zero",
            ));
        }
        Ok(Self { center, radius })
    }

    /// Returns area.
    pub fn area(self) -> Result<f32> {
        self.center.validate()?;
        if !self.radius.is_finite() || self.radius <= 0.0 {
            return Err(invalid_argument(
                "circle radius must be finite and greater than zero",
            ));
        }
        Ok(std::f32::consts::PI * self.radius * self.radius)
    }

    /// Returns contains point.
    pub fn contains_point(self, point: Point2f) -> Result<bool> {
        point.validate()?;
        Ok((point - self.center).length()? <= self.radius)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
/// Data type for polygon2f.
pub struct Polygon2f {
    points: Vec<Point2f>,
}

impl Polygon2f {
    /// Creates a new value.
    pub fn new(points: impl Into<Vec<Point2f>>) -> Result<Self> {
        let polygon = Self {
            points: points.into(),
        };
        polygon.validate()?;
        Ok(polygon)
    }

    /// Returns points.
    pub fn points(&self) -> &[Point2f] {
        &self.points
    }

    /// Validates this value.
    pub fn validate(&self) -> Result<()> {
        if self.points.len() < 3 {
            return Err(invalid_argument(
                "polygon must contain at least three points",
            ));
        }
        for point in &self.points {
            point.validate()?;
        }
        Ok(())
    }

    /// Returns bounds.
    pub fn bounds(&self) -> Result<Bounds2f> {
        self.validate()?;
        let mut min_x = self.points[0].x;
        let mut min_y = self.points[0].y;
        let mut max_x = self.points[0].x;
        let mut max_y = self.points[0].y;
        for point in &self.points[1..] {
            min_x = min_x.min(point.x);
            min_y = min_y.min(point.y);
            max_x = max_x.max(point.x);
            max_y = max_y.max(point.y);
        }
        Bounds2f::new(
            Point2f { x: min_x, y: min_y },
            Point2f { x: max_x, y: max_y },
        )
    }

    /// Returns signed area.
    pub fn signed_area(&self) -> Result<f32> {
        self.validate()?;
        let mut area = 0.0_f32;
        for index in 0..self.points.len() {
            let current = self.points[index];
            let next = self.points[(index + 1) % self.points.len()];
            area += current.x * next.y - next.x * current.y;
        }
        Ok(area * 0.5)
    }

    /// Returns area.
    pub fn area(&self) -> Result<f32> {
        Ok(self.signed_area()?.abs())
    }

    /// Returns whether this polygon is clockwise.
    pub fn is_clockwise(&self) -> Result<bool> {
        Ok(self.signed_area()? < 0.0)
    }

    /// Returns centroid.
    pub fn centroid(&self) -> Result<Point2f> {
        self.validate()?;
        let signed_area = self.signed_area()?;
        if signed_area.abs() <= f32::EPSILON {
            let sum = self
                .points
                .iter()
                .fold(Vector2f { x: 0.0, y: 0.0 }, |sum, point| {
                    sum + Vector2f {
                        x: point.x,
                        y: point.y,
                    }
                });
            return Point2f::new(
                sum.x / self.points.len() as f32,
                sum.y / self.points.len() as f32,
            );
        }
        let mut cx = 0.0_f32;
        let mut cy = 0.0_f32;
        for index in 0..self.points.len() {
            let current = self.points[index];
            let next = self.points[(index + 1) % self.points.len()];
            let cross = current.x * next.y - next.x * current.y;
            cx += (current.x + next.x) * cross;
            cy += (current.y + next.y) * cross;
        }
        let scale = 1.0 / (6.0 * signed_area);
        Point2f::new(cx * scale, cy * scale)
    }

    /// Returns contains point.
    pub fn contains_point(&self, point: Point2f) -> Result<bool> {
        self.validate()?;
        point.validate()?;
        let mut inside = false;
        let mut previous = self.points.len() - 1;
        for current in 0..self.points.len() {
            let a = self.points[current];
            let b = self.points[previous];
            if ((a.y > point.y) != (b.y > point.y))
                && point.x < (b.x - a.x) * (point.y - a.y) / (b.y - a.y) + a.x
            {
                inside = !inside;
            }
            previous = current;
        }
        Ok(inside)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
/// Data type for bounds2f.
pub struct Bounds2f {
    /// The min value.
    pub min: Point2f,
    /// The max value.
    pub max: Point2f,
}

impl Bounds2f {
    /// Creates a new value.
    pub fn new(min: Point2f, max: Point2f) -> Result<Self> {
        min.validate()?;
        max.validate()?;
        if min.x > max.x || min.y > max.y {
            return Err(invalid_argument("bounds min must not exceed max"));
        }
        Ok(Self { min, max })
    }

    /// Creates bounds from one or more finite points.
    pub fn from_points(points: &[Point2f]) -> Result<Self> {
        if points.is_empty() {
            return Err(invalid_argument("bounds require at least one point"));
        }
        let mut min_x = points[0].x;
        let mut min_y = points[0].y;
        let mut max_x = points[0].x;
        let mut max_y = points[0].y;
        for point in points {
            point.validate()?;
            min_x = min_x.min(point.x);
            min_y = min_y.min(point.y);
            max_x = max_x.max(point.x);
            max_y = max_y.max(point.y);
        }
        Self::new(Point2f::new(min_x, min_y)?, Point2f::new(max_x, max_y)?)
    }

    /// Returns bounds width.
    pub fn width(self) -> f32 {
        self.max.x - self.min.x
    }

    /// Returns bounds height.
    pub fn height(self) -> f32 {
        self.max.y - self.min.y
    }

    /// Returns bounds center.
    pub fn center(self) -> Result<Point2f> {
        Point2f::new(
            self.min.x + self.width() / 2.0,
            self.min.y + self.height() / 2.0,
        )
    }

    /// Expands bounds by a finite non-negative padding in all directions.
    pub fn expand(self, padding: f32) -> Result<Self> {
        if !padding.is_finite() || padding < 0.0 {
            return Err(invalid_argument(
                "bounds padding must be finite and non-negative",
            ));
        }
        Self::new(
            Point2f::new(self.min.x - padding, self.min.y - padding)?,
            Point2f::new(self.max.x + padding, self.max.y + padding)?,
        )
    }

    /// Returns contains.
    pub fn contains(self, point: Point2f) -> Result<bool> {
        point.validate()?;
        Ok(point.x >= self.min.x
            && point.x <= self.max.x
            && point.y >= self.min.y
            && point.y <= self.max.y)
    }

    /// Returns union.
    pub fn union(self, other: Self) -> Result<Self> {
        Self::new(
            Point2f {
                x: self.min.x.min(other.min.x),
                y: self.min.y.min(other.min.y),
            },
            Point2f {
                x: self.max.x.max(other.max.x),
                y: self.max.y.max(other.max.y),
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_rectangles_and_non_finite_points() {
        assert!(RectU32::new(0, 0, 0, 2).is_err());
        assert!(RectF32::new(0.0, 0.0, f32::NAN, 1.0).is_err());
        assert!(Point2f::new(f32::INFINITY, 0.0).is_err());
    }

    #[test]
    fn computes_intersection_and_union() {
        let a = RectU32::new(0, 0, 10, 10).unwrap();
        let b = RectU32::new(5, 5, 10, 10).unwrap();
        assert_eq!(
            a.intersection(b).unwrap(),
            Some(RectU32::new(5, 5, 5, 5).unwrap())
        );
        assert_eq!(a.union(b).unwrap(), RectU32::new(0, 0, 15, 15).unwrap());
    }

    #[test]
    fn vector_and_shape_helpers_cover_common_2d_operations() {
        let a = Point2f::new(0.0, 0.0).unwrap();
        let b = Point2f::new(3.0, 4.0).unwrap();
        let segment = LineSegment2::new(a, b).unwrap();
        assert_eq!(segment.length().unwrap(), 5.0);
        assert_eq!(segment.midpoint(), Point2f::new(1.5, 2.0).unwrap());

        let x = Vector2f::new(1.0, 0.0).unwrap();
        let y = Vector2f::new(0.0, 1.0).unwrap();
        assert_eq!(x.dot(y).unwrap(), 0.0);
        assert_eq!(x.cross(y).unwrap(), 1.0);

        let circle = Circle2::new(a, 2.0).unwrap();
        assert!(circle
            .contains_point(Point2f::new(1.0, 1.0).unwrap())
            .unwrap());
    }

    #[test]
    fn polygon_reports_area_centroid_winding_and_containment() {
        let polygon = Polygon2f::new([
            Point2f::new(0.0, 0.0).unwrap(),
            Point2f::new(2.0, 0.0).unwrap(),
            Point2f::new(2.0, 2.0).unwrap(),
            Point2f::new(0.0, 2.0).unwrap(),
        ])
        .unwrap();
        assert_eq!(polygon.area().unwrap(), 4.0);
        assert!(!polygon.is_clockwise().unwrap());
        assert_eq!(polygon.centroid().unwrap(), Point2f::new(1.0, 1.0).unwrap());
        assert!(polygon
            .contains_point(Point2f::new(1.0, 1.0).unwrap())
            .unwrap());
    }

    #[test]
    fn affine_round_trip_recovers_point() {
        let point = Point2f::new(4.0, 5.0).unwrap();
        let affine = Affine2::translation(3.0, -2.0)
            .compose(Affine2::scaling(2.0, 0.5))
            .unwrap();
        let restored = affine
            .invert()
            .unwrap()
            .apply_point(affine.apply_point(point));
        assert!((restored.x - point.x).abs() < 1.0e-5);
        assert!((restored.y - point.y).abs() < 1.0e-5);
    }

    #[test]
    fn converts_between_normalized_and_pixel_coordinates() {
        let point = NormalizedPoint2::new(0.25, 0.5).unwrap();
        let size = Size2u::new(200, 100).unwrap();
        assert_eq!(point.to_pixel_point(size), Point2i::new(50, 50));
        let normalized = Point2f::new(50.0, 50.0)
            .unwrap()
            .to_normalized(size)
            .unwrap();
        assert!((normalized.x - 0.25).abs() < 1.0e-6);
        assert!((normalized.y - 0.5).abs() < 1.0e-6);
    }

    #[test]
    fn broad_phase_strategies_match_for_mixed_rectangles() {
        let rects = [
            RectU32::new(0, 0, 10, 10).unwrap(),
            RectU32::new(5, 5, 8, 8).unwrap(),
            RectU32::new(30, 30, 4, 4).unwrap(),
            RectU32::new(31, 31, 4, 4).unwrap(),
            RectU32::new(100, 0, 10, 10).unwrap(),
        ];
        let brute = broad_phase_pairs_2d(
            &rects,
            BroadPhase2Config {
                strategy: BroadPhase2Strategy::BruteForce,
                ..BroadPhase2Config::default()
            },
        )
        .unwrap();
        let sweep = broad_phase_pairs_2d(
            &rects,
            BroadPhase2Config {
                strategy: BroadPhase2Strategy::SweepAndPrune,
                ..BroadPhase2Config::default()
            },
        )
        .unwrap();
        let grid = broad_phase_pairs_2d(
            &rects,
            BroadPhase2Config {
                strategy: BroadPhase2Strategy::SpatialHashGrid,
                cell_size: SpatialCellSize2::Fixed {
                    width: 8,
                    height: 8,
                },
                ..BroadPhase2Config::default()
            },
        )
        .unwrap();

        assert_eq!(brute, sweep);
        assert_eq!(brute, grid);
        assert_eq!(
            brute,
            vec![
                CollisionPair {
                    left_index: 0,
                    right_index: 1
                },
                CollisionPair {
                    left_index: 2,
                    right_index: 3
                }
            ]
        );
    }

    #[test]
    fn broad_phase_handles_touching_nested_and_spanning_rectangles() {
        let touching = [
            RectU32::new(0, 0, 10, 10).unwrap(),
            RectU32::new(10, 0, 10, 10).unwrap(),
        ];
        assert!(
            broad_phase_pairs_2d(&touching, BroadPhase2Config::default())
                .unwrap()
                .is_empty()
        );

        let nested = [
            RectU32::new(0, 0, 20, 20).unwrap(),
            RectU32::new(2, 2, 4, 4).unwrap(),
        ];
        assert_eq!(
            broad_phase_pairs_2d(
                &nested,
                BroadPhase2Config {
                    strategy: BroadPhase2Strategy::SpatialHashGrid,
                    cell_size: SpatialCellSize2::Fixed {
                        width: 2,
                        height: 2,
                    },
                    ..BroadPhase2Config::default()
                }
            )
            .unwrap(),
            vec![CollisionPair {
                left_index: 0,
                right_index: 1
            }]
        );
    }

    #[test]
    fn spatial_hash_grid_reports_cross_set_pairs_and_stats() {
        let left = [
            RectU32::new(0, 0, 10, 10).unwrap(),
            RectU32::new(40, 40, 4, 4).unwrap(),
        ];
        let right = [
            RectU32::new(4, 4, 10, 10).unwrap(),
            RectU32::new(80, 80, 4, 4).unwrap(),
        ];
        let mut grid = SpatialHashGrid2::new(BroadPhase2Config {
            strategy: BroadPhase2Strategy::SpatialHashGrid,
            cell_size: SpatialCellSize2::Fixed {
                width: 8,
                height: 8,
            },
            ..BroadPhase2Config::default()
        })
        .unwrap();
        let pairs = grid.candidate_pairs_between(&left, &right).unwrap();

        assert_eq!(
            pairs,
            &[CollisionPair {
                left_index: 0,
                right_index: 0
            }]
        );
        assert_eq!(grid.stats().object_count, 4);
        assert_eq!(grid.stats().candidate_pair_count, 1);
    }

    #[test]
    fn broad_phase_rejects_invalid_fixed_cell_sizes() {
        assert!(BroadPhase2Config {
            cell_size: SpatialCellSize2::Fixed {
                width: 0,
                height: 8,
            },
            ..BroadPhase2Config::default()
        }
        .validate()
        .is_err());
    }
}
