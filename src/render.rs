use std::sync::LazyLock;
use sdl2::pixels::Color;

use crate::ppu::{Ppu, PpuMask};

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

pub const SCREEN_WIDTH: usize = 32;
pub const SCREEN_HEIGHT: usize = 30;

pub struct Tile<'a> {
  pub palette: &'a [u8],
  pub pixels: &'a [u8],
  pub x: usize,
  pub y: usize,
  pub priority: SpritePriority,
  pub flip_horizontal: bool,
  pub flip_vertical: bool,
}
impl<'a> Tile<'a> {
  pub fn bg_sprite_from_idx(i: usize, ppu: &'a Ppu) -> Self {
    let x = i % (SCREEN_WIDTH);
    let y = i / (SCREEN_WIDTH);
    
    let tile_idx = ppu.vram[i] as usize;
    let bg_ptrntbl = ppu.ctrl.bg_ptrntbl_addr() as usize;
    let tile_start = bg_ptrntbl + tile_idx * 16;
    let tile = &ppu.chr[tile_start..tile_start+16];

    let attribute_idx = (y/4 * 8) + (x/4);
    let attribute_addr = (0x2000 + 0x3C0 + attribute_idx) as u16;
    let attribute = ppu.vram_peek(attribute_addr);

    let palette_id = match (x % 4, y % 4) {
      (0..2, 0..2) => (attribute & 0b0000_0011) >> 0 & 0b11,
      (2..4, 0..2) => (attribute & 0b0000_1100) >> 2 & 0b11,
      (0..2, 2..4) => (attribute & 0b0011_0000) >> 4 & 0b11,
      (2..4, 2..4) => (attribute & 0b1100_0000) >> 6 & 0b11,
      _ => unreachable!("mod 4 should always give value smaller than 4"),
    } as usize * 4;
    let palette = &ppu.palettes[palette_id..palette_id+4];

    Self {
      x: x*8, y: y*8, 
      pixels: tile, 
      palette, 
      priority: SpritePriority::Background, 
      flip_horizontal: false,
      flip_vertical: false
    }
  }

  pub fn oam_sprite_from_idx(i: usize, ppu: &'a Ppu) -> Self {
    let bytes = &ppu.oam[i..i+4];
    let sprite = OamEntry::from_bytes(bytes, i);
    
    let spr_ptrntbl = ppu.ctrl.spr_ptrntbl_addr() as usize;
    let tile_start = spr_ptrntbl + (sprite.tile_id as usize) * 16;
    let tile = &ppu.chr[tile_start..tile_start+16];
    let palette_id = sprite.palette_id as usize;
    let palette = &ppu.palettes[palette_id..palette_id+4];
    
    Self {
      x: sprite.x as usize,
      y: sprite.y as usize,
      pixels: tile, palette,
      priority: sprite.priority,
      flip_horizontal: sprite.flip_horizontal,
      flip_vertical: sprite.flip_vertical,
    }
  }
}

// TODO: eventually move out to ppu crate
#[derive(Debug, PartialEq, Eq, Default, Clone, Copy)]
pub enum SpritePriority { Front, #[default] Behind, Background }
#[derive(Debug, Default, Clone, Copy)]
pub struct OamEntry {
  pub index: usize,
  pub y: usize,
  pub tile_id: u8,
  pub palette_id: u8,
  pub priority: SpritePriority,
  pub flip_horizontal: bool,
  pub flip_vertical: bool,
  pub x: usize,
}
impl OamEntry {
  pub fn from_bytes(bytes: &[u8], index: usize) -> Self {
    let y = bytes[0] as usize;
    let tile = bytes[1];
    let attributes = bytes[2];
    let palette = 16 + (attributes & 0b11) * 4;
    let priority  = match (attributes >> 5) & 1 != 0 {
      false => SpritePriority::Front,
      true => SpritePriority::Behind,
    };
    let flip_horizontal = attributes >> 6 & 1 != 0;
    let flip_vertical = attributes >> 7 & 1 != 0;

    let x = bytes[3] as usize;

    Self {
      index, y, tile_id: tile, palette_id: palette, priority, flip_horizontal, flip_vertical, x,
    }
  }
}