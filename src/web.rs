use wasm_bindgen::prelude::wasm_bindgen;
use wasm_bindgen::JsValue;

use crate::{Canvas, Color, ColorMapping, Fill};

#[wasm_bindgen(start)]
pub fn main() -> Result<(), JsValue> {
    render_image(0.0, "black")?;
    Ok(())
}

#[wasm_bindgen]
pub fn render_image(opacity: f32, color: &str) -> Result<(), JsValue> {
    let mut canvas = Canvas::default_settings();
    canvas.colormap = ColorMapping {
        black: "#ffffff".into(),
        white: "#ffffff".into(),
        red: "#cf0a2b".into(),
        green: "#22e753".into(),
        blue: "#2734e6".into(),
        yellow: "#f8e21e".into(),
        orange: "#f05811".into(),
        purple: "#6a24ec".into(),
        brown: "#a05634".into(),
        pink: "#e92e76".into(),
        gray: "#81a0a8".into(),
        cyan: "#4fecec".into(),
    };

    canvas.set_grid_size(4, 4);

    let mut layer = canvas.random_layer(&color);
    layer.paint_all_objects(Fill::Translucent(color.into(), opacity));
    canvas.add_or_replace_layer(layer);

    let window = web_sys::window().expect("no global `window` exists");
    let document = window.document().expect("should have a document on window");
    let body = document.body().expect("document should have a body");

    let output = document.create_element("div")?;
    output.set_class_name("frame");
    output.set_attribute("data-color", color)?;
    output.set_inner_html(&canvas.render(&vec!["*"], false));
    body.append_child(&output)?;
    Ok(())
}
