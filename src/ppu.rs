use std::fmt;

use crate::cart::{Cart, NametblMirroring};
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

#[derive(Debug)]
pub enum WriteLatch {
  FirstWrite, SecondWrite
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
  pub vram: [u8; 0x5000],
  pub oam: [u8; 256],
  pub scanline: usize,
  pub cycles: usize,
  pub nmi_requested: bool,
  pub mirroring: NametblMirroring,
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
      vram: [0; 0x5000], oam: [0; 256],
      oam_addr: 0, data_buf: 0,
      scanline: 0, cycles: 21,
      ctrl: PpuCtrl::empty(),
      mask: PpuMask::empty(),
      stat: PpuStat::empty(),
      nmi_requested: false,
      mirroring: cart.header.nametbl_mirroring
    };

    let (left, _) = ppu.vram.split_at_mut(cart.chr_rom.len());
    left.copy_from_slice(&cart.chr_rom);
    ppu
  }

  pub fn step(&mut self, cycles: usize) {
    self.cycles += cycles;

    if self.cycles >= 341 {
      self.cycles -= 341;
      self.scanline += 1;

      // Post-render scanline (240)
      // Vertical blanking lines (240-260)
      if self.scanline == 241 {
        warn!("VBLANK!!");
        self.stat.insert(PpuStat::vblank);
        if self.ctrl.contains(PpuCtrl::vblank_nmi_on) {
          self.nmi_requested = true;
        }
      // Pre-render scanline (261)
      } else if self.scanline > 260 {
        self.stat = PpuStat::empty();
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
            self.w = WriteLatch::SecondWrite;
          }
          WriteLatch::SecondWrite => { 
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

  pub fn vram_read(&mut self) -> u8 {
    info!("PPU_DATA READ at {:04X}", self.v);

    let res = self.data_buf;
    let addr = if (0x2000..=0x2FFF).contains(&self.v){
      self.v & 0x2FFF
    } else { self.v };

    let data = self.vram[addr as usize];
    self.data_buf = data;
    self.v = self.v.wrapping_add(self.ctrl.vram_addr_incr());
    res
  }

  pub fn vram_write(&mut self, val: u8) {
    info!("PPU_DATA WRITE at {:04X} = {val:02X}", self.v);

    if (0..=0x1FFF).contains(&self.v) {
      warn!("Can't write to CHR");
      return;
    }

    let addr = if (0x2000..=0x2FFF).contains(&self.v){
      self.v & 0x2FFF
    } else { self.v };

    self.vram[addr as usize] = val;
    self.v = self.v.wrapping_add(self.ctrl.vram_addr_incr());
  }

  pub fn mirror_nametbl(&self, addr: u16) -> u16 {
    let addr = (addr & 0x2FFF) - 0x2000;
    let nametbl_idx = addr / 0x400;
    
    use NametblMirroring::*;
    let res = match (self.mirroring, nametbl_idx) {
      (Horizontally, 1) | (Horizontally, 2) => addr - 0x400,
      (Horizontally, 3) => addr - 0x400*2,
      (Vertically, 2) | (Vertically, 3) => addr - 0x400*2,
      (_, _) => addr,
    };

    res + 0x2000
  }
}