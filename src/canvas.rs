use std::{
    collections::HashMap,
    fs::File,
    io::{BufReader, Write},
    ops::Range,
};

use chrono::DateTime;
use rand::Rng;
use serde::Deserialize;

use crate::layer::Layer;

#[derive(Debug, Clone)]
pub struct Canvas {
    pub grid_size: (usize, usize),
    pub cell_size: usize,
    pub objects_count_range: Range<usize>,
    pub polygon_vertices_range: Range<usize>,
    pub canvas_outter_padding: usize,
    pub object_sizes: ObjectSizes,
    pub render_grid: bool,
    pub colormap: ColorMapping,
    /// The layers are in order of top to bottom: the first layer will be rendered on top of the second, etc.
    pub layers: Vec<Layer>,
    pub background: Option<Color>,
}

impl Canvas {
    /// Create a new canvas.
    /// The layers are in order of top to bottom: the first layer will be rendered on top of the second, etc.
    /// A layer named "root" will be added below all layers if you don't add it yourself.
    pub fn new(layer_names: Vec<&str>) -> Self {
        let mut layer_names = layer_names;
        if let None = layer_names.iter().find(|&&name| name == "root") {
            layer_names.push("root");
        }
        Self {
            layers: layer_names
                .iter()
                .map(|name| Layer {
                    objects: HashMap::new(),
                    name: name.to_string(),
                    _render_cache: None,
                })
                .collect(),
            ..Self::default_settings()
        }
    }

    pub fn layer(&mut self, name: &str) -> Option<&mut Layer> {
        self.layers.iter_mut().find(|layer| layer.name == name)
    }

    pub fn root(&mut self) -> &mut Layer {
        self.layer("root")
            .expect("Layer 'root' should always exist in a canvas")
    }

    pub fn add_object(
        &mut self,
        layer: &str,
        name: &str,
        object: Object,
        fill: Option<Fill>,
    ) -> Result<(), String> {
        match self.layer(&layer) {
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
            object_sizes: ObjectSizes {
                line_width: 2.0,
                empty_shape_stroke_width: 0.5,
                small_circle_radius: 5.0,
                dot_radius: 2.0,
                font_size: 20.0,
            },
            render_grid: false,
            colormap: ColorMapping::default(),
            layers: vec![],
            background: None,
        }
    }
    pub fn random_layer(&self, name: &'static str) -> Layer {
        let mut objects: HashMap<String, (Object, Option<Fill>)> = HashMap::new();
        let number_of_objects = rand::thread_rng().gen_range(self.objects_count_range.clone());
        for i in 0..number_of_objects {
            let object = self.random_object();
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
            name: name.to_string(),
            objects,
            _render_cache: None,
        }
    }

    pub fn random_object(&self) -> Object {
        let start = self.random_anchor();
        match rand::thread_rng().gen_range(1..=7) {
            1 => self.random_polygon(),
            2 => Object::BigCircle(self.random_center_anchor()),
            3 => Object::SmallCircle(start),
            4 => Object::Dot(start),
            5 => Object::CurveInward(start, self.random_end_anchor(start)),
            6 => Object::CurveOutward(start, self.random_end_anchor(start)),
            7 => Object::Line(self.random_anchor(), self.random_anchor()),
            _ => unreachable!(),
        }
    }

    pub fn random_end_anchor(&self, start: Anchor) -> Anchor {
        // End anchors are always a square diagonal from the start anchor (for now)
        // that means taking steps of the form n * (one of (1, 1), (1, -1), (-1, 1), (-1, -1))
        // Except that the end anchor needs to stay in the bounds of the shape.

        // Determine all possible end anchors that are in a square diagonal from the start anchor
        let mut possible_end_anchors = vec![];
        let grid_width = self.grid_size.0 as i32;
        let grid_height = self.grid_size.1 as i32;

        for x in -grid_width..=grid_width {
            for y in -grid_height..=grid_height {
                let end_anchor = Anchor(start.0 + x, start.1 + y);

                if end_anchor == start {
                    continue;
                }

                // Check that the end anchor is in a square diagonal from the start anchor and that the end anchor is in bounds
                if x.abs() == y.abs()
                    && end_anchor.0.abs() < grid_width
                    && end_anchor.1.abs() < grid_height
                    && end_anchor.0 >= 0
                    && end_anchor.1 >= 0
                {
                    possible_end_anchors.push(end_anchor);
                }
            }
        }

        // Pick a random end anchor from the possible end anchors
        possible_end_anchors[rand::thread_rng().gen_range(0..possible_end_anchors.len())]
    }

    pub fn random_polygon(&self) -> Object {
        let number_of_anchors = rand::thread_rng().gen_range(self.polygon_vertices_range.clone());
        let start = self.random_anchor();
        let mut lines: Vec<Line> = vec![];
        for _ in 0..number_of_anchors {
            let next_anchor = self.random_anchor();
            lines.push(self.random_line(next_anchor));
        }
        Object::Polygon(start, lines)
    }

    pub fn random_line(&self, end: Anchor) -> Line {
        match rand::thread_rng().gen_range(1..=3) {
            1 => Line::Line(end),
            2 => Line::InwardCurve(end),
            3 => Line::OutwardCurve(end),
            _ => unreachable!(),
        }
    }

