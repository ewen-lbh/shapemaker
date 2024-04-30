use shapemaker::{Anchor, Canvas, CenterAnchor, Color, Fill, Layer, Object, Region, Video};
mod cli;
pub use cli::{canvas_from_cli, cli_args};

fn main() {
    let args = cli_args();
    let mut canvas = canvas_from_cli(&args);

    if args.cmd_image && !args.cmd_video {
        canvas.layers.push(canvas.random_layer("root"));
        canvas.set_background(Color::White);
        let aspect_ratio = canvas.grid_size.0 as f32 / canvas.grid_size.1 as f32;
        match Canvas::save_as_png(
            &args.arg_file,
            aspect_ratio,
            args.flag_resolution.unwrap_or(1000),
            canvas.render(&vec!["*"], true),
        ) {
            Ok(_) => println!("Image saved to {}", args.arg_file),
            Err(e) => println!("Error saving image: {}", e),
        }
        return;
    }

    let mut video = Video::<State>::new(canvas);
    video.duration_override = args.flag_duration.map(|seconds| seconds * 1000);
    video.fps = args.flag_fps.unwrap_or(30);
    video.audiofile = args.flag_audio.unwrap().into();
    video = video
        .init(&|canvas: _, context: _| {
            context.extra = State {
                bass_pattern_at: Region::from_origin_and_size((6, 3), (3, 3)),
                first_kick_happened: false,
            };
            canvas.set_background(Color::Black);

            let mut kicks = Layer::new("anchor kick");

            let fill = Some(Fill::Translucent(Color::White, 0.0));
            let circle_at = |x: usize, y: usize| Object::SmallCircle(Anchor(x as i32, y as i32));

            let (end_x, end_y) = {
                let (x, y) = canvas.world_region.end;
                (x - 2, y - 2)
            };
            kicks.add_object("top left", circle_at(1, 1), fill);
            kicks.add_object("top right", circle_at(end_x, 1), fill);
            kicks.add_object("bottom left", circle_at(1, end_y), fill);
            kicks.add_object("bottom right", circle_at(end_x, end_y), fill);
            canvas.replace_or_create_layer(kicks);

            let mut ch = Layer::new("ch");
            ch.add_object("dot", Object::SmallCircle(Anchor(2, 1)), Some(Fill::Solid(Color::Gray)));
            canvas.replace_or_create_layer(ch);
        })
        .sync_audio_with(&args.flag_sync_with.unwrap())
        .on_note("anchor kick", &|canvas, ctx| {
            // ctx.extra.bass_pattern_at = region_cycle(&canvas.world_region, None);
            canvas
                .layer("anchor kick")
                .unwrap()
                .paint_all_objects(Fill::Translucent(Color::White, 1.0));

            canvas.layer("anchor kick").unwrap().flush();

            ctx.later_ms(200, &fade_out_kick_circles)
        })
        .on_note("bass", &|canvas, ctx| {
            let mut new_layer = canvas.random_layer_within("bass", &ctx.extra.bass_pattern_at);
            new_layer.paint_all_objects(Fill::Solid(Color::White));
            canvas.replace_or_create_layer(new_layer);
        })
        .on_note("powerful clap hit, clap, perclap", &|canvas, ctx| {
            let mut new_layer =
                canvas.random_layer_within("claps", &ctx.extra.bass_pattern_at.translated(2, 0));
            new_layer.paint_all_objects(Fill::Solid(Color::Red));
            canvas.replace_or_create_layer(new_layer)
        })
        .on_note("qanda", &|canvas, ctx| {
            let mut new_layer =
                canvas.random_layer_within("qanda", &ctx.extra.bass_pattern_at.translated(-2, 0));
            new_layer.paint_all_objects(Fill::Solid(Color::Orange));
            canvas.replace_or_create_layer(new_layer)
        })
        .on_note("brokenup", &|canvas, ctx| {
            let mut new_layer = canvas
                .random_layer_within("brokenup", &ctx.extra.bass_pattern_at.translated(0, -2));
            new_layer.paint_all_objects(Fill::Solid(Color::Yellow));
            canvas.replace_or_create_layer(new_layer);
        })
        .on_note("goup", &|canvas, ctx| {
            let mut new_layer =
                canvas.random_layer_within("goup", &ctx.extra.bass_pattern_at.translated(0, 2));
            new_layer.paint_all_objects(Fill::Solid(Color::Green));
            canvas.replace_or_create_layer(new_layer);
        })
        .on_note("ch", &|canvas, _| {
            let world = canvas.world_region.clone();
            let layer = canvas.layer("ch").unwrap();
            let (obj, _) = layer.objects.get_mut("dot").unwrap();
            obj.translate_with(hat_region_cycle(&world, &obj.region()));
            layer.flush();
        })
        .on_note("flavor kick", &|canvas, _| {
            let mut new_layer = canvas.random_layer_within(
                "flavor kick",
                &Region::from_origin_and_size((14, 0), (1, 1)),
            );
            new_layer.paint_all_objects(Fill::Solid(Color::White));
            canvas.replace_or_create_layer(new_layer);
        })
        .on_note("rimshot, glitchy percs", &|canvas, ctx| {
            let mut new_layer =
                canvas.random_layer_within("percs", &ctx.extra.bass_pattern_at.translated(2, 1));
            new_layer.paint_all_objects(Fill::Translucent(Color::Red, 0.5));
            canvas.replace_or_create_layer(new_layer);
        })
        .when_remaining(10, &|canvas, _| {
            canvas.root().add_object(
                "credits text",
                Object::RawSVG(Box::new(svg::node::Text::new("by ewen-lbh"))),
                None,
            );
        })
        .command("remove", &|argumentsline, canvas, _| {
            let args = argumentsline.splitn(3, ' ').collect::<Vec<_>>();
            canvas.remove_object(args[0]);
        });

    if args.flag_preview {
        video.preview_on(8888);
    } else {
        video.render_to(args.arg_file, args.flag_workers.unwrap_or(8), false);
    }
}

