use core::fmt;
use std::collections::VecDeque;

use crate::{cart::Mirroring, mapper::CartMapper, frame::{NesScreen, OamEntry, SpritePriority}};
use bitfield_struct::bitfield;
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
  pub fn spr_height(&self) -> usize {
    match self.contains(PpuCtrl::spr_size) {
      false => 8,
      true => 16
    }
  }
}


// https://www.nesdev.org/wiki/PPU_scrolling#PPU_internal_registers
#[bitfield(u16, order = Lsb)]
pub struct LoopyReg {
  #[bits(5)]
  coarse_x: u8,
  #[bits(5)]
  coarse_y: u8,
  #[bits(1)]
  nametbl_x: u8,
  #[bits(1)]
  nametbl_y: u8,
  #[bits(3)]
  fine_y: u8,
  #[bits(1)]
  __: u8,
}
impl LoopyReg {
  pub fn nametbl(&self) -> u8 {
    (self.nametbl_y() << 1) | self.nametbl_x()
  }

  pub fn nametbl_idx(&self) -> u16 {
    ((self.nametbl() as u16) << 10) 
    | ((self.coarse_y() as u16) << 5)
    | (self.coarse_x() as u16)
  }
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


#[derive(Debug)]
pub enum WriteLatch {
  FirstWrite, SecondWrite
}

pub enum VramDst {
  Patterntbl, Nametbl, Palettes, Unused
}

pub struct Ppu {
  pub screen: NesScreen,

  bg_buf: BgData,
  bg_fifo: VecDeque<(u8, u8)>,
  oam_buf: Vec<OamEntry>,
  scanline_sprites: [Option<SprData>; 32*8],

  v: LoopyReg, // current vram address
  t: LoopyReg, // temporary vram address / topleft onscreen tile
  x: u8, // Fine X Scroll
  w: WriteLatch, // First or second write toggle
  data_buf: u8,
  
  pub ctrl: PpuCtrl,
  pub mask: PpuMask,
  pub mask_update_delay: usize,
  pub mask_buf: u8,
  pub stat: PpuStat,
  pub oam_addr: u8,
  
  pub mapper: CartMapper,
  pub chr: Vec<u8>,
  pub vram: [u8; 0x800],
  pub palettes: [u8; 32],
  pub oam: [u8; 256],
  
  pub scanline: usize,
  pub cycle: usize,
  in_odd_frame: bool,

  pub mirroring: Mirroring,
  
  pub nmi_requested: Option<()>,
  pub vblank_started: Option<()>,
}

impl fmt::Debug for Ppu {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Ppu").field("v", &self.v).field("t", &self.t).field("x", &self.x).field("w", &self.w).field("data_buf", &self.data_buf).field("oam_addr", &self.oam_addr).field("ctrl", &self.ctrl).field("mask", &self.mask).field("stat", &self.stat).field("scanline", &self.scanline).field("cycles", &self.cycle).field("nmi_requested", &self.nmi_requested).field("mirroring", &self.mirroring).finish()
    }
}

impl Ppu {
  pub fn new(chr_rom: Vec<u8>, mapper: CartMapper, mirroring: Mirroring) -> Self {
    let mapper_mirroring = mapper.borrow().mirroring();
    
    Self {
      screen: NesScreen::new(),

      // TODO: WHY DOES THIS WORK? eventually find out and fix it.
      bg_fifo: VecDeque::from([(0, 0)].repeat(9)),
      bg_buf: BgData::default(),
      oam_buf: Vec::with_capacity(8),
      scanline_sprites: [const { None }; 32*8],
      
      v: LoopyReg::new(), t: LoopyReg::new(), 
      x: 0, w: WriteLatch::FirstWrite, 

      chr: chr_rom,
      mapper,
      vram: [0; 0x800],
      palettes: [0; 32],
      oam: [0; 256],
      oam_addr: 0, data_buf: 0,
      in_odd_frame: false,
      scanline: 261, cycle: 0,
      ctrl: PpuCtrl::empty(),
      mask: PpuMask::empty(),
      mask_update_delay: 0,
      mask_buf: 0,
      stat: PpuStat::empty(),
      
      mirroring: if let Some(mapper_mirroring) =  mapper_mirroring { mapper_mirroring } else { mirroring },

      nmi_requested: None,
      vblank_started: None,
    }
  }

