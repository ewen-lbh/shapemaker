use std::hash::Hash;

use wasm_bindgen::prelude::*;

use crate::RenderCSS;

#[wasm_bindgen]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FilterType {
    Glow,
    NaturalShadow,
    Saturation,
}

#[wasm_bindgen]
#[derive(Debug, Clone, Copy)]
pub struct Filter {
    pub kind: FilterType,
    pub parameter: f32,
}

#[wasm_bindgen]
impl Filter {
    pub fn name(&self) -> String {
        match self.kind {
            FilterType::Glow => "glow",
            FilterType::NaturalShadow => "natural-shadow-filter",
            FilterType::Saturation => "saturation",
        }
        .to_owned()
    }

    pub fn glow(intensity: f32) -> Self {
        Self {
            kind: FilterType::Glow,
            parameter: intensity,
        }
    }

    pub fn id(&self) -> String {
        format!(
            "{}-{}",
            self.name(),
            self.parameter.to_string().replace(".", "_")
        )
    }
}

impl Filter {
    pub fn definition(&self) -> svg::node::element::Filter {
        match self.kind {
            FilterType::Glow => {
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
                    .add(
                        // TODO parameterize stdDeviation
                        svg::node::element::FilterEffectGaussianBlur::new()
                            .set("stdDeviation", self.parameter)
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
            FilterType::NaturalShadow => {
                /*
                              <filter id="natural-shadow-filter" x="0" y="0" width="2" height="2">
                  <feOffset in="SourceGraphic" dx="3" dy="3" />
                  <feGaussianBlur stdDeviation="12" result="blur" />
                  <feMerge>
                    <feMergeNode in="blur" />
                    <feMergeNode in="SourceGraphic" />
                  </feMerge>
                </filter>
                               */
                svg::node::element::Filter::new()
                    .add(
                        svg::node::element::FilterEffectOffset::new()
                            .set("in", "SourceGraphic")
                            .set("dx", self.parameter)
                            .set("dy", self.parameter),
                    )
                    .add(
                        svg::node::element::FilterEffectGaussianBlur::new()
                            .set("stdDeviation", self.parameter * 4.0)
                            .set("result", "blur"),
                    )
                    .add(
                        svg::node::element::FilterEffectMerge::new()
                            .add(svg::node::element::FilterEffectMergeNode::new().set("in", "blur"))
                            .add(
                                svg::node::element::FilterEffectMergeNode::new()
                                    .set("in", "SourceGraphic"),
                            ),
                    )
            }
            FilterType::Saturation => {
                /*
                <filter id="saturation">
                    <feColorMatrix type="saturate" values="0.5"/>
                </filter>
                */
                svg::node::element::Filter::new().add(
                    svg::node::element::FilterEffectColorMatrix::new()
                        .set("type", "saturate")
                        .set("values", self.parameter),
                )
            }
        }
        .set("id", self.id())
        .set("filterUnit", "userSpaceOnUse")
    }
}

impl RenderCSS for Filter {
    fn render_fill_css(&self, _colormap: &crate::ColorMapping) -> String {
        format!("filter: url(#{}); overflow: visible;", self.id())
    }

    fn render_stroke_css(&self, colormap: &crate::ColorMapping) -> String {
        self.render_fill_css(colormap)
    }
}

impl PartialEq for Filter {
    fn eq(&self, other: &Self) -> bool {
        // TODO use way less restrictive epsilon
        self.kind == other.kind && (self.parameter - other.parameter).abs() < f32::EPSILON
    }
}

impl Hash for Filter {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id().hash(state)
    }
}

impl Eq for Filter {}
