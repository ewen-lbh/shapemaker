#!/usr/bin/env python
"""
Usage: flp_to_json.py <input_flp_file> <output_json_file>
"""
from pathlib import Path
from docopt import docopt
import pyflp
import json


def clip_type(clip) -> str:
    if hasattr(clip, "pattern"):
        return "pattern"
    elif hasattr(clip, "channel"):
        return "channel"


def clip_name(clip) -> str:
    if clip_type(clip) == "pattern":
        return clip.pattern.name
    elif clip_type(clip) == "channel":
        return clip.channel.name
    else:
        return ""


def key_to_midi_pitch(note: str) -> int:
    letter = note[0]
    if len(note) == 3:
        sharp = True
        octave = int(note[2])
    else:
        sharp = False
        octave = int(note[1])

    letter_delta = {"C": -9, "D": -7, "E": -5, "F": -4, "G": -2, "A": 0, "B": 2}.get(
        letter, 0
    )
    return 81 + 12 * (octave - 4) + letter_delta + int(sharp)


def note_data(note):
    return {
        "key": note.key,
        "pitch": key_to_midi_pitch(note.key),
        "length": note.length,
        "velocity": note.velocity,
    }


def clip_data(clip):
    if clip_type(clip) == "pattern":
        pat = clip.pattern
        return {
            "notes": {note.position: note_data(note) for note in pat.notes},
            "values": {},
            "length": pat.length,
        }
    elif clip_type(clip) == "channel":
        chan = clip.channel
        if isinstance(chan, pyflp.channel.Automation):
            return {
                "notes": {},
                "values": {point.position: point.value for point in chan},
                "length": chan.length,
            }
        return {
            "notes": {},
            "values": {},
            "length": chan.length,
        }
    return {
        "notes": {},
        "values": {},
        "length": 0,
    }


def track_name(track) -> str:
    if track.name:
        return track.name

    clips_names = [clip_name(clip) for clip in track if clip_name(clip)]
    if not clips_names:
        return ""

    return clips_names[0]


def main():
    args = docopt(__doc__)

    project = pyflp.parse(args["<input_flp_file>"])

    out = {
        "info": {
            "name": project.title,
            "bpm": project.tempo,
        },
        "arrangements": {},
    }
    for arrangement in project.arrangements:
        current_arrangement = {"tracks": {}, "markers": {}}
        for track in arrangement.tracks:
            current_track = {}
            for clip in track:
                current_track[clip.position] = {
                    "length": clip.length,
                    "name": clip_name(clip),
                    "data": clip_data(clip),
                }
            current_arrangement["tracks"][track_name(track)] = current_track
        for marker in arrangement.timemarkers:
            current_arrangement["markers"][marker.position] = marker.name
        out["arrangements"][arrangement.name] = current_arrangement

    Path(args["<output_json_file>"]).write_text(json.dumps(out, indent=4))


if __name__ == "__main__":
    main()