  pub fn reset(&mut self) { 
    // TODO: better ppu resetting, this works for now
    self.scanline = 0;
    self.cycle = 0;
  }

  pub fn step(&mut self) {
    if (0..=239).contains(&self.scanline) || self.scanline == 261 {
      // visible scanlines 
      if (1..=256).contains(&self.cycle) || (321..=336).contains(&self.cycle) {
        self.fetch_bg_step();
      }
      
      if self.cycle == 64 {
        self.oam_buf.clear();
      } else if self.cycle == 256 {
        self.increase_coarse_y();
        self.evaluate_sprites();
      } else if self.cycle == 257 {
        self.reset_render_x();
        self.scanline_sprites.fill(None);
      } else if self.cycle == 320 && self.scanline != 261 {
        self.fetch_sprites();
      }
    }

    if self.scanline == 241 && self.cycle == 0 {        
      info!("VBLANK!!");
      self.vblank_started = Some(());
      self.stat.insert(PpuStat::vblank);
      self.stat.remove(PpuStat::spr0_hit);

      if self.ctrl.contains(PpuCtrl::vblank_nmi_on) {
        self.nmi_requested = Some(());
      }
    } else if self.scanline == 261 && self.cycle == 1 {
      self.stat = PpuStat::empty();
      self.oam_addr = 0;
    } else if self.scanline == 261 && self.cycle == 304 {
      self.reset_render_y();
    } else if self.scanline < 241 && self.cycle == 260  {
      if self.is_rendering_on() && self.ctrl.contains(PpuCtrl::spr_size) 
      && !self.ctrl.contains(PpuCtrl::bg_ptrntbl)
      && self.ctrl.contains(PpuCtrl::spr_ptrntbl) {
        self.mapper.borrow_mut().scanline_ended();
      }
    } 
    else if self.scanline < 241 && self.cycle == 324 {
      if self.is_rendering_on() && self.ctrl.contains(PpuCtrl::spr_size) 
      && self.ctrl.contains(PpuCtrl::bg_ptrntbl)
      && !self.ctrl.contains(PpuCtrl::spr_ptrntbl) {
        self.mapper.borrow_mut().scanline_ended();
      }
    }

    if self.in_odd_frame
      && !self.is_rendering_on()
      && self.cycle == 339 
      && self.scanline == 261 {
      self.cycle += 1;
    }
    
    self.cycle += 1;
    if self.cycle > 340 {
      self.cycle = 0;
      self.scanline += 1;
      if self.scanline > 261 {
        self.scanline = 0;
        self.in_odd_frame = !self.in_odd_frame;
      }
    }
  }


