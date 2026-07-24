## Native build:
```bash
cargo run -r # for webgpu backend
cargo run -r --features=persistence # for savestates and settings saving
cargo run -r --features=opengl # for opengl backend
```

## WASM Build:
```bash
trunk build --release
```
