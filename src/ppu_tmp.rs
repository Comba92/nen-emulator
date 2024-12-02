use core::fmt;
use std::collections::VecDeque;
use crate::{cart::Mirroring, mapper::CartMapper, render::NesScreen};

use bitfield_struct::bitfield;
use bitflags::bitflags;
use log::{info, warn};

// https://www.nesdev.org/wiki/PPU_scrolling#PPU_internal_registers
#[bitfield(u16, order = Lsb)]
struct LoopyReg {
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

#[bitfield(u16, order = Lsb)]
struct ShiftReg {
  #[bits(8)]
  lo: u8,
  #[bits(8)]
  hi: u8,
}

bitflags! {
    #[derive(Debug)]
    pub struct PpuStatus: u8 {
      const open_bus     = 0b0001_1111;
      const spr_overflow = 0b0010_0000;
      const spr0_hit     = 0b0100_0000;
      const vblank       = 0b1000_0000;
    }
}

// pub enum NametblAddr {
//     First(u16), Second(u16), Third(u16), Fourth(u16) 
// }
pub enum VramAddrIncrement { Add1, Add32 } 
pub enum PatterntblAddr { First(u16), Second(u16) }
pub enum SpriteSize { Base8x8, Big8x16 }

pub struct PpuCtrl {
    // pub base_nametbl: NametblAddr,
    pub vram_increment: VramAddrIncrement,
    pub spr_patterntbl: PatterntblAddr,
    pub bg_patterntbl: PatterntblAddr,
    pub sprite_size: SpriteSize,
    pub vblank_nmi_on: bool,
}
impl PpuCtrl {
    pub fn new() -> Self {
      Self {
        // base_nametbl: NametblAddr::First(0x2000),
        vram_increment: VramAddrIncrement::Add1,
        spr_patterntbl: PatterntblAddr::First(0x0000),
        bg_patterntbl: PatterntblAddr::First(0x0000),
        sprite_size: SpriteSize::Base8x8,
        vblank_nmi_on: false,
      }
    }

    pub fn write(&mut self, val: u8) {
        // self.base_nametbl = match val & 0b11 {
        //     0 => NametblAddr::First(0x2000),
        //     1 => NametblAddr::Second(0x2400),
        //     2 => NametblAddr::Second(0x2800),
        //     3 => NametblAddr::Second(0x2C00),
        //     _ => unreachable!("matching with first 2 bits should give only 4 choices")
        // };

        self.vram_increment = match val & 0b100 != 0 {
          false => VramAddrIncrement::Add1,
          true => VramAddrIncrement::Add32,
        };

        self.spr_patterntbl = match val & 0b1000 != 0 {
          false => PatterntblAddr::First(0x0000),
          true => PatterntblAddr::Second(0x1000),
        };

        self.bg_patterntbl = match val & 0b1_0000 != 0 {
          false => PatterntblAddr::First(0x0000),
          true => PatterntblAddr::Second(0x1000),
        };

        self.sprite_size = match val & 0b10_0000 != 0 {
          false => SpriteSize::Base8x8,
          true => SpriteSize::Big8x16,
        };

        self.vblank_nmi_on = val & 0b1000_0000 != 0;
    }
}

#[derive(Default)]
pub struct PpuMask {
  // TODO: mask missing values
  pub spr_rendering: bool,
  pub bg_rendering: bool,
}
impl PpuMask {
  pub fn write(&mut self, val: u8) {
    self.bg_rendering = val & 0b0100 != 0;
    self.spr_rendering = val & 0b1000 != 0;
  }
}

#[derive(Debug)]
pub enum WriteLatch {
  FirstWrite, SecondWrite
}

pub enum VramDst {
  Patterntbl, Nametbl, Palettes, Unused
}

#[derive(Default)]
pub struct BgRenderData {
  pub tile_id: u8,
  pub palette_id: u8,
  pub plane0: u8,
  pub plane1: u8,
}

pub struct Ppu {
    pub screen: NesScreen,
  
    v: LoopyReg, // current vram address
    t: LoopyReg, // temporary vram address / topleft onscreen tile
    x: u8, // Fine X Scroll
    w: WriteLatch, // First or second write toggle
    data_buf: u8,
    data_bus: u8,
    bg_buf: BgRenderData,
    bg_fifo: VecDeque<u8>,
    
    mapper: CartMapper,
    chr: Vec<u8>,
    pub vram: [u8; 0x800],
    pub palettes: [u8; 32],
    pub oam: [u8; 256],
    oam_addr: u8,
    
