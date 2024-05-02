use crate::{Color, ColorMapping, RenderCSS};

#[derive(Debug, Clone, Copy)]
pub enum HatchDirection {
    Horizontal,
    Vertical,
    BottomUpDiagonal,
    TopDownDiagonal,
}

impl HatchDirection {
    pub fn svg_filter_name(&self) -> String {
        "hatch-".to_owned()
            + match self {
                HatchDirection::Horizontal => "horizontal",
                HatchDirection::Vertical => "vertical",
                HatchDirection::BottomUpDiagonal => "bottom-up",
                HatchDirection::TopDownDiagonal => "top-down",
            }
    }

    pub fn svg_pattern_definition(&self) -> String {
        // https://stackoverflow.com/a/14500054/9943464
        format!(
            r#"<pattern id="{}" patternUnits="userSpaceOnUse" width="{}" height="{}">"#,
            self.svg_filter_name(),
            todo!(),
            todo!()
        ) + &match self {
            HatchDirection::BottomUpDiagonal => format!(
                r#"<path 
                    d="M-1,1 l2,-2
                       M0,4 l4,-4
                       M3,5 l2,-2" 
                    style="stroke:black; stroke-width:1" 
                />"#
            ),
            HatchDirection::Horizontal => todo!(),
            HatchDirection::Vertical => todo!(),
            HatchDirection::TopDownDiagonal => todo!(),
        } + "</pattern>"
    }
}

#[derive(Debug, Clone, Copy)]
pub enum Fill {
    Solid(Color),
    Translucent(Color, f32),
    Hatched(HatchDirection),
    Dotted(f32),
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
            Fill::Dotted(radius) => unimplemented!(),
            Fill::Hatched(direction) => {
                format!("fill: url(#{});", direction.svg_filter_name())
            }
        }
    }

    fn render_stroke_css(&self, colormap: &ColorMapping) -> String {
        match self {
            Fill::Solid(color) => {
                format!("stroke: {}; fill: transparent;", color.to_string(colormap))
            }
            Fill::Translucent(color, opacity) => {
                format!(
                    "stroke: {}; opacity: {}; fill: transparent;",
                    color.to_string(colormap),
                    opacity
                )
            }
            Fill::Dotted(..) => unimplemented!(),
            Fill::Hatched(..) => unimplemented!(),
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
