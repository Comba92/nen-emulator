use std::rc::Rc;

use super::bus::Bus;

// +--------------------+ $10000
// |     Mirrors        |
// |  $0000 ~ $3fff     |
// +--------------------+ $4000
// |     Mirrors        |
// |  $3f00 ~ $3f1f     |
// +--------------------+ $3f20
// | sprite palette     |
// +--------------------+ $3f10
// |  image palette     |
// +--------------------+ $3f00
// |     Mirrors        |
// |  $2000 ~ $2eff     |
// +--------------------+ $3000
// |  attribute table 3 |
// +--------------------+ $2fc0
// |      name  table 3 |
// +--------------------+ $2c00
// |  attribute table 2 |
// +--------------------+ $2bc0
// |      name  table 2 |
// +--------------------+ $2800
// |  attribute table 1 |
// +--------------------+ $27c0
// |      name  table 1 |
// +--------------------+ $2400
// |  attribute table 0 |
// +--------------------+ $23c0
// |      name  table 0 |
// +--------------------+ $2000
// |   pattern table 1  |
// +--------------------+ $1000
// |   pattern table 0  |
// +--------------------+ $0000


const PIXELS_PER_ROW: usize = 240;

const PPU_MEM_SIZE: usize = 0x4000; // 16KB
const VRAM_SIZE: usize = 0x0800; // 2KB

const PATTERNS_START: usize = 0x0000;
const PATTERNS_END: usize = 0x1FFF;

const NAMES_START: usize = 0x2000;
const NAMES_END: usize = 0x3EFF;

const PALETTES_START: usize = 0x3F00;
const PALETTES_END: usize = 0x3FFF;

const PPU_MIRRORS_START: usize = 0x4000;

struct OAMEntry {
  y: u8,
  tile: u8,
  attribute: u8,
  x: u8,
}

pub struct Ppu {
  vram: [u8; VRAM_SIZE],
  oam: [u8; 256],
  bus: Rc<Bus>,
}