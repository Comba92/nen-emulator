native:
	cargo build -r --bin native --features="sdl"
static:
	cargo build -r --bin native --features="sdl-static"

web:
	wasm-pack build --target web --out-dir "./www/pkg/"

run-web:
	make web && simple-http-server

run:
	cargo run -r --bin native --features="sdl"