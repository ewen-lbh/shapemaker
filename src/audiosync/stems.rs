    // #[deprecated(note = "Use `sync_with` instead")]
    // pub fn sync_to(self, audio: &AudioSyncPaths, stem_audio_to_midi: AudioStemToMIDITrack) -> Self {
    //     let progress_bar_tree = MultiProgress::new();
    //     // Read BPM from file
    //     let bpm = std::fs::read_to_string(audio.bpm.clone())
    //         .map_err(|e| format!("Failed to read BPM file: {}", e))
    //         .and_then(|bpm| {
    //             bpm.trim()
    //                 .parse::<usize>()
    //                 .map(|parsed| parsed)
    //                 .map_err(|e| format!("Failed to parse BPM file: {}", e))
    //         })
    //         .unwrap();

    //     // Read landmakrs from JSON file
    //     let markers = std::fs::read_to_string(audio.landmarks.clone())
    //         .map_err(|e| format!("Failed to read landmarks file: {}", e))
    //         .and_then(|landmarks| {
    //             match serde_json::from_str::<HashMap<String, String>>(&landmarks)
    //                 .map_err(|e| format!("Failed to parse landmarks file: {}", e))
    //             {
    //                 Ok(unparsed_keys) => {
    //                     let mut parsed_keys: HashMap<usize, String> = HashMap::new();
    //                     for (key, value) in unparsed_keys {
    //                         parsed_keys.insert(key.parse::<usize>().unwrap(), value);
    //                     }
    //                     Ok(parsed_keys)
    //                 }
    //                 Err(e) => Err(e),
    //             }
    //         })
    //         .unwrap();

    //     // Read all WAV stem files: get their duration and amplitude per millisecond
    //     let mut stems: HashMap<String, Stem> = HashMap::new();

    //     let mut threads = vec![];
    //     let (tx, rx) = mpsc::channel();

    //     let stem_file_entries: Vec<_> = std::fs::read_dir(audio.stems.clone())
    //         .map_err(|e| format!("Failed to read stems directory: {}", e))
    //         .unwrap()
    //         .filter(|e| match e {
    //             Ok(e) => e.path().extension().unwrap_or_default() == "wav",
    //             Err(_) => false,
    //         })
    //         .collect();

    //     let main_progress_bar = progress_bar_tree.add(
    //         ProgressBar::new(stem_file_entries.len() as u64)
    //             .with_style(
    //                 ProgressStyle::with_template(
    //                     &(PROGRESS_BARS_STYLE.to_owned()
    //                         + " ({pos:.bold} stems loaded out of {len})"),
    //                 )
    //                 .unwrap()
    //                 .progress_chars("== "),
    //             )
    //             .with_message("Loading stems"),
    //     );

    //     main_progress_bar.tick();

    //     for (i, entry) in stem_file_entries.into_iter().enumerate() {
    //         let progress_bar = progress_bar_tree.add(
    //             ProgressBar::new(0).with_style(
    //                 ProgressStyle::with_template(&("  ".to_owned() + PROGRESS_BARS_STYLE))
    //                     .unwrap()
    //                     .progress_chars("== "),
    //             ),
    //         );
    //         let main_progress_bar = main_progress_bar.clone();
    //         let tx = tx.clone();
    //         threads.push(thread::spawn(move || {
    //             let path = entry.unwrap().path();
    //             let stem_name: String = path.file_stem().unwrap().to_string_lossy().into();
    //             let stem_cache_path = Stem::cbor_path(path.clone(), stem_name.clone());
    //             progress_bar.set_message(format!("Loading \"{}\"", stem_name));

    //             // Check if a cached CBOR of the stem file exists
    //             if Path::new(&stem_cache_path).exists() {
    //                 let stem = Stem::load_from_cbor(&stem_cache_path);
    //                 progress_bar.set_message("Loaded {} from cache".to_owned());
    //                 tx.send((progress_bar, stem_name, stem)).unwrap();
    //                 main_progress_bar.inc(1);
    //                 return;
    //             }

    //             let mut reader = hound::WavReader::open(path.clone())
    //                 .map_err(|e| format!("Failed to read stem file {:?}: {}", path, e))
    //                 .unwrap();
    //             let spec = reader.spec();
    //             let sample_to_frame = |sample: usize| {
    //                 (sample as f64 / spec.channels as f64 / spec.sample_rate as f64
    //                     * self.fps as f64) as usize
    //             };
    //             let mut amplitude_db: Vec<f32> = vec![];
    //             let mut current_amplitude_sum: f32 = 0.0;
    //             let mut current_amplitude_buffer_size: usize = 0;
    //             let mut latest_loaded_frame = 0;
    //             progress_bar.set_length(reader.samples::<i16>().len() as u64);
    //             for (i, sample) in reader.samples::<i16>().enumerate() {
    //                 let sample = sample.unwrap();
    //                 if sample_to_frame(i) > latest_loaded_frame {
    //                     amplitude_db
    //                         .push(current_amplitude_sum / current_amplitude_buffer_size as f32);
    //                     current_amplitude_sum = 0.0;
    //                     current_amplitude_buffer_size = 0;
    //                     latest_loaded_frame = sample_to_frame(i);
    //                 } else {
    //                     current_amplitude_sum += sample.abs() as f32;
    //                     current_amplitude_buffer_size += 1;
    //                 }
    //                 // main_progress_bar.tick();
    //                 progress_bar.inc(1);
    //             }
    //             amplitude_db.push(current_amplitude_sum / current_amplitude_buffer_size as f32);
    //             progress_bar.finish_with_message(format!(" Loaded \"{}\"", stem_name));

    //             let stem = Stem {
    //                 amplitude_max: *amplitude_db
    //                     .iter()
    //                     .max_by(|a, b| a.partial_cmp(b).unwrap())
    //                     .unwrap(),
    //                 amplitude_db,
    //                 duration_ms: (reader.duration() as f64 / spec.sample_rate as f64 * 1000.0)
    //                     as usize,
    //                 notes: HashMap::new(),
    //                 path: path.clone(),
    //                 name: stem_name.clone(),
    //             };

    //             main_progress_bar.inc(1);

    //             tx.send((progress_bar, stem_name, stem)).unwrap();
    //             drop(tx);
    //         }));
    //     }
    //     drop(tx);

    //     for (progress_bar, stem_name, stem) in rx {
    //         progress_bar.finish_and_clear();
    //         stems.insert(stem_name.to_string(), stem);
    //     }

    //     for thread in threads {
    //         thread.join().unwrap();
    //     }

    //     // Read MIDI file
    //     println!("Loading MIDI…");
    //     let midi_bytes = std::fs::read(audio.midi.clone())
    //         .map_err(|e| format!("While loading MIDI file {}: {:?}", audio.midi.clone(), e))
    //         .unwrap();
    //     let midi = midly::Smf::parse(&midi_bytes).unwrap();

    //     let mut timeline = HashMap::<u32, HashMap<String, midly::TrackEvent>>::new();
    //     let mut now_ms = 0.0;
    //     let mut now_tempo = 500_000.0;
    //     let ticks_per_beat = match midi.header.timing {
    //         midly::Timing::Metrical(ticks_per_beat) => ticks_per_beat.as_int(),
    //         midly::Timing::Timecode(fps, subframe) => (1.0 / fps.as_f32() / subframe as f32) as u16,
    //     };

    //     // Get track names
    //     let mut track_no = 0;
    //     let mut track_names = HashMap::<usize, String>::new();
    //     for track in midi.tracks.iter() {
    //         track_no += 1;
    //         let mut track_name = String::new();
    //         for event in track {
    //             match event.kind {
    //                 TrackEventKind::Meta(MetaMessage::TrackName(name_bytes)) => {
    //                     track_name = String::from_utf8(name_bytes.to_vec()).unwrap_or_default();
    //                 }
    //                 _ => {}
    //             }
    //         }
    //         let track_name = if !track_name.is_empty() {
    //             track_name
    //         } else {
    //             format!("Track #{}", track_no)
    //         };
    //         if !stems.contains_key(&track_name) {
    //             println!(
    //                 "MIDI track {} has no corresponding audio stem, skipping",
    //                 track_name
    //             );
    //         }
    //         track_names.insert(track_no, track_name);
    //     }

    //     // Convert ticks to absolute
    //     let mut track_no = 0;
    //     for track in midi.tracks.iter() {
    //         track_no += 1;
    //         let mut absolute_tick = 0;
    //         for event in track {
    //             absolute_tick += event.delta.as_int();
    //             timeline
    //                 .entry(absolute_tick)
    //                 .or_default()
    //                 .insert(track_names[&track_no].clone(), *event);
    //         }
    //     }

    //     // Convert ticks to ms
    //     let mut absolute_tick_to_ms = HashMap::<u32, f32>::new();
    //     let mut last_tick = 0;
    //     for (tick, tracks) in timeline.iter().sorted_by_key(|(tick, _)| *tick) {
    //         for (_, event) in tracks {
    //             match event.kind {
    //                 TrackEventKind::Meta(MetaMessage::Tempo(tempo)) => {
    //                     now_tempo = tempo.as_int() as f32;
    //                 }
    //                 _ => {}
    //             }
    //         }
    //         let delta = tick - last_tick;
    //         last_tick = *tick;
    //         let delta_µs = now_tempo * delta as f32 / ticks_per_beat as f32;
    //         now_ms += delta_µs / 1000.0;
    //         absolute_tick_to_ms.insert(*tick, now_ms);
    //     }

    //     // Add notes
    //     for (tick, tracks) in timeline.iter().sorted_by_key(|(tick, _)| *tick) {
    //         for (track_name, event) in tracks {
    //             match event.kind {
    //                 TrackEventKind::Midi {
    //                     channel: _,
    //                     message,
    //                 } => match message {
    //                     MidiMessage::NoteOn { key, vel } | MidiMessage::NoteOff { key, vel } => {
    //                         let note = Note {
    //                             tick: *tick,
    //                             pitch: key.as_int(),
    //                             velocity: if matches!(message, MidiMessage::NoteOff { .. }) {
    //                                 0
    //                             } else {
    //                                 vel.as_int()
    //                             },
    //                         };
    //                         let stem_name: &str = stem_audio_to_midi
    //                             .get(&track_name.as_str())
    //                             .unwrap_or(&track_name.as_str());
    //                         if stems.contains_key(stem_name) {
    //                             stems
    //                                 .get_mut(stem_name)
    //                                 .unwrap()
    //                                 .notes
    //                                 .entry(absolute_tick_to_ms[tick] as usize)
    //                                 .or_default()
    //                                 .push(note);
    //                         }
    //                     }
    //                     _ => {}
    //                 },
    //                 _ => {}
    //             }
    //         }
    //     }

    //     std::fs::write("stems.json", serde_json::to_vec(&stems).unwrap());

    //     for (name, stem) in &stems {
    //         // Write loaded stem to a CBOR cache file
    //         Stem::save_to_cbor(&stem, &Stem::cbor_path(stem.path.clone(), name.to_string()));
    //     }

    //     main_progress_bar.finish_and_clear();

    //     println!("Loaded {} stems", stems.len());

    //     Self {
    //         audio_paths: audio.clone(),
    //         markers,
    //         bpm,
    //         stems,
    //         ..self
    //     }
    // }