  // TODO: Clean this shit up...
  pub fn render_pixel(&mut self) {
    if !self.is_rendering_on() && self.cycle <= 32*8 && self.scanline <= 30*8 {
      self.screen.0.set_pixel(self.cycle-1, self.scanline, self.get_color_from_palette(0, 0));
    } else if self.is_rendering_on() && self.cycle <= 32*8 && self.scanline <= 30*8 {
      let (bg_pixel, bg_palette_id) = self.bg_fifo.get(self.x as usize).unwrap().to_owned();

      let spr_data = self.scanline_sprites[self.cycle-1]
        .clone().unwrap_or_default();

      if spr_data.is_sprite0
      && spr_data.pixel != 0 && bg_pixel != 0 
      && self.scanline < 239
      // Should check for 255, but we're putting pixel on the previous current cycle
      && self.cycle != 256
      && !(self.cycle <= 8 && (!self.mask.contains(PpuMask::spr_lstrip) || !self.mask.contains(PpuMask::bg_lstrip)))
      && self.mask.contains(PpuMask::bg_render_on) 
      && self.mask.contains(PpuMask::spr_render_on) {
        info!("SPRITE 0 HIT");
        self.stat.insert(PpuStat::spr0_hit);
      }

      if self.mask.contains(PpuMask::spr_render_on)
      && !(self.cycle <= 8 && !self.mask.contains(PpuMask::spr_lstrip))
      && (spr_data.priority == SpritePriority::Front || bg_pixel == 0) 
      && spr_data.pixel != 0 {
        let color = self.get_color_from_palette(spr_data.pixel, spr_data.palette_id);
        self.screen.0.set_pixel(self.cycle-1, self.scanline, color);
      } else if self.mask.contains(PpuMask::bg_render_on) && !(self.cycle <= 8 && !self.mask.contains(PpuMask::bg_lstrip)) {
        let color = self.get_color_from_palette(bg_pixel, bg_palette_id);
        self.screen.0.set_pixel(self.cycle-1, self.scanline, color);
      }
    }
  }

  pub fn evaluate_sprites(&mut self) {
    if !self.is_rendering_on() { return; }

    let mut visible_sprites = 0;

    for i in (0..256).step_by(4) {
      let spr_y = self.oam[i] as isize;
      if spr_y >= 30*8 { continue; }
      let dist_from_scanline = self.scanline as isize - spr_y;

      if dist_from_scanline >= 0 && dist_from_scanline < self.ctrl.spr_height() as isize {
        if self.oam_buf.len() < 8 {
          self.oam_buf.push(OamEntry::from_bytes(&self.oam[i..i+4], i));
        }
        visible_sprites += 1;
      }
    }

    let spr_overflow = self.stat.contains(PpuStat::spr_overflow)
      || (self.is_rendering_on() && visible_sprites > 8);
    self.stat.set(PpuStat::spr_overflow, spr_overflow);
  }

  pub fn fetch_sprites(&mut self) {
    for sprite in self.oam_buf.iter() {
      let vertical_start: usize = if sprite.flip_vertical { 7 } else { 0 };
      let dist_from_scanline = self.scanline - sprite.y;
  
      let spr_addr = match self.ctrl.spr_height() {
        8 => self.ctrl.spr_ptrntbl_addr()
          + sprite.tile_id as u16 * 16
          + (dist_from_scanline).abs_diff(vertical_start) as u16,
        16 => {
          let tbl = (sprite.tile_id & 1) as u16;
          let mut tile_id = sprite.tile_id as u16 & 0b1111_1110;
          tile_id += match sprite.flip_vertical {
            false =>  if dist_from_scanline >= 8 { 1 } else { 0 }
            true  =>  if dist_from_scanline >= 8 { 0 } else { 1 }
          };

          (tbl << 12)
            + tile_id * 16
            + (dist_from_scanline % 8).abs_diff(vertical_start) as u16
        }
        _ => unreachable!("sprite heights are either 8 or 16")
      };

      let mut plane0 = self.peek_vram(spr_addr);
      let mut plane1 = self.peek_vram(spr_addr + 8);

      // TODO: eventually fix this hack
      if !sprite.flip_horizontal { 
        plane0 = plane0.reverse_bits();
        plane1 = plane1.reverse_bits();
      }
  
      for i in (0..8usize).rev() {
        if sprite.x + i >= 32*8 { continue; }
  
        // sprite with higher priority already there
        if let Some(current_pixel) = &self.scanline_sprites[sprite.x + i] { 
          if current_pixel.pixel != 0 { continue; }
        }
        
        let pixel = self.get_pixel_from_planes(i as u8, plane0, plane1);
        self.scanline_sprites[sprite.x + i] = Some(SprData {
          pixel,
          palette_id: sprite.palette_id,
          priority: sprite.priority, 
          is_sprite0: sprite.index == 0
        });
      }
    }
  }

