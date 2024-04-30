#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Anchor(pub i32, pub i32);

impl Anchor {
    pub fn translate(&mut self, dx: i32, dy: i32) {
        self.0 += dx;
        self.1 += dy;
    }
}

impl From<(i32, i32)> for Anchor {
    fn from(value: (i32, i32)) -> Self {
        Anchor(value.0, value.1)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CenterAnchor(pub i32, pub i32);

impl CenterAnchor {
    pub fn translate(&mut self, dx: i32, dy: i32) {
        self.0 += dx;
        self.1 += dy;
    }
}

pub trait Coordinates {
    fn coords(&self, cell_size: usize) -> (f32, f32);
    fn center() -> Self;
}

impl Coordinates for Anchor {
    fn coords(&self, cell_size: usize) -> (f32, f32) {
        match self {
            Anchor(-1, -1) => (cell_size as f32 / 2.0, cell_size as f32 / 2.0),
            Anchor(i, j) => {
                let x = (i * cell_size as i32) as f32;
                let y = (j * cell_size as i32) as f32;
                (x, y)
            }
        }
    }

    fn center() -> Self {
        Anchor(-1, -1)
    }
}

impl Coordinates for CenterAnchor {
    fn coords(&self, cell_size: usize) -> (f32, f32) {
        match self {
            CenterAnchor(-1, -1) => ((cell_size / 2) as f32, (cell_size / 2) as f32),
            CenterAnchor(i, j) => {
                let x = *i as f32 * cell_size as f32 + cell_size as f32 / 2.0;
                let y = *j as f32 * cell_size as f32 + cell_size as f32 / 2.0;
                (x, y)
            }
        }
    }

    fn center() -> Self {
        CenterAnchor(-1, -1)
    }
}
