use shapemaker::{Anchor, Canvas, CenterAnchor, Color, Fill, Object, Video};
mod cli;
pub use cli::{canvas_from_cli, cli_args};

fn main() {
    let args = cli_args();
    let mut canvas = canvas_from_cli(&args);

    if args.cmd_image && !args.cmd_video {
        canvas.layers.push(canvas.random_layer("main"));
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

    Video::<(Anchor, CenterAnchor, Color, Color)>::new()
        .set_fps(args.flag_fps.unwrap_or(30))
        .set_initial_canvas(canvas)
        .init(&|canvas: _, context: _| {
            context.extra = (
                canvas.random_anchor(),
                canvas.random_center_anchor(),
                canvas.random_color(),
                canvas.random_color(),
            );
            canvas.set_background(context.extra.3);
        })
        .set_audio(args.flag_audio.unwrap().into())
        .sync_audio_with(&args.flag_sync_with.unwrap())
        .on_stem(
            "bass",
            0.7,
            &|canvas, _| {
                let mut layer = canvas.random_layer("root");
                for obj in layer.objects.iter_mut() {
                    if let Some(_) = obj.1 .1 {
                        obj.1 .1 = Some(Fill::Solid(Color::Black))
                    }
                }
                canvas.layers[0] = layer;
            },
            &|_, _| {},
        )
        .on_stem(
            "anchor kick",
            0.7,
            &|canvas, _| canvas.set_background(color_cycle(canvas.background.unwrap())),
            &|_, _| {},
        )
        // .on_stem(
        //     "bass",
        //     0.7,
        //     &|canvas, context| {
        //         println!(
        //             "anchor kick at {}: amplitude_relative is {}",
        //             context.timestamp,
        //             context.stem("anchor kick").amplitude_relative()
        //         );
        //         canvas.root().add_object(
        //             "kick",
        //             Object::BigCircle(context.extra.1),
        //             Some(Fill::Solid(Color::Cyan)),
        //         );
        //     },
        //     &|canvas, _| canvas.remove_object("kick"),
        // )
        .on_stem(
            "clap",
            0.7,
            &|canvas, _| {
                let polygon = canvas.random_polygon();
                let fill = Some(Fill::Solid(canvas.random_color()));
                canvas.root().add_object("clap", polygon, fill);
            },
            &|_, _| {},
        )
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