fn fade_out_kick_circles(canvas: &mut Canvas) {
    canvas
        .layer("anchor kick")
        .unwrap()
        .paint_all_objects(Fill::Translucent(Color::White, 0.0));

    canvas.layer("anchor kick").unwrap().flush();
}

fn update_stem_position(
    ctx: &mut shapemaker::Context<State>,
    canvas: &mut Canvas,
    layer_name: &str,
    offset: usize,
) {
    let (dx, dy) = ctx.extra.bass_pattern_at
        - region_cycle_with_offset(
            &canvas.world_region,
            Some(&ctx.extra.bass_pattern_at),
            offset,
        );
    match canvas.layer(layer_name) {
        Some(l) => l.move_all_objects(dx, dy),
        _ => (),
    }
}

#[derive(Default)]
struct State {
    first_kick_happened: bool,
    bass_pattern_at: Region,
}

fn color_cycle(current_color: Color) -> Color {
    match current_color {
        Color::Blue => Color::Cyan,
        Color::Cyan => Color::Green,
        Color::Green => Color::Yellow,
        Color::Yellow => Color::Orange,
        Color::Orange => Color::Red,
        Color::Red => Color::Purple,
        Color::Purple => Color::Pink,
        Color::Pink => Color::White,
        Color::White => Color::Blue,
        _ => unreachable!(),
    }
}

fn region_cycle_with_offset(world: &Region, current: Option<&Region>, offset: usize) -> Region {
    if offset == 0 {
        return current.unwrap().clone();
    }

    if offset == 1 {
        return region_cycle(world, current);
    }

    region_cycle_with_offset(world, current, offset - 1)
}

fn hat_region_cycle(world: &Region, current: &Region) -> (i32, i32) {
    let (end_x, end_y) = {
        let (x, y) = world.end;
        (x - 2, y - 2)
    };

    match current.start {
        // top row
        (x, 1) if x < end_x => (1, 0),
        // right column
        (x, y) if x == end_x && y < end_y => (0, 1),
        // bottom row
        (x, y) if y == end_y && x > 1 => (-1, 0),
        // left column
        (1, y) if y > 1 => (0, -1),
        _ => unreachable!(),
    }
}

fn region_cycle(world: &Region, current: Option<&Region>) -> Region {
    let mut region = if let Some(current) = current {
        current.clone()
    } else {
        Region::from_origin_and_size((1, 1), (2, 2))
    };

    let size = (region.width(), region.height());
    // Move along x axis if possible
    if region.end.0 + size.0 <= world.end.0 - 1 {
        region.translate(size.0 as i32, 0)
    }
    // Else go to x=0 and move along y axis
    else if region.end.1 + size.1 <= world.end.1 - 1 {
        region = Region::new(2, region.end.1, size.0 + 2, region.end.1 + size.1)
    }
    // Else go to origin
    else {
        region = Region::from_origin_and_size((1, 1), size)
    }
    region
}
