use std::collections::HashMap;

use crate::Stem;

pub type TimestampMS = usize;

pub trait Syncable {
    fn load(&self) -> SyncData;
    fn new(path: &str) -> Self;
}

#[derive(Debug, Default)]
pub struct SyncData {
    pub stems: HashMap<String, Stem>,
    pub markers: HashMap<TimestampMS, String>,
    pub bpm: usize,
}
