use std::{borrow::Borrow, collections::HashMap};

use docopt::Docopt;
use rand::Rng;
use serde::Deserialize;

const USAGE: &'static str = "
▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄▄
█░▄▄█░████░▄▄▀█▀▄▄▀█░▄▄█░▄▀▄░█░▄▄▀█░█▀█░▄▄█░▄▄▀█
█▄▄▀█░▄▄░█░▀▀░█░▀▀░█░▄▄█░█▄█░█░▀▀░█░▄▀█░▄▄█░▀▀▄█
█▄▄▄█▄██▄█▄██▄█░████▄▄▄█▄███▄█▄██▄█▄█▄█▄▄▄█▄█▄▄█
▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀vVERSION▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀▀

Usage: shapemaker [options] <file>
       shapemaker --help
       shapemaker --version
";

#[derive(Debug, Deserialize)]
struct Args {
    arg_file: String,
    flag_version: bool,
}

fn main() {
    let args: Args = Docopt::new(USAGE.replace("VERSION", env!("CARGO_PKG_VERSION")))
        .and_then(|d| d.deserialize())
        .unwrap_or_else(|e| e.exit());

    if args.flag_version {
        println!("shapemaker {}", env!("CARGO_PKG_VERSION"));
        std::process::exit(0);
    }

    let shape = random_shape("test");

    if let Err(e) = std::fs::write(args.arg_file, shape.render()) {
        eprintln!("Error: {:?}", e);
        std::process::exit(1);
    }
}

fn random_shape(name: &'static str) -> Shape {
    let mut objects: Vec<(Object, Option<Fill>)> = vec![];
    let number_of_objects = rand::thread_rng().gen_range(3..7);
    for _ in 0..number_of_objects {
        let object = random_object();
        objects.push((
            object,
            if rand::thread_rng().gen_bool(0.5) {
                Some(random_fill())
            } else {
                None
            },
        ));
    }
    Shape { name, objects }
}

fn random_object() -> Object {
    let start = random_anchor();
    match rand::thread_rng().gen_range(1..=7) {
        1 => random_polygon(),
        2 => Object::BigCircle(random_center_anchor()),
        3 => Object::SmallCircle(start),
        4 => Object::Dot(start),
        5 => Object::CurveInward(start, random_end_anchor(start)),
        6 => Object::CurveOutward(start, random_end_anchor(start)),
        7 => Object::Line(random_anchor(), random_anchor()),
        _ => unreachable!(),
    }
}

fn random_end_anchor(start: Anchor) -> Anchor {
    match start {
        Anchor::TopLeft => match rand::thread_rng().gen_range(1..=2) {
            1 => Anchor::Center,
            2 => Anchor::BottomRight,
            _ => unreachable!(),
        },
        Anchor::Top => match rand::thread_rng().gen_range(1..=2) {
            1 => Anchor::Left,
            2 => Anchor::Right,
            _ => unreachable!(),
        },
        Anchor::TopRight => match rand::thread_rng().gen_range(1..=2) {
            1 => Anchor::Center,
            2 => Anchor::BottomLeft,
            _ => unreachable!(),
        },
        Anchor::Left => match rand::thread_rng().gen_range(1..=2) {
            1 => Anchor::Top,
            2 => Anchor::Bottom,
            _ => unreachable!(),
        },
        Anchor::Center => match rand::thread_rng().gen_range(1..=4) {
            1 => Anchor::TopLeft,
            2 => Anchor::TopRight,
            3 => Anchor::BottomLeft,
            4 => Anchor::BottomRight,
            _ => unreachable!(),
        },
        Anchor::Right => match rand::thread_rng().gen_range(1..=2) {
            1 => Anchor::Top,
            2 => Anchor::Bottom,
            _ => unreachable!(),
        },
        Anchor::BottomLeft => match rand::thread_rng().gen_range(1..=2) {
            1 => Anchor::Center,
            2 => Anchor::TopRight,
            _ => unreachable!(),
        },
        Anchor::Bottom => match rand::thread_rng().gen_range(1..=2) {
            1 => Anchor::Left,
            2 => Anchor::Right,
            _ => unreachable!(),
        },
        Anchor::BottomRight => match rand::thread_rng().gen_range(1..=2) {
            1 => Anchor::Center,
            2 => Anchor::TopLeft,
            _ => unreachable!(),
        },
    }
}

