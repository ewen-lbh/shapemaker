use docopt::Docopt;
use serde::Deserialize;
use serde_json;
use shapemaker::{
    Anchor, AudioSyncPaths, Canvas, CenterAnchor, Color, ColorMapping, Fill,
    MusicalDurationUnit::*, Object, Parsable, Video,
};
use std::collections::HashMap;
use std::fs::File;
use std::io::BufReader;

const USAGE: &'static str = "
▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄
█░▄▄█░████░▄▄▀█▀▄▄▀█░▄▄█░▄▀▄░█░▄▄▀█░█▀█░▄▄█░▄▄▀█
█▄▄▀█░▄▄░█░▀▀░█░▀▀░█░▄▄█░█▄█░█░▀▀░█░▄▀█░▄▄█░▀▀▄█
█▄▄▄█▄██▄█▄██▄█░████▄▄▄█▄███▄█▄██▄█▄█▄█▄▄▄█▄█▄▄█
▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀vVERSION▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀

Usage: shapemaker (image|video) [options] [--color <mapping>...] <file>
       shapemaker --help
       shapemaker --version
    
Options:
    --resolution <pixelcount>      Size of the image (or frames)'s largest dimension in pixels [default: 1000]
    --colors <file>                JSON file mapping color names to hex values
                                   The supported color names are: black, white, red, green, blue, yellow, orange, purple, brown, pink, gray, and cyan.
    -c --color <mapping>           Color mapping in the form of <color>:<hex>. Can be used multiple times.
    --grid-size <WIDTHxHEIGHT>     Size of the grid (number of anchor points) [default: 3x3]
                                   Putting one of the dimensions to 1 can cause a crash.
    --cell-size <size>             Size of a cell in pixels [default: 50]
    --canvas-padding <size>        Outter canvas padding between cells in pixels [default: 10]
    --line-width <size>            Width of the lines in pixels [default: 2]
    --small-circle-radius <size>   Radius of small circles in pixels [default: 5]
    --dot-radius <size>            Radius of dots in pixels [default: 2]
    --empty-shape-stroke <size>    Width of the stroke when a closed shape is not filled [default: 0.5]
    --render-grid                  Render the grid of anchor points
    --objects-count <range>        Number of objects to render [default: 3..6]
    --polygon-vertices <range>     Number of vertices for polygons [default: 2..6]

        Note: <range>s are inclusive on both ends
    
    Video-specific:
    --fps <fps>                    Frames per second [default: 30]
    --sync-to <directory>          Directory containing the audio files to sync to.
                                   The directory must contain:
                                   - stems/(instrument name).wav — stems
                                   - landmarks.json — JSON file mapping time in milliseconds to marker text (see ./landmarks.py)
                                   - full.mp3 — the complete audio file to use as the video's audio
                                   - bpm.txt — the BPM of the audio file (see ./landmarks.py)
    

";