  pub fn fetch_bg_step(&mut self) {
    self.bg_fifo.pop_front();
    
    let step = ((self.cycle-1) % 8) + 1;
    // https://www.nesdev.org/wiki/PPU_scrolling#Tile_and_attribute_fetching
    match step {
      2 => {
        // Load bg fifo
        for i in (0..8).rev() {
          let pixel = self.get_pixel_from_planes(i, self.bg_buf.tile_plane0, self.bg_buf.tile_plane1);
          self.bg_fifo.push_back((pixel, self.bg_buf.palette_id));
        }

        let tile_addr = 0x2000 + self.v.nametbl_idx();
        self.bg_buf.tile_id = self.peek_vram(tile_addr);
      }
      4 => {
        let attribute_addr = 0x23C0
          + ((self.v.nametbl() as u16) << 10)
          + ((self.v.coarse_y() as u16)/4) * 8
          + ((self.v.coarse_x() as u16)/4);

        let attribute = self.peek_vram(attribute_addr);
        let palette_id = self.get_palette_from_attribute(attribute);

        self.bg_buf.palette_id = palette_id;
      }
      6 => {
        let tile_addr  = self.ctrl.bg_ptrntbl_addr() 
          + (self.bg_buf.tile_id as u16) * 16
          + self.v.fine_y() as u16;

        let plane0 = self.peek_vram(tile_addr);
        self.bg_buf.tile_plane0 = plane0;
      }
      7 => {
        let tile_addr  = self.ctrl.bg_ptrntbl_addr() 
          + (self.bg_buf.tile_id as u16) * 16
          + self.v.fine_y() as u16;

        let plane1 = self.peek_vram(tile_addr + 8);
        self.bg_buf.tile_plane1 = plane1;
      }
      8 => self.increase_coarse_x(),
      _ => {}
    }

    self.render_pixel();
  }

  // https://forums.nesdev.org/viewtopic.php?t=15926
  pub fn is_rendering_on(&self) -> bool {
    self.mask.contains(PpuMask::bg_render_on) ||
    self.mask.contains(PpuMask::spr_render_on)
  }
  
  pub fn read_reg(&mut self, addr: u16) -> u8 {
    match addr {
      0x2002 => {
        let old_stat = self.stat.bits();
        self.w = WriteLatch::FirstWrite;
        self.stat.remove(PpuStat::vblank);
        old_stat
      }
      0x2004 => self.oam[self.oam_addr as usize],
      0x2007 => self.read_vram(),
      _ => { info!("PPU REG_R {addr:04X} not implemented"); 0 }
    }
  }

  pub fn write_reg(&mut self, addr: u16, val: u8) {
    match addr {
      0x2000 => {
        let was_nmi_off = !self.ctrl.contains(PpuCtrl::vblank_nmi_on);
        self.ctrl = PpuCtrl::from_bits_retain(val);
        self.t.set_nametbl_x(val & 0b01);
        self.t.set_nametbl_y((val & 0b10) >> 1);

        if was_nmi_off 
        && self.ctrl.contains(PpuCtrl::vblank_nmi_on) 
        && self.stat.contains(PpuStat::vblank) {
          self.nmi_requested = Some(());
        }
      }
      0x2001 => {
        self.mask = PpuMask::from_bits_retain(val);
        // self.mask_buf = val;
      }
      0x2003 => self.oam_addr = val,
      0x2004 => {
        self.oam[self.oam_addr as usize] = val;
        self.oam_addr = self.oam_addr.wrapping_add(1);
      }
      0x2005 => {
        match self.w {
          WriteLatch::FirstWrite => {
            self.t.set_coarse_x((val & 0b1111_1000) >> 3);
            self.x = val & 0b0000_0111;
            
            // self.scroll.0 = val;
            self.w = WriteLatch::SecondWrite;
          }
          WriteLatch::SecondWrite => {
            let high = (val & 0b1111_1000) >> 3;
            let low = val & 0b0000_0111;
            self.t.set_coarse_y(high);
            self.t.set_fine_y(low);

            // self.scroll.1 = val;
            self.w = WriteLatch::FirstWrite;
          }
        }
      }
      0x2006 => {
        info!("PPU_ADDR WRITE {:?} {val:02X}", self.w);
        match self.w {
          WriteLatch::FirstWrite => {
            // val is set to low byte of t
            self.t.0 = ((val as u16) << 8) | (self.t.0 & 0x00FF);
            // cut bit 14 and 15
            self.t.0 = self.t.0 & 0x3FFF;
            self.w = WriteLatch::SecondWrite;
          }
          WriteLatch::SecondWrite => { 
            // val is set to high byte of t
            self.t.0 = (self.t.0 & 0xFF00) | (val as u16);
            self.v.0 = self.t.0;

            self.w = WriteLatch::FirstWrite;
            warn!("V addr is {:04X}", self.v.0);
          }
        }
      }
      0x2007 => self.write_vram(val),
      _ => info!("PPU REG_W {addr:04X} not implemented"),
    }
  }

