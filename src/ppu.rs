use std::fmt;

use crate::{cart::NametblMirroring, mapper::CartMapper, renderer::SCREEN_WIDTH};
use bitflags::bitflags;
use log::{info, warn};

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
    const greyscale     = 0b0000_0001;
    const bg_lstrip     = 0b0000_0010;
    const spr_lstrip    = 0b0000_0100;
    const bg_render_on  = 0b0000_1000;

    const spr_render_on = 0b0001_0000;
    const red_boost     = 0b0010_0000;
    const blue_boost    = 0b0100_0000;
    const green_boost   = 0b1000_0000;
  }

  #[derive(Debug)]
  pub struct PpuStat: u8 {
    const open_bus     = 0b0001_1111;
    const spr_overflow = 0b0010_0000;
    const spr0_hit     = 0b0100_0000;
    const vblank       = 0b1000_0000;
  }
}

impl PpuCtrl {
  pub fn base_nametbl_addr(&self) -> u16 {
    let nametbl_idx = self.bits() & PpuCtrl::base_nametbl.bits();
    0x2000 + 0x0400*nametbl_idx as u16
  }

  pub fn vram_addr_incr(&self) -> u16 {
    match self.contains(PpuCtrl::vram_incr) {
      false => 1,
      true  => 32
    }
  }
  pub fn spr_ptrntbl_addr(&self) -> u16 {
    match self.contains(PpuCtrl::spr_ptrntbl) {
      false => 0x0000,
      true  => 0x1000
    }
  }
  pub fn bg_ptrntbl_addr(&self) -> u16 {
    match self.contains(PpuCtrl::bg_ptrntbl) {
      false => 0x0000,
      true  => 0x1000
    }
  }
}
impl PpuMask {
  pub fn is_render_on(&self) -> bool {
    self.contains(PpuMask::bg_render_on) &&
    self.contains(PpuMask::spr_render_on)
  }
}

#[derive(Debug)]
pub enum WriteLatch {
  FirstWrite, SecondWrite
}

pub enum VramDst {
  Patterntbl, Nametbl, Palettes, Unused
}

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
      _ => unreachable!("mod 2 should always give 0 and 1"),
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

#[derive(Debug, PartialEq, Eq)]
pub enum SpritePriority { Front, Behind, Background }
#[derive(Debug)]
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

pub struct Ppu {
  pub v: u16, // current vram address
  pub t: u16, // temporary vram address / topleft onscreen tile
  pub x: u8, // Fine X Scroll
  pub w: WriteLatch, // First or second write toggle
  pub data_buf: u8,
  
  pub ctrl: PpuCtrl,
  pub mask: PpuMask,
  pub stat: PpuStat,
  pub scroll: (u8, u8),
  pub oam_addr: u8,
  
  pub mapper: CartMapper,
  pub patterns: Vec<u8>,
  pub vram: [u8; 0x1000],
  pub palettes: [u8; 32],
  pub oam: [u8; 256],
  
  pub scanline: usize,
  pub cycles: usize,
  pub mirroring: NametblMirroring,

  pub nmi_requested: bool,
  pub vblank_started: bool,
}

impl fmt::Debug for Ppu {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Ppu").field("v", &self.v).field("t", &self.t).field("x", &self.x).field("w", &self.w).field("data_buf", &self.data_buf).field("oam_addr", &self.oam_addr).field("ctrl", &self.ctrl).field("mask", &self.mask).field("stat", &self.stat).field("scanline", &self.scanline).field("cycles", &self.cycles).field("nmi_requested", &self.nmi_requested).field("mirroring", &self.mirroring).finish()
    }
}

impl Ppu {
  pub fn new(chr_rom: Vec<u8>, mapper: CartMapper, mirroring: NametblMirroring) -> Self {
    let ppu = Self {
      v: 0, t: 0, x: 0, w: WriteLatch::FirstWrite, 
      patterns: chr_rom,
      mapper,
      vram: [0; 0x1000], 
      palettes: [0; 32],
      oam: [0; 256],
      oam_addr: 0, data_buf: 0,
      scanline: 0, cycles: 21,
      ctrl: PpuCtrl::empty(),
      mask: PpuMask::empty(),
      stat: PpuStat::empty(),
      scroll: (0, 0),
      mirroring,

      nmi_requested: false,
      vblank_started: false,
    };

    ppu
  }

  pub fn step(&mut self, cycles: usize) {
    self.cycles += cycles;

    if self.is_spr0_hit() {
      self.stat.insert(PpuStat::spr0_hit);
    }

    if self.cycles >= 341 {
      self.cycles -= 341;
      self.scanline += 1;

      // Post-render scanline (240)

      // Vertical blanking lines (241-260)
      if self.scanline == 241 {
        info!("VBLANK!!");
        self.vblank_started = true;
        self.stat.insert(PpuStat::vblank);
        self.stat.remove(PpuStat::spr0_hit);

        if self.ctrl.contains(PpuCtrl::vblank_nmi_on) {
          self.nmi_requested = true;
        }

      // Pre-render scanline (261)
      } else if self.scanline > 260 {
        self.stat = PpuStat::empty();
        self.oam_addr = 0;
        self.nmi_requested = false;
        self.scanline = 0;
        self.cycles = 0;
      }
    }
  }

