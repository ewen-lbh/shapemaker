use core::panic;
use std::{
    cmp,
    collections::HashMap,
    io::Write,
    ops::{Range, RangeInclusive},
};

use chrono::DateTime;
use rand::Rng;

use crate::{layer::Layer, Color, ColorMapping};

#[derive(Debug, Clone, Default, Copy)]
pub struct Region {
    pub start: (usize, usize),
    pub end: (usize, usize),
}

impl From<((usize, usize), (usize, usize))> for Region {
    fn from(value: ((usize, usize), (usize, usize))) -> Self {
        Region {
            start: value.0,
            end: value.1,
        }
    }
}

impl From<(&Anchor, &Anchor)> for Region {
    fn from(value: (&Anchor, &Anchor)) -> Self {
        Region {
            start: (value.0 .0 as usize, value.0 .1 as usize),
            end: (value.1 .0 as usize, value.1 .1 as usize),
        }
    }
}

impl From<(&CenterAnchor, &CenterAnchor)> for Region {
    fn from(value: (&CenterAnchor, &CenterAnchor)) -> Self {
        Region {
            start: (value.0 .0 as usize, value.0 .1 as usize),
            end: (value.1 .0 as usize, value.1 .1 as usize),
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
            start: (start_x, start_y),
            end: (end_x, end_y),
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
            ),
            end: (
                (self.end.0 as i32 + dx) as usize,
                (self.end.1 as i32 + dy) as usize,
            ),
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
            ),
        }
        .ensure_valid()
    }

    /// resized is like enlarged, but transforms from the center, by first translating the region by (-dx, -dy)
    pub fn resized(&self, dx: i32, dy: i32) -> Self {
        self.translated(-dx, -dy).enlarged(dx, dy)
    }

    pub fn x_range(&self) -> Range<usize> {
        self.start.0..self.end.0
    }
    pub fn y_range(&self) -> Range<usize> {
        self.start.1..self.end.1
    }

    pub fn x_range_without_last(&self) -> Range<usize> {
        self.start.0..self.end.0 - 1
    }

    pub fn y_range_without_last(&self) -> Range<usize> {
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
            ),
            end: (self.end.0.min(within.end.0), self.end.1.min(within.end.1)),
        }
    }

    pub fn width(&self) -> usize {
        self.end.0 - self.start.0
    }

    pub fn height(&self) -> usize {
        self.end.1 - self.start.1
    }

    // goes from -width to width (inclusive on both ends)
    pub fn mirrored_width_range(&self) -> RangeInclusive<i32> {
        let w = self.width() as i32;
        -w..=w
    }

    pub fn mirrored_height_range(&self) -> RangeInclusive<i32> {
        let h = self.height() as i32;
        -h..=h
    }

    fn contains(&self, anchor: &Anchor) -> bool {
        self.x_range().contains(&(anchor.0 as usize))
            && self.y_range().contains(&(anchor.1 as usize))
    }
}

#[derive(Debug, Clone)]
pub struct Canvas {
    pub grid_size: (usize, usize),
    pub cell_size: usize,
    pub objects_count_range: Range<usize>,
    pub polygon_vertices_range: Range<usize>,
    pub canvas_outter_padding: usize,
    pub object_sizes: ObjectSizes,
    pub colormap: ColorMapping,
    /// The layers are in order of top to bottom: the first layer will be rendered on top of the second, etc.
    pub layers: Vec<Layer>,
    pub background: Option<Color>,

    pub world_region: Region,
}

impl Canvas {
    /// Create a new canvas.
    /// The layers are in order of top to bottom: the first layer will be rendered on top of the second, etc.
    /// A layer named "root" will be added below all layers if you don't add it yourself.
    pub fn new(layer_names: Vec<&str>) -> Self {
        let mut layer_names = layer_names;
        if !layer_names.iter().any(|&name| name == "root") {
            layer_names.push("root");
        }
        Self {
            layers: layer_names
                .iter()
                .map(|name| Layer {
                    object_sizes: ObjectSizes::default(),
                    objects: HashMap::new(),
                    name: name.to_string(),
                    _render_cache: None,
                })
                .collect(),
            ..Self::default_settings()
        }
    }

