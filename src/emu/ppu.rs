#![allow(dead_code)]

use std::{fmt, rc::Rc};

use bitflags::bitflags;
use log::info;

use super::{bus::{Bus, PPU_REG_START}, cart::NametblMirroring};

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

const PATTERNS_START: u16 = 0x0000;
const PATTERNS_END: u16 = 0x1FFF;

const VRAM_START: u16 = NAMETBLS_START;
const NAMETBLS_START: u16 = 0x2000;
const NAMETBLS_END: u16 = 0x2FFF;

const PALETTES_START: u16 = 0x3F00;
const PALETTES_SIZE: usize = 0x20;
const PALETTES_END: u16 = PALETTES_START - PALETTES_SIZE as u16;
const PALETTES_MIRRORS_END: u16 = 0x3FFF;

const PPU_MIRRORS_START: u16 = 0x4000;

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

const NAMETBL_WIDTH: usize = 30;
const NAMETBL_HEIGHT: usize = 32;
const NAMETBL_SIZE: u16 = 0x400; // 1 KB

bitflags! {
  #[derive(Debug)]
  pub struct PpuCtrl: u8 {
    const base_nametbl  = 0b0000_0011;
    const vram_incr     = 0b0000_0100;
    const spr_ptrntbl   = 0b0000_1000;

    const bg_ptrntbl    = 0b0001_0000;
    const spr_size      = 0b0010_0000;
    const master_slave  = 0b0100_0000;
    const vblank_nmi_on = 0b1000_0000;
  }

  #[derive(Debug)]
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

  #[derive(Debug)]
  pub struct PpuStat: u8 {
    const open_bus     = 0b0001_1111;
    const spr_overflow = 0b0010_0000;
    const spr_0hit     = 0b0100_0000;
    const vblank       = 0b1000_0000;
  }
}

impl PpuCtrl {
  pub fn base_nametbl_addr(&self) -> u16 {
    let nametbl_idx = self.bits() & PpuCtrl::base_nametbl.bits();
    NAMETBLS_START + NAMETBL_SIZE*nametbl_idx as u16
  }
  pub fn spr_ptrntbl_addr(&self) -> u16 {
    match self.contains(PpuCtrl::spr_ptrntbl) {
      false => 0x000,
      true  => 0x1000
    }
  }
  pub fn bg_ptrntbl_addr(&self) -> u16 {
    match self.contains(PpuCtrl::bg_ptrntbl) {
      false => 0x000,
      true  => 0x1000
    }
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

  v: u16,
  t: u16,
  x: u8,
  w: u8,
  pub cycles: usize,
  pub scanline: usize,

  pub ctrl: PpuCtrl,
  pub mask: PpuMask,
  pub stat: PpuStat,

  pub req_state: RequestAddr,
  pub req_addr: u16,
  pub req_buf: u8,

  pub mirroring: NametblMirroring,
  pub bus: Rc<Bus>,
}

#[derive(Debug)]
pub enum RequestAddr {
  Start, AddrHigh(u8)
}

impl fmt::Debug for Ppu {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Ppu").field("v", &self.v).field("t", &self.t).field("x", &self.x).field("w", &self.w).field("cycles", &self.cycles).field("scanline", &self.scanline).field("ctrl", &self.ctrl).field("mask", &self.mask).field("stat", &self.stat).field("req_state", &self.req_state).field("req_addr", &self.req_addr).field("req_buf", &self.req_buf).finish()
    }
}

impl Ppu {
  pub fn new(bus: Rc<Bus>, mirroring: NametblMirroring) -> Self {
    Self {
      vram: [0; VRAM_SIZE], oam: [0; 256],
      v: 0, t:0, x: 0, w: 0, 
      cycles: 21, scanline: 0,
      ctrl: PpuCtrl::empty(), 
      stat: PpuStat::empty(),
      mask: PpuMask::empty(),
      req_state: RequestAddr::Start, req_addr: 0, req_buf: 0,
      mirroring,
      bus,
    }
  }

  pub fn step(&mut self, cycles: usize) {
    self.cycles = self.cycles.wrapping_add(cycles);

    if self.cycles >= HORIZONTAL_OVERSCAN {
      self.cycles -= HORIZONTAL_OVERSCAN;
      self.scanline = self.scanline.wrapping_add(1);
    }

    if (0..VISIBLE_SCANLINES).contains(&self.scanline) {
      // drawing here
    } else if self.scanline == VISIBLE_SCANLINES {
      // send VBlank NMI
      self.send_nmi();
    } else if self.scanline >= VERTICAL_OVERSCAN {
      self.scanline = 0;
    }
  }