    pub fn random_anchor(&self) -> Anchor {
        if rand::thread_rng().gen_bool(1.0 / (self.grid_size.0 * self.grid_size.1) as f64) {
            // small change of getting center (-1, -1) even when grid size would not permit it (e.g. 4x4)
            Anchor(-1, -1)
        } else {
            Anchor(
                rand::thread_rng().gen_range(0..=self.grid_size.0 - 1) as i32,
                rand::thread_rng().gen_range(0..=self.grid_size.1 - 1) as i32,
            )
        }
    }

    pub fn random_center_anchor(&self) -> CenterAnchor {
        if rand::thread_rng()
            .gen_bool(1.0 / ((self.grid_size.0 as i32 - 1) * (self.grid_size.1 as i32 - 1)) as f64)
        {
            // small change of getting center (-1, -1) even when grid size would not permit it (e.g. 3x3)
            CenterAnchor(-1, -1)
        } else {
            CenterAnchor(
                rand::thread_rng().gen_range(0..=self.grid_size.0 - 2) as i32,
                rand::thread_rng().gen_range(0..=self.grid_size.1 - 2) as i32,
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
        let mut spawned = std::process::Command::new("convert")
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
        let background_color = self.background.unwrap_or(Color::default());
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
            svg = svg.add(layer.render(self.colormap.clone(), self.cell_size, self.object_sizes));
        }
        // render a dotted grid
        if self.render_grid {
            for i in 0..self.grid_size.0 as i32 {
                for j in 0..self.grid_size.1 as i32 {
                    let (x, y) = Anchor(i, j).coords(self.cell_size);
                    svg = svg.add(
                        svg::node::element::Circle::new()
                            .set("cx", x)
                            .set("cy", y)
                            .set("r", self.object_sizes.line_width / 4.0)
                            .set("fill", "#000"),
                    );

                    // if i < canvas.grid_size.0 as i32 - 1 && j < canvas.grid_size.1 as i32 - 1 {
                    //     let (x, y) = CenterAnchor(i, j).coords(&canvas);
                    //     svg = svg.add(
                    //         svg::node::element::Circle::new()
                    //             .set("cx", x)
                    //             .set("cy", y)
                    //             .set("r", canvas.line_width / 4.0)
                    //             .set("fill", "#fff"),
                    //     );
                    // }
                }
            }
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
    pub line_width: f32,
    pub font_size: f32,
}

#[derive(Debug, Clone)]
pub enum Object {
    Polygon(Anchor, Vec<Line>),
    Line(Anchor, Anchor),
    CurveOutward(Anchor, Anchor),
    CurveInward(Anchor, Anchor),
    SmallCircle(Anchor),
    Dot(Anchor),
    BigCircle(CenterAnchor),
    Text(Anchor, String),
    Rectangle(Anchor, Anchor),
    RawSVG(Box<dyn svg::Node>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Anchor(pub i32, pub i32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CenterAnchor(pub i32, pub i32);

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
pub enum Line {
    Line(Anchor),
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Color {
    Black,
    White,
    Red,
    Green,
    Blue,
    Yellow,
    Orange,
    Purple,
    Brown,
    Cyan,
    Pink,
    Gray,
}

impl Default for Color {
    fn default() -> Self {
        Self::Black
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct ColorMapping {
    pub black: String,
    pub white: String,
    pub red: String,
    pub green: String,
    pub blue: String,
    pub yellow: String,
    pub orange: String,
    pub purple: String,
    pub brown: String,
    pub cyan: String,
    pub pink: String,
    pub gray: String,
}

impl ColorMapping {
    pub fn default() -> Self {
        ColorMapping {
            black: "black".to_string(),
            white: "white".to_string(),
            red: "red".to_string(),
            green: "green".to_string(),
            blue: "blue".to_string(),
            yellow: "yellow".to_string(),
            orange: "orange".to_string(),
            purple: "purple".to_string(),
            brown: "brown".to_string(),
            pink: "pink".to_string(),
            gray: "gray".to_string(),
            cyan: "cyan".to_string(),
        }
    }
    pub fn from_json_file(path: &str) -> ColorMapping {
        let file = File::open(path).unwrap();
        let reader = BufReader::new(file);
        let json: serde_json::Value = serde_json::from_reader(reader).unwrap();
        ColorMapping {
            black: json["black"].as_str().unwrap().to_string(),
            white: json["white"].as_str().unwrap().to_string(),
            red: json["red"].as_str().unwrap().to_string(),
            green: json["green"].as_str().unwrap().to_string(),
            blue: json["blue"].as_str().unwrap().to_string(),
            yellow: json["yellow"].as_str().unwrap().to_string(),
            orange: json["orange"].as_str().unwrap().to_string(),
            purple: json["purple"].as_str().unwrap().to_string(),
            brown: json["brown"].as_str().unwrap().to_string(),
            cyan: json["cyan"].as_str().unwrap().to_string(),
            pink: json["pink"].as_str().unwrap().to_string(),
            gray: json["gray"].as_str().unwrap().to_string(),
        }
    }
}

impl Color {
    pub fn to_string(self, mapping: &ColorMapping) -> String {
        match self {
            Color::Black => mapping.black.to_string(),
            Color::White => mapping.white.to_string(),
            Color::Red => mapping.red.to_string(),
            Color::Green => mapping.green.to_string(),
            Color::Blue => mapping.blue.to_string(),
            Color::Yellow => mapping.yellow.to_string(),
            Color::Orange => mapping.orange.to_string(),
            Color::Purple => mapping.purple.to_string(),
            Color::Brown => mapping.brown.to_string(),
            Color::Cyan => mapping.cyan.to_string(),
            Color::Pink => mapping.pink.to_string(),
            Color::Gray => mapping.gray.to_string(),
        }
    }
}
