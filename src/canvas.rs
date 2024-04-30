use core::panic;
use std::{cmp, collections::HashMap, io::Write, ops::Range};

use chrono::DateTime;
use rand::Rng;

use crate::{
    layer::Layer, objects::Object, Anchor, CenterAnchor, Color, ColorMapping, Fill, LineSegment,
    ObjectSizes, Region,
};

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
