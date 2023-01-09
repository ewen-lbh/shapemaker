use chrono::NaiveDateTime;
use serde_json;
use std::collections::HashMap;
use std::fmt::Formatter;
use std::fs::{create_dir, remove_dir_all, File};
use std::io::{BufReader, Write};
use std::ops::{Add, Range};
use std::sync::Arc;

use docopt::Docopt;
use rand::Rng;
use serde::Deserialize;

const USAGE: &'static str = "
▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄
█░▄▄█░████░▄▄▀█▀▄▄▀█░▄▄█░▄▀▄░█░▄▄▀█░█▀█░▄▄█░▄▄▀█
█▄▄▀█░▄▄░█░▀▀░█░▀▀░█░▄▄█░█▄█░█░▀▀░█░▄▀█░▄▄█░▀▀▄█
█▄▄▄█▄██▄█▄██▄█░████▄▄▄█▄███▄█▄██▄█▄█▄█▄▄▄█▄█▄▄█
▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀vVERSION▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀

Usage: shapemaker [options] [--color <mapping>...] <file>
       shapemaker --help
       shapemaker --version
    
Options:
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
    

";

#[derive(Debug, Deserialize)]
struct Args {
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
}

type RenderFunction<C> = dyn Fn(&mut Canvas, &mut Context<C>);
type CommandAction<C> = dyn Fn(Vec<&str>, &mut Canvas, &mut Context<C>);

/// Arguments: canvas, context, previous rendered beat
type HookCondition<C> = dyn Fn(&mut Canvas, &mut Context<C>, usize) -> bool;

type LaterRenderFunction<C> = dyn Fn(&mut Canvas, &Context<C>);

/// Arguments: canvas, context, previous rendered beat
type LaterHookCondition<C> = dyn Fn(&mut Canvas, &Context<C>, usize) -> bool;

#[derive(Debug)]
struct Video<C> {
    fps: usize,
    initial_canvas: Canvas,
    hooks: Vec<Hook<C>>,
    commands: Vec<Box<Command<C>>>,
    frames: Vec<Canvas>,
    frames_output_directory: &'static str,
    audio_paths: AudioSyncPaths,
    bpm: usize,
    markers: HashMap<usize, String>,
    stems: HashMap<String, Stem>,
}

struct Hook<C> {
    when: Box<HookCondition<C>>,
    render_function: Box<RenderFunction<C>>,
}

struct LaterHook<C> {
    when: Box<LaterHookCondition<C>>,
    render_function: Box<LaterRenderFunction<C>>,
}

impl<C> std::fmt::Debug for Hook<C> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Hook")
            .field("when", &"Box<HookCondition>")
            .field("render_function", &"Box<RenderFunction>")
            .finish()
    }
}

#[derive(Debug)]
struct Stem {
    amplitude_db: Vec<f32>,
    /// in milliseconds
    duration_ms: usize,
}

#[derive(Debug)]
struct StemAtInstant {
    amplitude: f32,
    duration: usize,
}

struct Command<C> {
    name: String,
    arguments: Vec<String>,
    action: Box<CommandAction<C>>,
}

impl<C> std::fmt::Debug for Command<C> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Command")
            .field("name", &self.name)
            .field("arguments", &self.arguments)
            .field("action", &"Box<CommandAction>")
            .finish()
    }
}

struct Context<'a, AdditionalContext = ()> {
    frame: usize,
    beat: usize,
    timestamp: String,
    ms: usize,
    bpm: usize,
    stems: &'a HashMap<String, Stem>,
    markers: &'a HashMap<usize, String>, // milliseconds -> marker text
    later_hooks: Vec<LaterHook<AdditionalContext>>,
    u: AdditionalContext,
}

impl<'a, C> Context<'a, C> {
    fn stem(&self, name: &str) -> StemAtInstant {
        StemAtInstant {
            amplitude: self.stems[name].amplitude_db[self.ms],
            duration: self.stems[name].duration_ms,
        }
    }

    fn marker(&self) -> String {
        self.markers
            .get(&self.ms)
            .unwrap_or(&"".to_string())
            .to_string()
    }

    fn duration_ms(&self) -> usize {
        self.stems
            .values()
            .map(|stem| stem.duration_ms)
            // .map(|_| duration_override)
            .max()
            .unwrap()
    }

    fn later_frames(&mut self, delay: usize, render_function: &'static LaterRenderFunction<C>) {
        let current_frame = self.frame;
        self.later_hooks.insert(
            0,
            LaterHook {
                when: Box::new(move |_, context, _previous_beat| {
                    println!("{} == {} + {}", context.frame, current_frame, delay);
                    context.frame >= current_frame + delay
                }),
                render_function: Box::new(render_function),
            },
        );
    }

    fn later_ms(&mut self, delay: usize, render_function: &'static LaterRenderFunction<C>) {
        let current_ms = self.ms;
        self.later_hooks.insert(
            0,
            LaterHook {
                when: Box::new(move |_, context, _previous_beat| {
                    println!("{} == {} + {}", context.ms, current_ms, delay);
                    context.ms >= current_ms + delay
                }),
                render_function: Box::new(render_function),
            },
        );
    }

