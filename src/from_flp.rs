use anyhow::Result;
use serde::Deserialize;
use std::{collections::HashMap, path::PathBuf};

#[derive(Debug, Deserialize)]
pub struct FLStudioProject {
    pub info: FLStudioProjectMetadata,
    pub arrangements: HashMap<String, HashMap<String, ArrangementTrack>>,
}

#[derive(Debug, Deserialize)]
pub struct FLStudioProjectMetadata {
    pub name: String,
    pub bpm: f32,
}

// #[derive(Debug, Deserialize)]
// pub struct ArrangementTrack {
//     pub name: String,
//     pub clips: HashMap<u32, TrackClip>,
// }

type ArrangementTrack = HashMap<u32, TrackClip>;

#[derive(Debug, Deserialize)]
pub struct TrackClip {
    pub length: u32,
    pub name: String,
    pub data: TrackClipData,
}

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct TrackClipData {
    pub notes: HashMap<u32, ClipNote>,
    pub values: HashMap<u32, f32>,
    pub length: u32,
}

#[derive(Debug, Deserialize)]
pub struct ClipNote {
    pub key: NoteKey,
    pub pitch: u8,
    pub length: u32,
    pub velocity: u8,
}

/// A key for a note in a clip, in the "C5" notation
type NoteKey = String;

impl FLStudioProject {
    pub fn from_json(filepath: &PathBuf) -> Result<FLStudioProject> {
        let contents = std::fs::read_to_string(filepath)?;
        Ok(serde_json::from_str(&contents)?)
    }
}
