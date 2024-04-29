build:
    cargo build
    cp target/debug/shapemaker .

install:
    cp shapemaker ~/.local/bin/

example-video args='':
    ./shapemaker video --colors colorschemes/palenight.css out.mp4 --sync-with fixtures/schedule-hell.midi --audio fixtures/schedule-hell.flac --grid-size 16x10 --resolution 1920 {{args}}
