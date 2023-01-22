use chrono::NaiveDateTime;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use rand::Rng;
use serde::Deserialize;
use serde_json;
use std::cmp::min;
use std::collections::{HashMap, VecDeque};
use std::fmt::Formatter;
use std::fs::{create_dir, remove_dir_all, File};
use std::io::{BufReader, Write};
use std::ops::{Add, Mul, Range};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};
use std::time;

const PROGRESS_BARS_STYLE: &'static str =
    "{spinner:.cyan} {percent:03.bold.cyan}% {msg:<30} [{bar:100.bold.blue/dim.blue}] {eta:.cyan}";

pub type RenderFunction<C> = dyn Fn(&mut Canvas, &mut Context<C>);
pub type CommandAction<C> = dyn Fn(String, &mut Canvas, &mut Context<C>);

/// Arguments: canvas, context, previous rendered beat, previous rendered frame
pub type HookCondition<C> = dyn Fn(&Canvas, &Context<C>, usize, usize) -> bool;

pub type LaterRenderFunction<C> = dyn Fn(&mut Canvas, &Context<C>);

/// Arguments: canvas, context, previous rendered beat
pub type LaterHookCondition<C> = dyn Fn(&Canvas, &Context<C>, usize) -> bool;

#[derive(Debug)]
pub struct Video<C> {
    pub fps: usize,
    pub initial_canvas: Canvas,
    pub hooks: Vec<Hook<C>>,
    pub commands: Vec<Box<Command<C>>>,
    pub frames: Vec<Canvas>,
    pub frames_output_directory: &'static str,
    pub audio_paths: AudioSyncPaths,
    pub bpm: usize,
    pub markers: HashMap<usize, String>,
    pub stems: HashMap<String, Stem>,
    pub resolution: usize,
}

pub struct Hook<C> {
    pub when: Box<HookCondition<C>>,
    pub render_function: Box<RenderFunction<C>>,
}

pub struct LaterHook<C> {
    pub when: Box<LaterHookCondition<C>>,
    pub render_function: Box<LaterRenderFunction<C>>,
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
pub struct Stem {
    pub amplitude_db: Vec<f32>,
    /// max amplitude of this stem
    pub amplitude_max: f32,
    /// in milliseconds
    pub duration_ms: usize,
}

#[derive(Debug)]
pub struct StemAtInstant {
    pub amplitude: f32,
    pub amplitude_max: f32,
    pub duration: usize,
}
impl StemAtInstant {
    pub fn amplitude_relative(&self) -> f32 {
        self.amplitude / self.amplitude_max
    }
}

pub struct Command<C> {
    pub name: String,
    pub action: Box<CommandAction<C>>,
}

impl<C> std::fmt::Debug for Command<C> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Command")
            .field("name", &self.name)
            .field("action", &"Box<CommandAction>")
            .finish()
    }
}

pub struct Context<'a, AdditionalContext = ()> {
    pub frame: usize,
    pub beat: usize,
    pub beat_fractional: f32,
    pub timestamp: String,
    pub ms: usize,
    pub bpm: usize,
    pub stems: &'a HashMap<String, Stem>,
    pub markers: &'a HashMap<usize, String>, // milliseconds -> marker text
    pub later_hooks: Vec<LaterHook<AdditionalContext>>,
    pub extra: AdditionalContext,
}

const DURATION_OVERRIDE: Option<usize> = Some(4 * 1000);

pub trait GetOrDefault {
    type Item;
    fn get_or(&self, index: usize, default: Self::Item) -> Self::Item;
}

impl<T: Copy> GetOrDefault for Vec<T> {
    type Item = T;
    fn get_or(&self, index: usize, default: T) -> T {
        *self.get(index).unwrap_or(&default)
    }
}

impl<'a, C> Context<'a, C> {
    pub fn stem(&self, name: &str) -> StemAtInstant {
        if !self.stems.contains_key(name) {
            panic!("No stem named {:?} found.", name);
        }
        StemAtInstant {
            amplitude: self.stems[name].amplitude_db.get_or(self.frame, 0.0),
            amplitude_max: self.stems[name].amplitude_max,
            duration: self.stems[name].duration_ms,
        }
    }

    pub fn marker(&self) -> String {
        self.markers
            .get(&self.ms)
            .unwrap_or(&"".to_string())
            .to_string()
    }

    pub fn duration_ms(&self) -> usize {
        self.stems
            .values()
            .map(|stem| stem.duration_ms)
            .map(|duration| {
                if let Some(duration_override) = DURATION_OVERRIDE {
                    duration_override
                } else {
                    duration
                }
            })
            .max()
            .unwrap()
    }