fn main() {
    let args: Args = Docopt::new(USAGE.replace("VERSION", env!("CARGO_PKG_VERSION")))
        .and_then(|d| d.deserialize())
        .unwrap_or_else(|e| e.exit());

    if args.flag_version {
        println!("shapemaker {}", env!("CARGO_PKG_VERSION"));
        std::process::exit(0);
    }

    let mut canvas = Canvas::default_settings();
    canvas.colormap = load_colormap(&args);
    set_canvas_settings_from_args(&args, &mut canvas);

    if args.cmd_image && !args.cmd_video {
        canvas.set_shape(canvas.random_shape("main"));
        match canvas.save_as_png(&args.arg_file, args.flag_resolution.unwrap_or(1000)) {
            Ok(_) => println!("Image saved to {}", args.arg_file),
            Err(e) => println!("Error saving image: {}", e),
        }
        return;
    }

    let audiosync_dir = args.flag_sync_to.unwrap_or("audiosync".to_string());

    Video::<(Anchor, CenterAnchor, Color, Color)>::new()
        .sync_to(&AudioSyncPaths {
            stems: audiosync_dir.clone() + "/stems/",
            landmarks: audiosync_dir.clone() + "/landmarks.json",
            complete: audiosync_dir.clone() + "/full.mp3",
            bpm: audiosync_dir.clone() + "/bpm.txt",
        })
        .set_fps(args.flag_fps.unwrap_or(30))
        .set_initial_canvas(canvas)
        .init(&|canvas: _, context: _| {
            context.extra = (
                canvas.random_anchor(),
                canvas.random_center_anchor(),
                canvas.random_color(),
                canvas.random_color(),
            );
            canvas.set_background(context.extra.3);
        })
        .each_beat(&|canvas: _, context: _| {
            canvas.add_object(
                "beatdot",
                (
                    Object::BigCircle(context.extra.1),
                    Some(Fill::Solid(Color::Cyan)),
                ),
            );
            context.later_beats(0.5, &|canvas: &mut Canvas, _| {
                canvas.remove_object("beatdot");
            });
        })
        .every(0.5, Beats, &|canvas, context| {
            canvas.add_object(
                "halfbeat",
                (canvas.random_polygon(), Some(Fill::Solid(context.extra.2))),
            );
            context.later_beats(0.25, &|canvas, _| {
                canvas.remove_object("halfbeat");
            })
        })
        /*.each_frame(&|canvas: _, context: _| {
            if true {
                canvas.remove_object("time");
                canvas.add_object(
                    "time",
                    (
                        Object::RawSVG(Box::new(
                            svg::node::element::Text::new()
                                .set("x", 100)
                                .set("y", 200)
                                .set("font-size", 50)
                                .set("fill", "white")
                                .set("font-family", "monospace")
                                .add(svg::node::Text::new(format!(
                                    "{:04} &bull; {}",
                                    context.frame, context.timestamp
                                ))),
                        )),
                        None,
                    ),
                );
                let float_beat = context.bpm as f64 * context.ms as f64 / 1000.0 / 60.0;
                canvas.add_object(
                    "floatbeat",
                    (
                        Object::RawSVG(Box::new(
                            svg::node::element::Text::new()
                                .set("x", 100)
                                .set("y", 250)
                                .set("font-size", 30)
                                .set("fill", "white")
                                .set("font-family", "monospace")
                                .add(svg::node::Text::new(format!(
                                    "beat {} ({})",
                                    context.beat, float_beat
                                ))),
                        )),
                        None,
                    ),
                );
                canvas.remove_object("smallinfo");
                canvas.add_object(
                    "smallinfo",
                    (
                        Object::RawSVG(Box::new(
                            svg::node::element::Text::new()
                                .set("x", 100)
                                .set("y", 300)
                                .set("font-size", 15)
                                .set("fill", "white")
                                .set("font-family", "monospace")
                                .add(svg::node::Text::new(format!(
                                    "bpm {} duration {} current ms {}",
                                    context.bpm,
                                    milliseconds_to_timestamp(context.duration_ms()),
                                    context.ms,
                                ))),
                        )),
                        None,
                    ),
                )
            }
            canvas.remove_object("kickcircle");
            if context.stem("kick").amplitude_relative() > 0.5 {
                canvas.add_object(
                    "kickcircle",
                    (
                        Object::SmallCircle(context.extra.0),
                        Some(Fill::Solid(Color::Yellow)),
                    ),
                );
            }
        }) */
        .on("start credits", &|canvas, _| {
            canvas.add_object(
                "credits text",
                (
                    Object::RawSVG(Box::new(svg::node::Text::new("by ewen-lbh"))),
                    None,
                ),
            );
        })
        .on("end credits", &|canvas, _| {
            canvas.remove_object("credits text");
        })
        .command("add", &|argumentsline, canvas, _| {
            let args = argumentsline.splitn(3, ' ').collect::<Vec<_>>();
            let name = args[0];
            let object = Object::parse(args[2].to_string());
            let fill = Option::<Fill>::parse(args[1].to_string());
            canvas.add_object(name, (object, fill));
        })
        .command("remove", &|argumentsline, canvas, _| {
            let args = argumentsline.splitn(3, ' ').collect::<Vec<_>>();
            canvas.remove_object(args[0]);
        })
        .render_to(args.arg_file);
}

