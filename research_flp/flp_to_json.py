#!/usr/bin/env python
"""
Usage: flp_to_json.py <input_flp_file> <output_json_file>
"""
from pathlib import Path
from docopt import docopt
import pyflp
import json

here = Path(__file__).parent
args = docopt(__doc__)

project = pyflp.parse(here / args['<input_flp_file>'])

out = {
        "info": {
            "name": project.title,
            "bpm": project.tempo,
        },
        "arrangements": {}
    }

def clip_type(clip) -> str:
    if hasattr(clip, "pattern"):
        return "pattern"
    elif hasattr(clip, "channel"):
        return "channel"

def clip_name(clip) -> str :
    if clip_type(clip) == "pattern":
        return clip.pattern.name
    elif clip_type(clip) == "channel":
        return clip.channel.name
    else:
        return ""

def note_data(note):
    return {
            "key": note.key,
            "length": note.length,
            "velocity": note.velocity
            }

def clip_data(clip):
    if clip_type(clip) == "pattern":
        pat = clip.pattern
        return {
            "notes": {note.position: note_data(note) for note in pat.notes },
            "length": pat.length
        }
    elif clip_type(clip) == "channel":
        chan = clip.channel
        if isinstance(chan, pyflp.channel.Automation):
            return { point.position: point.value for point in chan }
        return {}
    return {}


def track_name(track) -> str:
    if track.name: return track.name

    clips_names = [ clip_name(clip) for clip in track if clip_name(clip) ]
    if not clips_names: return ""

    return clips_names[0]

for arrangement in project.arrangements:
    current_arrangement = {}
    for track in arrangement.tracks:
        current_track = {}
        for clip in track:
            current_track[clip.position] = {
                    "length": clip.length,
                    "name": clip_name(clip),
                    "data": clip_data(clip),
                    }
        current_arrangement[track_name(track)] = current_track
    out["arrangements"][arrangement.name] = current_arrangement

(here / args['<output_json_file>']).write_text(json.dumps(out, indent=4))

