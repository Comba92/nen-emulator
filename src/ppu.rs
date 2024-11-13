use std::fmt;

use crate::{cart::Mirroring, mapper::CartMapper, renderer::{NesScreen, SYS_PALETTES}, tile::{OamEntry, SpritePriority, Tile}};
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
pub struct RenderData {
  pub tile_id: u8,
  pub palette_id: u8,
  pub tile_plane0: u8,
  pub tile_plane1: u8
}

#[bitfield(u16, order = Lsb)]
pub struct ShiftReg {
  #[bits(8)]
  lo: u8,
  #[bits(8)]
  hi: u8
}
impl ShiftReg {
  pub fn get(&self) -> u8 {
    ((self.0 & 0x8000) != 0) as u8
  }
}

#[derive(Default)]
pub struct BgShifters {
  plane0: ShiftReg,
  plane1: ShiftReg,
  palette0: ShiftReg,
  palette1: ShiftReg,
}
impl BgShifters {
    pub fn update(&mut self) {
    self.plane0.0 <<= 1;
    self.plane1.0 <<= 1;
    self.palette0.0 <<= 1;
    self.palette1.0 <<= 1;
  }
  pub fn load(&mut self, data: &RenderData) {
    self.plane0.set_lo(data.tile_plane0);
    self.plane1.set_lo(data.tile_plane1);
    self.palette0.set_lo(if (data.palette_id & 0b01) != 0 {0xFF} else { 0x00 });
    self.palette1.set_lo(if (data.palette_id & 0b10) != 0 {0xFF} else { 0x00 });
  }
  pub fn get(&self, fine_x: u8) -> (u8, u8) {
    let mask = 0x8000 >> fine_x;

    let bit1 = ((self.plane1.0 & mask) != 0) as u8;
    let bit0 = ((self.plane0.0 & mask) != 0) as u8;
    let pixel = (bit1 << 1) | bit0;
    
    let pal1 = ((self.palette1.0  & mask) != 0) as u8;
    let pal0 = ((self.palette0.0  & mask) != 0) as u8;
    let palette_id = (pal1 << 1) | pal0;

    (pixel as u8, palette_id as u8)
  }
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

  render_buf: RenderData,
  bg_shifters: BgShifters,
  spr_shifters: [(ShiftReg, ShiftReg); 8],
  oam_secondary: Vec<OamEntry>,

  v: LoopyReg, // current vram address
  t: LoopyReg, // temporary vram address / topleft onscreen tile
  x: u8, // Fine X Scroll
  w: WriteLatch, // First or second write toggle
  data_buf: u8,
  in_odd_frame: bool,
  
  pub ctrl: PpuCtrl,
  pub mask: PpuMask,
  pub stat: PpuStat,
  pub oam_addr: u8,
  pub scroll: (u8, u8),
  
  pub mapper: CartMapper,
  pub patterns: Vec<u8>,
  pub vram: [u8; 0x1000],
  pub palettes: [u8; 32],
  pub oam: [u8; 256],
  