  pub fn reg_read(&mut self, addr: u16) -> u8 {
    if [PPU_CTRL, PPU_MASK, OAM_ADDR, PPU_SCROLL, PPU_DATA, OAM_DMA].contains(&addr) {
        info!("Invalid read to write-only PPU register ${addr:04X}");
        return 0;
    }

    match addr {
      PPU_STAT => self.stat.bits(),
      OAM_DATA => todo!("oam_data read"),
      PPU_DATA => self.mem_read(self.req_addr),
      _ => {
        info!("Read to PPU ${addr:04X} not implemented");
        0
      }
    }
  }

  pub fn reg_write(&mut self, addr: u16, val: u8) {
    let reg = addr - PPU_REG_START;
      match reg {
        PPU_CTRL => self.ctrl = PpuCtrl::from_bits_retain(val),
        PPU_MASK => info!("ppu mask write not implemented"),
        PPU_STAT => info!("Invalid write to read-only PPUSTAT register ${reg:04X}"),
        PPU_ADDR => {
          match self.req_state {
              RequestAddr::Start => self.req_state = RequestAddr::AddrHigh(val),
              RequestAddr::AddrHigh(high) => {
                self.req_state = RequestAddr::Start;
                self.req_addr = u16::from_le_bytes([val, high]);
              }
          }
        }
        PPU_DATA => {
          self.mem_write(self.req_addr, val);
        }
        _ => info!("Write to PPU ${addr:04X} not implemented")
      };
  }

  fn next_req_addr(&mut self) {
    match self.ctrl.contains(PpuCtrl::vram_incr) {
        false =>  self.req_addr = (self.req_addr + 1) % PALETTES_MIRRORS_END,
        true => self.req_addr = (self.req_addr + 32) % PALETTES_MIRRORS_END, 
      }
  }

  pub fn send_nmi(&mut self) {
    self.bus.send_nmi();
  }

  pub fn mem_read(&mut self, addr: u16) -> u8 {
    let res = self.req_buf;

    let data = match addr {
      0..=PATTERNS_END => {
        info!("chr rom read");
        0
      },
      NAMETBLS_START..=NAMETBLS_END => {
        let mirrored = self.mirror_nametbl(addr);
        self.vram[mirrored as usize]
      },
      PALETTES_START..=PALETTES_MIRRORS_END => {
        let palette = (addr & PALETTES_END) - PALETTES_START;
        self.vram[palette as usize]
      }
      _ => {
        info!("read to unused vram address ${addr:04X}");
        0
      }
    };

    self.req_buf = data;
    self.next_req_addr();
    res
  }

  pub fn mem_write(&mut self, addr: u16, val: u8) {
    match addr {
      0..=PATTERNS_END => {
        info!("chr rom write");
      },
      NAMETBLS_START..=NAMETBLS_END => {
        let mirrored = self.mirror_nametbl(addr);
        self.vram[mirrored as usize] = val;
      },
      PALETTES_START..=PALETTES_MIRRORS_END => {
        let palette = (addr & PALETTES_END) - PALETTES_START;
        self.vram[palette as usize] = val;
      }
      _ => {
        info!("write to unused vram address ${addr:04X}");
      }
    };

    self.next_req_addr();
  }

  // Horizontal:
  // 0x0800 [ B ]  [ A ] [ a ]
  // 0x0400 [ A ]  [ B ] [ b ]
 
  // Vertical:
  // 0x0800 [ B ]  [ A ] [ B ]
  // 0x0400 [ A ]  [ a ] [ b ]
  pub fn mirror_nametbl(&self, addr: u16) -> u16 {
    let addr = (addr & NAMETBLS_END) - NAMETBLS_START;
    let nametbl_idx = addr / NAMETBL_SIZE;
    
    use NametblMirroring::*;
    match (self.mirroring, nametbl_idx) {
      (Horizontally, 1) | (Horizontally, 2) => addr - NAMETBL_SIZE,
      (Horizontally, 3) => addr - NAMETBL_SIZE*2,
      (Vertically, 2) | (Vertically, 3) => addr - NAMETBL_SIZE*2,
      (_, _) => addr,
    }
  }

  // pub fn ctrl(&self) -> PpuCtrl { PpuCtrl::from_bits_retain(self.bus.read(PPU_CTRL)) }
  // pub fn set_ctrl(&self, val: u8) {
  //   self.bus.write(PPU_CTRL, val);
  // }
  // pub fn mask(&self) -> PpuMask { PpuMask::from_bits_retain(self.bus.read(PPU_MASK)) }
  // pub fn set_mask(&self, _val: u8) {
  //   todo!("set ppu mask")
  // }
  // pub fn stat(&self) -> PpuStat { PpuStat::from_bits_retain(self.bus.read(PPU_STAT)) }
  // pub fn set_stat(&self, val: u8) {
  //   self.bus.write(PPU_STAT, val);
  // }

  // pub fn set_data(&self) {

  // }
}