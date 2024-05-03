build:
    cargo build --bin shapemaker
    cp target/debug/shapemaker .

web:
    wasm-pack build --target web -d web

start-web:
    just web
    python3 -m http.server --directory web

install:
    cp shapemaker ~/.local/bin/

example-video args='':
    ./shapemaker video --colors colorschemes/palenight.css out.mp4 --sync-with fixtures/schedule-hell.midi --audio fixtures/schedule-hell.flac --grid-size 16x10 --resolution 1920 {{args}}

example-image args='':
    ./shapemaker image --colors colorschemes/palenight.css out.svg   {{args}}