  pub fn oam_dma(&mut self, page: &[u8]) {
    self.oam.copy_from_slice(page);
  }

  pub fn map_address(&self, addr: u16) -> (VramDst, usize) {
    match addr {
      0x0000..=0x1FFF => (VramDst::Patterntbl, addr as usize),
      0x2000..=0x2FFF => {
        let mirrored = self.mirror_nametbl(addr);
        (VramDst::Nametbl, mirrored as usize)
      }
      0x3F00..=0x3FFF => {
        let palette = self.mirror_palette(addr);
        (VramDst::Palettes, palette as usize)
      }
      _ => (VramDst::Unused, 0), 
    }
  }

  pub fn peek_vram(&self, addr: u16) -> u8 {
    let (dst, addr) = self.map_address(addr);
    match dst {
      VramDst::Patterntbl => self.mapper.borrow_mut()
        .read_chr(&self.chr, addr),
      VramDst::Nametbl => self.vram[addr],
      VramDst::Palettes => self.palettes[addr],
      VramDst::Unused => 0,
    }
  }

  pub fn increase_vram_address(&mut self) {
    self.v.0 = self.v.0.wrapping_add(self.ctrl.vram_addr_incr());
  }

  pub fn read_vram(&mut self) -> u8 {
    info!("PPU_DATA READ at {:04X}", self.v.0);

    // palettes shouldn't be buffered
    let res = if self.v.0 >= 0x3F00 {
      self.peek_vram(self.v.0)
    } else { self.data_buf };

    self.data_buf = self.peek_vram(self.v.0);
    self.increase_vram_address();
    res
  }

  pub fn write_vram(&mut self, val: u8) {
    info!("PPU_DATA WRITE at {:04X} = {val:02X}", self.v.0);

    let (dst, addr) = self.map_address(self.v.0);
    match dst {
      VramDst::Patterntbl => self.mapper.borrow_mut()
        .write_chr(&mut self.chr, addr, val),
      VramDst::Nametbl => self.vram[addr] = val,
      VramDst::Palettes => self.palettes[addr] = val & 0b0011_1111,
      VramDst::Unused => {}
    }

    self.increase_vram_address();
  }

  pub fn get_pixel_from_planes(&self, bit: u8, plane0: u8, plane1: u8) -> u8 {
    let bit0 = (plane0 >> bit) & 1;
    let bit1 = (plane1 >> bit) & 1;
    (bit1 << 1) | bit0
  }

  pub fn get_color_from_palette(&self, pixel: u8, palette_id: u8) -> u8 {
    if pixel == 0 { self.peek_vram(0x3F00) }
    else { self.peek_vram(0x3F00 + (palette_id + pixel) as u16) }
  }

