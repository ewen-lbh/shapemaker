use std::env;

use anyhow::Result;
use itertools::Itertools;
use rand::Rng;
use shapemaker::{
    cli::{canvas_from_cli, cli_args},
    *,
};

pub fn main() -> Result<()> {
    run(cli_args())
}

pub fn run(args: cli::Args) -> Result<()> {
    let mut canvas = canvas_from_cli(&args);

    let qrname = env::var("QRCODE_NAME").unwrap();

    if args.cmd_image && !args.cmd_video {
        canvas.set_grid_size(3, 3);
        canvas.add_or_replace_layer(canvas.random_layer("root"));
        canvas.new_layer("qr");
        let qrcode = Object::Image(
            vec![
                canvas.world_region.topleft(),
                canvas.world_region.topright(),
                canvas.world_region.bottomright(),
                canvas.world_region.bottomleft(),
            ][rand::thread_rng().gen_range(0..4)]
            .region(),
            format!("./{qrname}-qr.png"),
        );
        canvas.root().remove_all_objects_in(&qrcode.region());
        canvas.set_background(Color::White);
        canvas.add_object("qr", "qr", qrcode, None).unwrap();
        canvas.put_layer_on_top("qr");
        canvas.root().objects.values_mut().for_each(|o| {
            if !o.object.fillable() {
                o.fill = Some(Fill::Solid(Color::Black));
            }
        });

        let rendered = canvas.render(true)?;
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
        return Ok(());
    }

    let mut video = Video::<()>::new(canvas);
    video.duration_override = args.flag_duration.map(|seconds| seconds * 1000);
    video.start_rendering_at = args.flag_start.unwrap_or_default() * 1000;
    video.fps = args.flag_fps.unwrap_or(30);

    if args.flag_preview {
        video.preview_on(8888)
    } else {
        video.render_to(args.arg_file, args.flag_workers.unwrap_or(8), false)
    }
}
