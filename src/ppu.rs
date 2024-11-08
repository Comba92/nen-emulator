use std::fmt;

use crate::cart::{Cart, NametblMirroring};
use bitflags::bitflags;
use log::{error, info, warn};

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
    const spr_0hit     = 0b0100_0000;
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


#[derive(Debug)]
pub enum WriteLatch {
  FirstWrite, SecondWrite
}

pub enum VramDst {
  Patterntbl, Nametbl, Palettes, Unused
}

pub enum SpritePriority { Front, Behind }
pub struct Sprite {
  pub index: u8,
  pub y: u8,
  pub tile: u8,
  pub palette: usize,
  pub priority: SpritePriority,
  pub flip_horizontal: bool,
  pub flip_vertical: bool,
  pub x: u8,
}

pub struct AttributeEntry {
  pub top_left: u8,
  pub top_right: u8,
  pub btm_left: u8,
  pub btm_right: u8,
}

pub struct PatternEntry {
  pub backdrop: u8,
  pub colors: [u8; 3]
}

pub struct Ppu {
  pub v: u16, // current vram address
  pub t: u16, // temporary vram address / topleft onscreen tile
  pub x: u8, // Fine X Scroll
  pub w: WriteLatch, // First or second write toggle
  pub data_buf: u8,
  pub oam_addr: u8,
  
  pub ctrl: PpuCtrl,
  pub mask: PpuMask,
  pub stat: PpuStat,
  pub patterns: [u8; 0x2000], 
  pub vram: [u8; 0x1000],
  pub palettes: [u8; 0x20],
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
  pub fn new(cart: &Cart) -> Self {
    let mut ppu = Self {
      v: 0, t: 0, x: 0, w: WriteLatch::FirstWrite, 
      patterns: [0; 0x2000],
      vram: [0; 0x1000], 
      palettes: [0; 0x20],
      oam: [0; 256],
      oam_addr: 0, data_buf: 0,
      scanline: 0, cycles: 21,
      ctrl: PpuCtrl::empty(),
      mask: PpuMask::empty(),
      stat: PpuStat::empty(),
      mirroring: cart.header.nametbl_mirroring,

      nmi_requested: false,
      vblank_started: false,
    };

    let (left, _) = ppu.patterns.split_at_mut(cart.chr_rom.len());
    left.copy_from_slice(&cart.chr_rom);
    ppu
  }

  pub fn step(&mut self, cycles: usize) {
    self.cycles += cycles;

    if self.cycles >= 341 {
      self.cycles -= 341;
      self.scanline += 1;

      // Post-render scanline (240)

      // Vertical blanking lines (241-260)
      if self.scanline == 241 {
        info!("VBLANK!!");
        self.vblank_started = true;
        self.stat.insert(PpuStat::vblank);

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
        self.t = self.t | ((self.ctrl.bits() as u16) << 11);
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
            
            self.w = WriteLatch::SecondWrite;
          }
          WriteLatch::SecondWrite => {
            let low = val & 0b0000_0111;
            let high = val & 0b1111_1000;
            let res = ((low as u16) << 13) | ((high as u16) << 6);
            self.t = self.t | res;

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
        let palette = (addr - 0x3F00) % 0x20;
        (VramDst::Palettes, palette as usize)
      }
      _ => (VramDst::Unused, 0), 
    }
  }

  pub fn increase_vram_address(&mut self) {
    self.v = self.v.wrapping_add(self.ctrl.vram_addr_incr());
  }

  pub fn vram_read(&mut self) -> u8 {
    info!("PPU_DATA READ at {:04X}", self.v);

    let res = self.data_buf;
    let (dst, addr) = self.map(self.v);
    let data = match dst {
      VramDst::Patterntbl => self.patterns[addr],
      VramDst::Nametbl => self.vram[addr],
      VramDst::Palettes => self.palettes[addr],
      VramDst::Unused => 0,
    };

    self.data_buf = data;
    self.increase_vram_address();
    res
  }

  // pub fn vram_read(&mut self) -> u8 {
  //   info!("PPU_DATA READ at {:04X}", self.v);

  //   let res = self.data_buf;
  //   let addr = if (0x2000..=0x2FFF).contains(&self.v){
  //     self.v & 0x2FFF
  //   } else { self.v };

  //   let data = self.vram[addr as usize];
  //   self.data_buf = data;
  //   self.v = self.v.wrapping_add(self.ctrl.vram_addr_incr());
  //   res
  // }

  pub fn vram_write(&mut self, val: u8) {
    info!("PPU_DATA WRITE at {:04X} = {val:02X}", self.v);

    let (dst, addr) = self.map(self.v);
    match dst {
      VramDst::Patterntbl => error!("Illegal write to CHR_ROM"),
      VramDst::Nametbl => self.vram[addr] = val,
      VramDst::Palettes => self.palettes[addr] = val,
      VramDst::Unused => {}
    }

    self.increase_vram_address();
  }

  // pub fn vram_write(&mut self, val: u8) {
  //   info!("PPU_DATA WRITE at {:04X} = {val:02X}", self.v);

  //   if (0..=0x1FFF).contains(&self.v) {
  //     warn!("Can't write to CHR");
  //     return;
  //   }

  //   let addr = if (0x2000..=0x2FFF).contains(&self.v){
  //     self.v & 0x2FFF
  //   } else { self.v };

  //   self.vram[addr as usize] = val;
  //   self.v = self.v.wrapping_add(self.ctrl.vram_addr_incr());
  // }

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