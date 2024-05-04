pub mod animation;
pub mod audio;
pub mod canvas;
pub mod cli;
pub mod color;
pub mod examples;
pub mod fill;
pub mod filter;
pub mod layer;
pub mod midi;
pub mod objects;
pub mod point;
pub mod preview;
pub mod region;
pub mod sync;
pub mod video;
pub mod web;
pub use animation::*;
pub use audio::*;
pub use canvas::*;
pub use color::*;
pub use fill::*;
pub use filter::*;
pub use layer::*;
pub use midi::MidiSynchronizer;
pub use objects::*;
pub use point::*;
pub use region::*;
pub use sync::Syncable;
pub use video::*;
pub use web::log;

use indicatif::{ProgressBar, ProgressStyle};
use nanoid::nanoid;
use std::fs::{self};
use std::ops::{Add, Div, Range, Sub};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time;
use sync::SyncData;

const PROGRESS_BARS_STYLE: &str =
    "{spinner:.cyan} {percent:03.bold.cyan}% {msg:<30} [{bar:100.bold.blue/dim.blue}] {eta:.cyan}";

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
    pub duration_override: Option<usize>,
}

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

    pub fn dump_stems(&self, to: PathBuf) {
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
        match self.duration_override {
            Some(duration) => duration,
            None => self
                .syncdata
                .stems
                .values()
                .map(|stem| stem.duration_ms)
                .max()
                .unwrap(),
        }
    }

    pub fn later_frames(&mut self, delay: usize, render_function: &'static LaterRenderFunction) {
        let current_frame = self.frame;

        self.later_hooks.insert(
            0,
            LaterHook {
                once: true,
                when: Box::new(move |_, context, _previous_beat| {
                    context.frame >= current_frame + delay
                }),
                render_function: Box::new(render_function),
            },
        );
    }

    pub fn later_ms(&mut self, delay: usize, render_function: &'static LaterRenderFunction) {
        let current_ms = self.ms;

        self.later_hooks.insert(
            0,
            LaterHook {
                once: true,
                when: Box::new(move |_, context, _previous_beat| context.ms >= current_ms + delay),
                render_function: Box::new(render_function),
            },
        );
    }

    pub fn later_beats(&mut self, delay: f32, render_function: &'static LaterRenderFunction) {
        let current_beat = self.beat;

        self.later_hooks.insert(
            0,
            LaterHook {
                once: true,
                when: Box::new(move |_, context, _previous_beat| {
                    context.beat_fractional >= current_beat as f32 + delay
                }),
                render_function: Box::new(render_function),
            },
        );
    }

    /// duration is in milliseconds
    pub fn start_animation(&mut self, duration: usize, animation: Animation) {
        let start_ms = self.ms;
        let ms_range = start_ms..(start_ms + duration);

        self.later_hooks.push(LaterHook {
            once: false,
            when: Box::new(move |_, ctx, _| ms_range.contains(&ctx.ms)),
            render_function: Box::new(move |canvas, ms| {
                let t = (ms - start_ms) as f32 / duration as f32;
                (animation.update)(t, canvas, ms)
            }),
        })
    }

    /// duration is in milliseconds
    pub fn animate(&mut self, duration: usize, f: &'static AnimationUpdateFunction) {
        self.start_animation(
            duration,
            Animation::new(format!("unnamed animation {}", nanoid!()), f),
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
            finished,
            thread: spinner_thread,
        }
    }
    fn end(self, message: &str) {
        *self.finished.lock().unwrap() = true;
        self.thread.join().unwrap();
        println!("{}", message);
    }
}

fn main() {}