fn random_polygon() -> Object {
    let number_of_anchors = rand::thread_rng().gen_range(2..7);
    let start = random_anchor();
    let mut lines: Vec<Line> = vec![];
    let mut last_anchor = start.clone();
    for _ in 0..number_of_anchors {
        let next_anchor = random_anchor();
        lines.push(random_line(next_anchor));
        last_anchor = next_anchor.clone();
    }
    Object::Polygon(start, lines)
}

fn random_line(end: Anchor) -> Line {
    match rand::thread_rng().gen_range(1..=3) {
        1 => Line::Line(end),
        2 => Line::InwardCurve(end),
        3 => Line::OutwardCurve(end),
        _ => unreachable!(),
    }
}

fn random_anchor() -> Anchor {
    match rand::thread_rng().gen_range(1..=9) {
        1 => Anchor::TopLeft,
        2 => Anchor::Top,
        3 => Anchor::TopRight,
        4 => Anchor::Left,
        5 => Anchor::Center,
        6 => Anchor::Right,
        7 => Anchor::BottomLeft,
        8 => Anchor::Bottom,
        9 => Anchor::BottomRight,
        _ => unreachable!(),
    }
}

fn random_center_anchor() -> CenterAnchor {
    match rand::thread_rng().gen_range(1..=5) {
        1 => CenterAnchor::TopLeft,
        2 => CenterAnchor::TopRight,
        3 => CenterAnchor::Center,
        4 => CenterAnchor::BottomLeft,
        5 => CenterAnchor::BottomRight,
        _ => unreachable!(),
    }
}

fn random_fill() -> Fill {
    Fill::Solid(random_color())
    // match rand::thread_rng().gen_range(1..=3) {
    //     1 => Fill::Solid(random_color()),
    //     2 => Fill::Hatched,
    //     3 => Fill::Dotted,
    //     _ => unreachable!(),
    // }
}

fn random_color() -> Color {
    match rand::thread_rng().gen_range(1..=12) {
        1 => Color::Black,
        2 => Color::White,
        3 => Color::Red,
        4 => Color::Green,
        5 => Color::Blue,
        6 => Color::Yellow,
        7 => Color::Orange,
        8 => Color::Purple,
        9 => Color::Brown,
        10 => Color::Pink,
        11 => Color::Gray,
        12 => Color::Cyan,
        _ => unreachable!(),
    }
}

#[derive(Debug)]
struct Shape {
    name: &'static str,
    objects: Vec<(Object, Option<Fill>)>,
}

#[derive(Debug)]
enum Object {
    Polygon(Anchor, Vec<Line>),
    Line(Anchor, Anchor),
    CurveOutward(Anchor, Anchor),
    CurveInward(Anchor, Anchor),
    SmallCircle(Anchor),
    Dot(Anchor),
    BigCircle(CenterAnchor),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

impl Anchor {
    fn x(&self) -> f32 {
        match self {
            Anchor::TopLeft | Anchor::Left | Anchor::BottomLeft => 0.0,
            Anchor::Top | Anchor::Center | Anchor::Bottom => 50.0,
            Anchor::TopRight | Anchor::Right | Anchor::BottomRight => 100.0,
        }
    }
    fn y(&self) -> f32 {
        match self {
            Anchor::TopLeft | Anchor::Top | Anchor::TopRight => 0.0,
            Anchor::Left | Anchor::Center | Anchor::Right => 50.0,
            Anchor::BottomLeft | Anchor::Bottom | Anchor::BottomRight => 100.0,
        }
    }
}

#[derive(Debug, Clone)]
enum CenterAnchor {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    Center,
}

impl CenterAnchor {
    fn x(&self) -> f32 {
        match self {
            CenterAnchor::TopLeft | CenterAnchor::BottomLeft => 25.0,
            CenterAnchor::TopRight | CenterAnchor::BottomRight => 75.0,
            CenterAnchor::Center => 50.0,
        }
    }

