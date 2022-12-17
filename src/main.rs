use std::collections::HashMap;

use chumsky::prelude::*;
use chumsky::text;
use chumsky::text::{newline, whitespace};
use docopt::Docopt;
use serde::Deserialize;

const USAGE: &'static str = "
▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄
█░▄▄█░████░▄▄▀█▀▄▄▀█░▄▄█░▄▀▄░█░▄▄▀█░█▀█░▄▄█░▄▄▀█
█▄▄▀█░▄▄░█░▀▀░█░▀▀░█░▄▄█░█▄█░█░▀▀░█░▄▀█░▄▄█░▀▀▄█
█▄▄▄█▄██▄█▄██▄█░████▄▄▄█▄███▄█▄██▄█▄█▄█▄▄▄█▄█▄▄█
▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀

Usage: shapemaker [options] <file>
       shapemaker (-h | --help)
";

#[derive(Debug, Deserialize)]
struct Args {
    arg_file: String,
    flag_verbose: bool,
}

fn main() {
    let args: Args = Docopt::new(USAGE)
        .and_then(|d| d.deserialize())
        .unwrap_or_else(|e| e.exit());

    let file_contents = std::fs::read_to_string(&args.arg_file).unwrap();
    let shapes: Vec<Shape> = match parser().parse(file_contents) {
        Ok(shapes) => {
            println!("Parsed shapes: {:#?}", shapes);
            shapes
        }
        Err(e) => {
            println!("Error: {:?}", e);
            std::process::exit(1);
        }
    };
}

#[derive(Debug)]
struct Shape {
    name: String,
    objects: Vec<(Object, Option<Fill>)>,
}

#[derive(Debug)]
enum Object {
    Polygon(Vec<Line>),
    SmallCircle(Anchor),
    Dot(Anchor),
    BigCircle(CenterAnchor),
}

#[derive(Debug, Clone)]
enum Anchor {
    Top,
    TopRight,
    Right,
    BottomRight,
    Bottom,
    BottomLeft,
    Left,
    TopLeft,
    Center,
}

#[derive(Debug, Clone)]
enum CenterAnchor {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Center,
}

#[derive(Debug, Clone)]
enum Line {
    Line(Anchor, Anchor),
    InwardCurve(Anchor, Anchor),
    OutwardCurve(Anchor, Anchor),
}

#[derive(Debug, Clone)]
enum Fill {
    Solid(Color),
    Hatched,
    Dotted,
}

#[derive(Debug, Clone)]
enum Color {
    Named(ColorName),
    RGBA(u8, u8, u8, u8),
}

#[derive(Debug, Clone)]
enum ColorName {
    Black,
    White,
    Grey,
    Red,
    Green,
    Blue,
    Yellow,
    Cyan,
    Magenta,
    Orange,
}

fn parser() -> impl Parser<char, Vec<Shape>, Error = Simple<char>> {
    let anchor = choice((
        just("top").to(Anchor::Top),
        just("top right").to(Anchor::TopRight),
        just("right").to(Anchor::Right),
        just("bottom right").to(Anchor::BottomRight),
        just("bottom").to(Anchor::Bottom),
        just("bottom left").to(Anchor::BottomLeft),
        just("left").to(Anchor::Left),
        just("top left").to(Anchor::TopLeft),
        just("center").to(Anchor::Center),
    ));
    let center_anchor = choice((
        just("top left").to(CenterAnchor::TopLeft),
        just("top right").to(CenterAnchor::TopRight),
        just("bottom left").to(CenterAnchor::BottomLeft),
        just("bottom right").to(CenterAnchor::BottomRight),
        just("center").to(CenterAnchor::Center),
    ));

    let straight_line = anchor
        .then_ignore(just("--").padded())
        .then(anchor)
        .map(|(a, b)| Line::Line(a, b));
    let inward_curve = anchor
        .then_ignore(one_of("n(").padded())
        .then(anchor)
        .map(|(a, b)| Line::InwardCurve(a, b));
    let outward_curve = anchor
        .then_ignore(one_of("u(").padded())
        .then(anchor)
        .map(|(a, b)| Line::OutwardCurve(a, b));
    let line = choice((straight_line, inward_curve, outward_curve));
    let polygon = line.padded().repeated().boxed().map(Object::Polygon);
    let point = anchor
        .then_ignore(whitespace())
        .then_ignore(just("point"))
        .map(Object::SmallCircle);
    let circle = center_anchor
        .then_ignore(whitespace())
        .then_ignore(just("circle"))
        .map(Object::BigCircle);
    let dot = anchor
        .then_ignore(whitespace())
        .then_ignore(just("dot"))
        .map(Object::Dot);
    let object = choice((polygon, point, dot, circle));
    let color = choice((
        just("black").to(Color::Named(ColorName::Black)),
        just("white").to(Color::Named(ColorName::White)),
        just("grey").to(Color::Named(ColorName::Grey)),
        just("red").to(Color::Named(ColorName::Red)),
        just("green").to(Color::Named(ColorName::Green)),
        just("blue").to(Color::Named(ColorName::Blue)),
        just("yellow").to(Color::Named(ColorName::Yellow)),
        just("cyan").to(Color::Named(ColorName::Cyan)),
        just("magenta").to(Color::Named(ColorName::Magenta)),
        just("orange").to(Color::Named(ColorName::Orange)),
        just("#").ignored().then(text::int(16)).map(|(_, i)| {
            Color::RGBA(
                ((i >> 24) & 0xFF) as u8,
                ((i >> 16) & 0xFF) as u8,
                ((i >> 8) & 0xFF) as u8,
                (i & 0xFF) as u8,
            )
        }),
    ));
    let fill = choice((
        just("filled with")
            .ignored()
            .then_ignore(whitespace())
            .then(color)
            .map(|(_, c)| Fill::Solid(c)),
        just("hatched").to(Fill::Hatched),
        just("dotted").to(Fill::Dotted),
    ));
    let filled_object = just("[")
        .padded()
        .ignored()
        .then(object)
        .then_ignore(just("]").padded())
        .then(fill)
        .map(|((_, o), f)| (o, Some(f)));
    let separator = newline().then_ignore(whitespace()).then_ignore(newline());
    let header = take_until(just(":"))
        .then_ignore(whitespace())
        .then_ignore(newline())
        .map(|(i, _)| i);
    let shape = header
        .then_ignore(whitespace())
        .then(
            filled_object
                .or(object.map(|o| (o, None)))
                .separated_by(newline()),
        )
        .map(|(name, objects)| Shape {
            name: name.into_iter().collect(),
            objects,
        });

    shape.separated_by(separator)
}
