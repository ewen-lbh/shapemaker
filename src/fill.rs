
use crate::{Color, ColorMapping, RenderCSS};

#[derive(Debug, Clone, Copy)]
pub enum Fill {
    Solid(Color),
    Translucent(Color, f32),
    Hatched(Color, HatchDirection, f32, f32),
    Dotted(Color, f32),
}

#[derive(Debug, Clone, Copy)]
pub enum HatchDirection {
    Horizontal,
    Vertical,
    BottomUpDiagonal,
    TopDownDiagonal,
}

const PATTERN_SIZE: usize = 8;

impl HatchDirection {}

impl RenderCSS for Fill {
    fn render_fill_css(&self, colormap: &ColorMapping) -> String {
        match self {
            Fill::Solid(color) => {
                format!("fill: {};", color.render(colormap))
            }
            Fill::Translucent(color, opacity) => {
                format!("fill: {}; opacity: {};", color.render(colormap), opacity)
            }
            Fill::Dotted(..) => unimplemented!(),
            Fill::Hatched(..) => {
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
    pub fn pattern_name(&self) -> String {
        match self {
            Fill::Hatched(_, direction, ..) => format!(
                "hatched-{}",
                match direction {
                    HatchDirection::Horizontal => "horizontal",
                    HatchDirection::Vertical => "vertical",
                    HatchDirection::BottomUpDiagonal => "bottom-up",
                    HatchDirection::TopDownDiagonal => "top-down",
                }
            ),
            _ => String::from(""),
        }
    }

    pub fn pattern_id(&self) -> String {
        if let Fill::Hatched(color, _, thickness, spacing) = self {
            return format!(
                "pattern-{}-{}-{}",
                self.pattern_name(),
                color.name(),
                thickness
            );
        }
        String::from("")
    }

    pub fn pattern_definition(
        &self,
        colormapping: &ColorMapping,
    ) -> Option<svg::node::element::Pattern> {
        match self {
            Fill::Hatched(color, direction, size, thickness_ratio) => {
                let root = svg::node::element::Pattern::new()
                    .set("id", self.pattern_id())
                    .set("patternUnits", "userSpaceOnUse");

                let thickness = size * (2.0 * thickness_ratio);
                // TODO: to re-center when tickness ratio != ½
                let offset = 0.0;

                Some(match direction {
                    HatchDirection::BottomUpDiagonal => root
                        // https://stackoverflow.com/a/74205714/9943464
                        /*
                                          <polygon points="0,0 4,0 0,4" fill="yellow"></polygon>
                        <polygon points="0,8 8,0 8,4 4,8" fill="yellow"></polygon>
                        <polygon points="0,4 0,8 8,0 4,0" fill="green"></polygon>
                        <polygon points="4,8 8,8 8,4" fill="green"></polygon>
                                           */
                        .add(
                            svg::node::element::Polygon::new()
                                .set(
                                    "points",
                                    format!(
                                        "0,0 {},0 0,{}",
                                        offset + thickness / 2.0,
                                        offset + thickness / 2.0
                                    ),
                                )
                                .set("fill", color.render(colormapping)),
                        )
                        .add(
                            svg::node::element::Polygon::new()
                                .set(
                                    "points",
                                    format!(
                                        "0,{} {},0 {},{} {},{}",
                                        offset + size,
                                        offset + size,
                                        offset + size,
                                        offset + thickness / 2.0,
                                        offset + thickness / 2.0,
                                        offset + size,
                                    ),
                                )
                                .set("fill", color.render(colormapping)),
                        )
                        .set("height", size * 2.0)
                        .set("width", size * 2.0)
                        .set("viewBox", format!("0,0,{},{}", size, size)),
                    HatchDirection::Horizontal => todo!(),
                    HatchDirection::Vertical => todo!(),
                    HatchDirection::TopDownDiagonal => todo!(),
                })
            }
            _ => None,
        }
    }
}

