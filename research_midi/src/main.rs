use chrono_human_duration::ChronoHumanDuration;
use core::time;
use itertools::Itertools;
use midly::{MetaMessage, MidiMessage, TrackEvent, TrackEventKind};
use std::{
    collections::{HashMap, HashSet},
    fmt::{Debug, Display},
};

struct Note {
    tick: u32,
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

fn to_ms(delta: u32, bpm: f32) -> f32 {
    (delta as f32) * (60.0 / bpm) * 1000.0
}

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

fn main() {
    // Read midi file using midly
    let raw = std::fs::read("source.mid").unwrap();
    let result = midly::Smf::parse(&raw).unwrap();
    println!("# of tracks\n\t{}", result.tracks.len());
    let mut track_no = 0;
    println!("{:#?}", result.header);

    let mut timeline = Timeline::new();
    let mut now = Now {
        ms: 0.0,
        tempo: 500_000.0,
        ticks_per_beat: match result.header.timing {
            midly::Timing::Metrical(ticks_per_beat) => ticks_per_beat.as_int(),
            midly::Timing::Timecode(fps, subframe) => (1.0 / fps.as_f32() / subframe as f32) as u16,
        },
    };

    // Get track names
    let mut track_no = 0;
    let mut track_names = HashMap::<usize, String>::new();
    for track in result.tracks.iter() {
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
    for track in result.tracks.iter() {
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

    for (ms, notes) in stem_notes.iter().sorted_by_key(|(ms, _)| *ms) {
        for (track_name, note) in notes {
            println!(
                "{} {} {:?}",
                {
                    let duration = chrono::Duration::milliseconds(*ms as i64);
                    format!(
                        "{}'{}.{}\"#{}",
                        duration.num_minutes(),
                        duration.num_seconds() % 60,
                        duration.num_milliseconds() % 1000,
                        note.tick,
                    )
                },
                track_name,
                note
            );
        }
    }
}
