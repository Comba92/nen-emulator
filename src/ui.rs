use sdl2::{event::Event, pixels::{Color, PixelFormatEnum}, render::{Canvas, Texture, TextureCreator}, video::{Window, WindowContext}, EventPump, Sdl, VideoSubsystem};
pub struct Sdl2Context {
    pub ctx: Sdl,
    pub video: VideoSubsystem,
    pub canvas: Canvas<Window>,
    pub events: EventPump,
    pub texture_creator: TextureCreator<WindowContext>
}

pub struct FrameBuffer {
    pub buffer: Vec<u8>,
    pub width: usize,
    pub height: usize,
}
impl FrameBuffer {
    pub fn new(width: usize, height: usize) -> Self {
        let buffer = vec![0; width * height * 3];
        Self { buffer, width, height }
    }

    pub fn pitch(&self) -> usize {
        self.width * 3
    }

    pub fn set_pixel(&mut self, x: usize, y: usize, color: Color) {
        let (r, g, b) = color.rgb();
        let idx = (y*self.width + x) * 3;
        self.buffer[idx + 0] = r;
        self.buffer[idx + 1] = g;
        self.buffer[idx + 2] = b;
    }

    pub fn set_tile(&mut self, x: usize, y: usize, tile: Tile) {
        for row in 0..8 {
            for col in 0..8 {
                let color = tile[row][col];
                self.set_pixel(x+col, y+row, color);
            }
        }
    }
}

impl Sdl2Context {
    pub fn new(name: &str, width: u32, height: u32) -> Self {
        let ctx = sdl2::init().expect("Couldn't initialize SDL2");
        let video= ctx.video().expect("Couldn't initialize video subsystem");
        let canvas = video.window(name, width, height)
            .position_centered()
            .build().expect("Couldn't initialize window")
            .into_canvas()
            .accelerated().present_vsync()
            .build().expect("Couldn't initialize drawing canvas");
        let events = ctx.event_pump().expect("Couldn't get the event pump");
        let texture_creator = canvas.texture_creator();

        Self { ctx, video, canvas, events, texture_creator }
    }

    pub fn new_texture<'a>(&'a self, width: u32, height: u32) -> Texture<'a> {
        self.texture_creator
            .create_texture_target(PixelFormatEnum::RGB24, width, height)
            .expect("Could not create a texture")
    }
}

const GREYSCALE_PALETTE: [Color; 4] = [
    Color::BLACK,
    Color::RGB(123, 123, 123),
    Color::RGB(191, 191, 191),
    Color::RGB(238, 238, 238),
];

type Tile = [[Color; 8]; 8];
pub fn parse_tile(tile: &[u8]) -> Tile {
    let mut sprite = [[Color::BLACK; 8]; 8];

    for row in 0..8 {
        let plane0 = tile[row];
        let plane1 = tile[row + 8];

        for bit in (0..8).rev() {
            let bit0 = (plane0 >> bit) & 1;
            let bit1 = ((plane1 >> bit) & 1) << 1;
            let color = bit1 | bit0;

            sprite[row][7-bit] = GREYSCALE_PALETTE[color as usize];
        }
    }

    sprite
}

pub fn run() {
    let mut sdl = Sdl2Context::new("NenEmulator", 800, 600);
    sdl.canvas.set_scale(10.0, 10.0).unwrap();
    let mut _texture = sdl.texture_creator
    .create_texture_target(PixelFormatEnum::RGB24, 800, 600).unwrap();

    'running: loop {
        for event in sdl.events.poll_iter() {
            match event {
                Event::Quit { .. } => break 'running,
                _ => {}
            }
        }
    }
}
