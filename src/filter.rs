use wasm_bindgen::prelude::*;

use crate::RenderCSS;

#[wasm_bindgen]
#[derive(Debug, Clone, Copy)]
pub enum Filter {
    Glow,
}

impl Filter {
    pub fn definition(&self) -> svg::node::element::Filter {
        match self {
            Filter::Glow => {
                // format!(
                //     r#"
                //     <filter id="glow">
                //         <feGaussianBlur stdDeviation="{}" result="coloredBlur"/>
                //         <feMerge>
                //             <feMergeNode in="coloredBlur"/>
                //             <feMergeNode in="SourceGraphic"/>
                //         </feMerge>
                //     </filter>
                // "#,
                //     2.5
                // ) // TODO parameterize stdDeviation
                svg::node::element::Filter::new()
                    .set("id", "glow")
                    .add(
                        // TODO parameterize stdDeviation
                        svg::node::element::FilterEffectGaussianBlur::new()
                            .set("stdDeviation", 5)
                            .set("result", "coloredBlur"),
                    )
                    .add(
                        svg::node::element::FilterEffectMerge::new()
                            .add(
                                svg::node::element::FilterEffectMergeNode::new()
                                    .set("in", "coloredBlur"),
                            )
                            .add(
                                svg::node::element::FilterEffectMergeNode::new()
                                    .set("in", "SourceGraphic"),
                            ),
                    )
            }
        }
    }
}

impl RenderCSS for Filter {
    fn render_fill_css(&self, _colormap: &crate::ColorMapping) -> String {
        match self {
            Filter::Glow => {
                format!("filter: url(#glow);")
            }
        }
    }

    fn render_stroke_css(&self, colormap: &crate::ColorMapping) -> String {
        self.render_fill_css(colormap)
    }
}
