use rand::Rng;

use crate::*;

pub fn dna_analysis_machine() -> Canvas {
    let mut canvas = Canvas::new(vec!["root"]);
    canvas.set_grid_size(16, 9);
    canvas.set_background(Color::Black);
    let mut hatches_layer = Layer::new("root");

    let draw_in = canvas.world_region.resized(-1, -1).enlarged(-2, -2);
    let splines_area =
        Region::from_bottomleft(canvas.world_region.bottomleft(), (3, 3)).translated(3, -3);
    let red_circle_in = Region::from((
        Point(splines_area.topright().0 + 3, draw_in.topright().1),
        draw_in.bottomright(),
    ));

    println!("splines_area: {:?}", splines_area);
    println!("red_circle_in: {:?}", red_circle_in);

    let red_circle_at = red_circle_in.random_point_within();

    println!("Red circle at {:?}", red_circle_at);

    for (i, point) in draw_in.iter().enumerate() {
        if splines_area.contains(&point) {
            continue;
        }

        if point == red_circle_at {
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
                Object::BigCircle((x, y).into())
            } else {
                // XXX the .translated is a hack, centeranchor needs to disappear
                Object::Rectangle((x, y).into(), Anchor::from((x, y)).translated(1, 1))
            }
            .color(Fill::Hatched(
                Color::White,
                HatchDirection::BottomUpDiagonal,
                (i + 1) as f32 / 10.0,
                0.25,
            )),
        );
    }
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