    pub scanline: usize,
    pub cycle: usize,
    in_odd_frame: bool,
  
    pub status: PpuStatus,
    pub ctrl: PpuCtrl,
    pub mask: PpuMask,
    pub mirroring: Mirroring,
    
    pub nmi_requested: Option<()>,
    pub vblank_started: Option<()>,
}

impl Ppu {
    pub fn new(chr_rom: Vec<u8>, mapper: CartMapper, cart_mirroring: Mirroring) -> Self {
        let mapper_mirroring = mapper.borrow().mirroring();
        let base_mirroring = if let Some(mapper_mirroring) = mapper_mirroring { 
          mapper_mirroring 
        } else { cart_mirroring };

        Self {
          screen: NesScreen::new(),
          
          v: LoopyReg::new(), t: LoopyReg::new(), 
          x: 0, w: WriteLatch::FirstWrite,
          bg_buf: BgRenderData::default(),
          bg_fifo: VecDeque::new(),
    
          chr: chr_rom,
          mapper,
          vram: [0; 0x800],
          palettes: [0; 32],
          oam: [0; 256],
          oam_addr: 0,
          data_bus: 0,
          data_buf: 0,
          in_odd_frame: false,
          scanline: 261,
          cycle: 0,

          status: PpuStatus::empty(),
          ctrl: PpuCtrl::new(),
          mask: PpuMask::default(),
          
          mirroring: base_mirroring,
    
          nmi_requested: None,
          vblank_started: None,
        }
    }

    pub fn reset(&mut self) { 
      // TODO
    }

    // https://www.nesdev.org/w/images/default/4/4f/Ppu.svg
    pub fn step_accurate(&mut self) {
      if (0..=239).contains(&self.scanline) {
        if (1..=256).contains(&self.cycle)
        || (321..=336).contains(&self.cycle) {
          self.fetch_bg();
        }

        if self.cycle == 256 {
          self.increase_coarse_y();
        } else if self.cycle == 257 {
          self.reset_render_x();
        }
      } else if self.scanline == 241 && self.cycle == 1 {
        info!("VBLANK!!");
        self.vblank_started = Some(());
        self.status.insert(PpuStatus::vblank);
        self.status.remove(PpuStatus::spr0_hit);
  
        if self.ctrl.vblank_nmi_on {
          self.nmi_requested = Some(());
        }
      } else if self.scanline == 261 {
        if self.cycle == 1 {
          self.status = PpuStatus::empty();
          self.oam_addr = 0;
        } else if (280..=304).contains(&self.cycle) {
          self.reset_render_y();
        } else if (321..=336).contains(&self.cycle) {
          self.fetch_bg();
        }
      }

      self.cycle += 1;
      if self.cycle >= 341 {
        self.cycle = 0;
        self.scanline += 1;

        if self.scanline >= 262 {
          self.scanline = 0;
        }
      }
    }

    pub fn fetch_bg(&mut self) {
      self.bg_fifo.pop_front();

      let step = ((self.cycle-1) % 8) + 1;
      // https://www.nesdev.org/wiki/PPU_scrolling#Tile_and_attribute_fetching
      match step {
        1 => {
          for i in (0..8).rev() {
            let pixel = (((self.bg_buf.plane1 >> i) & 1) << 1) | ((self.bg_buf.plane0 >> i) & 1);
            let color = self.get_color_from_palette(pixel, self.bg_buf.palette_id);
            self.bg_fifo.push_back(color);
          }

          let tile_addr = 0x2000 + self.v.nametbl_idx();
          self.bg_buf.tile_id = self.vram_peek(tile_addr);
        }
        3 => {
          let attribute_addr = 0x23C0
            + ((self.v.nametbl() as u16) << 10)
            + ((self.v.coarse_y() as u16)/4) * 8
            + ((self.v.coarse_x() as u16)/4);
  
          let attribute = self.vram_peek(attribute_addr);
          let palette_id = self.get_palette_from_attribute(attribute);
  
          self.bg_buf.palette_id = palette_id;
        }
        5 => {
          let pattern_addr = match self.ctrl.bg_patterntbl {
            PatterntblAddr::First(addr) => addr,
            PatterntblAddr::Second(addr) => addr,
          };

          let tile_addr  = pattern_addr 
            + (self.bg_buf.tile_id as u16) * 16
            + self.v.fine_y() as u16;
  
          let plane0 = self.vram_peek(tile_addr);
          self.bg_buf.plane0 = plane0;
        }
        6 => {
          let pattern_addr = match self.ctrl.bg_patterntbl {
            PatterntblAddr::First(addr) => addr,
            PatterntblAddr::Second(addr) => addr,
          };

          let tile_addr  = pattern_addr
            + (self.bg_buf.tile_id as u16) * 16
            + self.v.fine_y() as u16;
  
          let plane1 = self.vram_peek(tile_addr + 8);
          self.bg_buf.plane1 = plane1;
        }
        7 => self.increase_coarse_x(),
        _ => {}
      }
  
      if self.is_rendering_on() && self.cycle < 32*8 && self.scanline < 30*8 {
        let color = self.bg_fifo.get(self.x as usize).unwrap();
        self.screen.0.set_pixel(self.cycle-1, self.scanline, *color);
      }
    }

