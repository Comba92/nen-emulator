use std::sync::LazyLock;

use sdl2::{event::Event, keyboard::Keycode, pixels::Color, render::{Canvas, TextureCreator}, video::{Window, WindowContext}, EventPump, Sdl, TimerSubsystem, VideoSubsystem};

use crate::{dev::{Joypad, JoypadStat}, ppu::{Ppu, PpuMask}, tile::{SpritePriority, Tile, SCREEN_HEIGHT, SCREEN_WIDTH}};

pub static SYS_PALETTES: LazyLock<[Color; 64]> = LazyLock::new(|| {
    let bytes = include_bytes!("../palettes/Composite_wiki.pal");
  
    let colors: Vec<Color> = bytes
      .chunks(3)
      // we take only the first palette set of 64 colors, more might be in a .pal file
      .take(64)
      .map(|rgb| Color::RGB(rgb[0], rgb[1], rgb[2]))
      .collect();
  
    colors.try_into().unwrap()
});

pub const GREYSCALE_PALETTE: [u8; 4] = [0x3F, 0x00, 0x10, 0x20];

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

    pub fn set_pixel(&mut self, x: usize, y: usize, color_id: u8) {
        let color = SYS_PALETTES[color_id as usize];
        let (r, g, b) = color.rgb();
        let idx = (y*self.width + x) * 3;
        self.buffer[idx + 0] = r;
        self.buffer[idx + 1] = g;
        self.buffer[idx + 2] = b;
    }

    pub fn set_tile(&mut self, tile: Tile) {
        for row in 0..8 {
            let plane0 = tile.pixels[row];
            let plane1 = tile.pixels[row + 8];

            let x_start: usize = match tile.flip_horizontal {
                false => 7,
                true => 0,
            };
            let y_start: usize = match tile.flip_vertical {
                false => 0,
                true => 7,
            };

            for bit in 0..8 {
                let bit0 = (plane0 >> bit) & 1;
                let bit1 = ((plane1 >> bit) & 1) << 1;
                let color_idx = bit1 | bit0;

                let x = x_start.abs_diff(bit as usize);
                let y = y_start.abs_diff(row);

                if tile.priority == SpritePriority::Background
                || color_idx != 0 {
                    let color_id = tile.palette[color_idx as usize];
                    self.set_pixel(tile.x + x, tile.y + y, color_id);
                }
            }
        }
    }
}

pub struct NesScreen(pub FrameBuffer);
impl NesScreen {
    pub fn new() -> Self {
        NesScreen(FrameBuffer::new(SCREEN_WIDTH*8, SCREEN_HEIGHT*8))
    }

    pub fn render_background(&mut self, ppu: &Ppu) {
        if !ppu.mask.contains(PpuMask::bg_render_on) { return; }
        
        for i in 0..32*30 {
          let tile = Tile::bg_sprite_from_idx(i, ppu);
          self.0.set_tile(tile);
        }
    }

    pub fn render_sprites(&mut self, ppu: &Ppu) {
        if !ppu.mask.contains(PpuMask::spr_render_on) { return; }

        for i in (0..256).step_by(4).rev() {
            let sprite = Tile::oam_sprite_from_idx(i, ppu);
            if sprite.x >= SCREEN_WIDTH*8 - 8 || sprite.y >= SCREEN_HEIGHT*8 - 8 { continue; }
            self.0.set_tile(sprite);
        }
    }
}

pub struct Sdl2Context {
    pub ctx: Sdl,
    pub video: VideoSubsystem,
    pub timer: TimerSubsystem,
    pub canvas: Canvas<Window>,
    pub events: EventPump,
    pub texture_creator: TextureCreator<WindowContext>
}

impl Sdl2Context {
    pub fn new(name: &str, width: u32, height: u32) -> Self {
        let ctx = sdl2::init().expect("Couldn't initialize SDL2");
        let video= ctx.video().expect("Couldn't initialize video subsystem");
        let window = video.window(name, width, height)
            .position_centered()
            .resizable()
            .build().expect("Couldn't initialize window");
        let canvas = window
            .into_canvas()
            .accelerated() // .present_vsync()
            .build().expect("Couldn't initialize drawing canvas");
        let events = ctx.event_pump().expect("Couldn't get the event pump");
        let timer = ctx.timer().expect("Couldn't initialize timer subsytem");
        let texture_creator = canvas.texture_creator();

        Self { ctx, video, canvas, events, texture_creator, timer }
    }
}

// TODO: this is hideous
pub fn handle_input(event: &Event, joypad: &mut Joypad) {
    match event {
        Event::KeyDown { keycode, .. } => {
            if let Some(keycode) = keycode {
                match *keycode {
                Keycode::Z => joypad.button.insert(JoypadStat::A),
                Keycode::X => joypad.button.insert(JoypadStat::B),
                Keycode::UP => joypad.button.insert(JoypadStat::UP),
                Keycode::DOWN => joypad.button.insert(JoypadStat::DOWN),
                Keycode::LEFT => joypad.button.insert(JoypadStat::LEFT),
                Keycode::RIGHT => joypad.button.insert(JoypadStat::RIGHT),
                Keycode::N => joypad.button.insert(JoypadStat::SELECT),
                Keycode::M => joypad.button.insert(JoypadStat::START),
                _ => {}
                }
            }
        }
        Event::KeyUp { keycode, .. } => {
            if let Some(keycode) = keycode {
                match *keycode {
                Keycode::Z => joypad.button.remove(JoypadStat::A),
                Keycode::X => joypad.button.remove(JoypadStat::B),
                Keycode::UP => joypad.button.remove(JoypadStat::UP),
                Keycode::DOWN => joypad.button.remove(JoypadStat::DOWN),
                Keycode::LEFT => joypad.button.remove(JoypadStat::LEFT),
                Keycode::RIGHT => joypad.button.remove(JoypadStat::RIGHT),
                Keycode::N => joypad.button.remove(JoypadStat::SELECT),
                Keycode::M => joypad.button.remove(JoypadStat::START),
                _ => {}
                }
            }
        }
        _ => {}
    }
}