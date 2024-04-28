use std::{
    collections::HashMap,
    fmt::Display,
    fs::File,
    io::{BufReader, Write},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::sync::SyncData;

#[derive(Debug, Deserialize, Serialize)]
pub struct Stem {
    pub amplitude_db: Vec<f32>,
    /// max amplitude of this stem
    pub amplitude_max: f32,
    /// in milliseconds
    pub duration_ms: usize,

    #[serde(default)]
    pub notes: HashMap<usize, Vec<Note>>,

    #[serde(default)]
    pub name: String,
}

impl Stem {
    pub fn load_from_cbor(path: &str) -> Stem {
        let file = File::open(path).unwrap();
        let reader = BufReader::new(file);
        let stem: Stem = serde_cbor::from_reader(reader).unwrap();
        stem
    }

    pub fn save_to_cbor(&self, path: &str) {
        let mut file = File::create(path).unwrap();
        let bytes = serde_cbor::to_vec(&self).unwrap();
        file.write_all(&bytes).unwrap();
    }

    pub fn cbor_path(path: PathBuf, name: String) -> String {
        format!(
            "{}/{}.cbor",
            path.parent().unwrap_or(Path::new("./")).to_string_lossy(),
            name,
        )
    }
}

#[derive(Debug, Deserialize, Serialize, Clone, Copy)]
pub struct Note {
    pub pitch: u8,
    pub velocity: u8,
    pub tick: u32,
}

impl Note {
    pub fn symbol(&self) -> String {
        let scale = vec![
            "C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B", "B",
        ];
        let (octave, scale_index) = (
            self.pitch as usize / scale.len(),
            self.pitch as usize % scale.len(),
        );
        format!("{}{}", scale[scale_index], octave)
    }

    pub fn is_off(&self) -> bool {
        self.velocity == 0
    }

    pub fn is_on(&self) -> bool {
        !self.is_off()
    }
}

impl Display for SyncData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SyncData @ {} bpm\n{} stems", self.bpm, self.stems.len())
    }
}

#[derive(Debug, Clone, Default)]
pub struct AudioSyncPaths {
    pub stems: String,
    pub landmarks: String,
    pub complete: String,
    pub bpm: String,
    pub midi: String,
}

pub type AudioStemToMIDITrack<'a> = HashMap<&'a str, &'a str>;

pub enum MusicalDurationUnit {
    Beats,
    Halfs,
    Thirds,
    Quarters,
    Eighths,
    Sixteenths,
}

#[derive(Debug)]
pub struct StemAtInstant {
    pub amplitude: f32,
    pub amplitude_max: f32,
    pub duration: usize,
    pub velocity_max: u8,
    pub notes: Vec<Note>,
}
impl StemAtInstant {
    pub fn amplitude_relative(&self) -> f32 {
        self.amplitude / self.amplitude_max
    }

    pub fn velocity_relative(&self) -> f32 {
        self.notes.iter().map(|n| n.velocity).sum::<u8>() as f32
            / self.notes.len() as f32
            / self.velocity_max as f32
    }
}
