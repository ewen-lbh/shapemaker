build:
    cargo build
    cp target/debug/shapemaker .

example-video:
    ./shapemaker video --colors colorschemes/afterglow.json out.mp4 --sync-with fixtures/schedule-hell.midi --audio fixtures/schedule-hell.flac --grid-size 16x9
