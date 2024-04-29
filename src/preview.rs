use std::{collections::HashMap, path::PathBuf};

use handlebars::Handlebars;
use serde_json::json;

use crate::{Canvas, ColorMapping};

pub fn render_template(
    frames: HashMap<usize, String>,
    canvas: &Canvas,
    path_to_audio_file: PathBuf,
) -> String {
    let template = String::from_utf8_lossy(include_bytes!("../preview/index.html.hbs"));
    let engine_js_source = String::from_utf8_lossy(include_bytes!("../preview/engine.js"));

    let mut hbs = Handlebars::new();
    hbs.render_template(
        &template,
        &json!({
            "frames":frames,
            "audiopath": path_to_audio_file,
            "enginesource": engine_js_source,
            "background": canvas.background.map_or("black".to_string(), |color| color.to_string(&canvas.colormap))
        }),
    )
    .unwrap()
}