  pub fn is_spr0_hit(&self) -> bool {
    let spr0_y = self.oam[0] as usize;
    let spr0_x = self.oam[3] as usize;

    spr0_y == self.scanline &&
    spr0_x <= self.cycles &&
    self.mask.contains(PpuMask::spr_render_on)
  }
  
  pub fn reg_read(&mut self, addr: u16) -> u8 {
    match addr {
      0x2002 => {
        let old_stat = self.stat.bits();
        self.w = WriteLatch::FirstWrite;
        self.stat.remove(PpuStat::vblank);
        old_stat
      }
      0x2004 => self.oam[self.oam_addr as usize],
      0x2007 => self.vram_read(),
      _ => { info!("PPU REG_R {addr:04X} not implemented"); 0 }
    }
  }

  pub fn reg_write(&mut self, addr: u16, val: u8) {
    match addr {
      0x2000 => {
        let was_nmi_off = !self.ctrl.contains(PpuCtrl::vblank_nmi_on);
        self.ctrl = PpuCtrl::from_bits_retain(val);
        self.t = self.t | (((val & 0b11) as u16) << 11);
        if was_nmi_off 
        && self.ctrl.contains(PpuCtrl::vblank_nmi_on) 
        && self.stat.contains(PpuStat::vblank) {
          self.nmi_requested = true;
        }
      }
      0x2001 => self.mask = PpuMask::from_bits_retain(val),
      0x2003 => self.oam_addr = val,
      0x2004 => {
        self.oam[self.oam_addr as usize] = val;
        self.oam_addr = self.oam_addr.wrapping_add(1);
      }
      0x2005 => {
        match self.w {
          WriteLatch::FirstWrite => {
            self.t = self.t | (val & 0b1111_1000) as u16;
            self.x = val & 0b0000_0111;
            
            self.scroll.0 = val;
            self.w = WriteLatch::SecondWrite;
          }
          WriteLatch::SecondWrite => {
            let low = val & 0b0000_0111;
            let high = val & 0b1111_1000;
            let res = ((low as u16) << 13) | ((high as u16) << 6);
            self.t = self.t | res;

            self.scroll.1 = val;
            self.w = WriteLatch::FirstWrite;
          }
        }
      }
      0x2006 => {
        info!("PPU_ADDR WRITE {:?} {val:02X}", self.w);
        match self.w {
          WriteLatch::FirstWrite => {
            self.t = (val as u16) << 8;
            // cut bit 14 and 15
            self.t = self.t & 0x3FFF;
            self.w = WriteLatch::SecondWrite;
          }
          WriteLatch::SecondWrite => { 
            self.t = self.t | (val as u16);
            self.v = self.t;
            self.w = WriteLatch::FirstWrite;
            warn!("V addr is {:04X}", self.v);
          }
        }
      }
      0x2007 => self.vram_write(val),
      _ => info!("PPU REG_W {addr:04X} not implemented"),
    }
  }

  pub fn oam_dma(&mut self, page: &[u8]) {
    self.oam.copy_from_slice(page);
  }

  pub fn map(&self, addr: u16) -> (VramDst, usize) {
    match addr {
      0x0000..=0x1FFF => (VramDst::Patterntbl, addr as usize),
      0x2000..=0x2FFF => {
        let mirrored = self.mirror_nametbl(addr);
        (VramDst::Nametbl, mirrored as usize)
      }
      0x3F00..0x3FFF => {
        let palette = (addr - 0x3F00) % 32;
        (VramDst::Palettes, palette as usize)
      }
      _ => (VramDst::Unused, 0), 
    }
  }

  pub fn increase_vram_address(&mut self) {
    self.v = self.v.wrapping_add(self.ctrl.vram_addr_incr());
  }

  pub fn vram_peek(&self, addr: u16) -> u8 {
    let (dst, addr) = self.map(addr);
    match dst {
      VramDst::Patterntbl => self.mapper.as_ref().borrow()
        .read_chr(&self.patterns, addr),
      VramDst::Nametbl => self.vram[addr],
      VramDst::Palettes => self.palettes[addr],
      VramDst::Unused => 0,
    }
  }

  pub fn vram_read(&mut self) -> u8 {
    info!("PPU_DATA READ at {:04X}", self.v);

    let res = self.data_buf;
    self.data_buf = self.vram_peek(self.v);
    self.increase_vram_address();
    res
  }

  pub fn vram_write(&mut self, val: u8) {
    info!("PPU_DATA WRITE at {:04X} = {val:02X}", self.v);

    let (dst, addr) = self.map(self.v);
    match dst {
      VramDst::Patterntbl => self.mapper.as_ref().borrow_mut()
        .write_chr(addr, val),
      VramDst::Nametbl => self.vram[addr] = val,
      VramDst::Palettes => self.palettes[addr] = val,
      VramDst::Unused => {}
    }

    self.increase_vram_address();
  }

  pub fn mirror_nametbl(&self, addr: u16) -> u16 {
    let addr = addr - 0x2000;
    let nametbl_idx = addr / 0x400;
    
    use NametblMirroring::*;
    let res = match (self.mirroring, nametbl_idx) {
      (Horizontally, 1) | (Horizontally, 2) => addr - 0x400,
      (Horizontally, 3) => addr - 0x400*2,
      (Vertically, 2) | (Vertically, 3) => addr - 0x400*2,
      (_, _) => addr,
    };

    res
  }
}