    pub fn set_grid_size(&mut self, new_width: usize, new_height: usize) {
        self.grid_size = (new_width, new_height);
        self.world_region = Region {
            start: (0, 0),
            end: self.grid_size,
        };
    }

    pub fn layer_safe(&mut self, name: &str) -> Option<&mut Layer> {
        self.layers.iter_mut().find(|layer| layer.name == name)
    }

    pub fn layer(&mut self, name: &str) -> &mut Layer {
        self.layer_safe(name).unwrap()
    }

    pub fn ensure_layer_exists(&self, name: &str) {
        self.layers
            .iter()
            .find(|layer| layer.name == name)
            .or_else(|| panic!("Layer {} does not exist", name));
    }

    /// puts this layer on top, and the others below, without changing their order
    pub fn put_layer_on_top(&mut self, name: &str) {
        self.ensure_layer_exists(name);
        self.layers.sort_by(|a, _| {
            if a.name == name {
                cmp::Ordering::Greater
            } else {
                cmp::Ordering::Less
            }
        })
    }

    /// puts this layer on bottom, and the others above, without changing their order
    pub fn put_layer_on_bottom(&mut self, name: &str) {
        self.ensure_layer_exists(name);
        self.layers.sort_by(|a, _| {
            if a.name == name {
                cmp::Ordering::Less
            } else {
                cmp::Ordering::Greater
            }
        })
    }

    pub fn root(&mut self) -> &mut Layer {
        self.layer_safe("root")
            .expect("Layer 'root' should always exist in a canvas")
    }

    pub fn add_object(
        &mut self,
        layer: &str,
        name: &str,
        object: Object,
        fill: Option<Fill>,
    ) -> Result<(), String> {
        match self.layer_safe(layer) {
            None => Err(format!("Layer {} does not exist", layer)),
            Some(layer) => {
                layer.objects.insert(name.to_string(), (object, fill));
                Ok(())
            }
        }
    }

    pub fn remove_object(&mut self, name: &str) {
        for layer in self.layers.iter_mut() {
            layer.remove_object(name);
        }
    }

    pub fn set_background(&mut self, color: Color) {
        self.background = Some(color);
    }

    pub fn remove_background(&mut self) {
        self.background = None;
    }

    pub fn default_settings() -> Self {
        Self {
            grid_size: (3, 3),
            cell_size: 50,
            objects_count_range: 3..7,
            polygon_vertices_range: 2..7,
            canvas_outter_padding: 10,
            object_sizes: ObjectSizes::default(),
            colormap: ColorMapping::default(),
            layers: vec![],
            world_region: Region::new(0, 0, 3, 3),
            background: None,
        }
    }

    pub fn random_layer(&self, name: &'static str) -> Layer {
        self.random_layer_within(name, &self.world_region)
    }

    pub fn random_object(&self) -> Object {
        self.random_object_within(&self.world_region)
    }

    pub fn replace_or_create_layer(&mut self, layer: Layer) {
        if let Some(existing_layer) = self.layer_safe(&layer.name) {
            existing_layer.replace(layer);
        } else {
            self.layers.push(layer);
        }
    }

    pub fn random_layer_within(&self, name: &'static str, region: &Region) -> Layer {
        let mut objects: HashMap<String, (Object, Option<Fill>)> = HashMap::new();
        let number_of_objects = rand::thread_rng().gen_range(self.objects_count_range.clone());
        for i in 0..number_of_objects {
            let object = self.random_object_within(region);
            objects.insert(
                format!("{}#{}", name, i),
                (
                    object,
                    if rand::thread_rng().gen_bool(0.5) {
                        Some(self.random_fill())
                    } else {
                        None
                    },
                ),
            );
        }
        Layer {
            object_sizes: self.object_sizes.clone(),
            name: name.to_string(),
            objects,
            _render_cache: None,
        }
    }