    pub fn later_frames(&mut self, delay: usize, render_function: &'static LaterRenderFunction<C>) {
        let current_frame = self.frame;

        self.later_hooks.insert(
            0,
            LaterHook {
                when: Box::new(move |_, context, _previous_beat| {
                    context.frame >= current_frame + delay
                }),
                render_function: Box::new(render_function),
            },
        );
    }

    pub fn later_ms(&mut self, delay: usize, render_function: &'static LaterRenderFunction<C>) {
        let current_ms = self.ms;

        self.later_hooks.insert(
            0,
            LaterHook {
                when: Box::new(move |_, context, _previous_beat| context.ms >= current_ms + delay),
                render_function: Box::new(render_function),
            },
        );
    }

    pub fn later_beats(&mut self, delay: f32, render_function: &'static LaterRenderFunction<C>) {
        let current_beat = self.beat;

        self.later_hooks.insert(
            0,
            LaterHook {
                when: Box::new(move |_, context, _previous_beat| {
                    context.beat_fractional >= current_beat as f32 + delay
                }),
                render_function: Box::new(render_function),
            },
        );
    }
}

#[derive(Debug, Clone, Default)]
pub struct AudioSyncPaths {
    pub stems: String,
    pub landmarks: String,
    pub complete: String,
    pub bpm: String,
}

pub enum MusicalDurationUnit {
    Beats,
    Halfs,
    Thirds,
    Quarters,
    Eighths,
    Sixteenths,
}

struct SpinState {
    pub spinner: ProgressBar,
    pub finished: Arc<Mutex<bool>>,
    pub thread: JoinHandle<()>,
}

impl SpinState {
    fn start(message: &str) -> Self {
        let spinner = ProgressBar::new(0).with_style(
            ProgressStyle::with_template(&("{spinner:.cyan} ".to_owned() + message)).unwrap(),
        );
        spinner.tick();

        let thread_spinner = spinner.clone();
        let finished = Arc::new(Mutex::new(false));
        let thread_finished = Arc::clone(&finished);
        let spinner_thread = thread::spawn(move || {
            while !*thread_finished.lock().unwrap() {
                thread_spinner.tick();
                thread::sleep(time::Duration::from_millis(100));
            }
            thread_spinner.finish_and_clear();
        });

        Self {
            spinner: spinner.clone(),
            finished: finished,
            thread: spinner_thread,
        }
    }
    fn end(self, message: &str) {
        *self.finished.lock().unwrap() = true;
        self.thread.join().unwrap();
        println!("{}", message);
    }
}

impl<AdditionalContext: Default> Video<AdditionalContext> {
    pub fn new() -> Self {
        Self {
            fps: 30,
            initial_canvas: Canvas::new(),
            hooks: vec![],
            commands: vec![],
            frames: vec![],
            audio_paths: AudioSyncPaths::default(),
            frames_output_directory: "audiosync/frames/",
            bpm: 0,
            resolution: 1000,
            markers: HashMap::new(),
            stems: HashMap::new(),
        }
    }

