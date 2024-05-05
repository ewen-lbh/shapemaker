use anyhow::Result;
use shapemaker::{
    cli::{canvas_from_cli, cli_args},
    *,
};

pub fn main() -> Result<()> {
    run(cli_args())
}

pub fn run(args: cli::Args) -> Result<()> {
    let mut canvas = canvas_from_cli(&args);

    if args.cmd_image && !args.cmd_video {
        canvas = examples::dna_analysis_machine();
        let rendered = canvas.render(&vec!["*"], true)?;
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
