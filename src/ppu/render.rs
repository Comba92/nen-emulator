use std::collections::VecDeque;

pub(super) struct Renderer {
  pub bg_buf: BgData,
  pub bg_fifo: VecDeque<(u8, u8)>,
  pub oam_buf: Vec<OamEntry>,
  pub scanline_sprites: [Option<SprData>; 32*8],
}

#[derive(Default)]
pub struct BgData {
  pub tile_id: u8,
  pub palette_id: u8,
  pub tile_plane0: u8,
  pub tile_plane1: u8
}

#[derive(Default, Clone)]
pub struct SprData {
  pub pixel: u8,
  pub palette_id: u8,
  pub priority: SpritePriority,
  pub is_sprite0: bool,
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

impl Renderer {
  pub fn new() -> Self {
    Self {
      // TODO: WHY DOES THIS WORK? eventually find out and fix it.
      bg_fifo: VecDeque::from([(0, 0)].repeat(9)),
      bg_buf: BgData::default(),
      oam_buf: Vec::with_capacity(8),
      scanline_sprites: [const { None }; 32*8],
    }
  }


}