use std::collections::HashMap;

use crate::{ColorMapping, Fill, Filter, Point, Region, Transformation};
use itertools::Itertools;
use wasm_bindgen::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineSegment {
    Straight(Point),
    InwardCurve(Point),
    OutwardCurve(Point),
}

#[derive(Debug, Clone)]
pub enum Object {
    Polygon(Point, Vec<LineSegment>),
    Line(Point, Point, f32),
    CurveOutward(Point, Point, f32),
    CurveInward(Point, Point, f32),
    SmallCircle(Point),
    Dot(Point),
    BigCircle(Point),
    Text(Point, String, f32),
    CenteredText(Point, String, f32),
    // FittedText(Region, String),
    Rectangle(Point, Point),
    RawSVG(Box<dyn svg::Node>),
}

impl Object {
    pub fn color(self, fill: Fill) -> ColoredObject {
        ColoredObject::from((self, Some(fill)))
    }

    pub fn filter(self, filter: Filter) -> ColoredObject {
        ColoredObject::from((self, None)).filter(filter)
    }
}

#[derive(Debug, Clone)]
pub struct ColoredObject {
    pub object: Object,
    pub fill: Option<Fill>,
    pub filters: Vec<Filter>,
    pub transformations: Vec<Transformation>,
}

impl ColoredObject {
    pub fn filter(mut self, filter: Filter) -> Self {
        self.filters.push(filter);
        self
    }

    pub fn clear_filters(&mut self) {
        self.filters.clear();
    }

    pub fn render(
        &self,
        cell_size: usize,
        object_sizes: ObjectSizes,
        colormap: &ColorMapping,
        id: &str,
    ) -> svg::node::element::Group {
        let group = self.object.render(cell_size, object_sizes, id);

        let rendered_transforms = self
            .transformations
            .render_attribute(colormap, !self.object.fillable());

        let mut css = String::new();
        if !matches!(self.object, Object::RawSVG(..)) {
            css = self.fill.render_css(colormap, !self.object.fillable());
        }

        css += self
            .filters
            .iter()
            .map(|f| f.render_fill_css(colormap))
            .into_iter()
            .join(" ")
            .as_ref();

        group
            .set("style", css)
            .set(rendered_transforms.0, rendered_transforms.1)
    }
}

impl std::fmt::Display for ColoredObject {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let ColoredObject {
            object,
            fill,
            filters,
            transformations,
        } = self;

        if fill.is_some() {
            write!(f, "{:?} {:?}", fill.unwrap(), object)?;
        } else {
            write!(f, "transparent {:?}", object)?;
        }

        if !filters.is_empty() {
            write!(f, " with filters {:?}", filters)?;
        }

        if !transformations.is_empty() {
            write!(f, " with transformations {:?}", transformations)?;
        }

        Ok(())
    }
}

impl From<Object> for ColoredObject {
    fn from(value: Object) -> Self {
        ColoredObject {
            object: value,
            fill: None,
            filters: vec![],
            transformations: vec![],
        }
    }
}

impl From<(Object, Option<Fill>)> for ColoredObject {
    fn from((object, fill): (Object, Option<Fill>)) -> Self {
        ColoredObject {
            object,
            fill,
            filters: vec![],
            transformations: vec![],
        }
    }
}

#[wasm_bindgen]
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

pub trait RenderAttribute {
    const MULTIPLE_VALUES_JOIN_BY: &'static str = ", ";

    fn render_fill_attribute(&self, colormap: &ColorMapping) -> (String, String);
    fn render_stroke_attribute(&self, colormap: &ColorMapping) -> (String, String);
    fn render_attribute(
        &self,
        colormap: &ColorMapping,
        fill_as_stroke_color: bool,
    ) -> (String, String) {
        if fill_as_stroke_color {
            self.render_stroke_attribute(colormap)
        } else {
            self.render_fill_attribute(colormap)
        }
    }
}
impl<T: RenderAttribute> RenderAttribute for Vec<T> {
    fn render_fill_attribute(&self, colormap: &ColorMapping) -> (String, String) {
        (
            self.first()
                .unwrap()
                .render_fill_attribute(colormap)
                .0
                .clone(),
            self.iter()
                .map(|v| v.render_fill_attribute(colormap).1.clone())
                .join(", "),
        )
    }

    fn render_stroke_attribute(&self, colormap: &ColorMapping) -> (String, String) {
        (
            self.first()
                .unwrap()
                .render_stroke_attribute(colormap)
                .0
                .clone(),
            self.iter()
                .map(|v| v.render_stroke_attribute(colormap).1.clone())
                .join(", "),
        )
    }
}

pub trait RenderCSS {
    fn render_fill_css(&self, colormap: &ColorMapping) -> String;
    fn render_stroke_css(&self, colormap: &ColorMapping) -> String;
    fn render_css(&self, colormap: &ColorMapping, fill_as_stroke_color: bool) -> String {
        if fill_as_stroke_color {
            self.render_stroke_css(colormap)
        } else {
            self.render_fill_css(colormap)
        }
    }
}

