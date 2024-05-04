use crate::Point;
use rand::Rng;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[derive(Debug, Clone, Default, Copy)]
pub struct Region {
    pub start: Point,
    pub end: Point,
}

impl Region {
    /// iterates from left to right then top to bottom (in a "row-major" order)
    pub fn iter(&self) -> RegionIterator {
        self.into()
    }

    pub fn random_point_within(&self) -> Point {
        Point::from(self.random_coordinates_within())
    }

    pub fn random_point_within_except(&self, except: &Region) -> Point {
        // XXX this is probably not a good idea lmao
        loop {
            let point = self.random_point_within();
            if !except.contains(&point) {
                return point;
            }
        }
    }
}

pub struct RegionIterator {
    region: Region,
    current: Point,
}

impl Iterator for RegionIterator {
    type Item = Point;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current.0 > self.region.end.0 {
            self.current.0 = self.region.start.0;
            self.current.1 += 1;
        }
        if self.current.1 > self.region.end.1 {
            return None;
        }
        let result = self.current;
        self.current.0 += 1;
        Some(result)
    }
}

impl From<&Region> for RegionIterator {
    fn from(region: &Region) -> Self {
        Self {
            region: region.clone(),
            current: region.start.clone(),
        }
    }
}

impl From<(&Point, &Point)> for Region {
    fn from(value: (&Point, &Point)) -> Self {
        Self {
            start: value.0.clone(),
            end: value.1.clone(),
        }
    }
}

impl From<(Point, Point)> for Region {
    fn from(value: (Point, Point)) -> Self {
        Self {
            start: value.0,
            end: value.1,
        }
    }
}

impl From<((usize, usize), (usize, usize))> for Region {
    fn from(value: ((usize, usize), (usize, usize))) -> Self {
        Region {
            start: value.0.into(),
            end: value.1.into(),
        }
    }
}

impl std::ops::Sub for Region {
    type Output = (i32, i32);

    fn sub(self, rhs: Self) -> Self::Output {
        (
            (self.start.0 as i32 - rhs.start.0 as i32),
            (self.start.1 as i32 - rhs.start.1 as i32),
        )
    }
}

#[test]
fn test_sub_and_transate_coherence() {
    let a = Region::from_origin(Point(3, 3));
    let mut b = a.clone();
    b.translate(2, 3);

    assert_eq!(b - a, (2, 3));
}

impl Region {
    pub fn new(start_x: usize, start_y: usize, end_x: usize, end_y: usize) -> Self {
        let region = Self {
            start: (start_x, start_y).into(),
            end: (end_x, end_y).into(),
        };
        region.ensure_valid();
        region
    }

    pub fn bottomleft(&self) -> Point {
        Point(self.start.0, self.end.1)
    }

    pub fn bottomright(&self) -> Point {
        Point(self.end.0, self.end.1)
    }

    pub fn topleft(&self) -> Point {
        Point(self.start.0, self.start.1)
    }

    pub fn topright(&self) -> Point {
        Point(self.end.0, self.start.1)
    }

    pub fn max<'a>(&'a self, other: &'a Region) -> &'a Region {
        if self.within(other) {
            other
        } else {
            self
        }
    }

    pub fn random_coordinates_within(&self) -> (i32, i32) {
        (
            rand::thread_rng().gen_range(self.start.0..self.end.0) as i32,
            rand::thread_rng().gen_range(self.start.1..self.end.1) as i32,
        )
    }

    pub fn from_origin(end: Point) -> Self {
        Self::new(0, 0, end.0, end.1)
    }

    pub fn from_topleft(origin: Point, size: (usize, usize)) -> Self {
        Self::from((
            origin,
            origin.translated_by(Point::from(size).translated(-1, -1)),
        ))
    }

    pub fn from_bottomleft(origin: Point, size: (usize, usize)) -> Self {
        Self::from_topleft(origin.translated(0, -(size.1 as i32 - 1)), size)
    }

