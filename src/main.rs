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

    Video::<State>::new()
        .set_fps(args.flag_fps.unwrap_or(30))
        .set_initial_canvas(canvas)
        .init(&|canvas: _, context: _| {
            context.extra = State {
                kick_region: Region::new(2, 2, 4, 4),
            };
            canvas.set_background(Color::Black);
        })
        .set_audio(args.flag_audio.unwrap().into())
        .sync_audio_with(&args.flag_sync_with.unwrap())
        .on_note("bass", &|canvas, ctx| {
            let mut new_layer = canvas.random_layer_within("bass", &ctx.extra.kick_region);
            new_layer.paint_all_objects(Fill::Solid(Color::White));
            canvas.replace_or_create_layer("bass", new_layer);
        })
        .on_note("anchor kick", &|canvas, ctx| {
            let new_kick_region = region_cycle(&canvas.world_region, &ctx.extra.kick_region);

            let (dx, dy) = new_kick_region - ctx.extra.kick_region;
            canvas.layer("bass").unwrap().move_all_objects(dx, dy);

            ctx.extra.kick_region = new_kick_region;
        })
        .on_note("powerful clap hit, clap, perclap", &|canvas, ctx| {
            let mut new_layer = canvas.random_layer_within(
                "claps",
                &region_cycle(&canvas.world_region, &ctx.extra.kick_region),
            );
            new_layer.paint_all_objects(Fill::Solid(Color::Red));
            canvas.replace_or_create_layer("claps", new_layer)
        })
        .on("start credits", &|canvas, _| {
            canvas.root().add_object(
                "credits text",
                Object::RawSVG(Box::new(svg::node::Text::new("by ewen-lbh"))),
                None,
            );
        })
        .on("end credits", &|canvas, _| {
            canvas.remove_object("credits text");
        })
        .command("remove", &|argumentsline, canvas, _| {
            let args = argumentsline.splitn(3, ' ').collect::<Vec<_>>();
            canvas.remove_object(args[0]);
        })
        .render_to(args.arg_file, args.flag_workers.unwrap_or(8))
        .unwrap();
}

#[derive(Default)]
struct State {
    kick_region: Region,
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

fn region_cycle(world: &Region, current: &Region) -> Region {
    let size = (current.width(), current.height());
    let mut new_region = current.clone();
    // Move along x axis if possible
    if current.end.0 + size.0 <= world.end.0 {
        new_region.translate(size.0 as i32, 0)
    }
    // Else go to x=0 and move along y axis
    else if current.end.1 + size.1 <= world.end.1 {
        new_region = Region::new(2, current.end.1, size.0 + 2, current.end.1 + size.1)
    }
    // Else go to origin
    else {
        new_region = Region::from_origin(size)
    }
    new_region
}