    pub fn build_video(&self, render_to: &str) -> Result<(), String> {
        let result = std::process::Command::new("ffmpeg")
            .args(["-hide_banner", "-loglevel", "error"])
            .arg("-framerate")
            .arg(self.fps.to_string())
            .arg("-pattern_type")
            .arg("glob")
            .arg("-i")
            .arg(format!("{}/*.png", self.frames_output_directory))
            .arg("-i")
            .arg(self.audio_paths.complete.clone())
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
            .arg(render_to)
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

    fn build_frame(
        svg_string: String,
        frame_no: usize,
        total_frames: usize,
        frames_output_directory: &str,
        aspect_ratio: f32,
        resolution: usize,
    ) -> Result<(), String> {
        Canvas::save_as_png(
            &format!(
                "{}/{:0width$}.png",
                frames_output_directory,
                frame_no,
                width = total_frames.to_string().len()
            ),
            aspect_ratio,
            resolution,
            svg_string,
        )
    }

    fn add_to_frame_build_pool(
        &self,
        frames_to_write: &mut Vec<(String, usize)>,
        canvas: &mut Canvas,
        frame_no: usize,
    ) {
        let rendered = canvas.render();
        println!("main thrd: {} rendered", frame_no);
        frames_to_write.push((rendered, frame_no));
    }

    pub fn set_fps(self, fps: usize) -> Self {
        Self { fps, ..self }
    }

    pub fn set_initial_canvas(self, canvas: Canvas) -> Self {
        Self {
            initial_canvas: canvas,
            ..self
        }
    }

    pub fn sync_to(self, audio: &AudioSyncPaths) -> Self {
        let progress_bar_tree = MultiProgress::new();
        // Read BPM from file
        let bpm = std::fs::read_to_string(audio.bpm.clone())
            .map_err(|e| format!("Failed to read BPM file: {}", e))
            .and_then(|bpm| {
                bpm.trim()
                    .parse::<usize>()
                    .map(|parsed| parsed)
                    .map_err(|e| format!("Failed to parse BPM file: {}", e))
            })
            .unwrap();

        // Read landmakrs from JSON file
        let markers = std::fs::read_to_string(audio.landmarks.clone())
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
        let mut threads = vec![];
        let (tx, rx) = mpsc::channel();

        let stem_file_entries: Vec<_> = std::fs::read_dir(audio.stems.clone())
            .map_err(|e| format!("Failed to read stems directory: {}", e))
            .unwrap()
            .filter(|e| match e {
                Ok(e) => e.path().extension().unwrap_or_default() == "wav",
                Err(_) => false,
            })
            .collect();

        let main_progress_bar = progress_bar_tree.add(
            ProgressBar::new(stem_file_entries.len() as u64)
                .with_style(
                    ProgressStyle::with_template(
                        &(PROGRESS_BARS_STYLE.to_owned()
                            + " ({pos:.bold} stems loaded out of {len})"),
                    )
                    .unwrap()
                    .progress_chars("== "),
                )
                .with_message("Loading stems"),
        );

        main_progress_bar.tick();

        for (i, entry) in stem_file_entries.into_iter().enumerate() {
            let progress_bar = progress_bar_tree.add(
                ProgressBar::new(0).with_style(
                    ProgressStyle::with_template(&("  ".to_owned() + PROGRESS_BARS_STYLE))
                        .unwrap()
                        .progress_chars("== "),
                ),
            );
            let main_progress_bar = main_progress_bar.clone();
            let tx = tx.clone();
            threads.push(thread::spawn(move || {
                let path = entry.unwrap().path();
                let stem_name = path.file_stem().unwrap().to_str().unwrap();
                progress_bar.set_message(format!("Loading \"{}\"", stem_name));
                let mut reader = hound::WavReader::open(path.clone())
                    .map_err(|e| format!("Failed to read stem file: {}", e))
                    .unwrap();
                let spec = reader.spec();
                let sample_to_frame = |sample: usize| {
                    (sample as f64 / spec.channels as f64 / spec.sample_rate as f64
                        * self.fps as f64) as usize
                };
                let mut amplitude_db: Vec<f32> = vec![];
                let mut current_amplitude_sum: f32 = 0.0;
                let mut current_amplitude_buffer_size: usize = 0;
                let mut last_frame = 0;
                progress_bar.set_length(reader.samples::<i16>().len() as u64);
                for (i, sample) in reader.samples::<i16>().enumerate() {
                    let sample = sample.unwrap();
                    if sample_to_frame(i) > last_frame {
                        amplitude_db
                            .push(current_amplitude_sum / current_amplitude_buffer_size as f32);
                        current_amplitude_sum = 0.0;
                        current_amplitude_buffer_size = 0;
                        last_frame = sample_to_frame(i);
                    } else {
                        current_amplitude_sum += sample.abs() as f32;
                        current_amplitude_buffer_size += 1;
                    }
                    // main_progress_bar.tick();
                    progress_bar.inc(1);
                }
                amplitude_db.push(current_amplitude_sum / current_amplitude_buffer_size as f32);
                progress_bar.finish_with_message(format!(" Loaded \"{}\"", stem_name));
                main_progress_bar.inc(1);

                tx.send((
                    progress_bar,
                    stem_name.to_string(),
                    Stem {
                        amplitude_max: *amplitude_db
                            .iter()
                            .max_by(|a, b| a.partial_cmp(b).unwrap())
                            .unwrap(),
                        amplitude_db,
                        duration_ms: (reader.duration() as f64 / spec.sample_rate as f64 * 1000.0)
                            as usize,
                    },
                ))
                .unwrap();
                drop(tx);
            }));
        }
        drop(tx);

        for (progress_bar, stem_name, stem) in rx {
            progress_bar.finish_and_clear();
            stems.insert(stem_name, stem);
        }

        for thread in threads {
            thread.join().unwrap();
        }

        main_progress_bar.finish_and_clear();

        println!("Loaded {} stems", stems.len());

        Self {
            audio_paths: audio.clone(),
            markers,
            bpm,
            stems,
            ..self
        }
    }

    pub fn init(self, render_function: &'static RenderFunction<AdditionalContext>) -> Self {
        let hook = Hook {
            when: Box::new(move |_, context, _, _| context.frame == 0),
            render_function: Box::new(render_function),
        };
        let mut hooks = self.hooks;
        hooks.push(hook);
        Self { hooks, ..self }
    }

    pub fn on(
        self,
        marker_text: &'static str,
        render_function: &'static RenderFunction<AdditionalContext>,
    ) -> Self {
        let hook = Hook {
            when: Box::new(move |_, context, _, _| context.marker() == marker_text),
            render_function: Box::new(render_function),
        };
        let mut hooks = self.hooks;
        hooks.push(hook);
        Self { hooks, ..self }
    }

    pub fn each_beat(self, render_function: &'static RenderFunction<AdditionalContext>) -> Self {
        let hook = Hook {
            when: Box::new(
                move |_, context, previous_rendered_beat, previous_rendered_frame| {
                    previous_rendered_frame != context.frame
                        && (context.ms == 0 || previous_rendered_beat != context.beat)
                },
            ),
            render_function: Box::new(render_function),
        };
        let mut hooks = self.hooks;
        hooks.push(hook);
        Self { hooks, ..self }
    }

    pub fn every(
        self,
        amount: f32,
        unit: MusicalDurationUnit,
        render_function: &'static RenderFunction<AdditionalContext>,
    ) -> Self {
        let beats = match unit {
            MusicalDurationUnit::Beats => amount,
            MusicalDurationUnit::Halfs => amount / 2.0,
            MusicalDurationUnit::Quarters => amount / 4.0,
            MusicalDurationUnit::Eighths => amount / 8.0,
            MusicalDurationUnit::Sixteenths => amount / 16.0,
            MusicalDurationUnit::Thirds => amount / 3.0,
        };

        let hook = Hook {
            when: Box::new(move |_, context, _, _| context.beat_fractional % beats < 0.01),
            render_function: Box::new(render_function),
        };
        let mut hooks = self.hooks;
        hooks.push(hook);
        Self { hooks, ..self }
    }

    pub fn each_frame(self, render_function: &'static RenderFunction<AdditionalContext>) -> Self {
        let hook = Hook {
            when: Box::new(move |_, context, _, previous_rendered_frame| {
                context.frame != previous_rendered_frame
            }),
            render_function: Box::new(render_function),
        };
        let mut hooks = self.hooks;
        hooks.push(hook);
        Self { hooks, ..self }
    }

    pub fn on_stem(
        self,
        stem_name: &'static str,
        threshold: f32,
        above_amplitude: &'static RenderFunction<AdditionalContext>,
        below_amplitude: &'static RenderFunction<AdditionalContext>,
    ) -> Self {
        let mut hooks = self.hooks;
        hooks.push(Hook {
            when: Box::new(move |_, context, _, _| {
                context.stem(stem_name).amplitude_relative() > threshold
            }),
            render_function: Box::new(above_amplitude),
        });
        hooks.push(Hook {
            when: Box::new(move |_, context, _, _| {
                context.stem(stem_name).amplitude_relative() <= threshold
            }),
            render_function: Box::new(below_amplitude),
        });
        Self { hooks, ..self }
    }

    pub fn at_frame(
        self,
        frame: usize,
        render_function: &'static RenderFunction<AdditionalContext>,
    ) -> Self {
        let hook = Hook {
            when: Box::new(move |_, context, _, _| context.frame == frame),
            render_function: Box::new(render_function),
        };
        let mut hooks = self.hooks;
        hooks.push(hook);
        Self { hooks, ..self }
    }

    pub fn at_timestamp(
        self,
        timestamp: &'static str,
        render_function: &'static RenderFunction<AdditionalContext>,
    ) -> Self {
        let hook = Hook {
            when: Box::new(move |_, context, _, previous_rendered_frame| {
                if previous_rendered_frame == context.frame {
                    return false;
                }
                let (precision, criteria_time): (&str, NaiveDateTime) =
                    if let Ok(criteria_time_parsed) =
                        NaiveDateTime::parse_from_str(timestamp, "%H:%M:%S%.3f")
                    {
                        ("milliseconds", criteria_time_parsed)
                    } else if let Ok(criteria_time_parsed) =
                        NaiveDateTime::parse_from_str(timestamp, "%M:%S%.3f")
                    {
                        ("milliseconds", criteria_time_parsed)
                    } else if let Ok(criteria_time_parsed) =
                        NaiveDateTime::parse_from_str(timestamp, "%S%.3f")
                    {
                        ("milliseconds", criteria_time_parsed)
                    } else if let Ok(criteria_time_parsed) =
                        NaiveDateTime::parse_from_str(timestamp, "%S")
                    {
                        ("seconds", criteria_time_parsed)
                    } else if let Ok(criteria_time_parsed) =
                        NaiveDateTime::parse_from_str(timestamp, "%M:%S")
                    {
                        ("seconds", criteria_time_parsed)
                    } else if let Ok(criteria_time_parsed) =
                        NaiveDateTime::parse_from_str(timestamp, "%H:%M:%S")
                    {
                        ("seconds", criteria_time_parsed)
                    } else {
                        panic!("Unhandled timestamp format: {}", timestamp);
                    };
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

    pub fn command(
        self,
        command_name: &'static str,
        action: &'static CommandAction<AdditionalContext>,
    ) -> Self {
        let mut commands = self.commands;
        commands.push(Box::new(Command {
            name: command_name.to_string(),
            action: Box::new(action),
        }));
        Self { commands, ..self }
    }

    pub fn total_frames(&self) -> usize {
        self.fps * self.duration_ms() / 1000
    }

    pub fn duration_ms(&self) -> usize {
        self.stems
            .values()
            .map(|stem| stem.duration_ms)
            .map(|duration| {
                if let Some(duration_override) = DURATION_OVERRIDE {
                    duration_override
                } else {
                    duration
                }
            })
            .max()
            .unwrap()
    }

    pub fn render_to(&self, output_file: String, workers_count: usize) {
        let mut context = Context {
            frame: 0,
            beat: 0,
            beat_fractional: 0.0,
            timestamp: "00:00:00.000".to_string(),
            ms: 0,
            bpm: self.bpm,
            stems: &self.stems,
            markers: &self.markers,
            extra: AdditionalContext::default(),
            later_hooks: vec![],
        };

        let mut canvas = self.initial_canvas.clone();
        let mut previous_rendered_beat = 0;
        let mut previous_rendered_frame = 0;

        let mut frame_writer_threads = vec![];
        let mut frames_to_write: Vec<(String, usize)> = vec![];

        remove_dir_all(self.frames_output_directory.clone()).unwrap();
        create_dir(self.frames_output_directory.clone()).unwrap();

        let progress_bar = indicatif::ProgressBar::new(self.total_frames() as u64).with_style(
            indicatif::ProgressStyle::with_template(
                &(PROGRESS_BARS_STYLE.to_owned() + " ({pos:.bold} frames out of {len})"),
            )
            .unwrap()
            .progress_chars("== "),
        );
        let total_frames = self.total_frames();
        let aspect_ratio = canvas.grid_size.0 as f32 / canvas.grid_size.1 as f32;
        let resolution = self.resolution;
        let frames_output_directory = self.frames_output_directory.clone();
        progress_bar.set_message("Rendering frames to SVG");

        for _ in 0..self.duration_ms() {
            context.ms += 1 as usize;
            context.timestamp = format!("{}", milliseconds_to_timestamp(context.ms));
            context.beat_fractional = (context.bpm * context.ms) as f32 / (1000.0 * 60.0);
            context.beat = context.beat_fractional as usize;
            context.frame = ((self.fps * context.ms) as f64 / 1000.0) as usize;

            if context.marker() != "" {
                progress_bar.println(format!(
                    "{}: marker {}",
                    context.timestamp,
                    context.marker()
                ));
            }

            if context.marker().starts_with(":") {
                let marker_text = context.marker();
                let commandline = marker_text.trim_start_matches(":").to_string();

                for command in &self.commands {
                    if commandline.starts_with(&command.name) {
                        let args = commandline
                            .trim_start_matches(&command.name)
                            .trim()
                            .to_string();
                        (command.action)(args, &mut canvas, &mut context);
                    }
                }
            }

            for hook in &self.hooks {
                if (hook.when)(
                    &canvas,
                    &context,
                    previous_rendered_beat,
                    previous_rendered_frame,
                ) {
                    (hook.render_function)(&mut canvas, &mut context);
                }
            }

            let mut later_hooks_to_delete: Vec<usize> = vec![];

            for (i, hook) in context.later_hooks.iter().enumerate() {
                if (hook.when)(&canvas, &context, previous_rendered_beat) {
                    (hook.render_function)(&mut canvas, &context);
                    later_hooks_to_delete.push(i);
                }
            }

            for i in later_hooks_to_delete {
                if i < context.later_hooks.len() {
                    context.later_hooks.remove(i);
                }
            }

            if context.frame != previous_rendered_frame {
                let rendered = canvas.render();
                frames_to_write.push((rendered, context.frame));
                previous_rendered_beat = context.beat;
                previous_rendered_frame = context.frame;
                progress_bar.inc(1);
            }
        }

        progress_bar.println("Rendered frames to SVG");
        progress_bar.set_message("Rendering SVG frames to PNG");
        progress_bar.set_position(0);

        let chunk_size = (frames_to_write.len() as f32 / workers_count as f32).ceil() as usize;
        let frames_to_write = Arc::new(frames_to_write);
        for i in 0..workers_count {
            let frames_to_write = Arc::clone(&frames_to_write);
            let progress_bar = progress_bar.clone();
            frame_writer_threads.push(
                thread::Builder::new()
                    .name(format!("worker-{}", i))
                    .spawn(move || {
                        for (frame_svg, frame_no) in &frames_to_write
                            [i * chunk_size..min((i + 1) * chunk_size, frames_to_write.len())]
                        {
                            Video::<AdditionalContext>::build_frame(
                                frame_svg.clone(),
                                *frame_no,
                                total_frames,
                                frames_output_directory,
                                aspect_ratio,
                                resolution,
                            );
                            progress_bar.inc(1);
                        }
                    })
                    .unwrap(),
            );
        }

        for handle in frame_writer_threads {
            handle.join().unwrap();
        }

        progress_bar.finish_and_clear();
        println!("Rendered SVG frames to PNG");

        let spinner = SpinState::start("Building video…");
        if let Err(e) = self.build_video(&output_file) {
            panic!("Failed to build video: {}", e);
        }
        spinner.end(&format!("Built video to {}", output_file));
    }
}

pub fn milliseconds_to_timestamp(ms: usize) -> String {
    format!(
        "{}",
        NaiveDateTime::from_timestamp_millis(ms as i64)
            .unwrap()
            .format("%H:%M:%S%.3f")
    )
}

#[derive(Debug, Clone)]
pub struct Canvas {
    pub grid_size: (usize, usize),
    pub cell_size: usize,
    pub objects_count_range: Range<usize>,
    pub polygon_vertices_range: Range<usize>,
    pub canvas_outter_padding: usize,
    pub line_width: f32,
    pub empty_shape_stroke_width: f32,
    pub small_circle_radius: f32,
    pub dot_radius: f32,
    pub render_grid: bool,
    pub colormap: ColorMapping,
    pub shape: Shape,
    pub background: Option<Color>,
    pub _render_cache: Option<String>,
}

impl Canvas {
    pub fn new() -> Self {
        Self::default_settings()
    }

    pub fn set_shape(&mut self, shape: Shape) {
        self.shape = shape;
        // println!("invalidating canvas render cache");
        self._render_cache = None;
    }

    pub fn add_object(&mut self, name: &str, object: (Object, Option<Fill>)) {
        self.shape.objects.insert(name.to_string(), object);
        // println!("invalidating canvas render cache");
        self._render_cache = None;
    }

    pub fn remove_object(&mut self, name: &str) {
        self.shape.objects.remove(name);
        // println!("invalidating canvas render cache");
        self._render_cache = None;
    }

    pub fn set_background(&mut self, color: Color) {
        self.background = Some(color);
    }

    pub fn remove_background(&mut self) {
        self.background = None;
    }

    pub fn default_settings() -> Self {
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
    pub fn random_shape(&self, name: &'static str) -> Shape {
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

    pub fn random_object(&self) -> Object {
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

    pub fn random_end_anchor(&self, start: Anchor) -> Anchor {
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

    pub fn random_polygon(&self) -> Object {
        let number_of_anchors = rand::thread_rng().gen_range(self.polygon_vertices_range.clone());
        let start = self.random_anchor();
        let mut lines: Vec<Line> = vec![];
        for _ in 0..number_of_anchors {
            let next_anchor = self.random_anchor();
            lines.push(self.random_line(next_anchor));
        }
        Object::Polygon(start, lines)
    }

    pub fn random_line(&self, end: Anchor) -> Line {
        match rand::thread_rng().gen_range(1..=3) {
            1 => Line::Line(end),
            2 => Line::InwardCurve(end),
            3 => Line::OutwardCurve(end),
            _ => unreachable!(),
        }
    }

    pub fn random_anchor(&self) -> Anchor {
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

    pub fn random_center_anchor(&self) -> CenterAnchor {
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

    pub fn random_fill(&self) -> Fill {
        Fill::Solid(self.random_color())
        // match rand::thread_rng().gen_range(1..=3) {
        //     1 => Fill::Solid(random_color()),
        //     2 => Fill::Hatched,
        //     3 => Fill::Dotted,
        //     _ => unreachable!(),
        // }
    }

    pub fn random_color(&self) -> Color {
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

    pub fn clear(&mut self) {
        self.shape = Shape {
            objects: HashMap::new(),
        };
        self.remove_background()
    }

    pub fn save_as_png(
        at: &str,
        aspect_ratio: f32,
        resolution: usize,
        rendered: String,
    ) -> Result<(), String> {
        let (height, width) = if aspect_ratio > 1.0 {
            // landscape: resolution is width
            (resolution, (resolution as f32 * aspect_ratio) as usize)
        } else {
            // portrait: resolution is height
            ((resolution as f32 / aspect_ratio) as usize, resolution)
        };
        let mut spawned = std::process::Command::new("convert")
            .arg("-")
            .args(&["-size", &format!("{}x{}", width, height)])
            .arg(at)
            .stdin(std::process::Stdio::piped())
            .spawn()
            .unwrap();

        let stdin = spawned.stdin.as_mut().unwrap();
        stdin.write_all(rendered.as_bytes()).unwrap();
        drop(stdin);

        match spawned.wait_with_output() {
            Ok(_) => Ok(()),
            Err(e) => Err(format!("Failed to execute convert: {}", e)),
        }
    }
}

pub trait Parsable {
    fn parse(input: String) -> Self;
}

impl Parsable for Object {
    fn parse(input: String) -> Self {
        let mut input: VecDeque<&str> = input.trim().split_whitespace().collect();
        if input.contains(&"line") {
            input.pop_front();
            if input.pop_front() != Some("from") {
                panic!("Expected 'from' after 'line'");
            };
            let start = Anchor::parse(input.pop_front().unwrap_or_default().to_string());
            if input.pop_front() != Some("to") {
                panic!("Expected 'to' after 'line'");
            };
            let end = Anchor::parse(input.pop_front().unwrap_or_default().to_string());
            Object::Line(start, end)
        } else if input.contains(&"outward") {
            input.pop_front();
            if input.pop_front() != Some("curve") {
                panic!("Expected 'curve' after 'outward'");
            };
            if input.pop_front() != Some("from") {
                panic!("Expected 'from' after 'outward curve'");
            };
            let start = Anchor::parse(input.pop_front().unwrap_or_default().to_string());
            if input.pop_front() != Some("to") {
                panic!("Expected 'to' after 'outward curve from'");
            };
            let end = Anchor::parse(input.pop_front().unwrap_or_default().to_string());
            Object::CurveOutward(start, end)
        } else if input.contains(&"inward") {
            input.pop_front();
            if input.pop_front() != Some("curve") {
                panic!("Expected 'curve' after 'inward'");
            };
            if input.pop_front() != Some("from") {
                panic!("Expected 'from' after 'inward curve'");
            };
            let start = Anchor::parse(input.pop_front().unwrap_or_default().to_string());
            if input.pop_front() != Some("to") {
                panic!("Expected 'to' after 'inward curve from'");
            };
            let end = Anchor::parse(input.pop_front().unwrap_or_default().to_string());
            Object::CurveInward(start, end)
        } else if input.contains(&"small") || input.contains(&"big") {
            let circle_type = input.pop_front();
            if input.pop_front() != Some("circle") {
                panic!("Expected 'circle' after 'small' or 'big'");
            };
            if input.pop_front() != Some("at") {
                panic!("Expected 'at' after 'small circle' or 'big circle'");
            };
            match circle_type {
                Some("small") => Object::SmallCircle(Anchor::parse(
                    input.pop_front().unwrap_or_default().to_string(),
                )),
                Some("big") => Object::BigCircle(CenterAnchor::parse(
                    input.pop_front().unwrap_or_default().to_string(),
                )),
                _ => unreachable!(),
            }
        } else if input.contains(&"dot") {
            input.pop_front();
            if input.pop_front() != Some("at") {
                panic!("Expected 'at' after 'dot'");
            };
            Object::Dot(Anchor::parse(
                input.pop_front().unwrap_or_default().to_string(),
            ))
        } else if input.contains(&"polygon") {
            input.pop_front();
            if input.pop_front() != Some("of") {
                panic!("Expected 'of' after 'polygon'");
            };
            let start = Anchor::parse(input.pop_front().unwrap_or_default().to_string());
            let mut lines = Vec::new();
            while input.len() > 0 {
                lines.push(Line::Line(Anchor::parse(
                    input.pop_front().unwrap_or_default().to_string(),
                )));
            }
            Object::Polygon(start, lines)
        } else {
            panic!(
                "Invalid object '{}'",
                input.pop_front().unwrap_or_default().to_string()
            );
        }
    }
}

impl Parsable for Anchor {
    fn parse(input: String) -> Self {
        let delimiters: &[_] = &['(', ')', '[', ']'];
        let mut input: VecDeque<&str> = input
            .trim()
            .trim_matches(delimiters)
            .split_whitespace()
            .collect();
        // TODO
        // if input.len() == 1 {
        //     match input[0] {
        //         "left" =>
        //     }
        // }
        let x = input
            .pop_front()
            .unwrap_or_default()
            .parse::<i32>()
            .unwrap_or_default();
        let y = input
            .pop_front()
            .unwrap_or_default()
            .parse::<i32>()
            .unwrap_or_default();
        Anchor(x, y)
    }
}

impl Parsable for CenterAnchor {
    fn parse(input: String) -> Self {
        let delimiters: &[_] = &['(', ')', '[', ']'];
        let mut input: VecDeque<&str> = input
            .trim()
            .trim_matches(delimiters)
            .split_whitespace()
            .collect();
        // TODO
        // if input.len() == 1 {
        //     match input[0] {
        //         "left" =>
        //     }
        // }
        let x = input
            .pop_front()
            .unwrap_or_default()
            .parse::<i32>()
            .unwrap_or_default();
        let y = input
            .pop_front()
            .unwrap_or_default()
            .parse::<i32>()
            .unwrap_or_default();
        CenterAnchor(x, y)
    }
}

impl Parsable for Color {
    fn parse(input: String) -> Self {
        match input.trim() {
            "black" => Color::Black,
            "white" => Color::White,
            "red" => Color::Red,
            "green" => Color::Green,
            "blue" => Color::Blue,
            "yellow" => Color::Yellow,
            "orange" => Color::Orange,
            "purple" => Color::Purple,
            "brown" => Color::Brown,
            "cyan" => Color::Cyan,
            "pink" => Color::Pink,
            "gray" => Color::Gray,
            _ => panic!("Invalid color '{}'", input),
        }
    }
}

impl Parsable for Option<Fill> {
    fn parse(input: String) -> Option<Fill> {
        match input.trim() {
            "empty" => None,
            _ => Some(Fill::Solid(Color::parse(input))),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Shape {
    pub objects: HashMap<String, (Object, Option<Fill>)>,
}

#[derive(Debug, Clone)]
pub enum Object {
    Polygon(Anchor, Vec<Line>),
    Line(Anchor, Anchor),
    CurveOutward(Anchor, Anchor),
    CurveInward(Anchor, Anchor),
    SmallCircle(Anchor),
    Dot(Anchor),
    BigCircle(CenterAnchor),
    RawSVG(Box<dyn svg::Node>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Anchor(i32, i32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CenterAnchor(i32, i32);

pub trait Coordinates {
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
pub enum Line {
    Line(Anchor),
    InwardCurve(Anchor),
    OutwardCurve(Anchor),
}

#[derive(Debug, Clone, Copy)]
pub enum Fill {
    Solid(Color),
    Hatched,
    Dotted,
}

#[derive(Debug, Clone, Copy)]
pub enum Color {
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
pub struct ColorMapping {
    pub black: String,
    pub white: String,
    pub red: String,
    pub green: String,
    pub blue: String,
    pub yellow: String,
    pub orange: String,
    pub purple: String,
    pub brown: String,
    pub cyan: String,
    pub pink: String,
    pub gray: String,
}

impl ColorMapping {
    pub fn default() -> Self {
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
    pub fn from_json_file(path: &str) -> ColorMapping {
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
    pub fn to_string(self, mapping: &ColorMapping) -> String {
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
    pub fn render(&mut self) -> String {
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
        for (_id, (object, maybe_fill)) in &self.shape.objects {
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
