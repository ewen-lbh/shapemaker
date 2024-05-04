use std::ptr::NonNull;
use std::sync::Mutex;

use once_cell::sync::Lazy;
use rand::Rng;
use wasm_bindgen::{closure::Closure, prelude::wasm_bindgen};
use wasm_bindgen::{JsValue, UnwrapThrowExt};

use crate::{
    examples, layer, Anchor, Canvas, CenterAnchor, Color, ColorMapping, Fill, Filter, FilterType,
    HatchDirection, Layer, Object, Point, Region,
};

static WEB_CANVAS: Lazy<Mutex<Canvas>> = Lazy::new(|| Mutex::new(Canvas::default_settings()));

fn canvas() -> std::sync::MutexGuard<'static, Canvas> {
    WEB_CANVAS.lock().unwrap()
}

#[wasm_bindgen(start)]
pub fn js_init() -> Result<(), JsValue> {
    render_image(0.0, Color::Black)?;
    Ok(())
}

// Can't bind Color.name directly, see https://github.com/rustwasm/wasm-bindgen/issues/1715
#[wasm_bindgen]
pub fn color_name(c: Color) -> String {
    c.name()
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    pub fn log(s: &str);
}

macro_rules! console_log {
    ($($t:tt)*) => (crate::log(&format_args!($($t)*).to_string()))
}

pub(crate) use console_log;

#[wasm_bindgen]
pub fn render_image(opacity: f32, color: Color) -> Result<(), JsValue> {
    let mut canvas = examples::dna_analysis_machine();
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

    canvas.remove_all_objects_in(&Region::from_topleft(Point(8, 2), (2, 2)));
    canvas.remove_all_objects_in(&Point(11, 7).region());

    *WEB_CANVAS.lock().unwrap() = canvas;
    render_canvas_at(String::from("body"));

    Ok(())
}

#[wasm_bindgen]
pub fn map_to_midi_controller() {

}

#[wasm_bindgen]
pub fn render_canvas_into(selector: String) -> () {
    let svgstring = canvas().render(&vec!["*"], false);
    append_new_div_inside(svgstring, selector)
}

#[wasm_bindgen]
pub fn render_canvas_at(selector: String) -> () {
    let svgstring = canvas().render(&vec!["*"], false);
    replace_content_with(svgstring, selector)
}

#[wasm_bindgen]
pub enum MidiEvent {
    Note,
    ControlChange,
}

#[wasm_bindgen]
pub struct MidiEventData([u8; 3]);

#[wasm_bindgen]
pub struct MidiPitch(u8);

#[wasm_bindgen]
impl MidiPitch {
    pub fn octave(&self) -> u8 {
        self.0 / 12
    }
}

pub struct Percentage(pub f32);

impl From<u8> for Percentage {
    fn from(value: u8) -> Self {
        Self(value as f32 / 127.0)
    }
}

pub enum MidiMessage {
    NoteOn(MidiPitch, Percentage),
    NoteOff(MidiPitch),
    PedalOn,
    PedalOff,
    ControlChange(u8, Percentage),
}

impl From<(MidiEvent, MidiEventData)> for MidiMessage {
    fn from(value: (MidiEvent, MidiEventData)) -> Self {
        match value {
            (MidiEvent::Note, MidiEventData([pitch, velocity, _])) => {
                if velocity == 0 {
                    MidiMessage::NoteOff(MidiPitch(pitch))
                } else {
                    MidiMessage::NoteOn(MidiPitch(pitch), velocity.into())
                }
            }
            (MidiEvent::ControlChange, MidiEventData([64, value, _])) => {
                if value == 0 {
                    MidiMessage::PedalOff
                } else {
                    MidiMessage::PedalOn
                }
            }
            (MidiEvent::ControlChange, MidiEventData([controller, value, _])) => {
                MidiMessage::ControlChange(controller, value.into())
            }
        }
    }
}

#[wasm_bindgen]
pub fn render_canvas(layers_pattern: Option<String>, render_background: Option<bool>) -> String {
    canvas().render(
        &match layers_pattern {
            Some(ref pattern) => vec![pattern],
            None => vec!["*"],
        },
        render_background.unwrap_or(false),
    )
}

#[wasm_bindgen]
pub fn set_palette(palette: ColorMapping) -> () {
    canvas().colormap = palette;
}

#[wasm_bindgen]
pub fn new_layer(name: &str) -> LayerWeb {
    canvas().add_or_replace_layer(Layer::new(name));
    LayerWeb {
        name: name.to_string(),
    }
}

