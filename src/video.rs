use std::{
    cmp::min,
    collections::HashMap,
    fmt::Formatter,
    fs::{create_dir, create_dir_all, remove_dir_all},
    panic,
    path::{Path, PathBuf},
    sync::Arc,
};

use std::thread;

use anyhow::Result;
use chrono::{DateTime, NaiveDateTime};
use indicatif::{ProgressBar, ProgressIterator};

use crate::{
    preview,
    sync::SyncData,
    ui::{self, setup_progress_bar, Log as _},
    Canvas, ColoredObject, Context, LayerAnimationUpdateFunction, MidiSynchronizer,
    MusicalDurationUnit, Syncable,
};

pub type BeatNumber = usize;
pub type FrameNumber = usize;
pub type Millisecond = usize;

pub type RenderFunction<C> = dyn Fn(&mut Canvas, &mut Context<C>) -> anyhow::Result<()>;
pub type CommandAction<C> = dyn Fn(String, &mut Canvas, &mut Context<C>) -> anyhow::Result<()>;

/// Arguments: canvas, context, previous rendered beat, previous rendered frame
pub type HookCondition<C> = dyn Fn(&Canvas, &Context<C>, BeatNumber, FrameNumber) -> bool;

/// Arguments: canvas, context, current milliseconds timestamp
pub type LaterRenderFunction = dyn Fn(&mut Canvas, Millisecond) -> anyhow::Result<()>;

/// Arguments: canvas, context, previous rendered beat
pub type LaterHookCondition<C> = dyn Fn(&Canvas, &Context<C>, BeatNumber) -> bool;

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
    pub duration_override: Option<usize>,
    pub start_rendering_at: usize,
    pub progress_bar: indicatif::ProgressBar,
}
pub struct Hook<C> {
    pub when: Box<HookCondition<C>>,
    pub render_function: Box<RenderFunction<C>>,
}

pub struct LaterHook<C> {
    pub when: Box<LaterHookCondition<C>>,
    pub render_function: Box<LaterRenderFunction>,
    /// Whether the hook should be run only once
    pub once: bool,
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

impl<AdditionalContext: Default> Default for Video<AdditionalContext> {
    fn default() -> Self {
        Self::new(Canvas::new(vec!["root"]))
    }
}

impl<AdditionalContext: Default> Video<AdditionalContext> {
    pub fn new(canvas: Canvas) -> Self {
        Self {
            fps: 30,
            initial_canvas: canvas,
            hooks: vec![],
            commands: vec![],
            frames: vec![],
            frames_output_directory: "frames/",
            resolution: 1000,
            syncdata: SyncData::default(),
            audiofile: PathBuf::new(),
            duration_override: None,
            start_rendering_at: 0,
            progress_bar: setup_progress_bar(0, ""),
        }
    }

    pub fn sync_audio_with(self, sync_data_path: &str) -> Self {
        if sync_data_path.ends_with(".mid") || sync_data_path.ends_with(".midi") {
            let loader = MidiSynchronizer::new(sync_data_path);
            let syncdata = loader.load(Some(&self.progress_bar));
            self.progress_bar.finish();
            return Self { syncdata, ..self };
        }

        panic!("Unsupported sync data format");
    }

