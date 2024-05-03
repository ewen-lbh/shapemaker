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

example-video out="out.mp4" args='':
    ./shapemaker video --colors colorschemes/palenight.css {{out}} --sync-with fixtures/schedule-hell.midi --audio fixtures/schedule-hell.flac --grid-size 16x10 --resolution 1920 {{args}}

example-image out="out.png" args='':
    ./shapemaker image --colors colorschemes/palenight.css {{out}}   {{args}}
