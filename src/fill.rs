use crate::{Color, ColorMapping, RenderCSS};

#[derive(Debug, Clone, Copy)]
pub enum Fill {
    Solid(Color),
    Translucent(Color, f32),
    Hatched,
    Dotted,
}

impl RenderCSS for Fill {
    fn render_fill_css(&self, colormap: &ColorMapping) -> String {
        match self {
            Fill::Solid(color) => {
                format!("fill: {};", color.to_string(colormap))
            }
            Fill::Translucent(color, opacity) => {
                format!("fill: {}; opacity: {};", color.to_string(colormap), opacity)
            }
            Fill::Dotted => unimplemented!(),
            Fill::Hatched => unimplemented!(),
        }
    }

    fn render_stroke_css(&self, colormap: &ColorMapping) -> String {
        match self {
            Fill::Solid(color) => {
                format!("stroke: {};", color.to_string(colormap))
            }
            Fill::Translucent(color, opacity) => {
                format!(
                    "stroke: {}; opacity: {};",
                    color.to_string(colormap),
                    opacity
                )
            }
            Fill::Dotted => unimplemented!(),
            Fill::Hatched => unimplemented!(),
        }
    }
}

impl RenderCSS for Option<Fill> {
    fn render_fill_css(&self, colormap: &ColorMapping) -> String {
        self.map(|fill| fill.render_fill_css(colormap))
            .unwrap_or_default()
    }

    fn render_stroke_css(&self, colormap: &ColorMapping) -> String {
        self.map(|fill| fill.render_stroke_css(colormap))
            .unwrap_or_default()
    }
}
