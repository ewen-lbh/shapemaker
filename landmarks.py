#!/usr/bin/env python
import json
from pathlib import Path
from datetime import timedelta
import dawtool

here = Path(__file__).parent

project_filename = (here / 'audiosync/sample.flp')
# project_filename = Path('/home/ewen/projects/music/.staging/databreach.flp')

with project_filename.open('rb') as file:
    project = dawtool.load_project(project_filename, file)
    project.parse()

(here / "audiosync" / "bpm.txt").write_text(str(int(project.beats_per_min)))
(here / "audiosync" / "landmarks.json").write_text(json.dumps({  str(int(marker.time*1000)): marker.text for marker in project.markers }))
    
