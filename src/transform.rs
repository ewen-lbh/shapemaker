use slug::slugify;
use wasm_bindgen::prelude::*;

use crate::RenderAttribute;

#[wasm_bindgen]
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransformationType {
    Scale,
    Rotate,
    Skew,
    Matrix,
}

#[wasm_bindgen(getter_with_clone)]
#[derive(Debug, Clone)]
pub struct TransformationWASM {
    pub kind: TransformationType,
    pub parameters: Vec<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Transformation {
    Scale(f32, f32),
    Rotate(f32),
    Skew(f32, f32),
    Matrix(f32, f32, f32, f32, f32, f32),
}

impl From<TransformationWASM> for Transformation {
    fn from(transformation: TransformationWASM) -> Self {
        match transformation.kind {
            TransformationType::Scale => {
                Transformation::Scale(transformation.parameters[0], transformation.parameters[1])
            }
            TransformationType::Rotate => Transformation::Rotate(transformation.parameters[0]),
            TransformationType::Skew => {
                Transformation::Skew(transformation.parameters[0], transformation.parameters[1])
            }
            TransformationType::Matrix => Transformation::Matrix(
                transformation.parameters[0],
                transformation.parameters[1],
                transformation.parameters[2],
                transformation.parameters[3],
                transformation.parameters[4],
                transformation.parameters[5],
            ),
        }
    }
}

impl Transformation {
    pub fn name(&self) -> String {
        match self {
            Transformation::Matrix(..) => "matrix",
            Transformation::Rotate(..) => "rotate",
            Transformation::Scale(..) => "scale",
            Transformation::Skew(..) => "skew",
        }
        .to_owned()
    }

    #[allow(non_snake_case)]
    pub fn ScaleUniform(scale: f32) -> Self {
        Transformation::Scale(scale, scale)
    }

    pub fn id(&self) -> String {
        slugify(format!("{:?}", self))
    }
}

impl RenderAttribute for Transformation {
    const MULTIPLE_VALUES_JOIN_BY: &'static str = " ";

    fn render_fill_attribute(&self, _colormap: &crate::ColorMapping) -> (String, String) {
        (
            "transform".to_owned(),
            match self {
                Transformation::Scale(x, y) => format!("scale({}  {})", x, y),
                Transformation::Rotate(angle) => format!("rotate({})", angle),
                Transformation::Skew(x, y) => format!("skewX({}) skewY({})", x, y),
                Transformation::Matrix(a, b, c, d, e, f) => {
                    format!("matrix({}, {}, {}, {}, {}, {})", a, b, c, d, e, f)
                }
            },
        )
    }

    fn render_stroke_attribute(&self, colormap: &crate::ColorMapping) -> (String, String) {
        self.render_fill_attribute(colormap)
    }
}
