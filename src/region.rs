use crate::{Anchor, CenterAnchor};
use rand::Rng;

use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Point(pub usize, pub usize);

impl From<(usize, usize)> for Point {
    fn from(value: (usize, usize)) -> Self {
        Self(value.0, value.1)
    }
}

impl From<(i32, i32)> for Point {
    fn from(value: (i32, i32)) -> Self {
        Self(value.0 as usize, value.1 as usize)
    }
}

impl PartialEq<(usize, usize)> for Point {
    fn eq(&self, other: &(usize, usize)) -> bool {
        self.0 == other.0 && self.1 == other.1
    }
}

#[wasm_bindgen]
#[derive(Debug, Clone, Default, Copy)]
pub struct Region {
    pub start: Point,
    pub end: Point,
}

impl From<((usize, usize), (usize, usize))> for Region {
    fn from(value: ((usize, usize), (usize, usize))) -> Self {
        Region {
            start: value.0.into(),
            end: value.1.into(),
        }
    }
}

impl From<(&Anchor, &Anchor)> for Region {
    fn from(value: (&Anchor, &Anchor)) -> Self {
        Region {
            start: (value.0 .0, value.0 .1).into(),
            end: (value.1 .0, value.1 .1).into(),
        }
    }
}

impl From<(&CenterAnchor, &CenterAnchor)> for Region {
    fn from(value: (&CenterAnchor, &CenterAnchor)) -> Self {
        Region {
            start: (value.0 .0, value.0 .1).into(),
            end: (value.1 .0, value.1 .1).into(),
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
    let a = Region::from_origin((3, 3));
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

    pub fn from_origin(end: (usize, usize)) -> Self {
        Self::new(0, 0, end.0, end.1)
    }

    pub fn from_origin_and_size(origin: (usize, usize), size: (usize, usize)) -> Self {
        Self::new(
            origin.0,
            origin.1,
            origin.0 + size.0 + 1,
            origin.1 + size.1 + 1,
        )
    }

    pub fn from_center_and_size(center: (usize, usize), size: (usize, usize)) -> Self {
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
        self.translated(-dx, -dy).enlarged(dx, dy)
    }

    pub fn x_range(&self) -> std::ops::Range<usize> {
        self.start.0..self.end.0
    }
    pub fn y_range(&self) -> std::ops::Range<usize> {
        self.start.1..self.end.1
    }

    pub fn x_range_without_last(&self) -> std::ops::Range<usize> {
        self.start.0..self.end.0 - 1
    }

    pub fn y_range_without_last(&self) -> std::ops::Range<usize> {
        self.start.1..self.end.1 - 1
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
        self.end.0 - self.start.0
    }

    pub fn height(&self) -> usize {
        self.end.1 - self.start.1
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

    pub fn contains(&self, anchor: &Anchor) -> bool {
        self.x_range().contains(&(anchor.0 as usize))
            && self.y_range().contains(&(anchor.1 as usize))
    }
}
