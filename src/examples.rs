use rand::Rng;

use crate::*;

pub fn dna_analysis_machine() -> Canvas {
    let mut canvas = Canvas::new(vec![]);

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

    canvas.canvas_outter_padding = 900;
    canvas.set_grid_size(16, 9);
    canvas.set_background(Color::Black);
    let mut hatches_layer = Layer::new("hatches");

    let draw_in = canvas.world_region.resized(-1, -1);

    let splines_area = Region::from_bottomleft(draw_in.bottomleft().translated(2, -1), (3, 3)).unwrap();
    let red_circle_in = Region::from_topright(draw_in.topright().translated(-3, 0), (4, 3)).unwrap();

    let red_circle_at = red_circle_in.random_point_within();

    let red_dot_layer = canvas.new_layer("red dot");
    let mut red_dot_friends = Layer::new("red dot friends");

    for (i, point) in draw_in.iter().enumerate() {
        if splines_area.contains(&point) {
            continue;
        }

        if point == red_circle_at {
            red_dot_layer.add_object(
                format!("red circle @ {}", point),
                Object::BigCircle(point)
                    .color(Fill::Solid(Color::Red))
                    .filter(Filter::glow(5.0)),
            );

            for point in red_circle_at.region().enlarged(1, 1).iter() {
                red_dot_friends.add_object(
                    format!("reddot @ {}", point),
                    Object::SmallCircle(point).color(Fill::Solid(Color::Red)),
                )
            }
        }

        hatches_layer.add_object(
            point,
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

    red_dot_friends.add_object(
        "line",
        Object::Line(
            draw_in.bottomright().translated(1, -3),
            draw_in.bottomright().translated(-3, 1),
            4.0,
        )
        .color(Fill::Solid(Color::Cyan))
        .filter(Filter::glow(4.0)),
    );

    canvas.layers.push(hatches_layer);
    canvas.layers.push(red_dot_friends);
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
    // let blackout = canvas.new_layer("black out splines");
    // splines_area.iter_upper_strict_triangle().for_each(|point| {
    //     println!("blacking out {}", point);
    //     blackout.add_object(
    //         point,
    //         Object::Rectangle(point, point).color(Fill::Solid(Color::Black)),
    //     )
    // });

    // canvas.put_layer_on_top("black out splines");
    canvas.reorder_layers(vec!["red dot friends", "hatches", "red dot"]);

    canvas
}

pub fn title() -> Canvas {
    let mut canvas = dna_analysis_machine();
    let text_zone = Region::from_topleft(Point(8, 2), (3, 3)).unwrap();
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
