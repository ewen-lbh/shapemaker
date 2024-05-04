use core::panic;
use std::{
    cmp,
    collections::HashMap,
    io::{empty, Write},
    ops::Range,
};

use chrono::DateTime;
use itertools::Itertools;
use rand::Rng;

use crate::{
    layer::Layer, objects::Object, random_color, Color, ColorMapping, ColoredObject, Containable,
    Fill, Filter, HatchDirection, LineSegment, ObjectSizes, Point, Region,
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
            start: Point(0, 0),
            end: Point::from(self.grid_size).translated(-1, -1),
        };
    }

    pub fn layer_safe(&mut self, name: &str) -> Option<&mut Layer> {
        self.layers.iter_mut().find(|layer| layer.name == name)
    }

    pub fn layer(&mut self, name: &str) -> &mut Layer {
        self.layer_safe(name).unwrap()
    }

    pub fn new_layer(&mut self, name: &str) -> &mut Layer {
        if self.layer_exists(name) {
            panic!("Layer {} already exists", name);
        }

        self.layers.push(Layer::new(name));
        self.layer(name)
    }

    pub fn layer_or_empty(&mut self, name: &str) -> &mut Layer {
        if self.layer_exists(name) {
            return self.layer(name);
        }

        self.new_layer(name)
    }

    pub fn layer_exists(&self, name: &str) -> bool {
        self.layers.iter().any(|layer| layer.name == name)
    }

    pub fn ensure_layer_exists(&self, name: &str) {
        if !self.layer_exists(name) {
            panic!("Layer {} does not exist", name);
        }
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
                layer.add_object(name, (object, fill).into());
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

    pub fn random_layer(&self, name: &str) -> Layer {
        self.random_layer_within(name, &self.world_region)
    }

    pub fn random_object(&self) -> Object {
        self.random_object_within(&self.world_region)
    }

    pub fn add_or_replace_layer(&mut self, layer: Layer) {
        if let Some(existing_layer) = self.layer_safe(&layer.name) {
            existing_layer.replace(layer);
        } else {
            self.layers.push(layer);
        }
    }

    pub fn random_layer_within(&self, name: &str, region: &Region) -> Layer {
        let mut objects: HashMap<String, ColoredObject> = HashMap::new();
        let number_of_objects = rand::thread_rng().gen_range(self.objects_count_range.clone());
        for i in 0..number_of_objects {
            let object = self.random_object_within(region);
            let hatchable = object.hatchable();
            objects.insert(
                format!("{}#{}", name, i),
                ColoredObject(
                    object,
                    if rand::thread_rng().gen_bool(0.5) {
                        Some(self.random_fill(hatchable))
                    } else {
                        None
                    },
                    vec![],
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

    pub fn random_linelikes(&self, layer_name: &str) -> Layer {
        self.random_linelikes_within(layer_name, &self.world_region)
    }

    pub fn n_random_linelikes_within(
        &self,
        layer_name: &str,
        region: &Region,
        count: usize,
    ) -> Layer {
        let mut objects: HashMap<String, ColoredObject> = HashMap::new();
        for i in 0..count {
            let object = self.random_linelike_within(region);
            let hatchable = object.fillable();
            objects.insert(
                format!("{}#{}", layer_name, i),
                ColoredObject(
                    object,
                    if rand::thread_rng().gen_bool(0.5) {
                        Some(self.random_fill(hatchable))
                    } else {
                        None
                    },
                    vec![],
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

    pub fn random_linelikes_within(&self, layer_name: &str, region: &Region) -> Layer {
        let number_of_objects = rand::thread_rng().gen_range(self.objects_count_range.clone());
        self.n_random_linelikes_within(layer_name, region, number_of_objects)
    }

    pub fn random_object_within(&self, region: &Region) -> Object {
        let start = self.random_point(region);
        match rand::thread_rng().gen_range(1..=7) {
            1 => self.random_polygon(region),
            2 => Object::BigCircle(start),
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
                self.random_point(region),
                self.random_point(region),
                self.object_sizes.default_line_width,
            ),
            _ => unreachable!(),
        }
    }

    pub fn random_linelike_within(&self, region: &Region) -> Object {
        let start = self.random_point(region);
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
                self.random_point(region),
                self.random_point(region),
                self.object_sizes.default_line_width,
            ),
            _ => unreachable!(),
        }
    }

    pub fn random_end_anchor(&self, start: Point, region: &Region) -> Point {
        // End anchors are always a square diagonal from the start anchor (for now)
        // that means taking steps of the form n * (one of (1, 1), (1, -1), (-1, 1), (-1, -1))
        // Except that the end anchor needs to stay in the bounds of the shape.

        // Determine all possible end anchors that are in a square diagonal from the start anchor
        let mut possible_end_anchors = vec![];

        // shapes can end on the next cell, since that's where they end
        let actual_region = region.enlarged(1, 1);

        for x in actual_region.mirrored_width_range() {
            for y in actual_region.mirrored_height_range() {
                let end_anchor = start.translated(x, y);

                if end_anchor == start {
                    continue;
                }

                // Check that the end anchor is in a square diagonal from the start anchor and that the end anchor is in bounds
                if x.abs() == y.abs() && actual_region.contains(&end_anchor) {
                    possible_end_anchors.push(end_anchor);
                }
            }
        }

        // Pick a random end anchor from the possible end anchors
        possible_end_anchors[rand::thread_rng().gen_range(0..possible_end_anchors.len())]
    }

    pub fn random_polygon(&self, region: &Region) -> Object {
        let number_of_anchors = rand::thread_rng().gen_range(self.polygon_vertices_range.clone());
        let start = self.random_point(region);
        let mut lines: Vec<LineSegment> = vec![];
        for _ in 0..number_of_anchors {
            let next_anchor = self.random_point(region);
            lines.push(self.random_line(next_anchor));
        }
        Object::Polygon(start, lines)
    }

    pub fn random_line(&self, end: Point) -> LineSegment {
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

    pub fn random_point(&self, region: &Region) -> Point {
        Point(
            rand::thread_rng().gen_range(region.x_range()),
            rand::thread_rng().gen_range(region.y_range()),
        )
    }

    pub fn random_fill(&self, hatchable: bool) -> Fill {
        if hatchable {
            match rand::thread_rng().gen_range(1..=2) {
                1 => Fill::Solid(random_color()),
                2 => {
                    let hatch_size = rand::thread_rng().gen_range(5..=100) as f32 * 1e-2;
                    Fill::Hatched(
                        random_color(),
                        HatchDirection::BottomUpDiagonal,
                        hatch_size,
                        // under a certain hatch size, we can't see the hatching if the ratio is not ½
                        if hatch_size < 8.0 {
                            0.5
                        } else {
                            rand::thread_rng().gen_range(1..=4) as f32 / 4.0
                        },
                    )
                }
                _ => unreachable!(),
            }
        } else {
            Fill::Solid(random_color())
        }
    }

    pub fn clear(&mut self) {
        self.layers.clear();
        self.remove_background()
    }

    pub fn save_as(
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
        self.cell_size * self.world_region.width() + 2 * self.canvas_outter_padding
    }

    pub fn height(&self) -> usize {
        self.cell_size * self.world_region.height() + 2 * self.canvas_outter_padding
    }

    pub fn aspect_ratio(&self) -> f32 {
        return self.height() as f32 / self.width() as f32;
    }

    pub fn remove_all_objects_in(&mut self, region: &Region) {
        self.layers
            .iter_mut()
            .for_each(|layer| layer.remove_all_objects_in(region));
    }

    /// returns a list of all unique filters used throughout the canvas
    /// used to only generate one definition per filter
    ///
    fn unique_filters(&self) -> Vec<Filter> {
        self.layers
            .iter()
            .flat_map(|layer| layer.objects.iter().flat_map(|(_, o)| o.2.clone()))
            .unique()
            .collect()
    }

    fn unique_pattern_fills(&self) -> Vec<Fill> {
        self.layers
            .iter()
            .flat_map(|layer| {
                layer
                    .objects
                    .iter()
                    .flat_map(|(_, o)| o.1.map(|fill| fill.clone()))
            })
            .filter(|fill| matches!(fill, Fill::Hatched(..)))
            .unique_by(|fill| fill.pattern_id())
            .collect()
    }

    pub fn debug_region(&mut self, region: &Region, color: Color) {
        let layer = self.layer_or_empty("debug plane");

        layer.add_object(
            format!("{}_corner_ss", region).as_str(),
            Object::Dot(region.topleft()).color(Fill::Solid(color)),
        );
        layer.add_object(
            format!("{}_corner_se", region).as_str(),
            Object::Dot(region.topright().translated(1, 0)).color(Fill::Solid(color)),
        );
        layer.add_object(
            format!("{}_corner_ne", region).as_str(),
            Object::Dot(region.bottomright().translated(1, 1)).color(Fill::Solid(color)),
        );
        layer.add_object(
            format!("{}_corner_nw", region).as_str(),
            Object::Dot(region.bottomleft().translated(0, 1)).color(Fill::Solid(color)),
        );
        layer.add_object(
            format!("{}_region", region).as_str(),
            Object::Rectangle(region.start, region.end).color(Fill::Translucent(color, 0.25)),
        )
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
                    .set("fill", background_color.render(&self.colormap)),
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

        let mut defs = svg::node::element::Definitions::new();
        for filter in self.unique_filters() {
            defs = defs.add(filter.definition())
        }

        for pattern_fill in self.unique_pattern_fills() {
            if let Some(patterndef) = pattern_fill.pattern_definition(&self.colormap) {
                defs = defs.add(patterndef)
            }
        }

        svg.add(defs)
            .set(
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