    pub fn random_linelikes_within(&self, layer_name: &'static str, region: &Region) -> Layer {
        let mut objects: HashMap<String, (Object, Option<Fill>)> = HashMap::new();
        let number_of_objects = rand::thread_rng().gen_range(self.objects_count_range.clone());
        for i in 0..number_of_objects {
            let object = self.random_linelike_within(region);
            objects.insert(
                format!("{}#{}", layer_name, i),
                (
                    object,
                    if rand::thread_rng().gen_bool(0.5) {
                        Some(self.random_fill())
                    } else {
                        None
                    },
                ),
            );
        }
        Layer {
            object_sizes: self.object_sizes.clone(),
            name: layer_name.to_owned(),
            objects,
            _render_cache: None,
        }
    }

    pub fn random_object_within(&self, region: &Region) -> Object {
        let start = self.random_anchor(region);
        match rand::thread_rng().gen_range(1..=7) {
            1 => self.random_polygon(region),
            2 => Object::BigCircle(self.random_center_anchor(region)),
            3 => Object::SmallCircle(start),
            4 => Object::Dot(start),
            5 => Object::CurveInward(
                start,
                self.random_end_anchor(start, region),
                self.object_sizes.default_line_width,
            ),
            6 => Object::CurveOutward(
                start,
                self.random_end_anchor(start, region),
                self.object_sizes.default_line_width,
            ),
            7 => Object::Line(
                self.random_anchor(region),
                self.random_anchor(region),
                self.object_sizes.default_line_width,
            ),
            _ => unreachable!(),
        }
    }

    pub fn random_linelike_within(&self, region: &Region) -> Object {
        let start = self.random_anchor(region);
        match rand::thread_rng().gen_range(1..=3) {
            1 => Object::CurveInward(
                start,
                self.random_end_anchor(start, region),
                self.object_sizes.default_line_width,
            ),
            2 => Object::CurveOutward(
                start,
                self.random_end_anchor(start, region),
                self.object_sizes.default_line_width,
            ),
            3 => Object::Line(
                self.random_anchor(region),
                self.random_anchor(region),
                self.object_sizes.default_line_width,
            ),
            _ => unreachable!(),
        }
    }

    pub fn random_end_anchor(&self, start: Anchor, region: &Region) -> Anchor {
        // End anchors are always a square diagonal from the start anchor (for now)
        // that means taking steps of the form n * (one of (1, 1), (1, -1), (-1, 1), (-1, -1))
        // Except that the end anchor needs to stay in the bounds of the shape.

        // Determine all possible end anchors that are in a square diagonal from the start anchor
        let mut possible_end_anchors = vec![];

        for x in region.mirrored_width_range() {
            for y in region.mirrored_height_range() {
                let end_anchor = Anchor(start.0 + x, start.1 + y);

                if end_anchor == start {
                    continue;
                }

                // Check that the end anchor is in a square diagonal from the start anchor and that the end anchor is in bounds
                if x.abs() == y.abs() && region.contains(&end_anchor) {
                    possible_end_anchors.push(end_anchor);
                }
            }
        }

        // Pick a random end anchor from the possible end anchors
        possible_end_anchors[rand::thread_rng().gen_range(0..possible_end_anchors.len())]
    }

    pub fn random_polygon(&self, region: &Region) -> Object {
        let number_of_anchors = rand::thread_rng().gen_range(self.polygon_vertices_range.clone());
        let start = self.random_anchor(region);
        let mut lines: Vec<LineSegment> = vec![];
        for _ in 0..number_of_anchors {
            let next_anchor = self.random_anchor(region);
            lines.push(self.random_line(next_anchor));
        }
        Object::Polygon(start, lines)
    }

    pub fn random_line(&self, end: Anchor) -> LineSegment {
        match rand::thread_rng().gen_range(1..=3) {
            1 => LineSegment::Straight(end),
            2 => LineSegment::InwardCurve(end),
            3 => LineSegment::OutwardCurve(end),
            _ => unreachable!(),
        }
    }

    pub fn region_is_whole_grid(&self, region: &Region) -> bool {
        region.start == (0, 0) && region.end == self.grid_size
    }