    pub fn from_bottomright(origin: Point, size: (usize, usize)) -> Self {
        Self::from((
            origin.translated_by(Point::from(size).translated(-1, -1)),
            origin,
        ))
    }

    pub fn from_topright(origin: Point, size: (usize, usize)) -> Self {
        Self::from_topleft(origin.translated(-(size.0 as i32 - 1), 0), size)
    }

    pub fn from_center_and_size(center: Point, size: (usize, usize)) -> Self {
        let half_size = (size.0 / 2, size.1 / 2);
        Self::new(
            center.0 - half_size.0,
            center.1 - half_size.1,
            center.0 + half_size.0,
            center.1 + half_size.1,
        )
    }

    // panics if the region is invalid
    pub fn ensure_valid(self) -> Self {
        if self.start.0 >= self.end.0 || self.start.1 >= self.end.1 {
            panic!(
                "Invalid region: start ({:?}) >= end ({:?})",
                self.start, self.end
            )
        }
        self
    }

    pub fn translate(&mut self, dx: i32, dy: i32) {
        *self = self.translated(dx, dy);
    }

    pub fn translated(&self, dx: i32, dy: i32) -> Self {
        Self {
            start: (
                (self.start.0 as i32 + dx) as usize,
                (self.start.1 as i32 + dy) as usize,
            )
                .into(),
            end: (
                (self.end.0 as i32 + dx) as usize,
                (self.end.1 as i32 + dy) as usize,
            )
                .into(),
        }
        .ensure_valid()
    }

    /// adds dx and dy to the end of the region (dx and dy are _not_ multiplicative but **additive** factors)
    pub fn enlarged(&self, dx: i32, dy: i32) -> Self {
        Self {
            start: self.start,
            end: (
                (self.end.0 as i32 + dx) as usize,
                (self.end.1 as i32 + dy) as usize,
            )
                .into(),
        }
        .ensure_valid()
    }

    /// resized is like enlarged, but transforms from the center, by first translating the region by (-dx, -dy)
    pub fn resized(&self, dx: i32, dy: i32) -> Self {
        self.translated(-dx, -dy).enlarged(dx - 1, dy - 1)
    }

    pub fn x_range(&self) -> std::ops::RangeInclusive<usize> {
        self.start.0..=self.end.0
    }
    pub fn y_range(&self) -> std::ops::RangeInclusive<usize> {
        self.start.1..=self.end.1
    }

    pub fn x_range_without_last(&self) -> std::ops::Range<usize> {
        self.start.0..self.end.0
    }

    pub fn y_range_without_last(&self) -> std::ops::Range<usize> {
        self.start.1..self.end.1
    }

    pub fn within(&self, other: &Region) -> bool {
        self.start.0 >= other.start.0
            && self.start.1 >= other.start.1
            && self.end.0 <= other.end.0
            && self.end.1 <= other.end.1
    }

    pub fn clamped(&self, within: &Region) -> Region {
        Region {
            start: (
                self.start.0.max(within.start.0),
                self.start.1.max(within.start.1),
            )
                .into(),
            end: (self.end.0.min(within.end.0), self.end.1.min(within.end.1)).into(),
        }
    }

    pub fn width(&self) -> usize {
        self.end.0 - self.start.0 + 1
    }

    pub fn height(&self) -> usize {
        self.end.1 - self.start.1 + 1
    }

    // goes from -width to width (inclusive on both ends)
    pub fn mirrored_width_range(&self) -> std::ops::RangeInclusive<i32> {
        let w = self.width() as i32;
        -w..=w
    }

    pub fn mirrored_height_range(&self) -> std::ops::RangeInclusive<i32> {
        let h = self.height() as i32;
        -h..=h
    }
}

pub trait Containable<T> {
    fn contains(&self, value: &T) -> bool;
}

impl Containable<Point> for Region {
    fn contains(&self, value: &Point) -> bool {
        self.x_range().contains(&value.0) && self.y_range().contains(&value.1)
    }
}

impl std::fmt::Display for Region {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{},{}]", self.start, self.end)
    }
}
