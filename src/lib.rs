#![allow(uncommon_codepoints)]

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
pub mod transform;
pub mod ui;
pub mod video;
pub mod web;
pub use animation::*;
use anyhow::Result;
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
pub use transform::*;
pub use video::*;
pub use web::log;

use nanoid::nanoid;
use std::fs::{self};
use std::path::PathBuf;
use sync::SyncData;

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

impl<'a, C> Context<'a, C> {
    pub fn stem(&self, name: &str) -> StemAtInstant {
        let stems = &self.syncdata.stems;
        if !stems.contains_key(name) {
            panic!("No stem named {:?} found.", name);
        }
        StemAtInstant {
            amplitude: *stems[name].amplitude_db.get(self.ms).unwrap_or(&0.0),
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

    pub fn dump_stems(&self, to: PathBuf) -> Result<()> {
        std::fs::create_dir_all(&to)?;
        for (name, stem) in self.syncdata.stems.iter() {
            fs::write(to.join(name), format!("{:?}", stem))?;
        }
        Ok(())
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

    pub fn animate_layer(
        &mut self,
        layer: &'static str,
        duration: usize,
        f: &'static LayerAnimationUpdateFunction,
    ) {
        let animation = Animation {
            name: format!("unnamed animation {}", nanoid!()),
            update: Box::new(move |progress, canvas, ms| {
                (f)(progress, canvas.layer(layer), ms)?;
                canvas.layer(layer).flush();
                Ok(())
            }),
        };

        self.start_animation(duration, animation);
    }
}

trait Toggleable {
    fn toggle(&mut self);
}

impl Toggleable for bool {
    fn toggle(&mut self) {
        *self = !*self;
    }
}

#[allow(unused)]
fn main() {}
