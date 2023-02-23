mod audio;
pub use audio::*;
mod layer;
pub use layer::*;
mod canvas;
pub use canvas::*;
use chrono::NaiveDateTime;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use itertools::Itertools;
use midly::{MetaMessage, MidiMessage, TrackEventKind};
use serde_json;
use std::cmp::min;
use std::collections::HashMap;
use std::fmt::Formatter;
use std::fs::{create_dir, create_dir_all, remove_dir_all};
use std::path::Path;
use std::sync::{mpsc, Arc, Mutex};
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

const DURATION_OVERRIDE: Option<usize> = Some(2 * 1000);

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
            velocity_max: self.stems[name]
                .notes
                .get(&self.ms)
                .iter()
                .map(|notes| notes.iter().map(|note| note.velocity).max().unwrap_or(0))
                .max()
                .unwrap_or(0),
            duration: self.stems[name].duration_ms,
            notes: self.stems[name]
                .notes
                .get(&self.ms)
                .cloned()
                .unwrap_or(vec![]),
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
            audio_paths: AudioSyncPaths::default(),
            frames_output_directory: "frames/",
            bpm: 0,
            resolution: 1000,
            markers: HashMap::new(),
            stems: HashMap::new(),
        }
    }

    pub fn build_video(&self, render_to: &str) -> Result<(), String> {
        let result = std::process::Command::new("ffmpeg")
            .args(["-hide_banner", "-loglevel", "error"])
            .args(["-framerate", &self.fps.to_string()])
            .args(["-pattern_type", "glob"])
            .args(["-i", &format!("{}/*.png", self.frames_output_directory)])
            .args(["-i", &self.audio_paths.complete.clone()])
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

    pub fn sync_to(self, audio: &AudioSyncPaths, stem_audio_to_midi: AudioStemToMIDITrack) -> Self {
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
                let stem_name: String = path.file_stem().unwrap().to_string_lossy().into();
                let stem_cache_path = Stem::cbor_path(path.clone(), stem_name.clone());
                progress_bar.set_message(format!("Loading \"{}\"", stem_name));

                // Check if a cached CBOR of the stem file exists
                if Path::new(&stem_cache_path).exists() {
                    let stem = Stem::load_from_cbor(&stem_cache_path);
                    progress_bar.set_message("Loaded {} from cache".to_owned());
                    tx.send((progress_bar, stem_name, stem)).unwrap();
                    main_progress_bar.inc(1);
                    return;
                }

                let mut reader = hound::WavReader::open(path.clone())
                    .map_err(|e| format!("Failed to read stem file {:?}: {}", path, e))
                    .unwrap();
                let spec = reader.spec();
                let sample_to_frame = |sample: usize| {
                    (sample as f64 / spec.channels as f64 / spec.sample_rate as f64
                        * self.fps as f64) as usize
                };
                let mut amplitude_db: Vec<f32> = vec![];
                let mut current_amplitude_sum: f32 = 0.0;
                let mut current_amplitude_buffer_size: usize = 0;
                let mut latest_loaded_frame = 0;
                progress_bar.set_length(reader.samples::<i16>().len() as u64);
                for (i, sample) in reader.samples::<i16>().enumerate() {
                    let sample = sample.unwrap();
                    if sample_to_frame(i) > latest_loaded_frame {
                        amplitude_db
                            .push(current_amplitude_sum / current_amplitude_buffer_size as f32);
                        current_amplitude_sum = 0.0;
                        current_amplitude_buffer_size = 0;
                        latest_loaded_frame = sample_to_frame(i);
                    } else {
                        current_amplitude_sum += sample.abs() as f32;
                        current_amplitude_buffer_size += 1;
                    }
                    // main_progress_bar.tick();
                    progress_bar.inc(1);
                }
                amplitude_db.push(current_amplitude_sum / current_amplitude_buffer_size as f32);
                progress_bar.finish_with_message(format!(" Loaded \"{}\"", stem_name));

                let stem = Stem {
                    amplitude_max: *amplitude_db
                        .iter()
                        .max_by(|a, b| a.partial_cmp(b).unwrap())
                        .unwrap(),
                    amplitude_db,
                    duration_ms: (reader.duration() as f64 / spec.sample_rate as f64 * 1000.0)
                        as usize,
                    notes: HashMap::new(),
                    path: path.clone(),
                    name: stem_name.clone(),
                };

                main_progress_bar.inc(1);

                tx.send((progress_bar, stem_name, stem)).unwrap();
                drop(tx);
            }));
        }
        drop(tx);

        for (progress_bar, stem_name, stem) in rx {
            progress_bar.finish_and_clear();
            stems.insert(stem_name.to_string(), stem);
        }

        for thread in threads {
            thread.join().unwrap();
        }

        // Read MIDI file
        println!("Loading MIDI…");
        let midi_bytes = std::fs::read(audio.midi.clone())
            .map_err(|e| format!("While loading MIDI file {}: {:?}", audio.midi.clone(), e))
            .unwrap();
        let midi = midly::Smf::parse(&midi_bytes).unwrap();

        let mut timeline = HashMap::<u32, HashMap<String, midly::TrackEvent>>::new();
        let mut now_ms = 0.0;
        let mut now_tempo = 500_000.0;
        let mut ticks_per_beat = match midi.header.timing {
            midly::Timing::Metrical(ticks_per_beat) => ticks_per_beat.as_int(),
            midly::Timing::Timecode(fps, subframe) => (1.0 / fps.as_f32() / subframe as f32) as u16,
        };

        // Get track names
        let mut track_no = 0;
        let mut track_names = HashMap::<usize, String>::new();
        for track in midi.tracks.iter() {
            track_no += 1;
            let mut track_name = String::new();
            for event in track {
                match event.kind {
                    TrackEventKind::Meta(MetaMessage::TrackName(name_bytes)) => {
                        track_name = String::from_utf8(name_bytes.to_vec()).unwrap_or_default();
                    }
                    _ => {}
                }
            }
            let track_name = if !track_name.is_empty() {
                track_name
            } else {
                format!("Track #{}", track_no)
            };
            if !stems.contains_key(&track_name) {
                println!(
                    "MIDI track {} has no corresponding audio stem, skipping",
                    track_name
                );
            }
            track_names.insert(track_no, track_name);
        }

        // Convert ticks to absolute
        let mut track_no = 0;
        for track in midi.tracks.iter() {
            track_no += 1;
            let mut absolute_tick = 0;
            for event in track {
                absolute_tick += event.delta.as_int();
                timeline
                    .entry(absolute_tick)
                    .or_default()
                    .insert(track_names[&track_no].clone(), *event);
            }
        }

        // Convert ticks to ms
        let mut absolute_tick_to_ms = HashMap::<u32, f32>::new();
        let mut last_tick = 0;
        for (tick, tracks) in timeline.iter().sorted_by_key(|(tick, _)| *tick) {
            for (_, event) in tracks {
                match event.kind {
                    TrackEventKind::Meta(MetaMessage::Tempo(tempo)) => {
                        now_tempo = tempo.as_int() as f32;
                    }
                    _ => {}
                }
            }
            let delta = tick - last_tick;
            last_tick = *tick;
            let delta_µs = now_tempo * delta as f32 / ticks_per_beat as f32;
            now_ms += delta_µs / 1000.0;
            absolute_tick_to_ms.insert(*tick, now_ms);
        }

        // Add notes
        for (tick, tracks) in timeline.iter().sorted_by_key(|(tick, _)| *tick) {
            for (track_name, event) in tracks {
                match event.kind {
                    TrackEventKind::Midi {
                        channel: _,
                        message,
                    } => match message {
                        MidiMessage::NoteOn { key, vel } | MidiMessage::NoteOff { key, vel } => {
                            let note = Note {
                                tick: *tick,
                                pitch: key.as_int(),
                                velocity: if matches!(message, MidiMessage::NoteOff { .. }) {
                                    0
                                } else {
                                    vel.as_int()
                                },
                            };
                            let stem_name: &str = stem_audio_to_midi
                                .get(&track_name.as_str())
                                .unwrap_or(&track_name.as_str());
                            if stems.contains_key(stem_name) {
                                stems
                                    .get_mut(stem_name)
                                    .unwrap()
                                    .notes
                                    .entry(absolute_tick_to_ms[tick] as usize)
                                    .or_default()
                                    .push(note);
                            }
                        }
                        _ => {}
                    },
                    _ => {}
                }
            }
        }

        std::fs::write("stems.json", serde_json::to_vec(&stems).unwrap());

        for (name, stem) in &stems {
            // Write loaded stem to a CBOR cache file
            Stem::save_to_cbor(&stem, &Stem::cbor_path(stem.path.clone(), name.to_string()));
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

    pub fn render_to(&self, output_file: String, workers_count: usize) -> &Self {
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

    pub fn render_layers_in(&self, output_directory: String, workers_count: usize) -> &Self {
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
            );
        }
        self
    }

    pub fn render_composition(
        &self,
        output_file: String,
        composition: Vec<&str>,
        render_background: bool,
        workers_count: usize,
    ) -> &Self {
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
                );
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
        self
    }
}
