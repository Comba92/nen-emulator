use crate::ppu::Ppu;

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
    let tile = &ppu.patterns[tile_start..tile_start+16];

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
    let sprite = OamEntry::from_bytes(bytes);
    
    let spr_ptrntbl = ppu.ctrl.spr_ptrntbl_addr() as usize;
    let tile_start = spr_ptrntbl + (sprite.tile_id as usize) * 16;
    let tile = &ppu.patterns[tile_start..tile_start+16];
    let palette = &ppu.palettes[sprite.palette_id..sprite.palette_id+4];
    
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

#[derive(Debug, PartialEq, Eq, Default, Clone, Copy)]
pub enum SpritePriority { Front, #[default] Behind, Background }
#[derive(Debug, Default, Clone, Copy)]
pub struct OamEntry {
  pub y: usize,
  pub tile_id: u8,
  pub palette_id: usize,
  pub priority: SpritePriority,
  pub flip_horizontal: bool,
  pub flip_vertical: bool,
  pub x: usize,
}
impl OamEntry {
  pub fn from_bytes(bytes: &[u8]) -> Self {
    let y = bytes[0] as usize;
    let tile = bytes[1];
    let attributes = bytes[2];
    let palette = 16 + (attributes & 0b11) as usize * 4;
    let priority  = match (attributes >> 5) & 1 == 0 {
      false => SpritePriority::Front,
      true => SpritePriority::Behind,
    };
    let flip_horizontal = attributes >> 6 & 1 != 0;
    let flip_vertical = attributes >> 7 & 1 != 0;

    let x = bytes[3] as usize;

    Self {
      y, tile_id: tile, palette_id: palette, priority, flip_horizontal, flip_vertical, x,
    }
  }
}