use shapemaker::{Canvas, Color, Fill, Layer, Object, Region, Video};
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
        })
        .sync_audio_with(&args.flag_sync_with.unwrap())
        .on_note("anchor kick", &|_, ctx| {
            // ctx.extra.bass_pattern_at = region_cycle(&canvas.world_region, None);
            ctx.extra.first_kick_happened = true;
        })
        .on_note("bass", &|canvas, ctx| {
            let mut new_layer = canvas.random_layer_within("bass", &ctx.extra.bass_pattern_at);
            new_layer.paint_all_objects(Fill::Solid(Color::White));
            canvas.replace_or_create_layer("bass", new_layer);
        })
        .on_note("powerful clap hit, clap, perclap", &|canvas, ctx| {
            let mut new_layer =
                canvas.random_layer_within("claps", &ctx.extra.bass_pattern_at.translated(2, 0));
            new_layer.paint_all_objects(Fill::Solid(Color::Red));
            canvas.replace_or_create_layer("claps", new_layer)
        })
        .on_note("qanda", &|canvas, ctx| {
            if ctx.stem("qanda").amplitude_relative() < 0.7 {
                return;
            }

            let mut new_layer =
                canvas.random_layer_within("qanda", &ctx.extra.bass_pattern_at.translated(-2, 0));
            new_layer.paint_all_objects(Fill::Solid(Color::Orange));
            canvas.replace_or_create_layer("qanda", new_layer)
        })
        .on_note("brokenup", &|canvas, ctx| {
            let mut new_layer = canvas
                .random_layer_within("brokenup", &ctx.extra.bass_pattern_at.translated(0, -2));
            new_layer.paint_all_objects(Fill::Solid(Color::Yellow));
            canvas.replace_or_create_layer("brokenup", new_layer);
        })
        .on_note("goup", &|canvas, ctx| {
            let mut new_layer =
                canvas.random_layer_within("goup", &ctx.extra.bass_pattern_at.translated(0, 2));
            new_layer.paint_all_objects(Fill::Solid(Color::Yellow));
            canvas.replace_or_create_layer("goup", new_layer);
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