  pub scanline: usize,
  pub cycle: usize,
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
    Self {
      screen: NesScreen::new(),

      bg_shifters: BgShifters::default(),
      spr_shifters: [(ShiftReg::new(), ShiftReg::new()); 8],
      render_buf: RenderData::default(),
      oam_secondary: Vec::with_capacity(8),
      
      v: LoopyReg::new(), t: LoopyReg::new(), 
      x: 0, w: WriteLatch::FirstWrite, 

      patterns: chr_rom,
      mapper,
      vram: [0; 0x1000], 
      palettes: [0; 32],
      oam: [0; 256],
      oam_addr: 0, data_buf: 0,
      in_odd_frame: false,
      scanline: 0, cycle: 21,
      ctrl: PpuCtrl::empty(),
      mask: PpuMask::empty(),
      stat: PpuStat::empty(),
      scroll: (0, 0),
      
      mirroring,

      nmi_requested: None,
      vblank_started: None,
    }
  }

  pub fn reset() { todo!() }

  pub fn step(&mut self) {
    self.cycle += 1;

    if self.is_spr0_hit() {
      self.stat.insert(PpuStat::spr0_hit);
    }

    if self.cycle == 341 {
      self.cycle = 0;
      self.scanline += 1;

      // Post-render scanline (240)

      // Vertical blanking lines (241-260)
      if self.scanline == 241 {
        info!("VBLANK!!");
        self.vblank_started = Some(());
        self.stat.insert(PpuStat::vblank);
        self.stat.remove(PpuStat::spr0_hit);

        if self.ctrl.contains(PpuCtrl::vblank_nmi_on) {
          self.nmi_requested = Some(());
        }

      // Pre-render scanline (261)
      } else if self.scanline == 261 {
        self.stat = PpuStat::empty();
        self.oam_addr = 0;
        self.scanline = 0;
      }
    }
  }

  pub fn step_accurate(&mut self) {
    match (self.cycle, self.scanline) {
      // Pre-render line
      (1, 261) => {
        self.stat = PpuStat::empty();
        self.oam_addr = 0;
      }

      // Post-render line
      (1, 241) => {
        if !self.mask.contains(PpuMask::spr_render_on) {  }
        
        info!("VBLANK!!");
        self.vblank_started = Some(());
        self.stat.insert(PpuStat::vblank);
        self.stat.remove(PpuStat::spr0_hit);

        if self.ctrl.contains(PpuCtrl::vblank_nmi_on) {
          self.nmi_requested = Some(());
        }
      }

      // Visible frames + Pre-render line background fetches
      (1..=256 | 321..=336, 0..=239) | (321..=336, 261) => {
        self.fetch_next_bg_pixel();
      }

      // Restore horizontal
      (257, 0..=239 | 261)  => self.reset_render_x(),
       // Restore vertical
      //(280..=304, 261)      => self.reset_render_y(),
      (280, 261)            => self.reset_render_y(),


      // Visible frames + Pre-render line sprite fetches
      (257..=320, 0..=239 | 261) => {
        self.fetch_next_scanline_spr_pixel();
      }

      _ => {}
    }
    
    // odd frame skip
    if self.in_odd_frame
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
        self.in_odd_frame = !self.in_odd_frame
      }
    }
  }

  pub fn evaluate_sprites(&mut self) {
    let mut visible_sprites = 0;

    for i in (0..256).step_by(4) {
      let spr_y = self.oam[i];

      // TODO: this is searching for sprites in the NEXT scanline, is this correct?
      let dist = self.scanline.abs_diff(spr_y as usize);
      if dist < self.ctrl.spr_height() {
        if self.oam_secondary.len() < 8 {
          let sprite = OamEntry::from_bytes(&self.oam[i..i+4]);
          self.oam_secondary.push(sprite);
        }
        visible_sprites += 1;
      }
    }

    self.stat.set(PpuStat::spr_overflow, visible_sprites > 8);
  }

  pub fn fetch_next_scanline_spr_pixel(&mut self) {
    let current_spr = (self.cycle - 256) % 8; 

    let sprite = self.oam_secondary.get(current_spr);
    if sprite.is_none() { return; }

    let sprite = sprite.unwrap();
    let vertical_start = if sprite.flip_vertical { 7 } else { 0 };
    let dist = self.scanline.abs_diff(sprite.y);

    let spr_addr = match self.ctrl.spr_height() {
      8 => self.ctrl.spr_ptrntbl_addr() 
        + sprite.tile_id as u16 * 16
        + (dist & 0x111).abs_diff(vertical_start) as u16,
      16 => {
        let bank = (sprite.tile_id & 1) as u16;
        let tile_id = ((sprite.tile_id as u16) & 0b1111_1110) + if dist < 8 { 0 } else { 1 };
        (bank << 12) 
          + tile_id * 16 
          + (dist & 0x111).abs_diff(vertical_start) as u16
      }
      _ => unreachable!("sprite heights are either 8 or 16")
    };

    match ((self.cycle-1) % 8) + 1 {
      5 => {
        let mut pattern_lo = self.vram_peek(spr_addr);
        if sprite.flip_horizontal { pattern_lo = pattern_lo.reverse_bits(); }
        self.spr_shifters[current_spr].0.set_hi(pattern_lo);
      }
      7 => {
        let mut pattern_hi = self.vram_peek(spr_addr + 8);
        if sprite.flip_horizontal { pattern_hi = pattern_hi.reverse_bits(); }
        self.spr_shifters[current_spr].1.set_hi(pattern_hi);
      }
      _ => {}
    }
  }

  pub fn fetch_next_bg_pixel(&mut self) {
    self.bg_shifters.update();
    for (i, sprite) in self.oam_secondary.iter_mut().enumerate() {
      if sprite.x > 0 { sprite.x -= 1; }
      else {
        self.spr_shifters[i].0.0 <<= 1;
        self.spr_shifters[i].1.0 <<= 1;
      }
    }

    // https://www.nesdev.org/wiki/PPU_scrolling#Tile_and_attribute_fetching
    match ((self.cycle-1) % 8) + 1 {
      1 => {
        self.bg_shifters.load(&self.render_buf);
        self.render_buf.tile_id = self.vram_peek(0x2000 + self.v.nametbl_idx());
      }
      3 => {
        let attribute_addr = 0x23C0
          | ((self.v.nametbl() as u16) << 10)
          | ((self.v.coarse_y() as u16)/4) << 3
          | ((self.v.coarse_x() as u16)/4);
        let attribute = self.vram_peek(attribute_addr);
        let palette_id = self.get_palette_id(attribute);

        self.render_buf.palette_id = palette_id;
      }
      5 => {
        let tile_addr  = self.ctrl.bg_ptrntbl_addr() 
          + (self.render_buf.tile_id as u16) * 16
          + self.v.fine_y() as u16;

        let plane0 = self.vram_peek(tile_addr);
        self.render_buf.tile_plane0 = plane0;
      }
      7 => {
        let tile_addr  = self.ctrl.bg_ptrntbl_addr() 
          + (self.render_buf.tile_id as u16) * 16
          + self.v.fine_y() as u16;

        let plane1 = self.vram_peek(tile_addr + 8);
        self.render_buf.tile_plane1 = plane1;
      }
      8 => {
        self.increase_coarse_x();
      }
      _ => {}
    }
    if self.cycle == 1 { 
      self.oam_secondary.clear();
      // self.spr_shifters.fill((ShiftReg::new(), ShiftReg::new()));
    }
    if self.cycle == 65 { self.evaluate_sprites(); }
    if self.cycle == 256 { self.increase_coarse_y(); }

    if self.is_rendering_on() && self.cycle < 32*8 && self.scanline < 30*8 {
      let (bg_pixel, palette_id) = self.bg_shifters.get(self.x);
      
      let sprite = self.oam_secondary.iter().enumerate().find(|(i, sprite)| {
        let spr_pixel = (self.spr_shifters[*i].1.get() << 1) | self.spr_shifters[*i].0.get();
        sprite.x == 0 && spr_pixel != 0
      });

      if let Some((i , sprite)) = sprite {
        if sprite.priority == SpritePriority::Front {
          let spr_pixel = (self.spr_shifters[i].1.get() << 1) | self.spr_shifters[i].0.get();

          self.screen.0.set_pixel(self.cycle-1, self.scanline, self.get_color_id(spr_pixel, sprite.palette_id));
        }
      } else {
        self.screen.0.set_pixel(self.cycle-1, self.scanline, self.get_color_id(bg_pixel, palette_id));
      }

    }
  }


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

  pub fn is_spr0_hit(&self) -> bool {
    let spr0_y = self.oam[0] as usize;
    let spr0_x = self.oam[3] as usize;

    // TODO: is this correct?
    self.is_rendering_on()
      && spr0_y == self.scanline
      && spr0_x <= self.cycle
  }

  pub fn is_rendering_on(&self) -> bool {
    self.mask.contains(PpuMask::bg_render_on) ||
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
        // self.t = self.t | (((val & 0b11) as u16) << 11);
        self.t.set_nametbl_x(val & 0b01);
        self.t.set_nametbl_y((val & 0b10) >> 1);

        if was_nmi_off 
        && self.ctrl.contains(PpuCtrl::vblank_nmi_on) 
        && self.stat.contains(PpuStat::vblank) {
          self.nmi_requested = Some(());
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
            // self.t = self.t | (val & 0b1111_1000) as u16;
            self.t.set_coarse_x((val & 0b1111_1000) >> 3);
            self.x = val & 0b0000_0111;
            
            self.scroll.0 = val;
            self.w = WriteLatch::SecondWrite;
          }
          WriteLatch::SecondWrite => {
            let high = (val & 0b1111_1000) >> 3;
            let low = val & 0b0000_0111;
            // let res = ((low as u16) << 13) | ((high as u16) << 6);
            // self.t = self.t | res;
            self.t.set_coarse_y(high);
            self.t.set_fine_y(low);

            self.scroll.1 = val;
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
      0x2007 => self.vram_write(val),
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
      0x3F00..0x3FFF => {
        let palette = (addr - 0x3F00) % 32;
        (VramDst::Palettes, palette as usize)
      }
      _ => (VramDst::Unused, 0), 
    }
  }

  pub fn increase_vram_address(&mut self) {
    self.v.0 = self.v.0.wrapping_add(self.ctrl.vram_addr_incr());
  }

  pub fn vram_peek(&self, addr: u16) -> u8 {
    let (dst, addr) = self.map_address(addr);
    match dst {
      VramDst::Patterntbl => self.mapper.as_ref().borrow()
        .read_chr(&self.patterns, addr),
      VramDst::Nametbl => self.vram[addr],
      VramDst::Palettes => self.palettes[addr],
      VramDst::Unused => 0,
    }
  }

  pub fn vram_read(&mut self) -> u8 {
    info!("PPU_DATA READ at {:04X}", self.v.0);

    let res = self.data_buf;
    self.data_buf = self.vram_peek(self.v.0);
    self.increase_vram_address();
    res
  }

  pub fn vram_write(&mut self, val: u8) {
    info!("PPU_DATA WRITE at {:04X} = {val:02X}", self.v.0);

    let (dst, addr) = self.map_address(self.v.0);
    match dst {
      VramDst::Patterntbl => self.mapper.as_ref().borrow_mut()
        .write_chr(addr, val),
      VramDst::Nametbl => self.vram[addr] = val,
      VramDst::Palettes => self.palettes[addr] = val,
      VramDst::Unused => {}
    }

    self.increase_vram_address();
  }

  pub fn get_color_id(&self, pixel: u8, palette_id: u8) -> u8 {
    self.palettes[((palette_id as usize) << 2) | pixel as usize]
  }

  pub fn get_palette_id(&self, attribute: u8) -> u8 {
    match (self.v.coarse_x() % 4, self.v.coarse_y() % 4) {
      (0..2, 0..2) => (attribute & 0b0000_0011) >> 0 & 0b11,
      (2..4, 0..2) => (attribute & 0b0000_1100) >> 2 & 0b11,
      (0..2, 2..4) => (attribute & 0b0011_0000) >> 4 & 0b11,
      (2..4, 2..4) => (attribute & 0b1100_0000) >> 6 & 0b11,
      _ => unreachable!("mod 4 should always give value smaller than 4"),
    }
  }

  pub fn mirror_nametbl(&self, addr: u16) -> u16 {
    let addr = addr - 0x2000;
    let nametbl_idx = addr / 0x400;
    
    use Mirroring::*;
    let res = match (self.mirroring, nametbl_idx) {
      (Horizontally, 1) | (Horizontally, 2) => addr - 0x400,
      (Horizontally, 3) => addr - 0x400*2,
      (Vertically, 2) | (Vertically, 3) => addr - 0x400*2,
      (_, _) => addr,
    };

    res
  }
}