    fn y(&self) -> f32 {
        match self {
            CenterAnchor::TopLeft | CenterAnchor::TopRight => 25.0,
            CenterAnchor::BottomLeft | CenterAnchor::BottomRight => 75.0,
            CenterAnchor::Center => 50.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Line {
    Line(Anchor),
    InwardCurve(Anchor),
    OutwardCurve(Anchor),
}

#[derive(Debug, Clone, Copy)]
enum Fill {
    Solid(Color),
    Hatched,
    Dotted,
}

#[derive(Debug, Clone, Copy)]
enum Color {
    Black,
    White,
    Red,
    Green,
    Blue,
    Yellow,
    Orange,
    Purple,
    Brown,
    Cyan,
    Pink,
    Gray,
}

impl Color {
    fn to_string(self) -> String {
        match self {
            Color::Black => "black",
            Color::White => "white",
            Color::Red => "red",
            Color::Green => "green",
            Color::Blue => "blue",
            Color::Yellow => "yellow",
            Color::Orange => "orange",
            Color::Purple => "purple",
            Color::Brown => "brown",
            Color::Cyan => "cyan",
            Color::Pink => "pink",
            Color::Gray => "gray",
        }
        .to_string()
    }
}

impl Shape {
    fn render(self) -> String {
        let default_color = "black";
        let mut output = String::new();
        let mut svg = svg::Document::new();
        for (object, maybe_fill) in self.objects {
            let mut group = svg::node::element::Group::new();
            match object {
                Object::Polygon(start, lines) => {
                    eprintln!("render: polygon({:?}, {:?})", start, lines);
                    let mut path = svg::node::element::path::Data::new();
                    path = path.move_to((start.x(), start.y()));
                    for line in lines {
                        path = match line {
                            Line::Line(end) | Line::InwardCurve(end) | Line::OutwardCurve(end) => {
                                path.line_to((end.x(), end.y()))
                            }
                        };
                    }
                    path = path.close();
                    group = group
                        .add(svg::node::element::Path::new().set("d", path))
                        .set(
                            "style",
                            match maybe_fill {
                                // TODO
                                Some(Fill::Solid(color)) => {
                                    format!("fill: {};", color.to_string())
                                }
                                _ => format!(
                                    "fill: none; stroke: {}; stroke-width: 0.5px;",
                                    default_color
                                ),
                            },
                        );
                }
                Object::Line(start, end) => {
                    eprintln!("render: line({:?}, {:?})", start, end);
                    group = group.add(
                        svg::node::element::Line::new()
                            .set("x1", start.x())
                            .set("y1", start.y())
                            .set("x2", end.x())
                            .set("y2", end.y())
                            .set(
                                "style",
                                match maybe_fill {
                                    // TODO
                                    Some(Fill::Solid(color)) => {
                                        format!(
                                            "fill: none; stroke: {}; stroke-width: 2px;",
                                            color.to_string()
                                        )
                                    }
                                    _ => format!(
                                        "fill: none; stroke: {}; stroke-width: 2px;",
                                        default_color
                                    ),
                                },
                            ),
                    );
                }
                Object::CurveInward(start, end) | Object::CurveOutward(start, end) => {
                    let inward = if matches!(object, Object::CurveInward(_, _)) {
                        eprintln!("render: curve_inward({:?}, {:?})", start, end);
                        true
                    } else {
                        eprintln!("render: curve_outward({:?}, {:?})", start, end);
                        false
                    };

                    let midpoint = ((start.x() + end.x()) / 2.0, (start.y() + end.y()) / 2.0);
                    let start_from_midpoint = (start.x() - midpoint.0, start.y() - midpoint.1);
                    let end_from_midpoint = (end.x() - midpoint.0, end.y() - midpoint.1);
                    eprintln!("        midpoint: {:?}", midpoint);
                    eprintln!(
                        "        from midpoint: {:?} -> {:?}",
                        start_from_midpoint, end_from_midpoint
                    );
                    let control = {
                        let relative = (end.x() - start.x(), end.y() - start.y());
                        eprintln!("        relative: {:?}", relative);
                        // diagonal line is going like this: \
                        if start_from_midpoint.0 * start_from_midpoint.1 > 0.0
                            && end_from_midpoint.0 * end_from_midpoint.1 > 0.0
                        {
                            eprintln!("        diagonal \\");
                            if inward {
                                (
                                    midpoint.0 + relative.0.abs() / 2.0,
                                    midpoint.1 - relative.1.abs() / 2.0,
                                )
                            } else {
                                (
                                    midpoint.0 - relative.0.abs() / 2.0,
                                    midpoint.1 + relative.1.abs() / 2.0,
                                )
                            }
                        // diagonal line is going like this: /
                        } else if start_from_midpoint.0 * start_from_midpoint.1 < 0.0
                            && end_from_midpoint.0 * end_from_midpoint.1 < 0.0
                        {
                            eprintln!("        diagonal /");
                            if inward {
                                (
                                    midpoint.0 - relative.0.abs() / 2.0,
                                    midpoint.1 - relative.1.abs() / 2.0,
                                )
                            } else {
                                (
                                    midpoint.0 + relative.0.abs() / 2.0,
                                    midpoint.1 + relative.1.abs() / 2.0,
                                )
                            }
                        // line is horizontal
                        } else if start.y() == end.y() {
                            eprintln!("        horizontal");
                            (
                                midpoint.0,
                                midpoint.1
                                    + (if inward { -1.0 } else { 1.0 }) * relative.0.abs() / 2.0,
                            )
                        // line is vertical
                        } else if start.x() == end.x() {
                            eprintln!("        vertical");
                            (
                                midpoint.0
                                    + (if inward { -1.0 } else { 1.0 }) * relative.1.abs() / 2.0,
                                midpoint.1,
                            )
                        } else {
                            unreachable!()
                        }
                    };
                    eprintln!("        control: {:?}", control);
                    group = group.add(
                        svg::node::element::Path::new()
                            .set(
                                "d",
                                svg::node::element::path::Data::new()
                                    .move_to((start.x(), start.y()))
                                    .quadratic_curve_to((control, (end.x(), end.y()))),
                            )
                            .set(
                                "style",
                                match maybe_fill {
                                    // TODO
                                    Some(Fill::Solid(color)) => {
                                        format!(
                                            "fill: none; stroke: {}; stroke-width: 2px;",
                                            color.to_string()
                                        )
                                    }
                                    _ => format!(
                                        "fill: none; stroke: {}; stroke-width: 2px;",
                                        default_color
                                    ),
                                },
                            ),
                    );
                }
                Object::SmallCircle(center) => {
                    eprintln!("render: small_circle({:?})", center);
                    group = group.add(
                        svg::node::element::Circle::new()
                            .set("cx", center.x())
                            .set("cy", center.y())
                            .set("r", 5)
                            .set(
                                "style",
                                match maybe_fill {
                                    // TODO
                                    Some(Fill::Solid(color)) => {
                                        format!("fill: {};", color.to_string())
                                    }
                                    _ => format!(
                                        "fill: none; stroke: {}; stroke-width: 0.5px;",
                                        default_color
                                    ),
                                },
                            ),
                    );
                }
                Object::Dot(center) => {
                    eprintln!("render: dot({:?})", center);
                    group = group.add(
                        svg::node::element::Circle::new()
                            .set("cx", center.x())
                            .set("cy", center.y())
                            .set("r", 2)
                            .set(
                                "style",
                                match maybe_fill {
                                    // TODO
                                    Some(Fill::Solid(color)) => {
                                        format!("fill: {};", color.to_string())
                                    }
                                    _ => format!(
                                        "fill: none; stroke: {}; stroke-width: 0.5px;",
                                        default_color
                                    ),
                                },
                            ),
                    );
                }
                Object::BigCircle(center) => {
                    eprintln!("render: big_circle({:?})", center);
                    group = group.add(
                        svg::node::element::Circle::new()
                            .set("cx", center.x())
                            .set("cy", center.y())
                            .set("r", 25)
                            .set(
                                "style",
                                match maybe_fill {
                                    // TODO
                                    Some(Fill::Solid(color)) => {
                                        format!("fill: {};", color.to_string())
                                    }
                                    _ => format!(
                                        "fill: none; stroke: {}; stroke-width: 0.5px;",
                                        default_color
                                    ),
                                },
                            ),
                    );
                }
            }
            eprintln!("        fill: {:?}", &maybe_fill);
            svg = svg.add(group);
        }
        // render a dotted grid
        if false {
            for x in vec![0, 25, 50, 75, 100] {
                for y in vec![0, 25, 50, 75, 100] {
                    svg = svg.add(
                        svg::node::element::Circle::new()
                            .set("cx", x)
                            .set("cy", y)
                            .set("r", 0.5)
                            .set("fill", "gray"),
                    );
                }
            }
        }
        svg.set("viewBox", "-10 -10 120 120").to_string()
    }
}

impl Object {
    fn closed(self) -> bool {
        matches!(
            self,
            Object::Polygon(_, _) | Object::BigCircle(_) | Object::SmallCircle(_) | Object::Dot(_)
        )
    }
}