    pub fn random_anchor(&self, region: &Region) -> Anchor {
        if self.region_is_whole_grid(region)
            && rand::thread_rng().gen_bool(1.0 / (self.grid_size.0 * self.grid_size.1) as f64)
        {
            // small change of getting center (-1, -1) even when grid size would not permit it (e.g. 4x4)
            Anchor(-1, -1)
        } else {
            Anchor(
                rand::thread_rng().gen_range(region.x_range()) as i32,
                rand::thread_rng().gen_range(region.y_range()) as i32,
            )
        }
    }

    pub fn random_center_anchor(&self, region: &Region) -> CenterAnchor {
        if self.region_is_whole_grid(region)
            && rand::thread_rng().gen_bool(
                1.0 / ((self.grid_size.0 as i32 - 1) * (self.grid_size.1 as i32 - 1)) as f64,
            )
        {
            // small change of getting center (-1, -1) even when grid size would not permit it (e.g. 3x3)
            CenterAnchor(-1, -1)
        } else {
            CenterAnchor(
                rand::thread_rng().gen_range(region.x_range_without_last()) as i32,
                rand::thread_rng().gen_range(region.y_range_without_last()) as i32,
            )
        }
    }

    pub fn random_fill(&self) -> Fill {
        Fill::Solid(self.random_color())
        // match rand::thread_rng().gen_range(1..=3) {
        //     1 => Fill::Solid(random_color()),
        //     2 => Fill::Hatched,
        //     3 => Fill::Dotted,
        //     _ => unreachable!(),
        // }
    }

    pub fn random_color(&self) -> Color {
        match rand::thread_rng().gen_range(1..=12) {
            1 => Color::Black,
            2 => Color::White,
            3 => Color::Red,
            4 => Color::Green,
            5 => Color::Blue,
            6 => Color::Yellow,
            7 => Color::Orange,
            8 => Color::Purple,
            9 => Color::Brown,
            10 => Color::Pink,
            11 => Color::Gray,
            12 => Color::Cyan,
            _ => unreachable!(),
        }
    }

    pub fn clear(&mut self) {
        self.layers.clear();
        self.remove_background()
    }

    pub fn save_as_png(
        at: &str,
        aspect_ratio: f32,
        resolution: usize,
        rendered: String,
    ) -> Result<(), String> {
        let (height, width) = if aspect_ratio > 1.0 {
            // landscape: resolution is width
            (resolution, (resolution as f32 * aspect_ratio) as usize)
        } else {
            // portrait: resolution is height
            ((resolution as f32 / aspect_ratio) as usize, resolution)
        };
        let mut spawned = std::process::Command::new("magick")
            .args(["-background", "none"])
            .args(["-size", &format!("{}x{}", width, height)])
            .arg("-")
            .arg(at)
            .stdin(std::process::Stdio::piped())
            .spawn()
            .unwrap();

        let stdin = spawned.stdin.as_mut().unwrap();
        stdin.write_all(rendered.as_bytes()).unwrap();
        drop(stdin);

        match spawned.wait_with_output() {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("Failed to execute convert: {}", e)),
        }
    }
}

impl Canvas {
    pub fn width(&self) -> usize {
        self.cell_size * (self.grid_size.0 - 1) + 2 * self.canvas_outter_padding
    }

    pub fn height(&self) -> usize {
        self.cell_size * (self.grid_size.1 - 1) + 2 * self.canvas_outter_padding
    }

    pub fn render(&mut self, layers: &Vec<&str>, render_background: bool) -> String {
        let background_color = self.background.unwrap_or_default();
        let mut svg = svg::Document::new();
        if render_background {
            svg = svg.add(
                svg::node::element::Rectangle::new()
                    .set("x", -(self.canvas_outter_padding as i32))
                    .set("y", -(self.canvas_outter_padding as i32))
                    .set("width", self.width())
                    .set("height", self.height())
                    .set("fill", background_color.to_string(&self.colormap)),
            );
        }
        for layer in self
            .layers
            .iter_mut()
            .filter(|layer| layers.contains(&"*") || layers.contains(&layer.name.as_str()))
            .rev()
        {
            svg = svg.add(layer.render(self.colormap.clone(), self.cell_size, layer.object_sizes));
        }

        svg.set(
            "viewBox",
            format!(
                "{0} {0} {1} {2}",
                -(self.canvas_outter_padding as i32),
                self.width(),
                self.height()
            ),
        )
        .set("width", self.width())
        .set("height", self.height())
        .to_string()
    }
}

