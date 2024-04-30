use docopt::Docopt;
use serde::Deserialize;
use shapemaker::{Canvas, ColorMapping};

const USAGE: &str = "
▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄
█░▄▄█░████░▄▄▀█▀▄▄▀█░▄▄█░▄▀▄░█░▄▄▀█░█▀█░▄▄█░▄▄▀█
█▄▄▀█░▄▄░█░▀▀░█░▀▀░█░▄▄█░█▄█░█░▀▀░█░▄▀█░▄▄█░▀▀▄█
█▄▄▄█▄██▄█▄██▄█░████▄▄▄█▄███▄█▄██▄█▄█▄█▄▄▄█▄█▄▄█
▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀v?.?.?▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀

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
    --workers <number>             Number of parallel threads to use for rendering [default: 8]
    --fps <fps>                    Frames per second [default: 30]
    --audio <file>                 Audio file to use for the video
    --duration <seconds>           Number of seconds to render. If not set, the video will be as long as the audio file.
    --start <seconds>              Start the video at this time in seconds. [default: 0]
    --preview                      Only create preview.html, not the output video. Preview.html will be created in the same directory as <file>, but <file> will not be created.
    --sync-with <directory>        Directory containing the audio files to sync to.
                                   The directory must contain:
                                   - stems/(instrument name).wav — stems
                                   - landmarks.json — JSON file mapping time in milliseconds to marker text (see ./landmarks.py)
                                   - full.mp3 — the complete audio file to use as the video's audio
                                   - bpm.txt — the BPM of the audio file (see ./landmarks.py)


";

pub fn cli_args() -> Args {
    let args: Args = Docopt::new(USAGE.replace("?.?.?", env!("CARGO_PKG_VERSION")))
        .and_then(|d| d.deserialize())
        .unwrap_or_else(|e| e.exit());

    if args.flag_version {
        println!("shapemaker {}", env!("CARGO_PKG_VERSION"));
        std::process::exit(0);
    }

    args
}

pub fn canvas_from_cli(args: &Args) -> Canvas {
    let mut canvas = Canvas::new(vec![]);
    canvas.colormap = load_colormap(args);
    set_canvas_settings_from_args(args, &mut canvas);
    canvas
}

#[derive(Debug, Deserialize)]
pub struct Args {
    pub cmd_image: bool,
    pub cmd_video: bool,
    pub arg_file: String,
    pub flag_version: bool,
    pub flag_color: Vec<String>,
    pub flag_colors: Option<String>,
    pub flag_grid_size: Option<String>,
    pub flag_cell_size: Option<usize>,
    pub flag_canvas_padding: Option<usize>,
    pub flag_line_width: Option<f32>,
    pub flag_small_circle_radius: Option<f32>,
    pub flag_dot_radius: Option<f32>,
    pub flag_empty_shape_stroke: Option<f32>,
    pub flag_render_grid: bool,
    pub flag_objects_count: Option<String>,
    pub flag_polygon_vertices: Option<String>,
    pub flag_fps: Option<usize>,
    pub flag_sync_with: Option<String>,
    pub flag_audio: Option<String>,
    pub flag_resolution: Option<usize>,
    pub flag_workers: Option<usize>,
    pub flag_duration: Option<usize>,
    pub flag_start: Option<usize>,
    pub flag_preview: bool,
}

fn set_canvas_settings_from_args(args: &Args, canvas: &mut Canvas) {
    if let Some(dimensions) = &args.flag_grid_size {
        let mut split = dimensions.split('x');
        let width = split.next().unwrap().parse::<usize>().unwrap();
        let height = split.next().unwrap().parse::<usize>().unwrap();
        canvas.set_grid_size(width, height);
    }
    if let Some(cell_size) = args.flag_cell_size {
        canvas.cell_size = cell_size;
    }
    if let Some(canvas_padding) = args.flag_canvas_padding {
        canvas.canvas_outter_padding = canvas_padding;
    }
    if let Some(line_width) = args.flag_line_width {
        canvas.object_sizes.default_line_width = line_width;
    }
    if let Some(small_circle_radius) = args.flag_small_circle_radius {
        canvas.object_sizes.small_circle_radius = small_circle_radius;
    }
    if let Some(dot_radius) = args.flag_dot_radius {
        canvas.object_sizes.dot_radius = dot_radius;
    }
    if let Some(empty_shape_stroke) = args.flag_empty_shape_stroke {
        canvas.object_sizes.empty_shape_stroke_width = empty_shape_stroke;
    }
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
        ColorMapping::from_file(file.into())
    } else {
        ColorMapping::from_cli_args(&args.flag_color)
    }
}
