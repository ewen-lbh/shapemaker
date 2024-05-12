use crate::{Color, ColorMapping, RenderCSS};

/// Angle, stored in degrees
#[derive(Debug, Clone, Copy, Default)]
pub struct Angle(pub f32);

impl Angle {
    pub const TURN: Self = Angle(360.0);

    pub fn degrees(&self) -> f32 {
        self.0
    }

    pub fn radians(&self) -> f32 {
        self.0 * std::f32::consts::PI / (Self::TURN.0 / 2.0)
    }

    pub fn turns(&self) -> f32 {
        self.0 / Self::TURN.0
    }

    pub fn without_turns(&self) -> Self {
        Self(self.0 % Self::TURN.0)
    }
}

impl std::ops::Sub for Angle {
    type Output = Angle;

    fn sub(self, rhs: Self) -> Self::Output {
        Angle(self.0 - rhs.0)
    }
}

impl std::fmt::Display for Angle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}deg", self.degrees())
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Fill {
    Solid(Color),
    Translucent(Color, f32),
    Hatched(Color, Angle, f32, f32),
    Dotted(Color, f32, f32),
}

// Operations that can be applied on fills.
// Applying them on Option<Fill> is also possible, and will return an Option<Fill>.
pub trait FillOperations {
    fn opacify(&self, opacity: f32) -> Self;
    fn bottom_up_hatches(color: Color, thickness: f32, spacing: f32) -> Self;
}

impl FillOperations for Fill {
    fn opacify(&self, opacity: f32) -> Self {
        match self {
            Fill::Solid(color) => Fill::Translucent(*color, opacity),
            Fill::Translucent(color, _) => Fill::Translucent(*color, opacity),
            _ => *self,
        }
    }

    fn bottom_up_hatches(color: Color, thickness: f32, spacing: f32) -> Self {
        Fill::Hatched(color, Angle(45.0), thickness, spacing)
    }
}

impl FillOperations for Option<Fill> {
    fn opacify(&self, opacity: f32) -> Self {
        self.as_ref().map(|fill| fill.opacify(opacity))
    }

    fn bottom_up_hatches(color: Color, thickness: f32, spacing: f32) -> Self {
        Some(Fill::bottom_up_hatches(color, thickness, spacing))
    }
}

impl RenderCSS for Fill {
    fn render_fill_css(&self, colormap: &ColorMapping) -> String {
        match self {
            Fill::Solid(color) => {
                format!("fill: {};", color.render(colormap))
            }
            Fill::Translucent(color, opacity) => {
                format!("fill: {}; opacity: {};", color.render(colormap), opacity)
            }
            Fill::Dotted(..) | Fill::Hatched(..) => {
                format!("fill: url(#{});", self.pattern_id())
            }
        }
    }

    fn render_stroke_css(&self, colormap: &ColorMapping) -> String {
        match self {
            Fill::Solid(color) => {
                format!("stroke: {}; fill: transparent;", color.render(colormap))
            }
            Fill::Translucent(color, opacity) => {
                format!(
                    "stroke: {}; opacity: {}; fill: transparent;",
                    color.render(colormap),
                    opacity
                )
            }
            Fill::Dotted(..) => unimplemented!(),
            Fill::Hatched(..) => unimplemented!(),
        }
    }
}

impl Fill {
    pub fn pattern_id(&self) -> String {
        if let Fill::Hatched(color, angle, thickness, spacing) = self {
            return format!(
                "pattern-hatched-{}-{}-{}-{}",
                angle,
                color.name(),
                thickness,
                spacing
            );
        }
        if let Fill::Dotted(color, diameter, spacing) = self {
            return format!("pattern-dotted-{}-{}-{}", color.name(), diameter, spacing);
        }
        String::from("")
    }

    pub fn pattern_definition(
        &self,
        colormapping: &ColorMapping,
    ) -> Option<svg::node::element::Pattern> {
        match self {
            Fill::Hatched(color, angle, size, thickness_ratio) => {
                let thickness = size * (2.0 * thickness_ratio);

                let pattern = svg::node::element::Pattern::new()
                    .set("id", self.pattern_id())
                    .set("patternUnits", "userSpaceOnUse")
                    .set("height", size * 2.0)
                    .set("width", size * 2.0)
                    .set("viewBox", format!("0,0,{},{}", size, size))
                    .set(
                        "patternTransform",
                        format!("rotate({})", (*angle - Angle(45.0)).degrees()),
                    )
                    // https://stackoverflow.com/a/55104220/9943464
                    .add(
                        svg::node::element::Polygon::new()
                            .set(
                                "points",
                                format!("0,0 {},0 0,{}", thickness / 2.0, thickness / 2.0),
                            )
                            .set("fill", color.render(colormapping)),
                    )
                    .add(
                        svg::node::element::Polygon::new()
                            .set(
                                "points",
                                format!(
                                    "0,{} {},0 {},{} {},{}",
                                    size,
                                    size,
                                    size,
                                    thickness / 2.0,
                                    thickness / 2.0,
                                    size,
                                ),
                            )
                            .set("fill", color.render(colormapping)),
                    );

                Some(pattern)
            }
            Fill::Dotted(color, diameter, spacing) => {
                let box_size = diameter + 2.0 * spacing;
                let pattern = svg::node::element::Pattern::new()
                    .set("id", self.pattern_id())
                    .set("patternUnits", "userSpaceOnUse")
                    .set("height", box_size)
                    .set("width", box_size)
                    .set("viewBox", format!("0,0,{},{}", box_size, box_size))
                    .add(
                        svg::node::element::Circle::new()
                            .set("cx", box_size / 2.0)
                            .set("cy", box_size / 2.0)
                            .set("r", diameter / 2.0)
                            .set("fill", color.render(colormapping)),
                    );

                Some(pattern)
            }
            _ => None,
        }
    }
}
