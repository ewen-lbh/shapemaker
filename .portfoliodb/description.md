---
tags:
- graphism
- programming
- experiment
- language
- command line
made with:
- svg
- rust
---

# Shapemaker

:: en

I wanted to dabble in generative art, and, as a first project, create a way to generate small, icon-like shapes from a predefined set of possible lines, curves and circles

## A restricted set of shapes

![]()

Those shapes are described as sequences of elements, such as lines, either straight or curved, that start and end on fixed "anchor points": south-west, south, south-east, east, north-east, north, north-west, west and center.

Lines can be grouped to form polygons (with some edges possibly curved), which can be filled with patterns (hatched or dotted) or colors.

Additionally, dots, points (small circles) or circles can be added. The latter objects take up a full quarter of the block and can be placed in each quarter's center. These can be filled just like polygons.

## A language to describe these shapes

I needed a language to describe these shapes with text, so that a program could generate random shapes, while keeping them coherent by reducing the amount of variables needed (randomly generating SVG text directly, for example, is unfeasable).

```
percent:
top left circle
bottom right -- top left
bottom right circle

abstract1:
[ bottom left -- left ) top -- bottom -- bottom left ] filled with green
top -- right -- bottom
top left red dot

lowercase j:
center point
center -- bottom -- bottom left
```


The idea was to generate SVG from this representation. 
