#!/usr/bin/env python
from copy import deepcopy
from pathlib import Path
from subprocess import run

if Path('.').resolve().name != 'gallery':
    print("Run this script from the gallery directory")
    exit(1)

for svg in Path('.').glob('*.svg'):
    png = svg.with_suffix('.png')
    if not png.exists():
        print(f"Converting {svg} to {png}")
        run(["convert", "-density", "1200", svg, png])

pngs = [ p for p in Path('.').glob('*.png') if p.name != 'gallery.png' ]

cells_per_row = 6

grid = []
for i in range(0, len(pngs), cells_per_row):
    grid.append(deepcopy(pngs[i:i+cells_per_row]))

print(f"Layout is {len(grid)} rows of {cells_per_row} cells:")
for row in grid:
    print([p.name for p in row])

for i, pngs in enumerate(grid):
    print(f"Smashing row {i}")
    run(["convert", "+append", *pngs, f"bar-{i}.png"])

print(f"Smashing all rows")
run(["convert", "-append", "bar-*.png", "gallery.png"])

for bar in Path('.').glob('bar-*.png'):
    print(f"Deleting {bar}")
    bar.unlink()
for png in pngs:
    print(f"Deleting {png}")
    png.unlink()
