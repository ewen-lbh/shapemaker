use itertools::Itertools;
use midly::{MetaMessage, MidiMessage, TrackEvent, TrackEventKind};
use std::{collections::HashMap, fmt::Debug, path::PathBuf};

use crate::{audio, sync::SyncData, Stem, Syncable};

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

fn is_kick_channel(name: &str) -> bool {
    return name.contains("kick");
}

impl Syncable for MidiSynchronizer {
    fn new(path: &str) -> Self {
        Self {
            midi_path: PathBuf::from(path),
        }
    }

    fn load(&self) -> SyncData {
        let (now, notes_per_instrument) = load_notes(&self.midi_path);

        SyncData {
            bpm: tempo_to_bpm(now.tempo),
            stems: HashMap::from_iter(notes_per_instrument.iter().map(|(name, notes)| {
                let mut notes_per_ms = HashMap::<usize, Vec<audio::Note>>::new();

                for note in notes.iter() {
                    notes_per_ms
                        .entry(note.ms as usize)
                        .or_default()
                        .push(audio::Note {
                            pitch: note.key,
                            tick: note.tick,
                            velocity: note.vel,
                        });

                    if is_kick_channel(name) {
                        // kicks might not have a note off event, so we added one manually after 100ms
                        notes_per_ms
                            .entry((note.ms + 100) as usize)
                            .or_default()
                            .push(audio::Note {
                                pitch: note.key,
                                tick: note.tick,
                                velocity: 0,
                            });
                    }
                }

                let duration_ms = notes_per_ms.keys().max().unwrap_or(&0);
                let mut amplitudes = Vec::<f32>::new();
                let mut last_amplitude = 0.0;
                for i in 0..*duration_ms {
                    if let Some(notes) = notes_per_ms.get(&i) {
                        last_amplitude = notes
                            .iter()
                            .map(|n| n.velocity as f32)
                            .collect::<Vec<f32>>()
                            .average();
                    }
                    amplitudes.push(last_amplitude);
                }

                (
                    name.clone(),
                    Stem {
                        amplitude_max: notes.iter().map(|n| n.vel).max().unwrap_or(0) as f32,
                        amplitude_db: amplitudes,
                        duration_ms: notes.iter().map(|n| n.tick).max().unwrap_or(0) as usize,
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
    ms: f32,
    tempo: f32,
    ticks_per_beat: u16,
}

type Timeline<'a> = HashMap<u32, HashMap<String, TrackEvent<'a>>>;

type StemNotes = HashMap<u32, HashMap<String, Note>>;

impl Note {
    fn is_off(&self) -> bool {
        self.vel == 0
    }
}

fn tempo_to_bpm(µs_per_beat: f32) -> usize {
    (60_000_000.0 / µs_per_beat) as usize
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

fn load_notes<'a>(source: &PathBuf) -> (Now, HashMap<String, Vec<Note>>) {
    // Read midi file using midly
    let raw = std::fs::read(source).unwrap();
    let midifile = midly::Smf::parse(&raw).unwrap();
    println!("# of tracks\n\t{}", midifile.tracks.len());
    println!("{:#?}", midifile.header);

    let mut timeline = Timeline::new();
    let mut now = Now {
        ms: 0.0,
        tempo: 500_000.0,
        ticks_per_beat: match midifile.header.timing {
            midly::Timing::Metrical(ticks_per_beat) => ticks_per_beat.as_int(),
            midly::Timing::Timecode(fps, subframe) => (1.0 / fps.as_f32() / subframe as f32) as u16,
        },
    };

    // Get track names
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

    println!("{:#?}", track_names);

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
    let mut absolute_tick_to_ms = HashMap::<u32, f32>::new();
    let mut last_tick = 0;
    for (tick, tracks) in timeline.iter().sorted_by_key(|(tick, _)| *tick) {
        for (_, event) in tracks {
            match event.kind {
                TrackEventKind::Meta(MetaMessage::Tempo(tempo)) => {
                    now.tempo = tempo.as_int() as f32;
                }
                _ => {}
            }
        }
        let delta = tick - last_tick;
        last_tick = *tick;
        let delta_µs = now.tempo * delta as f32 / now.ticks_per_beat as f32;
        now.ms += delta_µs / 1000.0;
        absolute_tick_to_ms.insert(*tick, now.ms);
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
        }
    }

    let mut result = HashMap::<String, Vec<Note>>::new();

    for (ms, notes) in stem_notes.iter().sorted_by_key(|(ms, _)| *ms) {
        for (track_name, note) in notes {
            // println!(
            //     "{} {} {:?}",
            //     {
            //         let duration = chrono::Duration::milliseconds(*ms as i64);
            //         format!(
            //             "{}'{}.{}\"#{}",
            //             duration.num_minutes(),
            //             duration.num_seconds() % 60,
            //             duration.num_milliseconds() % 1000,
            //             note.tick,
            //         )
            //     },
            //     track_name,
            //     note
            // );

            result
                .entry(track_name.clone())
                .or_default()
                .push(note.clone());
        }
    }

    (now, result)
}