impl<T: RenderCSS> RenderCSS for Option<T> {
    fn render_fill_css(&self, colormap: &ColorMapping) -> String {
        self.as_ref()
            .map(|v| v.render_fill_css(colormap))
            .unwrap_or_default()
    }

    fn render_stroke_css(&self, colormap: &ColorMapping) -> String {
        self.as_ref()
            .map(|v| v.render_stroke_css(colormap))
            .unwrap_or_default()
    }
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
            Object::Text(anchor, _, _)
            | Object::CenteredText(anchor, ..)
            | Object::Dot(anchor)
            | Object::SmallCircle(anchor) => anchor.translate(dx, dy),
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
        let Point(current_x, current_y) = self.region().start;
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
            Object::Text(anchor, _, _)
            | Object::CenteredText(anchor, ..)
            | Object::Dot(anchor)
            | Object::SmallCircle(anchor) => anchor.region(),
            Object::BigCircle(center) => center.region(),
            Object::RawSVG(_) => {
                unimplemented!()
            }
        }
    }
}

impl Object {
    pub fn fillable(&self) -> bool {
        !matches!(
            self,
            Object::Line(..) | Object::CurveInward(..) | Object::CurveOutward(..)
        )
    }

    pub fn hatchable(&self) -> bool {
        self.fillable() && !matches!(self, Object::Dot(..))
    }

    pub fn render(
        &self,
        cell_size: usize,
        object_sizes: ObjectSizes,
        id: &str,
    ) -> svg::node::element::Group {
        let group = svg::node::element::Group::new();

        let rendered = match self {
            Object::Text(..) | Object::CenteredText(..) => self.render_text(cell_size),
            Object::Rectangle(..) => self.render_rectangle(cell_size),
            Object::Polygon(..) => self.render_polygon(cell_size),
            Object::Line(..) => self.render_line(cell_size),
            Object::CurveInward(..) | Object::CurveOutward(..) => self.render_curve(cell_size),
            Object::SmallCircle(..) => self.render_small_circle(cell_size, object_sizes),
            Object::Dot(..) => self.render_dot(cell_size, object_sizes),
            Object::BigCircle(..) => self.render_big_circle(cell_size),
            Object::RawSVG(..) => self.render_raw_svg(),
        };

        group.set("data-object", id).add(rendered)
    }

    fn render_raw_svg(&self) -> Box<dyn svg::node::Node> {
        if let Object::RawSVG(svg) = self {
            return svg.clone();
        }

        panic!("Expected RawSVG, got {:?}", self);
    }

    fn render_text(&self, cell_size: usize) -> Box<dyn svg::node::Node> {
        if let Object::Text(position, content, font_size)
        | Object::CenteredText(position, content, font_size) = self
        {
            let centered = matches!(self, Object::CenteredText(..));

            let coords = if centered {
                position.center_coords(cell_size)
            } else {
                position.coords(cell_size)
            };

            let mut node = svg::node::element::Text::new(content.clone())
                .set("x", coords.0)
                .set("y", coords.1)
                .set("font-size", format!("{}pt", font_size))
                .set("font-family", "Victor Mono");

            if centered {
                node = node
                    .set("text-anchor", "middle")
                    // FIXME does not work with imagemagick
                    .set("dominant-baseline", "middle");
            } else {
                // FIXME does not work with imagemagick
                // see https://legacy.imagemagick.org/discourse-server/viewtopic.php?t=31540
                node = node.set("dominant-baseline", "hanging")
            }

            return Box::new(node);
        }

        panic!("Expected Text, got {:?}", self);
    }

    // fn render_fitted_text(&self, cell_size: usize) -> Box<dyn svg:node::Node> {
    //     if let Object::FittedText(region, content) = self {
    //         let (x, y) = region.start.coords(cell_size);
    //         let width = region.width() * cell_size as f32;
    //         let height = region.height() * cell_size as f32;

    //         return Box::new(
    //             svg::node::element::Text::new(content.clone())
    //                 .set("x", x)
    //                 .set("y", y)
    //                 .set("")
    //                 .set("font-size", format!("{}pt", 10.0))
    //                 .set("font-family", "sans-serif"),
    //         );
    //     }

    //     panic!("Expected FittedText, got {:?}", self);
    // }

    fn render_rectangle(&self, cell_size: usize) -> Box<dyn svg::node::Node> {
        if let Object::Rectangle(start, end) = self {
            return Box::new(
                svg::node::element::Rectangle::new()
                    .set("x", start.coords(cell_size).0)
                    .set("y", start.coords(cell_size).1)
                    .set("width", start.distances(end).0 * cell_size)
                    .set("height", start.distances(end).1 * cell_size),
            );
        }

        panic!("Expected Rectangle, got {:?}", self);
    }