    fn is_rendering_on(&self) -> bool {
      self.mask.bg_rendering || self.mask.spr_rendering
    }

    pub fn reg_read(&mut self, addr: u16) -> u8 {
        match addr {
            0x2002 => {
              let old_stat = self.status.bits();
              self.w = WriteLatch::FirstWrite;
              self.status.remove(PpuStatus::vblank);
              self.data_bus = self.data_bus & 0b1111 | old_stat & 0b1110_0000;
              self.data_bus
            }
            0x2004 => self.oam[self.oam_addr as usize],
            0x2007 => self.vram_read(),
            _ => self.data_bus,
        }
    }

    pub fn reg_write(&mut self, addr: u16, val: u8) {
        match addr {
          0x2000 => {
            let was_nmi_off = !self.ctrl.vblank_nmi_on;
            self.ctrl.write(val);

            self.t.set_nametbl_x(val & 0b01);
            self.t.set_nametbl_y((val & 0b10) >> 1);

            if was_nmi_off 
            && self.ctrl.vblank_nmi_on
            && self.status.contains(PpuStatus::vblank) {
              self.nmi_requested = Some(());
            }
          }
          0x2001 => self.mask.write(val),
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
          0x2007 => self.vram_write(val),
          _ => {}
        }
    }

    fn map_address(&self, addr: u16) -> (VramDst, usize) {
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

    fn increase_vram_address(&mut self) {
      let incr_value = match self.ctrl.vram_increment {
        VramAddrIncrement::Add1 => 1,
        VramAddrIncrement::Add32 => 32,
      };

      self.v.0 = self.v.0.wrapping_add(incr_value);
    }

    pub fn vram_peek(&self, addr: u16) -> u8 {
      let (dst, addr) = self.map_address(addr);
      match dst {
        VramDst::Patterntbl => self.mapper.borrow_mut()
          .read_chr(&self.chr, addr),
        VramDst::Nametbl => self.vram[addr],
        VramDst::Palettes => self.palettes[addr],
        VramDst::Unused => self.data_bus,
      }
    }

    fn vram_read(&mut self)  -> u8 {
      info!("PPU_DATA READ at {:04X}", self.v.0);

      // palettes shouldn't be buffered
      let res = if self.v.0 >= 0x3F00 {
        self.vram_peek(self.v.0)
      } else { self.data_buf };

      self.data_buf = self.vram_peek(self.v.0);
      self.increase_vram_address();
      res
    }

    fn vram_write(&mut self, val: u8) {
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

    // Horizontal:
  // 0x0800 [ B ]  [ A ] [ a ]
  // 0x0400 [ A ]  [ B ] [ b ]
 
  // Vertical:
  // 0x0800 [ B ]  [ A ] [ B ]
  // 0x0400 [ A ]  [ a ] [ b ]

  // Single-page: (based on mapper register)
  // 0x0800 [ B ]  [ A ] [ a ]    [ B ] [ b ]
  // 0x0400 [ A ]  [ a ] [ a ] or [ b ] [ b ]
  fn mirror_nametbl(&self, addr: u16) -> u16 {
    let addr = addr - 0x2000;
    let nametbl_idx = addr / 0x400;
    
    let mirroring = if let Some(mirroring) = self.mapper.borrow().mirroring() {
      mirroring
    } else { self.mirroring };
    
    use Mirroring::*;
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
    if addr % 4 == 0 { 0 } else { addr % 32 }
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

  pub fn get_color_from_palette(&self, pixel: u8, palette_id: u8) -> u8 {
    if pixel == 0 { self.vram_peek(0x3F00) }
    else { self.vram_peek(0x3F00 + (palette_id + pixel) as u16) }
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