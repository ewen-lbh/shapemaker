use itertools::Itertools;
use rand::Rng;
use shapemaker::{
    cli::{canvas_from_cli, cli_args},
    *,
};

pub fn main() {
    run(cli_args());
}

pub fn run(args: cli::Args) {
    let mut canvas = canvas_from_cli(&args);

    if args.cmd_image && !args.cmd_video {
        canvas.root().add_object(
            "hello",
            Object::Text(Anchor(3, 4), "hello world!".into(), 16.0)
                .color(Fill::Solid(Color::Black)),
        );
        canvas.set_background(Color::White);
        let rendered = canvas.render(&vec!["*"], true);
        if args.arg_file.ends_with(".svg") {
            std::fs::write(args.arg_file, rendered).unwrap();
        } else {
            match Canvas::save_as(
                &args.arg_file,
                canvas.aspect_ratio(),
                args.flag_resolution.unwrap_or(1000),
                rendered,
            ) {
                Ok(_) => println!("Image saved to {}", args.arg_file),
                Err(e) => println!("Error saving image: {}", e),
            }
        }
        return;
    }

    let mut video = Video::<State>::new(canvas);
    video.duration_override = args.flag_duration.map(|seconds| seconds * 1000);
    video.start_rendering_at = args.flag_start.unwrap_or_default() * 1000;
    video.fps = args.flag_fps.unwrap_or(30);
    video.audiofile = args.flag_audio.unwrap().into();
    video = video
        .init(&|canvas: _, context: _| {
            context.extra = State {
                bass_pattern_at: Region::from_topleft(Point(6, 3), (3, 3)),
                first_kick_happened: false,
            };
            canvas.set_background(Color::Black);

            let mut kicks = Layer::new("anchor kick");

            let fill = Fill::Translucent(Color::White, 0.0);
            let circle_at = |x: usize, y: usize| Object::SmallCircle(Anchor(x as i32, y as i32));

            let (end_x, end_y) = {
                let Point(x, y) = canvas.world_region.end;
                (x - 2, y - 2)
            };
            kicks.add_object("top left", circle_at(1, 1).color(fill));
            kicks.add_object("top right", circle_at(end_x, 1).color(fill));
            kicks.add_object("bottom left", circle_at(1, end_y).color(fill));
            kicks.add_object("bottom right", circle_at(end_x, end_y).color(fill));
            canvas.add_or_replace_layer(kicks);

            let mut ch = Layer::new("ch");
            ch.add_object("0", Object::Dot(Anchor(0, 0)).into());
            canvas.add_or_replace_layer(ch);
        })
        .sync_audio_with(&args.flag_sync_with.unwrap())
        .on_note("anchor kick", &|canvas, ctx| {
            // ctx.extra.bass_pattern_at = region_cycle(&canvas.world_region, None);
            canvas
                .layer("anchor kick")
                .paint_all_objects(Fill::Translucent(Color::White, 1.0));

            canvas.layer("anchor kick").flush();

            ctx.later_ms(200, &fade_out_kick_circles)
        })
        .on_note("bass", &|canvas, ctx| {
            let mut new_layer = canvas.random_layer_within("bass", &ctx.extra.bass_pattern_at);
            new_layer.paint_all_objects(Fill::Solid(Color::White));
            canvas.add_or_replace_layer(new_layer);
        })
        .on_note("powerful clap hit, clap, perclap", &|canvas, ctx| {
            let mut new_layer =
                canvas.random_layer_within("claps", &ctx.extra.bass_pattern_at.translated(2, 0));
            new_layer.paint_all_objects(Fill::Solid(Color::Red));
            canvas.add_or_replace_layer(new_layer)
        })
        .on_note(
            "rimshot, glitchy percs, hitting percs, glitchy percs",
            &|canvas, ctx| {
                let mut new_layer = canvas
                    .random_layer_within("percs", &ctx.extra.bass_pattern_at.translated(2, 0));
                new_layer.paint_all_objects(Fill::Translucent(Color::Red, 0.5));
                canvas.add_or_replace_layer(new_layer);
            },
        )
        .on_note("qanda", &|canvas, ctx| {
            let mut new_layer = canvas.random_linelikes_within(
                "qanda",
                &ctx.extra.bass_pattern_at.translated(-1, -1).enlarged(1, 1),
            );
            new_layer.paint_all_objects(Fill::Solid(Color::Orange));
            new_layer.object_sizes.default_line_width = canvas.object_sizes.default_line_width
                * 4.0
                * ctx.stem("qanda").velocity_relative();
            canvas.add_or_replace_layer(new_layer)
        })
        .on_note("brokenup", &|canvas, ctx| {
            let mut new_layer = canvas
                .random_linelikes_within("brokenup", &ctx.extra.bass_pattern_at.translated(0, -2));
            new_layer.paint_all_objects(Fill::Solid(Color::Yellow));
            new_layer.object_sizes.default_line_width = canvas.object_sizes.default_line_width
                * 4.0
                * ctx.stem("brokenup").velocity_relative();
            canvas.add_or_replace_layer(new_layer);
        })
        .on_note("goup", &|canvas, ctx| {
            let mut new_layer =
                canvas.random_linelikes_within("goup", &ctx.extra.bass_pattern_at.translated(0, 2));
            new_layer.paint_all_objects(Fill::Solid(Color::Green));
            new_layer.object_sizes.default_line_width =
                canvas.object_sizes.default_line_width * 4.0 * ctx.stem("goup").velocity_relative();
            canvas.add_or_replace_layer(new_layer);
        })
        .on_note("ch", &|canvas, ctx| {
            let world = canvas.world_region.clone();

            // keep only the last 2 dots
            let dots_to_keep = canvas
                .layer("ch")
                .objects
                .iter()
                .sorted_by_key(|(name, _)| name.parse::<usize>().unwrap())
                .rev()
                .take(2)
                .map(|(name, _)| (name.clone()))
                .collect::<Vec<_>>();

            let layer = canvas.layer("ch");
            layer.object_sizes.empty_shape_stroke_width = 2.0;
            layer.objects.retain(|name, _| dots_to_keep.contains(name));

            let object_name = format!("{}", ctx.ms);
            layer.add_object(
                &object_name,
                Object::Dot(world.resized(-1, -1).random_coordinates_within().into())
                    .color(Fill::Solid(Color::Cyan)),
            );

            canvas.put_layer_on_top("ch");
            canvas.layer("ch").flush();
        })
        .when_remaining(10, &|canvas, _| {
            canvas.root().add_object(
                "credits text",
                Object::RawSVG(Box::new(svg::node::Text::new("by ewen-lbh"))).into(),
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
        .paint_all_objects(Fill::Translucent(Color::White, 0.0));

    canvas.layer("anchor kick").flush();
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
    match canvas.layer_safe(layer_name) {
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
        let Point(x, y) = world.end;
        (x - 2, y - 2)
    };

    match current.start {
        // top row
        Point(x, 1) if x < end_x => (1, 0),
        // right column
        Point(x, y) if x == end_x && y < end_y => (0, 1),
        // bottom row
        Point(x, y) if y == end_y && x > 1 => (-1, 0),
        // left column
        Point(1, y) if y > 1 => (0, -1),
        _ => unreachable!(),
    }
}

fn region_cycle(world: &Region, current: Option<&Region>) -> Region {
    let mut region = if let Some(current) = current {
        current.clone()
    } else {
        Region::from_topleft(Point(1, 1), (2, 2))
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
        region = Region::from_topleft(Point(1, 1), size)
    }
    region
}