  pub fn get_palette_from_attribute(&self, attribute: u8) -> u8 {
    4 * match (self.v.coarse_x() % 4, self.v.coarse_y() % 4) {
      (0..2, 0..2) => (attribute & 0b0000_0011) >> 0 & 0b11,
      (2..4, 0..2) => (attribute & 0b0000_1100) >> 2 & 0b11,
      (0..2, 2..4) => (attribute & 0b0011_0000) >> 4 & 0b11,
      (2..4, 2..4) => (attribute & 0b1100_0000) >> 6 & 0b11,
      _ => unreachable!("mod 4 should always give value smaller than 4"),
    }
  }

  // Horizontal:
  // 0x0800 [ B ]  [ A ] [ a ]
  // 0x0400 [ A ]  [ B ] [ b ]
 
  // Vertical:
  // 0x0800 [ B ]  [ A ] [ B ]
  // 0x0400 [ A ]  [ a ] [ b ]

  // Single-page: (based on mapper register)
  // 0x0800 [ B ]  [ A ] [ a ]    [ B ] [ b ]
  // 0x0400 [ A ]  [ a ] [ a ] or [ b ] [ b ]
  pub fn mirror_nametbl(&self, addr: u16) -> u16 {
    let addr = addr - 0x2000;
    let nametbl_idx = addr / 0x400;
    
    use Mirroring::*;
    // TODO: consider moving this only on the mapper
    let mirroring = if let Some(mirroring) = self.mapper.borrow().mirroring() {
      mirroring
    } else { self.mirroring };

    match (mirroring, nametbl_idx) {
      (Horizontally, 1) | (Horizontally, 2) => addr - 0x400,
      (Horizontally, 3) => addr - 0x400*2,
      (Vertically, 2) | (Vertically, 3) => addr - 0x400*2,
      (SingleScreenFirstPage, _) => addr % 0x400,
      (SingleScreenSecondPage, _) => (addr % 0x400) + 0x400,
      (FourScreen, _) => todo!("Four screen mirroring not implemented"),
      _ => addr
    }
  }

  pub fn mirror_palette(&self, addr: u16) -> u16 {
    let addr = addr - 0x3F00;
    if addr >= 16 && addr % 4 == 0 { addr - 16 } else { addr % 32 }
  }
}


impl Ppu {
    // https://www.nesdev.org/wiki/PPU_scrolling#Wrapping_around
    pub fn increase_coarse_x(&mut self) {
      if !self.is_rendering_on() { return; }
  
      if self.v.coarse_x() == 31 {
        self.v.set_coarse_x(0);
        self.v.set_nametbl_x(self.v.nametbl_x() ^ 1); // flip horizontal nametbl
      } else { self.v.set_coarse_x(self.v.coarse_x() + 1); }
    }
  
    // https://www.nesdev.org/wiki/PPU_scrolling#Wrapping_around
    pub fn increase_coarse_y(&mut self) {
      if !self.is_rendering_on() { return; }
  
      if self.v.fine_y() < 7 {
        self.v.set_fine_y(self.v.fine_y() + 1);
      } else {
        self.v.set_fine_y(0);
        let mut y = self.v.coarse_y();
        if y == 29 {
          y = 0;
          self.v.set_nametbl_y(self.v.nametbl_y() ^ 1); // flip vertical nametbl
        } else if y == 31 {
          y = 0;
        } else { y += 1; }
  
        self.v.set_coarse_y(y);
      }
    }
  
    // https://forums.nesdev.org/viewtopic.php?p=5578#p5578
    pub fn reset_render_x(&mut self) {
      if !self.is_rendering_on() { return; }
  
      self.v.set_coarse_x(self.t.coarse_x());
      self.v.set_nametbl_x(self.t.nametbl_x());
    }
  
    // https://forums.nesdev.org/viewtopic.php?p=229928#p229928
    pub fn reset_render_y(&mut self) {
      if !self.is_rendering_on() { return; }
  
      self.v.set_coarse_y(self.t.coarse_y());
      self.v.set_fine_y(self.t.fine_y());
      self.v.set_nametbl_y(self.t.nametbl_y());
    }
}