    fn later_beats(&mut self, delay: usize, render_function: &'static LaterRenderFunction<C>) {
        let current_beat = self.beat;
        self.later_hooks.insert(
            0,
            LaterHook {
                when: Box::new(move |_, context, _previous_beat| {
                    context.beat >= current_beat + delay
                }),
                render_function: Box::new(render_function),
            },
        );
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct AudioSyncPaths {
    stems: &'static str,
    landmarks: &'static str,
    complete: &'static str,
    bpm: &'static str,
}

impl<AdditionalContext: Default> Video<AdditionalContext> {
    fn new() -> Self {
        Self {
            fps: 30,
            initial_canvas: Canvas::new(),
            hooks: vec![],
            commands: vec![],
            frames: vec![],
            audio_paths: AudioSyncPaths::default(),
            frames_output_directory: "audiosync/frames/",
            bpm: 0,
            markers: HashMap::new(),
            stems: HashMap::new(),
        }
    }

    fn build_video(&self, render_to: String) -> Result<(), String> {
        let result = std::process::Command::new("ffmpeg")
            // .arg(format!("-framerate {}", self.fps))
            // .arg("-pattern_type glob")
            // .arg(format!("-i '{}/*.png'", self.frames_output_directory))
            // .arg(format!("-i '{}'", self.audio_paths.complete))
            .arg("-framerate")
            .arg(self.fps.to_string())
            .arg("-pattern_type")
            .arg("glob")
            .arg("-i")
            .arg(format!("{}/*.png", self.frames_output_directory))
            .arg("-i")
            .arg(self.audio_paths.complete)
            .args([
                "-c:a",
                "copy",
                "-shortest",
                "-c:v",
                "libx264",
                "-pix_fmt",
                "yuv420p",
                "-y",
            ])
            .arg::<String>(render_to)
            .output();

        match result {
            Err(e) => Err(format!("Failed to execute ffmpeg: {}", e)),
            Ok(r) => {
                println!("{}", std::str::from_utf8(&r.stdout).unwrap());
                println!("{}", std::str::from_utf8(&r.stderr).unwrap());
                Ok(())
            }
        }
    }

    fn build_frame(&self, canvas: &mut Canvas, frame_no: usize) -> Result<(), String> {
        let mut spawned = std::process::Command::new("convert")
            .arg("-")
            .arg(format!(
                "{}/{:0width$}.png",
                self.frames_output_directory,
                frame_no,
                width = self.total_frames().to_string().len()
            ))
            .stdin(std::process::Stdio::piped())
            .spawn()
            .unwrap();

        let stdin = spawned.stdin.as_mut().unwrap();
        stdin.write_all(canvas.render().as_bytes()).unwrap();
        drop(stdin);

        match spawned.wait_with_output() {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("Failed to execute convert: {}", e)),
        }
    }

    fn set_fps(self, fps: usize) -> Self {
        Self { fps, ..self }
    }

    fn set_initial_canvas(self, canvas: Canvas) -> Self {
        Self {
            initial_canvas: canvas,
            ..self
        }
    }

    fn sync_to(self, audio: AudioSyncPaths) -> Self {
        // Read BPM from file
        let bpm = std::fs::read_to_string(audio.bpm)
            .map_err(|e| format!("Failed to read BPM file: {}", e))
            .and_then(|bpm| {
                println!("BPM in file: {}", bpm);
                bpm.trim()
                    .parse::<usize>()
                    .map(|parsed| {
                        println!("Parsed BPM: {}", parsed);
                        parsed
                    })
                    .map_err(|e| format!("Failed to parse BPM file: {}", e))
            })
            .unwrap();

        // Read landmakrs from JSON file
        let markers = std::fs::read_to_string(audio.landmarks)
            .map_err(|e| format!("Failed to read landmarks file: {}", e))
            .and_then(|landmarks| {
                match serde_json::from_str::<HashMap<String, String>>(&landmarks)
                    .map_err(|e| format!("Failed to parse landmarks file: {}", e))
                {
                    Ok(unparsed_keys) => {
                        let mut parsed_keys: HashMap<usize, String> = HashMap::new();
                        for (key, value) in unparsed_keys {
                            parsed_keys.insert(key.parse::<usize>().unwrap(), value);
                        }
                        Ok(parsed_keys)
                    }
                    Err(e) => Err(e),
                }
            })
            .unwrap();

        // Read all WAV stem files: get their duration and amplitude per millisecond
        let mut stems: HashMap<String, Stem> = HashMap::new();
        for entry in std::fs::read_dir(audio.stems)
            .map_err(|e| format!("Failed to read stems directory: {}", e))
            .unwrap()
            .filter(|e| match e {
                Ok(e) => e.path().extension().unwrap_or_default() == "wav",
                Err(_) => false,
            })
        {
            println!(
                "Reading stem file {}",
                entry.as_ref().unwrap().path().display()
            );
            let entry = entry.unwrap();
            let path = entry.path();
            let stem_name = path.file_stem().unwrap().to_str().unwrap();
            let mut reader = hound::WavReader::open(path.clone())
                .map_err(|e| format!("Failed to read stem file: {}", e))
                .unwrap();
            let spec = reader.spec();
            let samples_per_millisecond =
                (spec.sample_rate as usize / 1000 * spec.channels as usize);
            let mut amplitude_db: Vec<f32> = vec![];
            let mut current_amplitude_mean: f32 = 0.0;
            for (i, sample) in reader.samples::<i16>().enumerate() {
                let sample = sample.unwrap();
                if i % samples_per_millisecond == 0 {
                    amplitude_db.push(current_amplitude_mean);
                    current_amplitude_mean = 0.0;
                } else {
                    current_amplitude_mean += (20.0 * (sample as f32).log10()).abs();
                }
            }
            stems.insert(
                stem_name.to_string(),
                Stem {
                    amplitude_db,
                    duration_ms: (reader.duration() as f32 / spec.sample_rate as f32 * 1000.0)
                        as usize,
                },
            );
        }

        println!("registered bpm: {}", bpm);

        Self {
            audio_paths: audio,
            markers,
            bpm,
            stems,
            ..self
        }
    }

    fn on(
        self,
        marker_text: &'static str,
        render_function: &'static RenderFunction<AdditionalContext>,
    ) -> Self {
        let hook = Hook {
            when: Box::new(move |_, context, _| context.marker() == marker_text),
            render_function: Box::new(render_function),
        };
        let mut hooks = self.hooks;
        hooks.push(hook);
        Self { hooks, ..self }
    }

    fn on_beat(self, render_function: &'static RenderFunction<AdditionalContext>) -> Self {
        let hook = Hook {
            when: Box::new(move |_, context, previous_rendered_beat| {
                previous_rendered_beat != context.beat
            }),
            render_function: Box::new(render_function),
        };
        let mut hooks = self.hooks;
        hooks.push(hook);
        Self { hooks, ..self }
    }

    fn on_tick(self, render_function: &'static RenderFunction<AdditionalContext>) -> Self {
        let hook = Hook {
            when: Box::new(move |_, _, _| true),
            render_function: Box::new(render_function),
        };
        let mut hooks = self.hooks;
        hooks.push(hook);
        Self { hooks, ..self }
    }

    fn on_timestamp(
        self,
        timestamp: &'static str,
        render_function: &'static RenderFunction<AdditionalContext>,
    ) -> Self {
        let hook = Hook {
            when: Box::new(move |_, context, _| {
                let mut precision = "";
                let mut criteria_time = NaiveDateTime::default();
                if let Ok(criteria_time_parsed) =
                    NaiveDateTime::parse_from_str(timestamp, "%H:%M:%S%.3f")
                {
                    precision = "milliseconds";
                    criteria_time = criteria_time_parsed;
                } else if let Ok(criteria_time_parsed) =
                    NaiveDateTime::parse_from_str(timestamp, "%M:%S%.3f")
                {
                    precision = "milliseconds";
                    criteria_time = criteria_time_parsed;
                } else if let Ok(criteria_time_parsed) =
                    NaiveDateTime::parse_from_str(timestamp, "%S%.3f")
                {
                    precision = "milliseconds";
                    criteria_time = criteria_time_parsed;
                } else if let Ok(criteria_time_parsed) =
                    NaiveDateTime::parse_from_str(timestamp, "%S")
                {
                    precision = "seconds";
                    criteria_time = criteria_time_parsed;
                } else if let Ok(criteria_time_parsed) =
                    NaiveDateTime::parse_from_str(timestamp, "%M:%S")
                {
                    precision = "seconds";
                    criteria_time = criteria_time_parsed;
                } else if let Ok(criteria_time_parsed) =
                    NaiveDateTime::parse_from_str(timestamp, "%H:%M:%S")
                {
                    precision = "seconds";
                    criteria_time = criteria_time_parsed;
                } else {
                    panic!("Unhandled timestamp format: {}", timestamp);
                }
                match precision {
                    "milliseconds" => {
                        let current_time: NaiveDateTime =
                            NaiveDateTime::parse_from_str(timestamp, "%H:%M:%S%.3f").unwrap();
                        current_time == criteria_time
                    }
                    "seconds" => {
                        let current_time: NaiveDateTime =
                            NaiveDateTime::parse_from_str(timestamp, "%H:%M:%S").unwrap();
                        current_time == criteria_time
                    }
                    _ => panic!("Unknown precision"),
                }
            }),
            render_function: Box::new(render_function),
        };
        let mut hooks = self.hooks;
        hooks.push(hook);
        Self { hooks, ..self }
    }

    fn command(
        self,
        command_name: &'static str,
        arguments: Vec<&str>,
        action: &'static CommandAction<AdditionalContext>,
    ) -> Self {
        let mut commands = self.commands;
        commands.push(Box::new(Command {
            name: command_name.to_string(),
            arguments: arguments.iter().map(|s| s.to_string()).collect(),
            action: Box::new(action),
        }));
        Self { commands, ..self }
    }

    fn total_frames(&self) -> usize {
        // let duration_override = 5;
        self.fps * self.duration_ms() / 1000
    }

    fn duration_ms(&self) -> usize {
        self.stems
            .values()
            .map(|stem| stem.duration_ms)
            // .map(|_| duration_override)
            .max()
            .unwrap()
    }

    fn render_to(&self, output_file: String) {
        let mut context = Context {
            frame: 0,
            beat: 0,
            timestamp: "00:00:00.000".to_string(),
            ms: 0,
            bpm: self.bpm,
            stems: &self.stems,
            markers: &self.markers,
            u: AdditionalContext::default(),
            later_hooks: vec![],
        };

        let mut canvas = self.initial_canvas.clone();
        let mut previous_rendered_beat = 0;

        remove_dir_all(self.frames_output_directory.clone()).unwrap();
        create_dir(self.frames_output_directory.clone()).unwrap();

        let progress_bar = indicatif::ProgressBar::new(self.total_frames() as u64);

        for _ in 0..self.total_frames() {
            for hook in &self.hooks {
                if (hook.when)(&mut canvas, &mut context, previous_rendered_beat) {
                    (hook.render_function)(&mut canvas, &mut context);
                }
            }

            let mut later_hooks_to_delete: Vec<usize> = vec![];

            for (i, hook) in context.later_hooks.iter().enumerate() {
                println!("new late hook! checking if relevant…");
                if (hook.when)(&mut canvas, &context, previous_rendered_beat) {
                    (hook.render_function)(&mut canvas, &context);
                    later_hooks_to_delete.push(i);
                }
            }

            for i in later_hooks_to_delete {
                println!("deleting old late hook");
                context.later_hooks.remove(i);
            }

            if context.marker().starts_with("*") {
                let marker = context.marker();
                let args: Vec<&str> = marker.split(" ").map(|arg| arg.trim()).collect();
                let command_name = &args.get(0).unwrap();
                let arguments_count = args.len() - 1;

                for command in &self.commands {
                    if command.name == command_name.to_string() {
                        if arguments_count != command.arguments.len() {
                            panic!(
                                "Invalid number of arguments for command '{}'. Expected {}, got {}",
                                command_name,
                                command.arguments.len(),
                                arguments_count,
                            );
                        }
                        (command.action)(args.clone(), &mut canvas, &mut context);
                    }
                }
            }

            if let Err(e) = self.build_frame(&mut canvas, context.frame) {
                panic!("Failed to build frame: {}", e);
            }

            previous_rendered_beat = context.beat.clone();
            context.frame += 1;
            context.beat = (context.bpm as f32 * context.ms as f32 / 1000.0 / 60.0) as usize;
            context.ms += (1.0 / (self.fps as f32) * 1000.0) as usize;
            context.timestamp = format!("{}", milliseconds_to_timestamp(context.ms));
            progress_bar.inc(1);
        }

        progress_bar.finish();

        if let Err(e) = self.build_video(output_file) {
            panic!("Failed to build video: {}", e);
        }
    }
}

fn milliseconds_to_timestamp(ms: usize) -> String {
    format!(
        "{}",
        NaiveDateTime::from_timestamp_millis(ms as i64)
            .unwrap()
            .format("%H:%M:%S%.3f")
    )
}

fn main() {
    let args: Args = Docopt::new(USAGE.replace("VERSION", env!("CARGO_PKG_VERSION")))
        .and_then(|d| d.deserialize())
        .unwrap_or_else(|e| e.exit());

    if args.flag_version {
        println!("shapemaker {}", env!("CARGO_PKG_VERSION"));
        std::process::exit(0);
    }

    let mut canvas = Canvas::default_settings();

    canvas.colormap = if let Some(file) = args.flag_colors {
        let file = File::open(file).unwrap();
        let reader = BufReader::new(file);
        serde_json::from_reader(reader).unwrap()
    } else {
        let mut colormap: HashMap<String, String> = HashMap::new();
        for mapping in args.flag_color {
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
    };

    if let Some(dimensions) = args.flag_grid_size {
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

    if let Some(objects_count) = args.flag_objects_count {
        let mut split = objects_count.split("..");
        let min = split.next().unwrap().parse::<usize>().unwrap();
        let max = split.next().unwrap().parse::<usize>().unwrap();
        // +1 because the range is exclusive, using ..= raises a type error
        canvas.objects_count_range = min..(max + 1);
    }

    if let Some(polygon_vertices) = args.flag_polygon_vertices {
        let mut split = polygon_vertices.split("..");
        let min = split.next().unwrap().parse::<usize>().unwrap();
        let max = split.next().unwrap().parse::<usize>().unwrap();
        canvas.polygon_vertices_range = min..(max + 1);
    }

    Video::new()
        .sync_to(AudioSyncPaths {
            stems: "audiosync/stems/",
            landmarks: "audiosync/landmarks.json",
            complete: "audiosync/sample.mp3",
            bpm: "audiosync/bpm.txt",
        })
        .set_fps(30)
        .set_initial_canvas(canvas)
        .on_beat(&|canvas: &mut Canvas, context: &mut Context<()>| {
            canvas.clear();
            canvas.set_background(if context.beat % 2 == 0 {
                Color::Black
            } else {
                Color::Red
            });
            canvas.add_object(
                "beatdot".to_string(),
                (
                    Object::BigCircle(CenterAnchor(-1, -1)),
                    Some(Fill::Solid(Color::Cyan)),
                ),
            );
            context.later_ms(200, &|canvas: &mut Canvas, _| {
                println!("removing beatdot");
                canvas.remove_object("beatdot");
            });
            canvas.add_object(
                "beat".to_string(),
                (
                    Object::RawSVG(Box::new(
                        svg::node::element::Text::new()
                            .set("x", 100)
                            .set("y", 100)
                            .set("font-size", 100)
                            .set("fill", "white")
                            .set("font-family", "monospace")
                            .add(svg::node::Text::new(format!("{}", context.beat))),
                    )),
                    None,
                ),
            );
        })
        .on_tick(&|canvas: &mut Canvas, context: &mut Context<()>| {
            // println!(
            //     "frame {} @ {} beat {}",
            //     context.frame, context.timestamp, context.beat
            // );
            canvas.remove_object("time");
            canvas.add_object(
                "time".to_string(),
                (
                    Object::RawSVG(Box::new(
                        svg::node::element::Text::new()
                            .set("x", 100)
                            .set("y", 200)
                            .set("font-size", 50)
                            .set("fill", "white")
                            .set("font-family", "monospace")
                            .add(svg::node::Text::new(format!("{}", context.timestamp))),
                    )),
                    None,
                ),
            );
            let float_beat = context.bpm as f32 * context.ms as f32 / 1000.0 / 60.0;
            canvas.add_object(
                "floatbeat".to_string(),
                (
                    Object::RawSVG(Box::new(
                        svg::node::element::Text::new()
                            .set("x", 100)
                            .set("y", 250)
                            .set("font-size", 30)
                            .set("fill", "white")
                            .set("font-family", "monospace")
                            .add(svg::node::Text::new(format!("beat {}", float_beat))),
                    )),
                    None,
                ),
            );
            canvas.add_object(
                "staticinfo".to_string(),
                (
                    Object::RawSVG(Box::new(
                        svg::node::element::Text::new()
                            .set("x", 100)
                            .set("y", 300)
                            .set("font-size", 15)
                            .set("fill", "white")
                            .set("font-family", "monospace")
                            .add(svg::node::Text::new(format!(
                                "bpm {} duration {}",
                                context.bpm,
                                milliseconds_to_timestamp(context.duration_ms()),
                            ))),
                    )),
                    None,
                ),
            )
        })
        .on("start credits", &|canvas: &mut Canvas, _| {
            canvas.add_object(
                "credits text".to_string(),
                (
                    Object::RawSVG(Box::new(svg::node::Text::new("by ewen-lbh"))),
                    None,
                ),
            );
        })
        .on("end credits", &|canvas: &mut Canvas, _| {
            canvas.remove_object("credits text");
        })
        // .command("add", vec!["name", "shape", "color", "at"], &|arguments: Vec<&str>, canvas: &mut Canvas, context: &mut Context| {
        //     let name = arguments[0].to_string();
        //     let shape = canvas.parse_shape(arguments[1]);
        //     let color = canvas.parse_color(arguments[2]);
        //     let at = arguments[3].parse::<usize>().unwrap();
        //     canvas.add_object(name, (shape, Some((color, at))));
        // })
        // .command("remove", vec!["name"], &|arguments: Vec<&str>, canvas: &mut Canvas, context: &mut Context| {
        //     let name = arguments[0].to_string();
        //     canvas.remove_object(name);
        // })
        .render_to(args.arg_file);

    // let shape = canvas.random_shape("test");

    // if let Err(e) = std::fs::write(args.arg_file, shape.render(&canvas)) {
    //     eprintln!("Error: {:?}", e);
    //     std::process::exit(1);
    // }
}

#[derive(Debug, Clone)]
struct Canvas {
    grid_size: (usize, usize),
    cell_size: usize,
    objects_count_range: Range<usize>,
    polygon_vertices_range: Range<usize>,
    canvas_outter_padding: usize,
    line_width: f32,
    empty_shape_stroke_width: f32,
    small_circle_radius: f32,
    dot_radius: f32,
    render_grid: bool,
    colormap: ColorMapping,
    shape: Shape,
    background: Option<Color>,
    _render_cache: Option<String>,
}

impl Canvas {
    fn new() -> Self {
        Self::default_settings()
    }

    fn set_shape(&mut self, shape: Shape) {
        self.shape = shape;
        // println!("invalidating canvas render cache");
        self._render_cache = None;
    }

    fn add_object(&mut self, name: String, object: (Object, Option<Fill>)) {
        self.shape.objects.insert(name, object);
        // println!("invalidating canvas render cache");
        self._render_cache = None;
    }

    fn remove_object(&mut self, name: &str) {
        self.shape.objects.remove(name);
        // println!("invalidating canvas render cache");
        self._render_cache = None;
    }

    fn set_background(&mut self, color: Color) {
        self.background = Some(color);
    }

    fn remove_background(&mut self) {
        self.background = None;
    }

    fn default_settings() -> Self {
        Self {
            grid_size: (3, 3),
            cell_size: 50,
            objects_count_range: 3..7,
            polygon_vertices_range: 2..7,
            canvas_outter_padding: 10,
            line_width: 2.0,
            empty_shape_stroke_width: 0.5,
            small_circle_radius: 5.0,
            dot_radius: 2.0,
            render_grid: false,
            colormap: ColorMapping::default(),
            shape: Shape {
                objects: HashMap::new(),
            },
            _render_cache: None,
            background: None,
        }
    }
    fn random_shape(&self, name: &'static str) -> Shape {
        let mut objects: HashMap<String, (Object, Option<Fill>)> = HashMap::new();
        let number_of_objects = rand::thread_rng().gen_range(self.objects_count_range.clone());
        for i in 0..number_of_objects {
            let object = self.random_object();
            objects.insert(
                format!("{}#{}", name, i),
                (
                    object,
                    if rand::thread_rng().gen_bool(0.5) {
                        Some(self.random_fill())
                    } else {
                        None
                    },
                ),
            );
        }
        Shape { objects }
    }

    fn random_object(&self) -> Object {
        let start = self.random_anchor();
        match rand::thread_rng().gen_range(1..=7) {
            1 => self.random_polygon(),
            2 => Object::BigCircle(self.random_center_anchor()),
            3 => Object::SmallCircle(start),
            4 => Object::Dot(start),
            5 => Object::CurveInward(start, self.random_end_anchor(start)),
            6 => Object::CurveOutward(start, self.random_end_anchor(start)),
            7 => Object::Line(self.random_anchor(), self.random_anchor()),
            _ => unreachable!(),
        }
    }

    fn random_end_anchor(&self, start: Anchor) -> Anchor {
        // End anchors are always a square diagonal from the start anchor (for now)
        // that means taking steps of the form n * (one of (1, 1), (1, -1), (-1, 1), (-1, -1))
        // Except that the end anchor needs to stay in the bounds of the shape.

        // Determine all possible end anchors that are in a square diagonal from the start anchor
        let mut possible_end_anchors = vec![];
        let grid_width = self.grid_size.0 as i32;
        let grid_height = self.grid_size.1 as i32;

        for x in -grid_width..=grid_width {
            for y in -grid_height..=grid_height {
                let end_anchor = Anchor(start.0 + x, start.1 + y);

                if end_anchor == start {
                    continue;
                }

                // Check that the end anchor is in a square diagonal from the start anchor and that the end anchor is in bounds
                if x.abs() == y.abs()
                    && end_anchor.0.abs() < grid_width
                    && end_anchor.1.abs() < grid_height
                    && end_anchor.0 >= 0
                    && end_anchor.1 >= 0
                {
                    possible_end_anchors.push(end_anchor);
                }
            }
        }

        // Pick a random end anchor from the possible end anchors
        possible_end_anchors[rand::thread_rng().gen_range(0..possible_end_anchors.len())]
    }

    fn random_polygon(&self) -> Object {
        let number_of_anchors = rand::thread_rng().gen_range(self.polygon_vertices_range.clone());
        let start = self.random_anchor();
        let mut lines: Vec<Line> = vec![];
        for _ in 0..number_of_anchors {
            let next_anchor = self.random_anchor();
            lines.push(self.random_line(next_anchor));
        }
        Object::Polygon(start, lines)
    }

    fn random_line(&self, end: Anchor) -> Line {
        match rand::thread_rng().gen_range(1..=3) {
            1 => Line::Line(end),
            2 => Line::InwardCurve(end),
            3 => Line::OutwardCurve(end),
            _ => unreachable!(),
        }
    }

    fn random_anchor(&self) -> Anchor {
        if rand::thread_rng().gen_bool(1.0 / (self.grid_size.0 * self.grid_size.1) as f64) {
            // small change of getting center (-1, -1) even when grid size would not permit it (e.g. 4x4)
            Anchor(-1, -1)
        } else {
            Anchor(
                rand::thread_rng().gen_range(0..=self.grid_size.0 - 1) as i32,
                rand::thread_rng().gen_range(0..=self.grid_size.1 - 1) as i32,
            )
        }
    }

    fn random_center_anchor(&self) -> CenterAnchor {
        if rand::thread_rng()
            .gen_bool(1.0 / ((self.grid_size.0 as i32 - 1) * (self.grid_size.1 as i32 - 1)) as f64)
        {
            // small change of getting center (-1, -1) even when grid size would not permit it (e.g. 3x3)
            CenterAnchor(-1, -1)
        } else {
            CenterAnchor(
                rand::thread_rng().gen_range(0..=self.grid_size.0 - 2) as i32,
                rand::thread_rng().gen_range(0..=self.grid_size.1 - 2) as i32,
            )
        }
    }

    fn random_fill(&self) -> Fill {
        Fill::Solid(self.random_color())
        // match rand::thread_rng().gen_range(1..=3) {
        //     1 => Fill::Solid(random_color()),
        //     2 => Fill::Hatched,
        //     3 => Fill::Dotted,
        //     _ => unreachable!(),
        // }
    }

    fn random_color(&self) -> Color {
        match rand::thread_rng().gen_range(1..=12) {
            1 => Color::Black,
            2 => Color::White,
            3 => Color::Red,
            4 => Color::Green,
            5 => Color::Blue,
            6 => Color::Yellow,
            7 => Color::Orange,
            8 => Color::Purple,
            9 => Color::Brown,
            10 => Color::Pink,
            11 => Color::Gray,
            12 => Color::Cyan,
            _ => unreachable!(),
        }
    }

    fn clear(&mut self) {
        self.shape = Shape {
            objects: HashMap::new(),
        };
        self.remove_background()
    }
}

#[derive(Debug, Clone)]
struct Shape {
    objects: HashMap<String, (Object, Option<Fill>)>,
}

#[derive(Debug, Clone)]
enum Object {
    Polygon(Anchor, Vec<Line>),
    Line(Anchor, Anchor),
    CurveOutward(Anchor, Anchor),
    CurveInward(Anchor, Anchor),
    SmallCircle(Anchor),
    Dot(Anchor),
    BigCircle(CenterAnchor),
    RawSVG(Box<dyn svg::Node>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Anchor(i32, i32);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CenterAnchor(i32, i32);

trait Coordinates {
    fn coords(&self, canvas: &Canvas) -> (f32, f32);
    fn center() -> Self;
}

impl Coordinates for Anchor {
    fn coords(&self, canvas: &Canvas) -> (f32, f32) {
        match self {
            Anchor(-1, -1) => (canvas.cell_size as f32 / 2.0, canvas.cell_size as f32 / 2.0),
            Anchor(i, j) => {
                let x = (i * canvas.cell_size as i32) as f32;
                let y = (j * canvas.cell_size as i32) as f32;
                (x, y)
            }
        }
    }

    fn center() -> Self {
        Anchor(-1, -1)
    }
}

impl Coordinates for CenterAnchor {
    fn coords(&self, canvas: &Canvas) -> (f32, f32) {
        match self {
            CenterAnchor(-1, -1) => ((canvas.cell_size / 2) as f32, (canvas.cell_size / 2) as f32),
            CenterAnchor(i, j) => {
                let x = *i as f32 * canvas.cell_size as f32 + canvas.cell_size as f32 / 2.0;
                let y = *j as f32 * canvas.cell_size as f32 + canvas.cell_size as f32 / 2.0;
                (x, y)
            }
        }
    }

    fn center() -> Self {
        CenterAnchor(-1, -1)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Line {
    Line(Anchor),
    InwardCurve(Anchor),
    OutwardCurve(Anchor),
}

#[derive(Debug, Clone, Copy)]
enum Fill {
    Solid(Color),
    Hatched,
    Dotted,
}

#[derive(Debug, Clone, Copy)]
enum Color {
    Black,
    White,
    Red,
    Green,
    Blue,
    Yellow,
    Orange,
    Purple,
    Brown,
    Cyan,
    Pink,
    Gray,
}

impl Default for Color {
    fn default() -> Self {
        Self::Black
    }
}

#[derive(Debug, Deserialize, Clone)]
struct ColorMapping {
    black: String,
    white: String,
    red: String,
    green: String,
    blue: String,
    yellow: String,
    orange: String,
    purple: String,
    brown: String,
    cyan: String,
    pink: String,
    gray: String,
}

impl ColorMapping {
    fn default() -> Self {
        ColorMapping {
            black: "black".to_string(),
            white: "white".to_string(),
            red: "red".to_string(),
            green: "green".to_string(),
            blue: "blue".to_string(),
            yellow: "yellow".to_string(),
            orange: "orange".to_string(),
            purple: "purple".to_string(),
            brown: "brown".to_string(),
            pink: "pink".to_string(),
            gray: "gray".to_string(),
            cyan: "cyan".to_string(),
        }
    }
    fn from_json_file(path: &str) -> ColorMapping {
        let file = File::open(path).unwrap();
        let reader = BufReader::new(file);
        let json: serde_json::Value = serde_json::from_reader(reader).unwrap();
        ColorMapping {
            black: json["black"].as_str().unwrap().to_string(),
            white: json["white"].as_str().unwrap().to_string(),
            red: json["red"].as_str().unwrap().to_string(),
            green: json["green"].as_str().unwrap().to_string(),
            blue: json["blue"].as_str().unwrap().to_string(),
            yellow: json["yellow"].as_str().unwrap().to_string(),
            orange: json["orange"].as_str().unwrap().to_string(),
            purple: json["purple"].as_str().unwrap().to_string(),
            brown: json["brown"].as_str().unwrap().to_string(),
            cyan: json["cyan"].as_str().unwrap().to_string(),
            pink: json["pink"].as_str().unwrap().to_string(),
            gray: json["gray"].as_str().unwrap().to_string(),
        }
    }
}

impl Color {
    fn to_string(self, mapping: &ColorMapping) -> String {
        match self {
            Color::Black => mapping.black.to_string(),
            Color::White => mapping.white.to_string(),
            Color::Red => mapping.red.to_string(),
            Color::Green => mapping.green.to_string(),
            Color::Blue => mapping.blue.to_string(),
            Color::Yellow => mapping.yellow.to_string(),
            Color::Orange => mapping.orange.to_string(),
            Color::Purple => mapping.purple.to_string(),
            Color::Brown => mapping.brown.to_string(),
            Color::Cyan => mapping.cyan.to_string(),
            Color::Pink => mapping.pink.to_string(),
            Color::Gray => mapping.gray.to_string(),
        }
    }
}

impl Canvas {
    fn render(&mut self) -> String {
        if let Some(cached_svg_string) = &self._render_cache {
            return cached_svg_string.clone();
        }
        let canvas_width = self.cell_size * (self.grid_size.0 - 1) + 2 * self.canvas_outter_padding;
        let canvas_height =
            self.cell_size * (self.grid_size.1 - 1) + 2 * self.canvas_outter_padding;
        let default_color = Color::Black.to_string(&self.colormap);
        let background_color = self.background.unwrap_or(Color::default());
        // eprintln!("render: background_color({:?})", background_color);
        let mut svg = svg::Document::new().add(
            svg::node::element::Rectangle::new()
                .set("x", -(self.canvas_outter_padding as i32))
                .set("y", -(self.canvas_outter_padding as i32))
                .set("width", canvas_width)
                .set("height", canvas_height)
                .set("fill", background_color.to_string(&self.colormap)),
        );
        for (id, (object, maybe_fill)) in &self.shape.objects {
            let mut group = svg::node::element::Group::new();
            match object {
                Object::RawSVG(svg) => {
                    // eprintln!("render: raw_svg [{}]", id);
                    group = group.add(svg.clone());
                }
                Object::Polygon(start, lines) => {
                    // eprintln!("render: polygon({:?}, {:?}) [{}]", start, lines, id);
                    let mut path = svg::node::element::path::Data::new();
                    path = path.move_to(start.coords(&self));
                    for line in lines {
                        path = match line {
                            Line::Line(end) | Line::InwardCurve(end) | Line::OutwardCurve(end) => {
                                path.line_to(end.coords(&self))
                            }
                        };
                    }
                    path = path.close();
                    group = group
                        .add(svg::node::element::Path::new().set("d", path))
                        .set(
                            "style",
                            match maybe_fill {
                                // TODO
                                Some(Fill::Solid(color)) => {
                                    format!("fill: {};", color.to_string(&self.colormap))
                                }
                                _ => format!(
                                    "fill: none; stroke: {}; stroke-width: {}px;",
                                    default_color, self.empty_shape_stroke_width
                                ),
                            },
                        );
                }
                Object::Line(start, end) => {
                    // eprintln!("render: line({:?}, {:?}) [{}]", start, end, id);
                    group = group.add(
                        svg::node::element::Line::new()
                            .set("x1", start.coords(&self).0)
                            .set("y1", start.coords(&self).1)
                            .set("x2", end.coords(&self).0)
                            .set("y2", end.coords(&self).1)
                            .set(
                                "style",
                                match maybe_fill {
                                    // TODO
                                    Some(Fill::Solid(color)) => {
                                        format!(
                                            "fill: none; stroke: {}; stroke-width: 2px;",
                                            color.to_string(&self.colormap)
                                        )
                                    }
                                    _ => format!(
                                        "fill: none; stroke: {}; stroke-width: 2px;",
                                        default_color
                                    ),
                                },
                            ),
                    );
                }
                Object::CurveInward(start, end) | Object::CurveOutward(start, end) => {
                    let inward = if matches!(object, Object::CurveInward(_, _)) {
                        // eprintln!("render: curve_inward({:?}, {:?}) [{}]", start, end, id);
                        true
                    } else {
                        // eprintln!("render: curve_outward({:?}, {:?}) [{}]", start, end, id);
                        false
                    };

                    let (start_x, start_y) = start.coords(&self);
                    let (end_x, end_y) = end.coords(&self);

                    let midpoint = ((start_x + end_x) / 2.0, (start_y + end_y) / 2.0);
                    let start_from_midpoint = (start_x - midpoint.0, start_y - midpoint.1);
                    let end_from_midpoint = (end_x - midpoint.0, end_y - midpoint.1);
                    // eprintln!("        midpoint: {:?}", midpoint);
                    // eprintln!(
                    // "        from midpoint: {:?} -> {:?}",
                    // start_from_midpoint, end_from_midpoint
                    // );
                    let control = {
                        let relative = (end_x - start_x, end_y - start_y);
                        // eprintln!("        relative: {:?}", relative);
                        // diagonal line is going like this: \
                        if start_from_midpoint.0 * start_from_midpoint.1 > 0.0
                            && end_from_midpoint.0 * end_from_midpoint.1 > 0.0
                        {
                            // eprintln!("        diagonal \\");
                            if inward {
                                (
                                    midpoint.0 + relative.0.abs() / 2.0,
                                    midpoint.1 - relative.1.abs() / 2.0,
                                )
                            } else {
                                (
                                    midpoint.0 - relative.0.abs() / 2.0,
                                    midpoint.1 + relative.1.abs() / 2.0,
                                )
                            }
                        // diagonal line is going like this: /
                        } else if start_from_midpoint.0 * start_from_midpoint.1 < 0.0
                            && end_from_midpoint.0 * end_from_midpoint.1 < 0.0
                        {
                            // eprintln!("        diagonal /");
                            if inward {
                                (
                                    midpoint.0 - relative.0.abs() / 2.0,
                                    midpoint.1 - relative.1.abs() / 2.0,
                                )
                            } else {
                                (
                                    midpoint.0 + relative.0.abs() / 2.0,
                                    midpoint.1 + relative.1.abs() / 2.0,
                                )
                            }
                        // line is horizontal
                        } else if start_y == end_y {
                            // eprintln!("        horizontal");
                            (
                                midpoint.0,
                                midpoint.1
                                    + (if inward { -1.0 } else { 1.0 }) * relative.0.abs() / 2.0,
                            )
                        // line is vertical
                        } else if start_x == end_x {
                            // eprintln!("        vertical");
                            (
                                midpoint.0
                                    + (if inward { -1.0 } else { 1.0 }) * relative.1.abs() / 2.0,
                                midpoint.1,
                            )
                        } else {
                            unreachable!()
                        }
                    };
                    // eprintln!("        control: {:?}", control);
                    group = group.add(
                        svg::node::element::Path::new()
                            .set(
                                "d",
                                svg::node::element::path::Data::new()
                                    .move_to(start.coords(&self))
                                    .quadratic_curve_to((control, end.coords(&self))),
                            )
                            .set(
                                "style",
                                match maybe_fill {
                                    // TODO
                                    Some(Fill::Solid(color)) => {
                                        format!(
                                            "fill: none; stroke: {}; stroke-width: {}px;",
                                            color.to_string(&self.colormap),
                                            self.line_width
                                        )
                                    }
                                    _ => format!(
                                        "fill: none; stroke: {}; stroke-width: {}px;",
                                        default_color, self.line_width
                                    ),
                                },
                            ),
                    );
                }
                Object::SmallCircle(center) => {
                    // eprintln!("render: small_circle({:?}) [{}]", center, id);
                    group = group.add(
                        svg::node::element::Circle::new()
                            .set("cx", center.coords(&self).0)
                            .set("cy", center.coords(&self).1)
                            .set("r", self.small_circle_radius)
                            .set(
                                "style",
                                match maybe_fill {
                                    // TODO
                                    Some(Fill::Solid(color)) => {
                                        format!("fill: {};", color.to_string(&self.colormap))
                                    }
                                    _ => format!(
                                        "fill: none; stroke: {}; stroke-width: {}px;",
                                        default_color, self.empty_shape_stroke_width
                                    ),
                                },
                            ),
                    );
                }
                Object::Dot(center) => {
                    // eprintln!("render: dot({:?}) [{}]", center, id);
                    group = group.add(
                        svg::node::element::Circle::new()
                            .set("cx", center.coords(&self).0)
                            .set("cy", center.coords(&self).1)
                            .set("r", self.dot_radius)
                            .set(
                                "style",
                                match maybe_fill {
                                    // TODO
                                    Some(Fill::Solid(color)) => {
                                        format!("fill: {};", color.to_string(&self.colormap))
                                    }
                                    _ => format!(
                                        "fill: none; stroke: {}; stroke-width: {}px;",
                                        default_color, self.empty_shape_stroke_width
                                    ),
                                },
                            ),
                    );
                }
                Object::BigCircle(center) => {
                    // eprintln!("render: big_circle({:?}) [{}]", center, id);
                    group = group.add(
                        svg::node::element::Circle::new()
                            .set("cx", center.coords(&self).0)
                            .set("cy", center.coords(&self).1)
                            .set("r", self.cell_size / 2)
                            .set(
                                "style",
                                match maybe_fill {
                                    // TODO
                                    Some(Fill::Solid(color)) => {
                                        format!("fill: {};", color.to_string(&self.colormap))
                                    }
                                    _ => format!(
                                        "fill: none; stroke: {}; stroke-width: 0.5px;",
                                        default_color
                                    ),
                                },
                            ),
                    );
                }
            }
            // eprintln!("        fill: {:?}", &maybe_fill);
            svg = svg.add(group);
        }
        // render a dotted grid
        if self.render_grid {
            for i in 0..self.grid_size.0 as i32 {
                for j in 0..self.grid_size.1 as i32 {
                    let (x, y) = Anchor(i, j).coords(&self);
                    svg = svg.add(
                        svg::node::element::Circle::new()
                            .set("cx", x)
                            .set("cy", y)
                            .set("r", self.line_width / 4.0)
                            .set("fill", "#000"),
                    );

                    // if i < canvas.grid_size.0 as i32 - 1 && j < canvas.grid_size.1 as i32 - 1 {
                    //     let (x, y) = CenterAnchor(i, j).coords(&canvas);
                    //     svg = svg.add(
                    //         svg::node::element::Circle::new()
                    //             .set("cx", x)
                    //             .set("cy", y)
                    //             .set("r", canvas.line_width / 4.0)
                    //             .set("fill", "#fff"),
                    //     );
                    // }
                }
            }
        }
        self._render_cache = Some(
            svg.set(
                "viewBox",
                format!(
                    "{0} {0} {1} {2}",
                    -(self.canvas_outter_padding as i32),
                    canvas_width,
                    canvas_height
                ),
            )
            .set("width", canvas_width)
            .set("height", canvas_height)
            .to_string(),
        );

        self._render_cache.as_ref().unwrap().to_string()
    }
}
