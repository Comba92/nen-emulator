use std::{path::Path, sync::LazyLock};

use sdl2::{event::Event, keyboard::Keycode, pixels::{Color, PixelFormatEnum}, render::{Canvas, Texture, TextureCreator}, video::{Window, WindowContext}, EventPump, Sdl, VideoSubsystem};

use crate::{cart::Cart, cpu::Cpu, dev::{Joypad, JoypadStat}, ppu::Ppu};

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

pub const SCREEN_WIDTH: usize = 32;
pub const SCREEN_HEIGHT: usize = 30;

pub struct FrameBuffer {
    pub buffer: Vec<u8>,
    pub width: usize,
    pub height: usize,
}
type Tile = [[Color; 8]; 8];
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

    pub fn set_tile(&mut self, x: usize, y: usize, tile: &[u8], palette: &[u8]) {
        let parsed_tile = tile_to_colors(tile, palette);
        for row in 0..8 {
            for col in 0..8 {
                let color = parsed_tile[row][col];
                self.set_pixel(x+col, y+row, color);
            }
        }
    }
}

pub fn tile_to_colors(tile: &[u8], palette: &[u8]) -> Tile {
    let mut sprite = [[Color::BLACK; 8]; 8];

    for row in 0..8 {
        let plane0 = tile[row];
        let plane1 = tile[row + 8];

        for bit in (0..8).rev() {
            let bit0 = (plane0 >> bit) & 1;
            let bit1 = ((plane1 >> bit) & 1) << 1;
            let color = bit1 | bit0;
            let color_id = palette[color as usize] as usize;
            sprite[row][7-bit] = SYS_PALETTES[color_id];
        }
    }

    sprite
}

pub struct NesScreen(FrameBuffer);
impl NesScreen {
    pub fn new() -> Self {
        NesScreen(FrameBuffer::new(SCREEN_WIDTH*8, SCREEN_HEIGHT*8))
    }

    pub fn render_background(&mut self, ppu: &Ppu) {
        let bg_ptrntbl = ppu.ctrl.bg_ptrntbl_addr();
        for i in 0..32*30 {
          let tile_idx = ppu.vram[i];
          let x = i % (self.0.width/8);
          let y = i / (self.0.width/8);

          let tile_start = (bg_ptrntbl as usize) + (tile_idx as usize) * 16;
          let tile = &ppu.patterns[tile_start..tile_start+16];

          let attribute_idx = (y/4 * 8) + (x/4);
          // need to do mirroring here
          let attribute = ppu.vram[0x3C0 + attribute_idx as usize];
          let palette_id = match (x % 4, y % 4) {
            (0..2, 0..2) => (attribute & 0b0000_0011) >> 0 & 0b11,
            (2..4, 0..2) => (attribute & 0b0000_1100) >> 2 & 0b11,
            (0..2, 2..4) => (attribute & 0b0011_0000) >> 4 & 0b11,
            (2..4, 2..4) => (attribute & 0b1100_0000) >> 6 & 0b11,
            _ => unreachable!("mod 2 should always give 0 and 1"),
          } as usize * 4;

          let palette = &ppu.palettes[palette_id..palette_id+4];
          self.0.set_tile(8*x as usize, 8*y as usize, tile, palette);
        }
    }

    pub fn render_sprites(&mut self, ppu: &Ppu) {
        let spr_ptrntbl = ppu.ctrl.spr_ptrntbl_addr();

        // Sprites with lower OAM indices are drawn in front
        for i in (0..256).step_by(4).rev() {
          let tile_idx = ppu.oam[i + 1];
          let x = ppu.oam[i + 3] as usize;
          let y = ppu.oam[i] as usize;
          let tile_start = (spr_ptrntbl as usize) + (tile_idx as usize) * 16;
          let tile = &ppu.patterns[tile_start..tile_start+16];

          self.0.set_tile(8*x as usize, 8*y as usize, tile, &GREYSCALE_PALETTE);
        }
    }
}

pub struct Sdl2Context {
    pub ctx: Sdl,
    pub video: VideoSubsystem,
    pub canvas: Canvas<Window>,
    pub events: EventPump,
    pub texture_creator: TextureCreator<WindowContext>
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

    pub fn new_texture<'a>(&'a self, width: usize, height: usize) -> Texture<'a> {
        self.texture_creator
            .create_texture_target(PixelFormatEnum::RGB24, width as u32, height as u32)
            .expect("Could not create a texture")
    }

    pub fn handle_input(&mut self, joypad: &mut Joypad) -> bool {
        for event in self.events.poll_iter() {
            match event {
              Event::Quit { .. } => return true,
              Event::KeyDown { keycode, .. } => {
                if let Some(keycode) = keycode {
                  match keycode {
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
                  match keycode {
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

        false
    }
}

pub fn run() {
    let mut sdl = Sdl2Context::new("NenEmulator", SCREEN_WIDTH as u32*8, SCREEN_HEIGHT as u32*8);
    
    let mut framebuf = NesScreen::new();
    let mut texture = sdl.texture_creator
    .create_texture_target(PixelFormatEnum::RGB24, framebuf.0.width as u32, framebuf.0.height as u32)
    .unwrap();

    let rom_path = &Path::new("tests/test_roms/Donkey Kong.nes");
    let cart = Cart::new(rom_path);
    let mut emu = Cpu::new(cart);

    'running: loop {
        emu.step_until_vblank();

        for event in sdl.events.poll_iter() {
            match event {
                Event::Quit { .. } => break 'running,
                _ => {}
            }
        }

        framebuf.render_background(&emu.bus.ppu);
        texture.update(None, &framebuf.0.buffer, framebuf.0.pitch()).unwrap();
        sdl.canvas.copy(&texture, None, None).unwrap();
        sdl.canvas.present();
    }
}
