use rand::Rng;

use crate::*;

pub fn dna_analysis_machine() -> Canvas {
    let mut canvas = Canvas::new(vec!["root"]);

    canvas.colormap = ColorMapping {
        black: "#000000".into(),
        white: "#ffffff".into(),
        red: "#cf0a2b".into(),
        green: "#22e753".into(),
        blue: "#2734e6".into(),
        yellow: "#f8e21e".into(),
        orange: "#f05811".into(),
        purple: "#6a24ec".into(),
        brown: "#a05634".into(),
        pink: "#e92e76".into(),
        gray: "#81a0a8".into(),
        cyan: "#4fecec".into(),
    };

    canvas.set_grid_size(16, 9);
    canvas.set_background(Color::Black);
    let mut hatches_layer = Layer::new("root");

    let draw_in = canvas.world_region.resized(-1, -1);

    let splines_area = Region::from_bottomleft(draw_in.bottomleft().translated(2, -1), (3, 3));
    let red_circle_in = Region::from((
        Point(splines_area.topright().0 + 3, draw_in.topright().1),
        draw_in.bottomright(),
    ));

    let red_circle_at = red_circle_in.random_point_within();

    for (i, point) in draw_in.iter().enumerate() {
        println!("{}", point);
        if splines_area.contains(&point) {
            println!("skipping {} has its contained in {}", point, splines_area);
            continue;
        }

        if point == red_circle_at {
            println!("adding red circle at {} instead of sqr", point);
            hatches_layer.add_object(
                "redpoint",
                Object::BigCircle(point.into())
                    .color(Fill::Solid(Color::Red))
                    .filter(Filter::glow(5.0)),
            );
            continue;
        }

        let Point(x, y) = point;

        hatches_layer.add_object(
            &format!("{}-{}", x, y),
            if rand::thread_rng().gen_bool(0.5) {
                Object::BigCircle(point)
            } else {
                Object::Rectangle(point, point)
            }
            .color(Fill::Hatched(
                Color::White,
                HatchDirection::BottomUpDiagonal,
                (i + 5) as f32 / 10.0,
                0.25,
            )),
        );
    }
    println!("{:?}", hatches_layer.objects.keys());
    canvas.layers.push(hatches_layer);
    let mut splines = canvas.n_random_linelikes_within("splines", &splines_area, 30);
    for (i, ColoredObject(_, ref mut fill, _)) in splines.objects.values_mut().enumerate() {
        *fill = Some(Fill::Solid(if i % 2 == 0 {
            Color::Cyan
        } else {
            Color::Pink
        }))
    }
    splines.filter_all_objects(Filter::glow(4.0));
    canvas.layers.push(splines);

    canvas
}

pub fn title() -> Canvas {
    let mut canvas = dna_analysis_machine();
    let text_zone = Region::from_topleft(Point(8, 2), (3, 3));
    canvas.remove_all_objects_in(&text_zone);
    let last_letter_at = &text_zone.bottomright().translated(1, 0);
    canvas.remove_all_objects_in(&last_letter_at.region());

    let text_layer = canvas.new_layer("title");

    let title = String::from("shapemaker");

    for (i, point) in text_zone
        .iter()
        .chain(last_letter_at.region().iter())
        .enumerate()
    {
        println!("{}: {} '{}'", i, point, &title[i..i + 1]);
        let character = title[i..i + 1].to_owned();

        text_layer.add_object(
            &i.to_string(),
            Object::CenteredText(point, character, 30.0).color(Fill::Solid(Color::White)),
        );
    }

    canvas
}