    pub fn build_video(&self, render_to: &str) -> Result<()> {
        let mut command = std::process::Command::new("ffmpeg");

        command
            .args(["-hide_banner", "-loglevel", "error"])
            .args(["-framerate", &self.fps.to_string()])
            .args(["-pattern_type", "glob"]) // not available on Windows
            .args([
                "-i",
                &format!(
                    "{}/*.png",
                    self.frames_output_directory,
                    // self.total_frames().to_string().len()
                ),
            ])
            .args([
                "-ss",
                &format!("{}", self.start_rendering_at as f32 / 1000.0),
            ])
            .args(["-i", self.audiofile.to_str().unwrap()])
            .args(["-t", &format!("{}", self.duration_ms() as f32 / 1000.0)])
            .args(["-c:v", "libx264"])
            .args(["-pix_fmt", "yuv420p"])
            .arg("-y")
            .arg(render_to);

        println!("Running command: {:?}", command);

        match command.output() {
            Err(e) => Err(anyhow::format_err!("Failed to execute ffmpeg: {}", e).into()),
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
        Canvas::save_as(
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

    pub fn with_hook(self, hook: Hook<AdditionalContext>) -> Self {
        let mut hooks = self.hooks;
        hooks.push(hook);
        Self { hooks, ..self }
    }

    pub fn init(self, render_function: &'static RenderFunction<AdditionalContext>) -> Self {
        self.with_hook(Hook {
            when: Box::new(move |_, context, _, _| context.frame == 0),
            render_function: Box::new(render_function),
        })
    }

    pub fn on(
        self,
        marker_text: &'static str,
        render_function: &'static RenderFunction<AdditionalContext>,
    ) -> Self {
        self.with_hook(Hook {
            when: Box::new(move |_, context, _, _| context.marker() == marker_text),
            render_function: Box::new(render_function),
        })
    }

    pub fn each_beat(self, render_function: &'static RenderFunction<AdditionalContext>) -> Self {
        self.with_hook(Hook {
            when: Box::new(
                move |_, context, previous_rendered_beat, previous_rendered_frame| {
                    previous_rendered_frame != context.frame
                        && (context.ms == 0 || previous_rendered_beat != context.beat)
                },
            ),
            render_function: Box::new(render_function),
        })
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

        self.with_hook(Hook {
            when: Box::new(move |_, context, _, _| context.beat_fractional % beats < 0.01),
            render_function: Box::new(render_function),
        })
    }

    pub fn each_frame(self, render_function: &'static RenderFunction<AdditionalContext>) -> Self {
        let hook = Hook {
            when: Box::new(move |_, context, _, previous_rendered_frame| {
                context.frame != previous_rendered_frame
            }),
            render_function: Box::new(render_function),
        };
        self.with_hook(hook)
    }

    pub fn each_n_frame(
        self,
        n: usize,
        render_function: &'static RenderFunction<AdditionalContext>,
    ) -> Self {
        self.with_hook(Hook {
            when: Box::new(move |_, context, _, previous_rendered_frame| {
                context.frame != previous_rendered_frame && context.frame % n == 0
            }),
            render_function: Box::new(render_function),
        })
    }

    /// threshold is a value between 0 and 1: current amplitude / max amplitude of stem
    pub fn on_stem(
        self,
        stem_name: &'static str,
        threshold: f32,
        above_amplitude: &'static RenderFunction<AdditionalContext>,
        below_amplitude: &'static RenderFunction<AdditionalContext>,
    ) -> Self {
        self.with_hook(Hook {
            when: Box::new(move |_, context, _, _| {
                context.stem(stem_name).amplitude_relative() > threshold
            }),
            render_function: Box::new(above_amplitude),
        })
        .with_hook(Hook {
            when: Box::new(move |_, context, _, _| {
                context.stem(stem_name).amplitude_relative() <= threshold
            }),
            render_function: Box::new(below_amplitude),
        })
    }

    /// Triggers when a note starts on one of the stems in the comma-separated list of stem names `stems`.
    pub fn on_note(
        self,
        stems: &'static str,
        render_function: &'static RenderFunction<AdditionalContext>,
    ) -> Self {
        self.with_hook(Hook {
            when: Box::new(move |_, ctx, _, _| {
                stems
                    .split(',')
                    .map(|n| ctx.stem(n.trim()))
                    .any(|stem| stem.notes.iter().any(|note| note.is_on()))
            }),
            render_function: Box::new(render_function),
        })
    }

    /// Triggers when a note stops on one of the stems in the comma-separated list of stem names `stems`.
    pub fn on_note_end(
        self,
        stems: &'static str,
        render_function: &'static RenderFunction<AdditionalContext>,
    ) -> Self {
        self.with_hook(Hook {
            when: Box::new(move |_, ctx, _, _| {
                stems
                    .split(',')
                    .map(|n| ctx.stem(n.trim()))
                    .any(|stem| stem.notes.iter().any(|note| note.is_off()))
            }),
            render_function: Box::new(render_function),
        })
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
        ) -> Result<ColoredObject>,
    ) -> Self {
        self.with_hook(Hook {
            when: Box::new(move |_, ctx, _, _| {
                stems
                    .split(',')
                    .any(|stem_name| ctx.stem(stem_name).notes.iter().any(|note| note.is_on()))
            }),
            render_function: Box::new(move |canvas, ctx| {
                let object = create_object(canvas, ctx)?;
                canvas.layer(&layer_name).set_object(object_name, object);
                Ok(())
            }),
        })
        .with_hook(Hook {
            when: Box::new(move |_, ctx, _, _| {
                stems.split(',').any(|stem_name| {
                    ctx.stem(stem_name).amplitude_relative() < cutoff_amplitude
                        || ctx.stem(stem_name).notes.iter().any(|note| note.is_off())
                })
            }),
            render_function: Box::new(move |canvas, _| {
                canvas.remove_object(object_name);
                Ok(())
            }),
        })
    }

    pub fn at_frame(
        self,
        frame: usize,
        render_function: &'static RenderFunction<AdditionalContext>,
    ) -> Self {
        self.with_hook(Hook {
            when: Box::new(move |_, context, _, _| context.frame == frame),
            render_function: Box::new(render_function),
        })
    }

    pub fn when_remaining(
        self,
        seconds: usize,
        render_function: &'static RenderFunction<AdditionalContext>,
    ) -> Self {
        self.with_hook(Hook {
            when: Box::new(move |_, ctx, _, _| {
                ctx.ms >= ctx.duration_ms().max(seconds * 1000) - seconds * 1000
            }),
            render_function: Box::new(render_function),
        })
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
        self.with_hook(hook)
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

    pub fn bind_amplitude(
        self,
        layer: &'static str,
        stem: &'static str,
        update: &'static LayerAnimationUpdateFunction,
    ) -> Self {
        self.with_hook(Hook {
            when: Box::new(move |_, _, _, _| true),
            render_function: Box::new(move |canvas, context| {
                let amplitude = context.stem(stem).amplitude_relative();
                update(amplitude, canvas.layer(layer), context.ms)?;
                canvas.layer(layer).flush();
                Ok(())
            }),
        })
    }

    pub fn total_frames(&self) -> usize {
        self.fps * (self.duration_ms() + self.start_rendering_at) / 1000
    }

    pub fn duration_ms(&self) -> usize {
        if let Some(duration_override) = self.duration_override {
            return duration_override;
        }

        self.syncdata
            .stems
            .values()
            .map(|stem| stem.duration_ms)
            .max()
            .unwrap()
    }

    pub fn preview_on(&self, port: usize) -> Result<()> {
        let mut rendered_frames: HashMap<usize, String> = HashMap::new();
        let progress_bar = self.setup_progress_bar();

        for (frame, _, ms) in self.render_frames(&progress_bar, true)? {
            rendered_frames.insert(ms, frame);
        }

        progress_bar.finish_and_clear();

        preview::output_preview(
            &self.initial_canvas,
            &rendered_frames,
            port,
            PathBuf::from(".").join("preview.html"),
            self.audiofile.clone(),
        )?;

        preview::start_preview_server(port, rendered_frames)
    }

    pub fn render_to(
        &self,
        output_file: String,
        workers_count: usize,
        preview_only: bool,
    ) -> Result<()> {
        self.render(output_file, true, workers_count, preview_only)
    }

    pub fn render_layers_in(&self, output_directory: String, workers_count: usize) -> Result<()> {
        for composition in self
            .initial_canvas
            .layers
            .iter()
            .map(|l| vec![l.name.as_str()])
        {
            self.render(
                format!("{}/{}.mov", output_directory, composition.join("+")),
                false,
                workers_count,
                false,
            )?;
        }
        Ok(())
    }

    // Returns a triple of (SVG content, frame number, millisecond at frame)
    pub fn render_frames(
        &self,
        progress_bar: &ProgressBar,
        render_background: bool,
    ) -> Result<Vec<(String, usize, usize)>> {
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
            duration_override: self.duration_override,
        };

        let mut canvas = self.initial_canvas.clone();

        let mut previous_rendered_beat = 0;
        let mut previous_rendered_frame = 0;
        let mut frames_to_write: Vec<(String, usize, usize)> = vec![];

        let render_ms_range = 0..self.duration_ms() + self.start_rendering_at;

        self.progress_bar.set_length(render_ms_range.len() as u64);

        for _ in render_ms_range
            .into_iter()
            .progress_with(self.progress_bar.clone())
        {
            context.ms += 1_usize;
            context.timestamp = milliseconds_to_timestamp(context.ms).to_string();
            context.beat_fractional = (context.bpm * context.ms) as f32 / (1000.0 * 60.0);
            context.beat = context.beat_fractional as usize;
            context.frame = ((self.fps * context.ms) as f64 / 1000.0) as usize;

            progress_bar.set_message(context.timestamp.clone());

            if context.marker() != "" {
                progress_bar.println(format!(
                    "{}: marker {}",
                    context.timestamp,
                    context.marker()
                ));
            }

            if context.marker().starts_with(':') {
                let marker_text = context.marker();
                let commandline = marker_text.trim_start_matches(':').to_string();

                for command in &self.commands {
                    if commandline.starts_with(&command.name) {
                        let args = commandline
                            .trim_start_matches(&command.name)
                            .trim()
                            .to_string();
                        (command.action)(args, &mut canvas, &mut context)?;
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
                    (hook.render_function)(&mut canvas, &mut context)?;
                }
            }

            let mut later_hooks_to_delete: Vec<usize> = vec![];

            for (i, hook) in context.later_hooks.iter().enumerate() {
                if (hook.when)(&canvas, &context, previous_rendered_beat) {
                    (hook.render_function)(&mut canvas, context.ms)?;
                    if hook.once {
                        later_hooks_to_delete.push(i);
                    }
                } else if !hook.once {
                    later_hooks_to_delete.push(i);
                }
            }

            for i in later_hooks_to_delete {
                if i < context.later_hooks.len() {
                    context.later_hooks.remove(i);
                }
            }

            if context.frame != previous_rendered_frame {
                let rendered = canvas.render(render_background)?;

                previous_rendered_beat = context.beat;
                previous_rendered_frame = context.frame;

                frames_to_write.push((rendered, context.frame, context.ms))
            }
        }

        Ok(frames_to_write)
    }

    pub fn setup_progress_bar(&self) -> ProgressBar {
        ui::setup_progress_bar(self.total_frames() as u64, "Rendering")
    }

    pub fn render(
        &self,
        output_file: String,
        render_background: bool,
        workers_count: usize,
        _preview_only: bool,
    ) -> Result<()> {
        let mut frame_writer_threads = vec![];
        let mut frames_to_write: Vec<(String, usize, usize)> = vec![];

        remove_dir_all(self.frames_output_directory)?;
        create_dir(self.frames_output_directory)?;
        create_dir_all(Path::new(&output_file).parent().unwrap())?;

        let total_frames = self.total_frames();
        let aspect_ratio =
            self.initial_canvas.grid_size.0 as f32 / self.initial_canvas.grid_size.1 as f32;
        let resolution = self.resolution;

        self.progress_bar.set_position(0);
        self.progress_bar.set_prefix("Rendering");
        self.progress_bar.set_message("");

        for (frame, no, ms) in self.render_frames(&self.progress_bar, render_background)? {
            frames_to_write.push((frame, no, ms));
        }

        self.progress_bar.log(
            "Finished",
            &format!("rendering {} frames to SVG", frames_to_write.len()),
        );

        frames_to_write.retain(|(_, _, ms)| *ms >= self.start_rendering_at);

        self.progress_bar.set_prefix("Converting");
        self.progress_bar
            .set_message("converting SVG frames to PNG");
        self.progress_bar.set_position(0);
        self.progress_bar.set_length(frames_to_write.len() as u64);

        for (frame, no, _) in &frames_to_write {
            std::fs::write(
                format!("{}/{}.svg", self.frames_output_directory, no),
                &frame,
            )?;
        }

        let chunk_size = (frames_to_write.len() as f32 / workers_count as f32).ceil() as usize;
        let frames_to_write = Arc::new(frames_to_write);
        let frames_output_directory = self.frames_output_directory;
        for i in 0..workers_count {
            let frames_to_write = Arc::clone(&frames_to_write);
            let progress_bar = self.progress_bar.clone();
            frame_writer_threads.push(
                thread::Builder::new()
                    .name(format!("worker-{}", i))
                    .spawn(move || {
                        for (frame_svg, frame_no, _) in &frames_to_write
                            [i * chunk_size..min((i + 1) * chunk_size, frames_to_write.len())]
                        {
                            Video::<AdditionalContext>::build_frame(
                                frame_svg.clone(),
                                *frame_no,
                                total_frames,
                                frames_output_directory,
                                aspect_ratio,
                                resolution,
                            )
                            .unwrap();
                            progress_bar.inc(1);
                        }
                    })
                    .unwrap(),
            );
        }

        for handle in frame_writer_threads {
            handle.join().unwrap();
        }

        self.progress_bar.log("Rendered", "SVG frames to PNG");
        self.progress_bar.finish_and_clear();

        let spinner = ui::Spinner::start("Building video…");
        let result = self.build_video(&output_file);
        spinner.end(&format!("Built video to {}", output_file));

        result
    }
}

pub fn milliseconds_to_timestamp(ms: usize) -> String {
    format!(
        "{}",
        DateTime::from_timestamp_millis(ms as i64)
            .unwrap()
            .format("%H:%M:%S%.3f")
    )
}
