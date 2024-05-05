use std::{collections::HashMap, fs, path::PathBuf};

use anyhow::Result;
use handlebars::Handlebars;
use itertools::Itertools;
use serde_json::json;

use crate::Canvas;

const FRAMES_BUFFER_SIZE: usize = 500;

pub fn render_template(
    frames: &HashMap<usize, String>,
    canvas: &Canvas,
    path_to_audio_file: PathBuf,
    port: usize,
) -> String {
    let template = String::from_utf8_lossy(include_bytes!("../preview/index.html.hbs"));
    let engine_js_source = String::from_utf8_lossy(include_bytes!("../preview/engine.js"));

    let hbs = Handlebars::new();
    hbs.render_template(
        &template,
        &json!({
            "frames":frames,
            "audiopath": path_to_audio_file,
            "enginesource": engine_js_source,
            "background": canvas.background.map_or("black".to_string(), |color| color.render(&canvas.colormap)),
            "serverorigin": format!("http://localhost:{}", port),
            "framesbuffersize": FRAMES_BUFFER_SIZE,
        }),
    )
    .unwrap()
}

// rendered_svg_frames should map ms timestamps to SVG strings
pub fn output_preview(
    canvas: &Canvas,
    rendered_svg_frames: &HashMap<usize, String>,
    server_port: usize,
    output_file: PathBuf,
    audio_file: PathBuf,
) -> Result<()> {
    let first_frames = rendered_svg_frames
        .iter()
        // over 3000 loaded frames get really heavy on the browser (too much DOM nodes)
        .sorted_by_key(|(ms, _)| *ms)
        .take((2 * FRAMES_BUFFER_SIZE).min(10_000))
        .map(|(ms, svg)| (*ms, svg.clone()))
        .collect::<HashMap<usize, String>>();

    let contents = render_template(&first_frames, canvas, audio_file, server_port);
    fs::write(output_file, contents)?;
    Ok(())
}

pub fn start_preview_server(port: usize, frames: HashMap<usize, String>) -> Result<()> {
    let server = tiny_http::Server::http(format!("0.0.0.0:{}", port)).unwrap();
    println!("Preview server running on port {}", port);
    let sorted_frames: Vec<(&usize, &String)> =
        frames.iter().sorted_by_key(|(ms, _)| *ms).collect();
    println!("{} frames available", sorted_frames.len());

    for request in server.incoming_requests() {
        let (frame_start_ms, requested_frames_count) = get_request_params(request.url());

        println!(
            "Request for {} frames @ {}ms",
            requested_frames_count, frame_start_ms,
        );

        let contents = sorted_frames
            .iter()
            .filter(|(ms, _)| **ms >= frame_start_ms)
            .take(requested_frames_count)
            .map(|(ms, svg_string)| {
                format!(
                    r#"<div style="display: none;" id="frame-{}" class="frame">{}</div>"#,
                    ms, svg_string
                )
            })
            .join("\n");

        request.respond(tiny_http::Response::from_string(contents).with_header(
            tiny_http::Header {
                field: "Access-Control-Allow-Origin".parse().unwrap(),
                value: "*".parse().unwrap(),
            },
        ))?;
    }
    Ok(())
}

// returns (ms timestamp of first frame to send, number of frames to send)
fn get_request_params(url: &str) -> (usize, usize) {
    let mut first_frame_ms = 0;
    let mut num_frames = 1;

    let (_, querystring) = url.split_once("?").unwrap_or(("", ""));
    for (key, value) in querystring
        .split("&")
        .map(|pair| pair.split_once("=").unwrap_or(("", "")))
    {
        match key {
            "from" => first_frame_ms = value.parse().unwrap_or(0),
            "next" => num_frames = value.parse().unwrap_or(1),
            _ => (),
        }
    }

    (first_frame_ms, num_frames)
}