#[derive(Debug, Deserialize)]
struct Args {
    cmd_image: bool,
    cmd_video: bool,
    arg_file: String,
    flag_version: bool,
    flag_color: Vec<String>,
    flag_colors: Option<String>,
    flag_grid_size: Option<String>,
    flag_cell_size: Option<usize>,
    flag_canvas_padding: Option<usize>,
    flag_line_width: Option<f32>,
    flag_small_circle_radius: Option<f32>,
    flag_dot_radius: Option<f32>,
    flag_empty_shape_stroke: Option<f32>,
    flag_render_grid: bool,
    flag_objects_count: Option<String>,
    flag_polygon_vertices: Option<String>,
    flag_fps: Option<usize>,
    flag_sync_to: Option<String>,
    flag_resolution: Option<usize>,
}

fn set_canvas_settings_from_args(args: &Args, canvas: &mut Canvas) {
    if let Some(dimensions) = &args.flag_grid_size {
        let mut split = dimensions.split('x');
        let width = split.next().unwrap().parse::<usize>().unwrap();
        let height = split.next().unwrap().parse::<usize>().unwrap();
        canvas.grid_size = (width, height);
    }
    if let Some(cell_size) = args.flag_cell_size {
        canvas.cell_size = cell_size;
    }
    if let Some(canvas_padding) = args.flag_canvas_padding {
        canvas.canvas_outter_padding = canvas_padding;
    }
    if let Some(line_width) = args.flag_line_width {
        canvas.line_width = line_width;
    }
    if let Some(small_circle_radius) = args.flag_small_circle_radius {
        canvas.small_circle_radius = small_circle_radius;
    }
    if let Some(dot_radius) = args.flag_dot_radius {
        canvas.dot_radius = dot_radius;
    }
    if let Some(empty_shape_stroke) = args.flag_empty_shape_stroke {
        canvas.empty_shape_stroke_width = empty_shape_stroke;
    }
    canvas.render_grid = args.flag_render_grid;
    if let Some(objects_count) = &args.flag_objects_count {
        let mut split = objects_count.split("..");
        let min = split.next().unwrap().parse::<usize>().unwrap();
        let max = split.next().unwrap().parse::<usize>().unwrap();
        // +1 because the range is exclusive, using ..= raises a type error
        canvas.objects_count_range = min..(max + 1);
    }
    if let Some(polygon_vertices) = &args.flag_polygon_vertices {
        let mut split = polygon_vertices.split("..");
        let min = split.next().unwrap().parse::<usize>().unwrap();
        let max = split.next().unwrap().parse::<usize>().unwrap();
        canvas.polygon_vertices_range = min..(max + 1);
    }
}

fn load_colormap(args: &Args) -> ColorMapping {
    if let Some(file) = &args.flag_colors {
        let file = File::open(file).unwrap();
        let reader = BufReader::new(file);
        serde_json::from_reader(reader).unwrap()
    } else {
        let mut colormap: HashMap<String, String> = HashMap::new();
        for mapping in &args.flag_color {
            let mut split = mapping.split(':');
            let color = split.next().unwrap();
            let hex = split.next().unwrap();
            colormap.insert(color.to_string(), hex.to_string());
        }
        ColorMapping {
            black: colormap
                .get("black")
                .unwrap_or(&ColorMapping::default().black)
                .to_string(),
            white: colormap
                .get("white")
                .unwrap_or(&ColorMapping::default().white)
                .to_string(),
            red: colormap
                .get("red")
                .unwrap_or(&ColorMapping::default().red)
                .to_string(),
            green: colormap
                .get("green")
                .unwrap_or(&ColorMapping::default().green)
                .to_string(),
            blue: colormap
                .get("blue")
                .unwrap_or(&ColorMapping::default().blue)
                .to_string(),
            yellow: colormap
                .get("yellow")
                .unwrap_or(&ColorMapping::default().yellow)
                .to_string(),
            orange: colormap
                .get("orange")
                .unwrap_or(&ColorMapping::default().orange)
                .to_string(),
            purple: colormap
                .get("purple")
                .unwrap_or(&ColorMapping::default().purple)
                .to_string(),
            brown: colormap
                .get("brown")
                .unwrap_or(&ColorMapping::default().brown)
                .to_string(),
            pink: colormap
                .get("pink")
                .unwrap_or(&ColorMapping::default().pink)
                .to_string(),
            gray: colormap
                .get("gray")
                .unwrap_or(&ColorMapping::default().gray)
                .to_string(),
            cyan: colormap
                .get("cyan")
                .unwrap_or(&ColorMapping::default().cyan)
                .to_string(),
        }
    }
}
