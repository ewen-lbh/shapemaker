mod audio;
pub use audio::*;
mod sync;
use sync::SyncData;
pub use sync::Syncable;
mod layer;
pub use layer::*;
mod canvas;
pub use canvas::*;
mod midi;
use anyhow::Result;
use chrono::NaiveDateTime;
use indicatif::{ProgressBar, ProgressStyle};
pub use midi::MidiSynchronizer;
use std::cmp::min;
use std::fmt::{self, Formatter};
use std::fs::{self, create_dir, create_dir_all, remove_dir_all};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
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
    pub syncdata: SyncData,
    pub audiofile: PathBuf,
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
    pub syncdata: &'a SyncData,
    pub audiofile: PathBuf,
    pub later_hooks: Vec<LaterHook<AdditionalContext>>,
    pub extra: AdditionalContext,
}

const DURATION_OVERRIDE: Option<usize> = Some(2 * 60 * 1000);

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
        let stems = &self.syncdata.stems;
        if !stems.contains_key(name) {
            panic!("No stem named {:?} found.", name);
        }
        StemAtInstant {
            amplitude: stems[name].amplitude_db.get_or(self.ms, 0.0),
            amplitude_max: stems[name].amplitude_max,
            velocity_max: stems[name]
                .notes
                .get(&self.ms)
                .iter()
                .map(|notes| notes.iter().map(|note| note.velocity).max().unwrap_or(0))
                .max()
                .unwrap_or(0),
            duration: stems[name].duration_ms,
            notes: stems[name].notes.get(&self.ms).cloned().unwrap_or(vec![]),
        }
    }

    pub fn dump_stems(&self, to: PathBuf) -> () {
        std::fs::create_dir_all(&to);
        for (name, stem) in self.syncdata.stems.iter() {
            fs::write(to.join(name), format!("{:?}", stem));
        }
    }

    pub fn marker(&self) -> String {
        self.syncdata
            .markers
            .get(&self.ms)
            .unwrap_or(&"".to_string())
            .to_string()
    }

    pub fn duration_ms(&self) -> usize {
        self.syncdata
            .stems
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
            initial_canvas: Canvas::new(vec!["root"]),
            hooks: vec![],
            commands: vec![],
            frames: vec![],
            frames_output_directory: "frames/",
            resolution: 1000,
            syncdata: SyncData::default(),
            audiofile: PathBuf::new(),
        }
    }

    pub fn set_audio(self, final_audio: PathBuf) -> Self {
        Self {
            audiofile: final_audio,
            ..self
        }
    }

    pub fn sync_audio_with(self, sync_data_path: &str) -> Self {
        if sync_data_path.ends_with(".mid") || sync_data_path.ends_with(".midi") {
            let loader = MidiSynchronizer::new(sync_data_path);
            let syncdata = loader.load();
            println!("Loaded MIDI sync data: {}", syncdata);
            return Self { syncdata, ..self };
        }

        panic!("Unsupported sync data format");
    }

    pub fn build_video(&self, render_to: &str) -> Result<(), String> {
        let result = std::process::Command::new("ffmpeg")
            .args(["-hide_banner", "-loglevel", "error"])
            .args(["-framerate", &self.fps.to_string()])
            .args(["-pattern_type", "glob"])
            .args(["-i", &format!("{}/*.png", self.frames_output_directory)])
            .args(["-i", self.audiofile.to_str().unwrap()])
            .args(["-t", &format!("{}", self.duration_ms() as f32 / 1000.0)])
            .args(["-vcodec", "png"])
            .arg("-y")
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

    pub fn set_fps(self, fps: usize) -> Self {
        Self { fps, ..self }
    }

    pub fn set_initial_canvas(self, canvas: Canvas) -> Self {
        Self {
            initial_canvas: canvas,
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

    /// threshold is a value between 0 and 1: current amplitude / max amplitude of stem
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

    /// Triggers when a note starts on one of the stems in the comma-separated list of stem names `stems`.
    pub fn on_note(
        self,
        stems: &'static str,
        render_function: &'static RenderFunction<AdditionalContext>,
    ) -> Self {
        let mut hooks = self.hooks;
        hooks.push(Hook {
            when: Box::new(move |_, ctx, _, _| {
                for stem_name in stems.split(",") {
                    let stem = ctx.stem(stem_name);
                    if stem.notes.iter().any(|note| note.is_on()) {
                        return true;
                    }
                }
                return false;
            }),
            render_function: Box::new(render_function),
        });
        Self { hooks, ..self }
    }

    /// Triggers when a note stops on one of the stems in the comma-separated list of stem names `stems`.
    pub fn on_note_end(
        self,
        stems: &'static str,
        render_function: &'static RenderFunction<AdditionalContext>,
    ) -> Self {
        let mut hooks = self.hooks;
        hooks.push(Hook {
            when: Box::new(move |_, ctx, _, _| {
                for stem_name in stems.split(",") {
                    let stem = ctx.stem(stem_name);
                    if stem.notes.iter().any(|note| note.is_off()) {
                        return true;
                    }
                }
                return false;
            }),
            render_function: Box::new(render_function),
        });
        Self { hooks, ..self }
    }

    // Adds an object using object_creation on note start and removes it on note end
    pub fn with_note(
        self,
        stems: &'static str,
        cutoff_amplitude: f32,
        layer_name: &'static str,
        object_name: &'static str,
        create_object: &'static dyn Fn(
            &Canvas,
            &mut Context<AdditionalContext>,
        ) -> (Object, Option<Fill>),
    ) -> Self {
        let mut hooks = self.hooks;
        hooks.push(Hook {
            when: Box::new(move |_, ctx, _, _| {
                stems
                    .split(",")
                    .any(|stem_name| ctx.stem(stem_name).notes.iter().any(|note| note.is_on()))
            }),
            render_function: Box::new(move |canvas, ctx| {
                let (object, fill) = create_object(canvas, ctx);
                canvas.add_object(layer_name, object_name, object, fill);
            }),
        });
        hooks.push(Hook {
            when: Box::new(move |_, ctx, _, _| {
                stems.split(",").any(|stem_name| {
                    ctx.stem(stem_name).amplitude_relative() < cutoff_amplitude
                        || ctx.stem(stem_name).notes.iter().any(|note| note.is_off())
                })
            }),
            render_function: Box::new(move |canvas, _| canvas.remove_object(object_name)),
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
        if let Some(duration_override) = DURATION_OVERRIDE {
            return duration_override;
        }

        self.syncdata
            .stems
            .values()
            .map(|stem| stem.duration_ms)
            .max()
            .unwrap()
    }

    pub fn render_to(&self, output_file: String, workers_count: usize) -> Result<&Self> {
        self.render_composition(
            output_file,
            self.initial_canvas
                .layers
                .iter()
                .map(|l| l.name.as_str())
                .collect(),
            true,
            workers_count,
        )
    }

    pub fn render_layers_in(
        &self,
        output_directory: String,
        workers_count: usize,
    ) -> Result<&Self> {
        for composition in self
            .initial_canvas
            .layers
            .iter()
            .map(|l| vec![l.name.as_str()])
        {
            self.render_composition(
                format!("{}/{}.mov", output_directory, composition.join("+")),
                composition,
                false,
                workers_count,
            )?;
        }
        Ok(self)
    }

    pub fn render_composition(
        &self,
        output_file: String,
        composition: Vec<&str>,
        render_background: bool,
        workers_count: usize,
    ) -> Result<&Self> {
        let mut context = Context {
            frame: 0,
            beat: 0,
            beat_fractional: 0.0,
            timestamp: "00:00:00.000".to_string(),
            ms: 0,
            bpm: self.syncdata.bpm,
            syncdata: &self.syncdata,
            extra: AdditionalContext::default(),
            later_hooks: vec![],
            audiofile: self.audiofile.clone(),
        };

        let mut canvas = self.initial_canvas.clone();
        let mut previous_rendered_beat = 0;
        let mut previous_rendered_frame = 0;

        let mut frame_writer_threads = vec![];
        let mut frames_to_write: Vec<(String, usize)> = vec![];

        remove_dir_all(self.frames_output_directory.clone());
        create_dir(self.frames_output_directory.clone()).unwrap();
        create_dir_all(Path::new(&output_file).parent().unwrap()).unwrap();

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
                let rendered = canvas.render(&composition, render_background);
                std::fs::write(
                    format!("{}/{}.svg", frames_output_directory, context.frame),
                    &rendered,
                )?;
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
        Ok(self)
    }
}