    fn render_polygon(&self, cell_size: usize) -> Box<dyn svg::node::Node> {
        if let Object::Polygon(start, lines) = self {
            let mut path = svg::node::element::path::Data::new();
            path = path.move_to(start.coords(cell_size));
            for line in lines {
                path = match line {
                    LineSegment::Straight(end)
                    | LineSegment::InwardCurve(end)
                    | LineSegment::OutwardCurve(end) => path.line_to(end.coords(cell_size)),
                };
            }
            path = path.close();
            return Box::new(svg::node::element::Path::new().set("d", path));
        }

        panic!("Expected Polygon, got {:?}", self);
    }

    fn render_line(&self, cell_size: usize) -> Box<dyn svg::node::Node> {
        if let Object::Line(start, end, width) = self {
            return Box::new(
                svg::node::element::Line::new()
                    .set("x1", start.coords(cell_size).0)
                    .set("y1", start.coords(cell_size).1)
                    .set("x2", end.coords(cell_size).0)
                    .set("y2", end.coords(cell_size).1)
                    .set("stroke-width", *width),
            );
        }

        panic!("Expected Line, got {:?}", self);
    }

    fn render_curve(&self, cell_size: usize) -> Box<dyn svg::node::Node> {
        if let Object::CurveOutward(start, end, _) | Object::CurveInward(start, end, _) = self {
            let inward = matches!(self, Object::CurveInward(..));

            let (start_x, start_y) = start.coords(cell_size);
            let (end_x, end_y) = end.coords(cell_size);

            let midpoint = ((start_x + end_x) / 2.0, (start_y + end_y) / 2.0);
            let start_from_midpoint = (start_x - midpoint.0, start_y - midpoint.1);
            let end_from_midpoint = (end_x - midpoint.0, end_y - midpoint.1);

            let control = {
                let relative = (end_x - start_x, end_y - start_y);
                if start_from_midpoint.0 * start_from_midpoint.1 > 0.0
                    && end_from_midpoint.0 * end_from_midpoint.1 > 0.0
                {
                    if inward {
                        (
                            midpoint.0 + relative.0.abs() / 2.0,
                            midpoint.1 - relative.1.abs() / 2.0,
                        )
                    } else {
                        (
                            midpoint.0 - relative.0.abs() / 2.0,
                            midpoint.1 + relative.1.abs() / 2.0,
                        )
                    }
                // diagonal line is going like this: /
                } else if start_from_midpoint.0 * start_from_midpoint.1 < 0.0
                    && end_from_midpoint.0 * end_from_midpoint.1 < 0.0
                {
                    if inward {
                        (
                            midpoint.0 - relative.0.abs() / 2.0,
                            midpoint.1 - relative.1.abs() / 2.0,
                        )
                    } else {
                        (
                            midpoint.0 + relative.0.abs() / 2.0,
                            midpoint.1 + relative.1.abs() / 2.0,
                        )
                    }
                // line is horizontal
                } else if start_y == end_y {
                    (
                        midpoint.0,
                        midpoint.1 + (if inward { -1.0 } else { 1.0 }) * relative.0.abs() / 2.0,
                    )
                // line is vertical
                } else if start_x == end_x {
                    (
                        midpoint.0 + (if inward { -1.0 } else { 1.0 }) * relative.1.abs() / 2.0,
                        midpoint.1,
                    )
                } else {
                    unreachable!()
                }
            };

            return Box::new(
                svg::node::element::Path::new().set(
                    "d",
                    svg::node::element::path::Data::new()
                        .move_to(start.coords(cell_size))
                        .quadratic_curve_to((control, end.coords(cell_size))),
                ),
            );
        }

        panic!("Expected Curve, got {:?}", self);
    }

    fn render_small_circle(
        &self,
        cell_size: usize,
        object_sizes: ObjectSizes,
    ) -> Box<dyn svg::node::Node> {
        if let Object::SmallCircle(center) = self {
            return Box::new(
                svg::node::element::Circle::new()
                    .set("cx", center.coords(cell_size).0)
                    .set("cy", center.coords(cell_size).1)
                    .set("r", object_sizes.small_circle_radius),
            );
        }

        panic!("Expected SmallCircle, got {:?}", self);
    }

    fn render_dot(&self, cell_size: usize, object_sizes: ObjectSizes) -> Box<dyn svg::node::Node> {
        if let Object::Dot(center) = self {
            return Box::new(
                svg::node::element::Circle::new()
                    .set("cx", center.coords(cell_size).0)
                    .set("cy", center.coords(cell_size).1)
                    .set("r", object_sizes.dot_radius),
            );
        }

        panic!("Expected Dot, got {:?}", self);
    }

    fn render_big_circle(&self, cell_size: usize) -> Box<dyn svg::node::Node> {
        if let Object::BigCircle(topleft) = self {
            let (cx, cy) = {
                let (x, y) = topleft.coords(cell_size);
                (x + cell_size as f32 / 2.0, y + cell_size as f32 / 2.0)
            };

            return Box::new(
                svg::node::element::Circle::new()
                    .set("cx", cx)
                    .set("cy", cy)
                    .set("r", cell_size / 2),
            );
        }

        panic!("Expected BigCircle, got {:?}", self);
    }
}
