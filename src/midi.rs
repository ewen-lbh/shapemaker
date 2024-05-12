use indicatif::ProgressBar;
use itertools::Itertools;
use midly::{MetaMessage, MidiMessage, TrackEvent, TrackEventKind};
use std::{collections::HashMap, fmt::Debug, path::PathBuf};

use crate::{audio, sync::SyncData, ui::Log as _, ui::MaybeProgressBar as _, Stem, Syncable};

pub struct MidiSynchronizer {
    pub midi_path: PathBuf,
}

trait Averageable {
    fn average(&self) -> f32;
}

impl Averageable for Vec<f32> {
    fn average(&self) -> f32 {
        self.iter().sum::<f32>() / self.len() as f32
    }
}

impl Syncable for MidiSynchronizer {
    fn new(path: &str) -> Self {
        Self {
            midi_path: PathBuf::from(path),
        }
    }

    fn load(&self, progressbar: Option<&ProgressBar>) -> SyncData {
        let (now, notes_per_instrument) = load_notes(&self.midi_path, progressbar);

        SyncData {
            bpm: tempo_to_bpm(now.tempo),
            stems: HashMap::from_iter(notes_per_instrument.iter().map(|(name, notes)| {
                let mut notes_per_ms = HashMap::<usize, Vec<audio::Note>>::new();

                if let Some(pb) = progressbar {
                    pb.set_length(notes.len() as u64);
                    pb.set_position(0);
                }
                progressbar.set_message(format!("Adding loaded notes for {name}"));

                for note in notes.iter() {
                    notes_per_ms
                        .entry(note.ms as usize)
                        .or_default()
                        .push(audio::Note {
                            pitch: note.key,
                            tick: note.tick,
                            velocity: note.vel,
                        });
                    progressbar.inc(1);
                }

                let duration_ms = *notes_per_ms.keys().max().unwrap_or(&0);

                if let Some(pb) = progressbar {
                    pb.set_length(duration_ms as u64 - 1);
                    pb.set_position(0);
                }
                progressbar.set_message(format!("Infering amplitudes for {name}"));

                let mut amplitudes = Vec::<f32>::new();
                let mut last_amplitude = 0.0;
                for i in 0..duration_ms {
                    if let Some(notes) = notes_per_ms.get(&i) {
                        last_amplitude = notes
                            .iter()
                            .map(|n| n.velocity as f32)
                            .collect::<Vec<f32>>()
                            .average();
                    }
                    amplitudes.push(last_amplitude);
                    progressbar.inc(1);
                }

                (
                    name.clone(),
                    Stem {
                        amplitude_max: notes.iter().map(|n| n.vel).max().unwrap_or(0) as f32,
                        amplitude_db: amplitudes,
                        duration_ms,
                        notes: notes_per_ms,
                        name: name.clone(),
                    },
                )
            })),
            markers: HashMap::new(),
        }
    }
}

#[derive(Clone)]
struct Note {
    tick: u32,
    ms: u32,
    key: u8,
    vel: u8,
}

struct Now {
    ms: usize,
    tempo: usize,
    ticks_per_beat: u16,
}

type Timeline<'a> = HashMap<u32, HashMap<String, TrackEvent<'a>>>;

type StemNotes = HashMap<u32, HashMap<String, Note>>;

impl Note {
    fn is_off(&self) -> bool {
        self.vel == 0
    }
}

fn tempo_to_bpm(µs_per_beat: usize) -> usize {
    (60_000_000.0 / µs_per_beat as f32).round() as usize
}

// fn to_ms(delta: u32, bpm: f32) -> f32 {
//     (delta as f32) * (60.0 / bpm) * 1000.0
// }

impl Debug for Note {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}{}",
            self.key,
            if self.is_off() {
                "↓".to_string()
            } else if self.vel == 100 {
                "".to_string()
            } else {
                format!("@{}", self.vel)
            }
        )
    }
}

