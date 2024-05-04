use wasm_bindgen::prelude::*;

use crate::Region;

#[wasm_bindgen]
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Point(pub usize, pub usize);

impl Point {
    pub fn translated(&self, dx: i32, dy: i32) -> Self {
        Self((self.0 as i32 + dx) as usize, (self.1 as i32 + dy) as usize)
    }

    pub fn translated_by(&self, point: Point) -> Self {
        Self(self.0 + point.0, self.1 + point.1)
    }

    pub fn region(&self) -> Region {
        Region {
            start: self.clone(),
            end: self.clone(),
        }
    }

    pub fn translate(&mut self, dx: i32, dy: i32) {
        self.0 = (self.0 as i32 + dx) as usize;
        self.1 = (self.1 as i32 + dy) as usize;
    }

    pub fn coords(&self, cell_size: usize) -> (f32, f32) {
        ((self.0 * cell_size) as f32, (self.1 * cell_size) as f32)
    }

    /// get SVG coordinates of the cell's center instead of its origin (top-left)
    pub fn center_coords(&self, cell_size: usize) -> (f32, f32) {
        let (x, y) = self.coords(cell_size);
        (x + cell_size as f32 / 2.0, y + cell_size as f32 / 2.0)
    }

    pub fn distances(&self, other: &Point) -> (usize, usize) {
        (self.0.abs_diff(other.0) + 1, self.1.abs_diff(other.1) + 1)
    }
}

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

impl Eq for Point {}

impl std::fmt::Display for Point {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "({}, {})", self.0, self.1)
    }
}
