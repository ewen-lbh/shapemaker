use std::collections::HashMap;

use crate::canvas::{Color, ColorMapping, Coordinates, Fill, Line, Object, ObjectSizes};

#[derive(Debug, Clone, Default)]
pub struct Layer {
    pub objects: HashMap<String, (Object, Option<Fill>)>,
    pub name: String,
    pub _render_cache: Option<svg::node::element::Group>,
}

impl Layer {
    pub fn new(name: &str) -> Self {
        Layer {
            objects: HashMap::new(),
            name: name.to_string(),
            _render_cache: None,
        }
    }

    pub fn add_object(&mut self, name: &str, object: Object, fill: Option<Fill>) {
        self.objects.insert(name.to_string(), (object, fill));
        self._render_cache = None;
    }

    pub fn remove_object(&mut self, name: &str) {
        self.objects.remove(name);
        self._render_cache = None;
    }

    /// Render the layer to a SVG group element.
    pub fn render(
        &mut self,
        colormap: ColorMapping,
        cell_size: usize,
        object_sizes: ObjectSizes,
    ) -> svg::node::element::Group {
        if let Some(cached_svg) = &self._render_cache {
            return cached_svg.clone();
        }
        let default_color = Color::Black.to_string(&colormap);
        // eprintln!("render: background_color({:?})", background_color);
        let mut layer_group = svg::node::element::Group::new()
            .set("class", "layer")
            .set("data-layer", self.name.clone());
        for (_id, (object, maybe_fill)) in &self.objects {
            let mut group = svg::node::element::Group::new();
            match object {
                Object::RawSVG(svg) => {
                    // eprintln!("render: raw_svg [{}]", id);
                    group = group.add(svg.clone());
                }
                Object::Text(position, content) => {
                    group = group.add(
                        svg::node::element::Text::new(content.clone())
                            .set("x", position.coords(cell_size).0)
                            .set("y", position.coords(cell_size).1)
                            .set("font-size", format!("{}pt", object_sizes.font_size))
                            .set("font-family", "sans-serif")
                            .set("text-anchor", "middle")
                            .set("dominant-baseline", "middle")
                            .set(
                                "style",
                                match maybe_fill {
                                    Some(Fill::Solid(color)) => {
                                        format!("fill: {};", color.to_string(&colormap))
                                    }
                                    Some(Fill::Translucent(color, opacity)) => {
                                        format!(
                                            "fill: {}; opacity: {};",
                                            color.to_string(&colormap),
                                            opacity
                                        )
                                    }
                                    _ => "".to_string(),
                                },
                            ),
                    );
                }
                Object::Rectangle(start, end) => {
                    group = group.add(
                        svg::node::element::Rectangle::new()
                            .set("x1", start.coords(cell_size).0)
                            .set("y1", start.coords(cell_size).1)
                            .set("x2", end.coords(cell_size).0)
                            .set("y2", end.coords(cell_size).1)
                            .set(
                                "style",
                                match maybe_fill {
                                    Some(Fill::Solid(color)) => {
                                        format!("fill: {};", color.to_string(&colormap))
                                    }
                                    Some(Fill::Translucent(color, opacity)) => {
                                        format!(
                                            "fill: {}; opacity: {};",
                                            color.to_string(&colormap),
                                            opacity
                                        )
                                    }
                                    _ => "".to_string(),
                                },
                            ),
                    );
                }
                Object::Polygon(start, lines) => {
                    // eprintln!("render: polygon({:?}, {:?}) [{}]", start, lines, id);
                    let mut path = svg::node::element::path::Data::new();
                    path = path.move_to(start.coords(cell_size));
                    for line in lines {
                        path = match line {
                            Line::Line(end) | Line::InwardCurve(end) | Line::OutwardCurve(end) => {
                                path.line_to(end.coords(cell_size))
                            }
                        };
                    }
                    path = path.close();
                    group = group
                        .add(svg::node::element::Path::new().set("d", path))
                        .set(
                            "style",
                            match maybe_fill {
                                // TODO
                                Some(Fill::Solid(color)) => {
                                    format!("fill: {};", color.to_string(&colormap))
                                }
                                Some(Fill::Translucent(color, opacity)) => {
                                    format!(
                                        "fill: {}; opacity: {};",
                                        color.to_string(&colormap),
                                        opacity
                                    )
                                }
                                _ => format!(
                                    "fill: none; stroke: {}; stroke-width: {}px;",
                                    default_color, object_sizes.empty_shape_stroke_width
                                ),
                            },
                        );
                }
                Object::Line(start, end) => {
                    // eprintln!("render: line({:?}, {:?}) [{}]", start, end, id);
                    group = group.add(
                        svg::node::element::Line::new()
                            .set("x1", start.coords(cell_size).0)
                            .set("y1", start.coords(cell_size).1)
                            .set("x2", end.coords(cell_size).0)
                            .set("y2", end.coords(cell_size).1)
                            .set(
                                "style",
                                match maybe_fill {
                                    // TODO
                                    Some(Fill::Solid(color)) => {
                                        format!(
                                            "fill: none; stroke: {}; stroke-width: 2px;",
                                            color.to_string(&colormap)
                                        )
                                    }
                                    _ => format!(
                                        "fill: none; stroke: {}; stroke-width: 2px;",
                                        default_color
                                    ),
                                },
                            ),
                    );
                }
                Object::CurveInward(start, end) | Object::CurveOutward(start, end) => {
                    let inward = if matches!(object, Object::CurveInward(_, _)) {
                        // eprintln!("render: curve_inward({:?}, {:?}) [{}]", start, end, id);
                        true
                    } else {
                        // eprintln!("render: curve_outward({:?}, {:?}) [{}]", start, end, id);
                        false
                    };

                    let (start_x, start_y) = start.coords(cell_size);
                    let (end_x, end_y) = end.coords(cell_size);

                    let midpoint = ((start_x + end_x) / 2.0, (start_y + end_y) / 2.0);
                    let start_from_midpoint = (start_x - midpoint.0, start_y - midpoint.1);
                    let end_from_midpoint = (end_x - midpoint.0, end_y - midpoint.1);
                    // eprintln!("        midpoint: {:?}", midpoint);
                    // eprintln!(
                    // "        from midpoint: {:?} -> {:?}",
                    // start_from_midpoint, end_from_midpoint
                    // );
                    let control = {
                        let relative = (end_x - start_x, end_y - start_y);
                        // eprintln!("        relative: {:?}", relative);
                        // diagonal line is going like this: \
                        if start_from_midpoint.0 * start_from_midpoint.1 > 0.0
                            && end_from_midpoint.0 * end_from_midpoint.1 > 0.0
                        {
                            // eprintln!("        diagonal \\");
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
                            // eprintln!("        diagonal /");
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
                            // eprintln!("        horizontal");
                            (
                                midpoint.0,
                                midpoint.1
                                    + (if inward { -1.0 } else { 1.0 }) * relative.0.abs() / 2.0,
                            )
                        // line is vertical
                        } else if start_x == end_x {
                            // eprintln!("        vertical");
                            (
                                midpoint.0
                                    + (if inward { -1.0 } else { 1.0 }) * relative.1.abs() / 2.0,
                                midpoint.1,
                            )
                        } else {
                            unreachable!()
                        }
                    };
                    // eprintln!("        control: {:?}", control);
                    group = group.add(
                        svg::node::element::Path::new()
                            .set(
                                "d",
                                svg::node::element::path::Data::new()
                                    .move_to(start.coords(cell_size))
                                    .quadratic_curve_to((control, end.coords(cell_size))),
                            )
                            .set(
                                "style",
                                match maybe_fill {
                                    // TODO
                                    Some(Fill::Solid(color)) => {
                                        format!(
                                            "fill: none; stroke: {}; stroke-width: {}px;",
                                            color.to_string(&colormap),
                                            object_sizes.line_width
                                        )
                                    }
                                    _ => format!(
                                        "fill: none; stroke: {}; stroke-width: {}px;",
                                        default_color, object_sizes.line_width
                                    ),
                                },
                            ),
                    );
                }
                Object::SmallCircle(center) => {
                    // eprintln!("render: small_circle({:?}) [{}]", center, id);
                    group = group.add(
                        svg::node::element::Circle::new()
                            .set("cx", center.coords(cell_size).0)
                            .set("cy", center.coords(cell_size).1)
                            .set("r", object_sizes.small_circle_radius)
                            .set(
                                "style",
                                match maybe_fill {
                                    // TODO
                                    Some(Fill::Solid(color)) => {
                                        format!("fill: {};", color.to_string(&colormap))
                                    }
                                    _ => format!(
                                        "fill: none; stroke: {}; stroke-width: {}px;",
                                        default_color, object_sizes.empty_shape_stroke_width
                                    ),
                                },
                            ),
                    );
                }
                Object::Dot(center) => {
                    // eprintln!("render: dot({:?}) [{}]", center, id);
                    group = group.add(
                        svg::node::element::Circle::new()
                            .set("cx", center.coords(cell_size).0)
                            .set("cy", center.coords(cell_size).1)
                            .set("r", object_sizes.dot_radius)
                            .set(
                                "style",
                                match maybe_fill {
                                    // TODO
                                    Some(Fill::Solid(color)) => {
                                        format!("fill: {};", color.to_string(&colormap))
                                    }
                                    _ => format!(
                                        "fill: none; stroke: {}; stroke-width: {}px;",
                                        default_color, object_sizes.empty_shape_stroke_width
                                    ),
                                },
                            ),
                    );
                }
                Object::BigCircle(center) => {
                    // eprintln!("render: big_circle({:?}) [{}]", center, id);
                    group = group.add(
                        svg::node::element::Circle::new()
                            .set("cx", center.coords(cell_size).0)
                            .set("cy", center.coords(cell_size).1)
                            .set("r", cell_size / 2)
                            .set(
                                "style",
                                match maybe_fill {
                                    // TODO
                                    Some(Fill::Solid(color)) => {
                                        format!("fill: {};", color.to_string(&colormap))
                                    }
                                    _ => format!(
                                        "fill: none; stroke: {}; stroke-width: 0.5px;",
                                        default_color
                                    ),
                                },
                            ),
                    );
                }
            }
            // eprintln!("        fill: {:?}", &maybe_fill);
            layer_group = layer_group.add(group);
        }
        self._render_cache = Some(layer_group.clone());
        layer_group
    }
}