fn load_notes<'a>(
    source: &PathBuf,
    progressbar: Option<&ProgressBar>,
) -> (Now, HashMap<String, Vec<Note>>) {
    // Read midi file using midly
    if let Some(pb) = progressbar {
        pb.set_length(1);
        pb.set_prefix("Loading");
        pb.set_message("reading MIDI file");
        pb.set_position(0);
    }

    let raw = std::fs::read(source).unwrap();
    let midifile = midly::Smf::parse(&raw).unwrap();

    let mut timeline = Timeline::new();
    progressbar.set_message(format!("MIDI file has {} tracks", midifile.tracks.len()));

    let mut now = Now {
        ms: 0,
        tempo: 0,
        ticks_per_beat: match midifile.header.timing {
            midly::Timing::Metrical(ticks_per_beat) => ticks_per_beat.as_int(),
            midly::Timing::Timecode(fps, subframe) => (1.0 / fps.as_f32() / subframe as f32) as u16,
        },
    };

    // Get track names and (initial) BPM
    let mut track_no = 0;
    let mut track_names = HashMap::<usize, String>::new();
    for track in midifile.tracks.iter() {
        track_no += 1;
        let mut track_name = String::new();
        for event in track {
            match event.kind {
                TrackEventKind::Meta(MetaMessage::TrackName(name_bytes)) => {
                    track_name = String::from_utf8(name_bytes.to_vec()).unwrap_or_default();
                }
                TrackEventKind::Meta(MetaMessage::Tempo(tempo)) => {
                    if now.tempo == 0 {
                        now.tempo = tempo.as_int() as usize;
                    }
                }
                _ => {}
            }
        }
        track_names.insert(
            track_no,
            if !track_name.is_empty() {
                track_name
            } else {
                format!("Track #{}", track_no)
            },
        );
    }

    progressbar.log(
        "Detected",
        &format!(
            "MIDI file {} with {} stems and initial tempo of {} BPM",
            source.to_str().unwrap(),
            track_names.len(),
            tempo_to_bpm(now.tempo)
        ),
    );

    // Convert ticks to absolute
    let mut track_no = 0;
    for track in midifile.tracks.iter() {
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
    let mut absolute_tick_to_ms = HashMap::<u32, usize>::new();
    let mut last_tick = 0;
    for (tick, tracks) in timeline.iter().sorted_by_key(|(tick, _)| *tick) {
        for (_, event) in tracks {
            match event.kind {
                TrackEventKind::Meta(MetaMessage::Tempo(tempo)) => {
                    now.tempo = tempo.as_int() as usize;
                }
                _ => {}
            }
        }
        let delta = tick - last_tick;
        last_tick = *tick;
        now.ms += midi_tick_to_ms(delta, now.tempo, now.ticks_per_beat as usize);
        absolute_tick_to_ms.insert(*tick, now.ms);
    }

    if let Some(pb) = progressbar {
        pb.set_length(midifile.tracks.iter().map(|t| t.len() as u64).sum::<u64>());
        pb.set_prefix("Loading");
        pb.set_message("parsing MIDI events");
        pb.set_position(0);
    }

    // Add notes
    let mut stem_notes = StemNotes::new();
    for (tick, tracks) in timeline.iter().sorted_by_key(|(tick, _)| *tick) {
        for (track_name, event) in tracks {
            match event.kind {
                TrackEventKind::Midi {
                    channel: _,
                    message,
                } => match message {
                    MidiMessage::NoteOn { key, vel } | MidiMessage::NoteOff { key, vel } => {
                        stem_notes
                            .entry(absolute_tick_to_ms[tick] as u32)
                            .or_default()
                            .insert(
                                track_name.clone(),
                                Note {
                                    tick: *tick,
                                    ms: absolute_tick_to_ms[tick] as u32,
                                    key: key.as_int(),
                                    vel: if matches!(message, MidiMessage::NoteOff { .. }) {
                                        0
                                    } else {
                                        vel.as_int()
                                    },
                                },
                            );
                    }
                    _ => {}
                },
                _ => {}
            }
            progressbar.inc(1)
        }
    }

    let mut result = HashMap::<String, Vec<Note>>::new();

    for (_ms, notes) in stem_notes.iter().sorted_by_key(|(ms, _)| *ms) {
        for (track_name, note) in notes {
            result
                .entry(track_name.clone())
                .or_default()
                .push(note.clone());
        }
    }

    (now, result)
}

fn midi_tick_to_ms(tick: u32, tempo: usize, ppq: usize) -> usize {
    let with_floats = (tempo as f32 / 1e3) / ppq as f32 * tick as f32;
    with_floats.round() as usize
}
