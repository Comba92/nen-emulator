#![allow(dead_code)]
use std::rc::Rc;

use bitflags::bitflags;

use super::bus::Bus;

// +--------------------+ $10000
// |     Mirrors        |
// |  $0000 ~ $3fff     |
// +--------------------+ $4000
// |     Mirrors        |
// |  $3f00 ~ $3fff     |
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


const VRAM_SIZE: usize = 0x0800; // 2KB

const PATTERNS_START: usize = 0x0000;
const PATTERNS_END: usize = 0x1FFF;

const NAMES_START: usize = 0x2000;
const NAMES_END: usize = 0x3EFF;

const PALETTES_START: usize = 0x3F00;
const PALETTES_SIZE: usize = 0x20;
const PALETTES_MIRRORS_END: usize = 0x3FFF;

const PPU_MIRRORS_START: usize = 0x4000;

pub const PPU_CTRL: u16 = 0x2000;
pub const PPU_MASK: u16 = 0x2001;
pub const PPU_STAT: u16 = 0x2002;
pub const OAM_ADDR: u16 = 0x2003;
pub const OAM_DATA: u16 = 0x2004;
pub const PPU_SCROLL: u16 = 0x2005;
pub const PPU_ADDR: u16 = 0x2006;
pub const PPU_DATA: u16 = 0x2007;
pub const OAM_DMA: u16 = 0x4014;

const VISIBLE_SCANLINES: usize = 240;
const VERTICAL_OVERSCAN: usize = 261;
const SCANLINE_PIXELS: usize = 256;
const HORIZONTAL_OVERSCAN: usize = 341;

bitflags! {
  pub struct PpuCtrl: u8 {
    const base_nametbl  = 0b0000_0011;
    const vram_incr     = 0b0000_0100;
    const spr_ptrntbl   = 0b0000_1000;

    const bg_ptrntbl    = 0b0001_0000;
    const spr_size      = 0b0010_0000;
    const master_slave  = 0b0100_0000;
    const vblank_nmi_on = 0b1000_0000;
  }

  pub struct PpuMask: u8 {
    const greyscale   = 0b0000_0001;
    const bg_lstrip   = 0b0000_0010;
    const spr_lstrip  = 0b0000_0100;
    const bg_render   = 0b0000_1000;

    const spr_render  = 0b0001_0000;
    const red_boost   = 0b0010_0000;
    const blue_boost  = 0b0100_0000;
    const green_boost = 0b1000_0000;
  }

  pub struct PpuStat: u8 {
    const open_bus     = 0b0001_1111;
    const spr_overflow = 0b0010_0000;
    const spr_0hit     = 0b0100_0000;
    const vblank       = 0b1000_0000;
  }
}

struct OAMEntry {
  y: u8,
  tile: u8,
  attribute: u8,
  x: u8,
}

pub struct Ppu {
  pub vram: [u8; VRAM_SIZE],
  pub oam: [u8; 256],
  pub bus: Rc<Bus>,

  v: u8,
  t: u8,
  x: u8,
  w: u8,
  pub cycles: usize,
  pub scanline: usize,
}

impl Ppu {
  pub fn new(bus: Rc<Bus>) -> Self {
    Self { 
      vram: [0; VRAM_SIZE], oam: [0; 256], bus, 
      v: 0, t:0, x: 0, w: 0, cycles: 21, scanline: 0
    }
  }

  pub fn step(&mut self, steps: usize) {
    self.cycles = self.cycles.wrapping_add(steps);

    if self.cycles >= HORIZONTAL_OVERSCAN {
      self.cycles -= HORIZONTAL_OVERSCAN;
      self.scanline = self.scanline.wrapping_add(1);
    }

    if (0..VISIBLE_SCANLINES).contains(&self.scanline) {
      // drawing here
    } else if self.scanline == VISIBLE_SCANLINES {
      // send VBlank NMI
      self.bus.send_nmi();
    }

    if self.scanline >= VERTICAL_OVERSCAN {
      self.scanline = 0;
    }
  }

  pub fn ctrl(&self) -> PpuCtrl { PpuCtrl::from_bits_retain(self.bus.read(PPU_CTRL)) }
  pub fn set_ctrl(&self, val: u8) {
    self.bus.write(PPU_CTRL, val);
  }
  pub fn mask(&self) -> PpuMask { PpuMask::from_bits_retain(self.bus.read(PPU_MASK)) }
  pub fn set_mask(&self, _val: u8) {
    todo!("set ppu mask")
  }
  pub fn stat(&self) -> PpuStat { PpuStat::from_bits_retain(self.bus.read(PPU_STAT)) }
  pub fn set_stat(&self, val: u8) {
    self.bus.write(PPU_STAT, val);
  }
}