#[wasm_bindgen]
pub fn get_layer(name: &str) -> Result<LayerWeb, JsValue> {
    match canvas().layer_safe(name) {
        Some(layer) => Ok(LayerWeb {
            name: layer.name.clone(),
        }),
        None => Err(JsValue::from_str("Layer not found")),
    }
}

#[wasm_bindgen]
pub fn random_linelikes(name: &str) -> LayerWeb {
    let layer = canvas().random_linelikes(name);
    canvas().add_or_replace_layer(layer);
    LayerWeb {
        name: name.to_string(),
    }
}

fn document() -> web_sys::Document {
    let window = web_sys::window().expect_throw("no global `window` exists");
    window
        .document()
        .expect_throw("should have a document on window")
}

fn query_selector(selector: String) -> web_sys::Element {
    document()
        .query_selector(&selector)
        .expect_throw(&format!("selector '{}' not found", selector))
        .expect_throw("could not get the element, but is was found (shouldn't happen)")
}

fn append_new_div_inside(content: String, selector: String) -> () {
    let output = document().create_element("div").unwrap();
    output.set_class_name("frame");
    output.set_inner_html(&content);
    query_selector(selector).append_child(&output).unwrap();
}

fn replace_content_with(content: String, selector: String) -> () {
    query_selector(selector).set_inner_html(&content);
}

#[wasm_bindgen(getter_with_clone)]
pub struct LayerWeb {
    pub name: String,
}

// #[wasm_bindgen()]

#[wasm_bindgen]
impl LayerWeb {
    pub fn render(&self) -> String {
        canvas().render(&vec![&self.name], false)
    }

    pub fn render_into(&self, selector: String) -> () {
        append_new_div_inside(self.render(), selector)
    }

    pub fn render_at(self, selector: String) -> () {
        replace_content_with(self.render(), selector)
    }

    pub fn paint_all(&self, color: Color, opacity: Option<f32>, filter: Filter) -> () {
        canvas()
            .layer(&self.name)
            .paint_all_objects(Fill::Translucent(color, opacity.unwrap_or(1.0)));
        canvas().layer(&self.name).filter_all_objects(filter);
    }

    pub fn random(name: &str) -> Self {
        let layer = canvas().random_layer(name);
        canvas().add_or_replace_layer(layer);
        LayerWeb {
            name: name.to_string(),
        }
    }

    pub fn new_line(
        &self,
        name: &str,
        start: Anchor,
        end: Anchor,
        thickness: f32,
        color: Color,
    ) -> () {
        canvas().layer(name).add_object(
            name,
            (
                Object::Line(start, end, thickness),
                Some(Fill::Solid(color)),
            )
                .into(),
        )
    }
    pub fn new_curve_outward(
        &self,
        name: &str,
        start: Anchor,
        end: Anchor,
        thickness: f32,
        color: Color,
    ) -> () {
        canvas().layer(name).add_object(
            name,
            Object::CurveOutward(start, end, thickness).color(Fill::Solid(color)),
        )
    }
    pub fn new_curve_inward(
        &self,
        name: &str,
        start: Anchor,
        end: Anchor,
        thickness: f32,
        color: Color,
    ) -> () {
        canvas().layer(name).add_object(
            name,
            Object::CurveInward(start, end, thickness).color(Fill::Solid(color)),
        )
    }
    pub fn new_small_circle(&self, name: &str, center: Anchor, color: Color) -> () {
        canvas()
            .layer(name)
            .add_object(name, Object::SmallCircle(center).color(Fill::Solid(color)))
    }
    pub fn new_dot(&self, name: &str, center: Anchor, color: Color) -> () {
        canvas()
            .layer(name)
            .add_object(name, Object::Dot(center).color(Fill::Solid(color)))
    }
    pub fn new_big_circle(&self, name: &str, center: CenterAnchor, color: Color) -> () {
        canvas()
            .layer(name)
            .add_object(name, Object::BigCircle(center).color(Fill::Solid(color)))
    }
    pub fn new_text(
        &self,
        name: &str,
        anchor: Anchor,
        text: String,
        font_size: f32,
        color: Color,
    ) -> () {
        canvas().layer(name).add_object(
            name,
            Object::Text(anchor, text, font_size).color(Fill::Solid(color)),
        )
    }
    pub fn new_rectangle(
        &self,
        name: &str,
        topleft: Anchor,
        bottomright: Anchor,
        color: Color,
    ) -> () {
        canvas().layer(name).add_object(
            name,
            Object::Rectangle(topleft, bottomright).color(Fill::Solid(color)),
        )
    }
}