pub fn milliseconds_to_timestamp(ms: usize) -> String {
    format!(
        "{}",
        DateTime::from_timestamp_millis(ms as i64)
            .unwrap()
            .format("%H:%M:%S%.3f")
    )
}

#[derive(Debug, Clone, Copy)]
pub struct ObjectSizes {
    pub empty_shape_stroke_width: f32,
    pub small_circle_radius: f32,
    pub dot_radius: f32,
    pub default_line_width: f32,
}

impl Default for ObjectSizes {
    fn default() -> Self {
        Self {
            empty_shape_stroke_width: 0.5,
            small_circle_radius: 5.0,
            dot_radius: 2.0,
            default_line_width: 2.0,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Object {
    Polygon(Anchor, Vec<LineSegment>),
    Line(Anchor, Anchor, f32),
    CurveOutward(Anchor, Anchor, f32),
    CurveInward(Anchor, Anchor, f32),
    SmallCircle(Anchor),
    Dot(Anchor),
    BigCircle(CenterAnchor),
    Text(Anchor, String, f32),
    Rectangle(Anchor, Anchor),
    RawSVG(Box<dyn svg::Node>),
}

impl Object {
    pub fn translate(&mut self, dx: i32, dy: i32) {
        match self {
            Object::Polygon(start, lines) => {
                start.translate(dx, dy);
                for line in lines {
                    match line {
                        LineSegment::InwardCurve(anchor)
                        | LineSegment::OutwardCurve(anchor)
                        | LineSegment::Straight(anchor) => anchor.translate(dx, dy),
                    }
                }
            }
            Object::Line(start, end, _)
            | Object::CurveInward(start, end, _)
            | Object::CurveOutward(start, end, _)
            | Object::Rectangle(start, end) => {
                start.translate(dx, dy);
                end.translate(dx, dy);
            }
            Object::Text(anchor, _, _) | Object::Dot(anchor) | Object::SmallCircle(anchor) => {
                anchor.translate(dx, dy)
            }
            Object::BigCircle(center) => center.translate(dx, dy),
            Object::RawSVG(_) => {
                unimplemented!()
            }
        }
    }

    pub fn translate_with(&mut self, delta: (i32, i32)) {
        self.translate(delta.0, delta.1)
    }

    pub fn teleport(&mut self, x: i32, y: i32) {
        let (current_x, current_y) = self.region().start;
        let delta_x = x - current_x as i32;
        let delta_y = y - current_y as i32;
        self.translate(delta_x, delta_y);
    }

    pub fn teleport_with(&mut self, position: (i32, i32)) {
        self.teleport(position.0, position.1)
    }

    pub fn region(&self) -> Region {
        match self {
            Object::Polygon(start, lines) => {
                let mut region: Region = (start, start).into();
                for line in lines {
                    match line {
                        LineSegment::InwardCurve(anchor)
                        | LineSegment::OutwardCurve(anchor)
                        | LineSegment::Straight(anchor) => {
                            region = *region.max(&(start, anchor).into())
                        }
                    }
                }
                region
            }
            Object::Line(start, end, _)
            | Object::CurveInward(start, end, _)
            | Object::CurveOutward(start, end, _)
            | Object::Rectangle(start, end) => (start, end).into(),
            Object::Text(anchor, _, _) | Object::Dot(anchor) | Object::SmallCircle(anchor) => {
                (anchor, anchor).into()
            }
            Object::BigCircle(center) => (center, center).into(), // FIXME will be wrong lmao,
            Object::RawSVG(_) => {
                unimplemented!()
            }
        }
    }
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineSegment {
    Straight(Anchor),
    InwardCurve(Anchor),
    OutwardCurve(Anchor),
}

#[derive(Debug, Clone, Copy)]
pub enum Fill {
    Solid(Color),
    Translucent(Color, f32),
    Hatched,
    Dotted,
}
