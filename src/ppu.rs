use std::{collections::HashMap, hash::Hash, ops::{Shl, Shr}, sync::LazyLock};

use bitflags::Flags;

use crate::{emu::{self, Emu}, utils::{byte_set_hi, byte_set_lo}};

bitflags::bitflags! {
  #[derive(Default)]
  struct Ctrl: u8 {
    const AddrIncr  = 1 << 2;
    const SprTable  = 1 << 3;
    const BgTable   = 1 << 4;
    const SprSize   = 1 << 5;
    const NmiEnable = 1 << 7;
  }

  #[derive(Default)]
  struct Status: u8 {
    const SprOverflow = 1 << 5;
    const Spr0Hit = 1 << 6;
    const Vblank  = 1 << 7;
  }

  #[derive(Default)]
  struct Mask: u8 {
    const LstripBg  = 1 << 1;
    const LstripSpr = 1 << 2;
    const EnableBg  = 1 << 3;
    const EnableSpr = 1 << 4;
  }
}

#[derive(Default, Debug)]
struct CtrlStrut {
  vram_addr_inc: u16,
  spr_pttrntbl_addr: u16,
  bg_pttrntbl_addr: u16,
  spr_size: u16,
  vblank_nmi_enabled: bool,
}

#[bitfields::bitfield(u16)]
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
  _unused: u16,
}

#[derive(Default)]
enum WriteToggle {
  #[default] First, Second
}
impl WriteToggle {
  pub fn next(&mut self) {
    *self = match self {
      Self::First  => Self::Second,
      Self::Second => Self::First,
    }
  }
}

#[derive(Default)]
struct FetcherData {
  nametbl: u8,
  attribute: u8,
  pttrn_lo: u8,
  pttrn_hi: u8,
}

// #[derive(Default)]
// enum FetcherState {
//   #[default]
//   Nametable,
//   Attribute,
//   PatternLo,
//   PatternHi,
// }

#[derive(Default)]
struct ShifterData {
  shift_ptrn_lo: u16,
  shift_ptrn_hi: u16,
  // we simulate the 1bit latch by always pushing 8 bytes (thus having a 16bit shifter)
  shift_attr_lo: u16,
  shift_attr_hi: u16,

  spr_pttrn_lo: [u8; 8],
  spr_pttrn_hi: [u8; 8],
}

// TODO: get rid of this
#[derive(Default, Clone)]
struct Sprite {
  index: u8,
  spr0: bool,
  y: u8,
  x: u8,
  nametbl: u8,
  palette: u8,
  priority: bool,
  flip_vert: bool,
  flip_hori: bool,
}
impl Sprite {
  pub fn new(index: u8, bytes: &[u8]) -> Self {
    Self {
      index,
      spr0: index == 0,
      y: bytes[0],
      x: bytes[3],
      nametbl: bytes[1],
      palette: bytes[2] & 0b11,
      // 0: in front of background; 1: behind background
      priority: bytes[2] & 0x20 == 0,
      flip_hori: bytes[2] & 0x40 != 0,
      flip_vert: bytes[2] & 0x80 != 0,
    }
  }

  pub fn empty() -> Self {
    Self {
      nametbl: 0xff,
      ..Default::default()
    }
  }
}

// #[derive(Default)]
// enum RenderState {
//   #[default]
//   PreRenderScanline,
//   FirstIdleCycle,
//   RenderingBg,
//   RenderingUnusedBg,
//   RenderingSpr,
//   RenderingEnd,
//   PostRenderScanline,
//   Vblank,
// }


pub struct Oam(pub [u8; 256]);
impl Default for Oam {
  fn default() -> Self { Self([0; 256]) }
}

#[bitfields::bitfield(u8)]
#[derive(Clone, Copy)]
struct SprScanlineData {
  #[bits(5)]
  index: u8,
  priority: bool,
  #[bits(2)]
  pixel: u8,
}

struct SprScanline([SprScanlineData; 256]);
impl Default for SprScanline {
  fn default() -> Self { Self([SprScanlineData::default(); 256]) }
}

#[derive(Default)]
pub struct Ppu2C02 {
  ctrl: CtrlStrut,
  mask: Mask,
  stat: Status,
  
  v: LoopyReg,
  t: LoopyReg,
  x: u8,
  w: WriteToggle,

  oam_addr: u8,
  ppu_data: u8,

  pub oam: Oam,
  oam_line: Vec<Sprite>,
  spr_scanline: SprScanline,

  pub cycle: i16,
  pub scanline: i16,
  odd_frame: bool,

  fetcher: FetcherData,
  shifter: ShifterData,
}

// impl Default for Ppu2C02 {
//   fn default() -> Self {
//     Self { ctrl: Default::default(), mask: Default::default(), stat: Default::default(), v: Default::default(), t: Default::default(), x: Default::default(), w: Default::default(), oam_addr: Default::default(), ppu_data: Default::default(), oam: [0; 256], oam_line: Default::default(), cycle: Default::default(), scanline: Default::default(), odd_frame: Default::default(), fetcher: Default::default(), shifter: Default::default() }
//   }
// }

impl Ppu2C02 {
  pub fn new() -> Self {
    Self::default()
  }

  fn increase_addr(&mut self) {
    self.v.0 = self.v.0
      .wrapping_add(self.ctrl.vram_addr_inc);
  }

  fn scroll_x_inc(&mut self) {
    if self.rendering_enabled() {
      if self.v.coarse_x() == 31 {
        self.v.set_coarse_x(0);
        self.v.set_nametbl_x(!self.v.nametbl_x());
      } else {
        self.v.set_coarse_x(self.v.coarse_x() + 1);
      }
    }
  }

  fn scroll_y_inc(&mut self) {
    if self.rendering_enabled() {
      let v = &mut self.v;
      if v.fine_y() < 7 {                            // if fine Y < 7
        v.set_fine_y(v.fine_y() + 1);          // increment fine Y
      } else {                  
        v.set_fine_y(0);                       // fine Y = 0
        let mut y = v.coarse_y();               // let y = coarse Y
        if y == 29 {
          y = 0;                                    // coarse Y = 0
          v.set_nametbl_y(!v.nametbl_y());    // switch vertical nametable
        } else if y == 31 {
          y = 0;                                   // coarse Y = 0, nametable not switched
        } else {
          y += 1;
        }                                         // increment coarse Y
        v.set_coarse_y(y);                  // put coarse Y back into v
      }
    }
  }

  fn scroll_x_tx(&mut self) {
    if self.rendering_enabled() {
      self.v.set_nametbl_x(self.t.nametbl_x());
      self.v.set_coarse_x(self.t.coarse_x());
    }
  }

  fn scroll_y_tx(&mut self) {
    if self.rendering_enabled() {
      self.v.set_nametbl_y(self.t.nametbl_y());
      self.v.set_coarse_y(self.t.coarse_y());
      self.v.set_fine_y(self.t.fine_y());
    }
  }

  fn rendering_enabled(&self) -> bool {
    self.mask.contains(Mask::EnableBg) || self.mask.contains(Mask::EnableSpr)  
  }

  fn shifter_load(&mut self) {
    // On every 8th dot in these background fetch regions (the same dot on which the coarse x component of v is incremented), the pattern and attributes data are transferred into registers used for producing pixel data. 
    let fetcher = &self.fetcher;
    let shifter = &mut self.shifter;

    // For the pattern data, these transfers are into the high 8 bits of two 16-bit shift registers.
    shifter.shift_ptrn_lo = byte_set_hi(shifter.shift_ptrn_lo, fetcher.pttrn_lo);
    shifter.shift_ptrn_hi = byte_set_hi(shifter.shift_ptrn_hi, fetcher.pttrn_hi);
    
    //  For the attributes data, only 2 bits are transferred and into two 1-bit latches that feed 8-bit shift registers. 
    let attr_lo = if fetcher.attribute & 0b01 != 0 { 0xff } else { 0 };
    shifter.shift_attr_lo = byte_set_hi(shifter.shift_attr_lo, attr_lo);
    
    let attr_hi = if fetcher.attribute & 0b10 != 0 { 0xff } else { 0 };
    shifter.shift_attr_hi = byte_set_hi(shifter.shift_attr_hi, attr_hi);
  }

  fn shifter_update(&mut self) {
    if self.rendering_enabled() {
      self.shifter.shift_ptrn_lo <<= 1;
      self.shifter.shift_ptrn_hi <<= 1;
      self.shifter.shift_attr_lo <<= 1;
      self.shifter.shift_attr_hi <<= 1;
    }
  }
}

impl Emu {
  // https://www.nesdev.org/wiki/PPU_registers
  pub fn ppu_reg_read(&mut self, addr: u16) -> u8 {
    let ppu = &mut self.ppu;
    
    match addr {
      // Status
      0x2002 => {
        // Reading this register has the side effect of clearing the PPU's internal w register.
        ppu.w = WriteToggle::default();

        // TODO: open bus
        let res = ppu.stat.bits();
        // Reading PPUSTATUS will return the current state of this flag and then clear it.
        
        // TODO: reading edge cases
        ppu.stat.remove(Status::Vblank);

        res
      }
      // OamData
      0x2004 => ppu.oam.0[ppu.oam_addr as usize],
      // PpuData
      0x2007 => self.ppu_read8(),

      // TODO: open bus
      _ => 0,
    }
  }

  // https://www.nesdev.org/wiki/PPU_registers
  pub fn ppu_reg_write(&mut self, addr: u16, val: u8) {
    let ppu = &mut self.ppu;
    
    match addr {
      // Ctrl
      0x2000 => {
        let old_nmi_enabled = ppu.ctrl.vblank_nmi_enabled;
        let new_nmi_enabled = val & 0x80 != 0;

        // Changing NMI enable from 0 to 1 while the vblank flag in PPUSTATUS is 1 will immediately trigger an NMI.
        if !old_nmi_enabled && new_nmi_enabled && ppu.stat.contains(Status::Vblank) {
          self.events.insert(emu::Events::NMI);
        }
        ppu.ctrl.vblank_nmi_enabled = new_nmi_enabled;
        
        ppu.t.set_nametbl_x(val & 1);
        ppu.t.set_nametbl_y((val >> 1) & 1);

        ppu.ctrl.vram_addr_inc = if val & 0x4 == 0 { 1 } else { 32 };
        ppu.ctrl.spr_pttrntbl_addr = if val & 0x8 == 0 { 0 } else { 0x1000 };
        ppu.ctrl.bg_pttrntbl_addr = if val & 0x10 == 0 { 0 } else { 0x1000 };
        ppu.ctrl.spr_size = if val & 0x20 == 0 { 8 } else { 16 };
      }
      // Mask
      0x2001 => ppu.mask = Mask::from_bits_retain(val),
      // OamAddr
      0x2003 => ppu.oam_addr = val,
      // OamData
      0x2004 => {
        ppu.oam.0[ppu.oam_addr as usize] = val;
        ppu.oam_addr = ppu.oam_addr.wrapping_add(1);
      }
      // Scroll
      0x2005 => {
        match ppu.w {
          WriteToggle::First  => {
            // Scroll X
            ppu.t.set_coarse_x(val >> 3);
            // coarse x
            ppu.x = val & 0b111;
            ppu.w = WriteToggle::Second;
          }
          WriteToggle::Second => {
            // Scroll Y
            ppu.t.set_coarse_y(val >> 3);
            ppu.t.set_fine_y(val & 0b111);
            ppu.w = WriteToggle::First;
          }
        };
      }
      // PpuAddr
      0x2006 => {
        match ppu.w {
          WriteToggle::First  => {
            // The 16-bit address is written to PPUADDR one byte at a time, high byte first.
            // ppu.ppu_addr = byte_hi_set(ppu.ppu_addr, val);
            ppu.t.0 = byte_set_hi(ppu.t.0, val);
            // bit 14 of the internal t register that holds the data written to PPUADDR is forced to 0 when writing the PPUADDR high byte.
            ppu.t.0 &= 0x3fff;
            ppu.w = WriteToggle::Second;
          }
          WriteToggle::Second => {
            // ppu.ppu_addr = byte_lo_set(ppu.ppu_addr, val);
            ppu.t.0 = byte_set_lo(ppu.t.0, val);
            ppu.v.0 = ppu.t.0;
            ppu.w = WriteToggle::First;
          }
        };
      }
      // PpuData
      0x2007 => self.ppu_write8(val),
      _ => {}
    }
  }

  fn fetching_read(&mut self, addr: u16) -> u8 {
    self.ppu_dispatch_read(addr)
  }

  fn ppu_read8(&mut self) -> u8 {
    // This read buffer is updated on every PPUDATA read, but only after the previous contents have been returned to the CPU, effectively delaying PPUDATA reads by one. 
    let res = self.ppu.ppu_data;
    self.ppu.ppu_data = self.ppu_dispatch_read(self.ppu.v.0);
    // println!("Read {res} from PPU {}", self.ppu.v.0);
    self.ppu.increase_addr();

    // TODO: palette ram read
    // https://www.nesdev.org/wiki/PPU_registers#Reading_palette_RAM

    res
  }

  fn ppu_write8(&mut self, val: u8) {
    self.ppu_dispatch_write(self.ppu.v.0, val);
    // println!("Wrote {val} to PPU {}", self.ppu.v.0);
    self.ppu.increase_addr();
  }

  #[deprecated]
  pub fn ppu_step_simple(&mut self) {
    if self.ppu.scanline == 241 && self.ppu.cycle == 1 {
      if self.ppu.ctrl.vblank_nmi_enabled {
        self.events.insert(emu::Events::NMI);
      }
    }
    
    if self.ppu.cycle >= 340 {
      if self.ppu.scanline >= 261 {
        self.ppu.scanline = -1;
      }

      self.ppu.scanline += 1;
      self.ppu.cycle = -1;
    }
    self.ppu.cycle += 1;
  }

  // TODO: clean up this
  pub fn ppu_step(&mut self) {
    let ppu = &mut self.ppu;

    if ppu.scanline == 261 || ppu.scanline <= 239 {
      if ppu.scanline == 261 && ppu.cycle == 1 {
        // PostRender Line
        ppu.stat.clear();
      } else if ppu.scanline == 261 && (ppu.cycle == 280 || ppu.cycle == 304) {
        ppu.scroll_y_tx();
      } else if ppu.cycle == 0 {
        self.compute_sprite_scanline();
      } else if (ppu.cycle >= 1 && ppu.cycle < 256) || (ppu.cycle >= 321 && ppu.cycle <= 336) {
        self.fetch_step_bg();
      } else if ppu.cycle == 256 {
        ppu.scroll_y_inc();
        self.evaluate_sprites();
      } else if ppu.cycle == 257 {
        ppu.scroll_x_tx();
        self.fetch_step_spr();
      } else if ppu.cycle >= 257 && ppu.cycle <= 320 {
        self.fetch_step_spr();
      }
    } else if ppu.scanline >= 241 && ppu.scanline < 261 {
      if ppu.scanline == 241 && ppu.cycle == 1 {
        ppu.stat.insert(Status::Vblank);
        
        if ppu.ctrl.vblank_nmi_enabled {
          self.events.insert(emu::Events::NMI);
        }
      }
    }

    if self.ppu.rendering_enabled() && self.ppu.cycle >= 1 && self.ppu.cycle < 257 && self.ppu.scanline < 240 { 
      self.push_pixel();
    }

    let ppu = &mut self.ppu;
    ppu.cycle += 1;
    if ppu.cycle >= 341 {
      ppu.cycle = 0;
      ppu.scanline += 1;
      if ppu.scanline > 261 {
        ppu.scanline = 0;
        self.events.insert(emu::Events::FRAME);
      }
    }
  }

  fn fetch_step_bg(&mut self) {
    self.ppu.shifter_update();
    
    // we do cycle - 1 as we skip the idle cycle to be aligned to 8
    match (self.ppu.cycle - 1) % 8 {
      // https://www.nesdev.org/wiki/PPU_scrolling#Tile_and_attribute_fetching
      0 => {
        self.ppu.shifter_load();

        // TODO. optimize this read (ends always in vram)
        let nametbl = self.fetching_read(0x2000 | (self.ppu.v.0 & 0x0fff));
        self.ppu.fetcher.nametbl = nametbl;
      }
      2 => {
        let v = &self.ppu.v.0;
        let addr = 0x23c0 | (v & 0xc00) | ((v >> 4) & 0x38) | ((v >> 2) & 0x7);
        // let addr = 0x23c0 
        //   | ((v.nametbl_y() as u16) << 11)
        //   | ((v.nametbl_x() as u16) << 10)
        //   | (((v.coarse_y() as u16) >> 2) << 3)
        //   | ((v.coarse_x() as u16) >> 2);
        
        // TODO. optimize this read (ends always in vram)
        let attr = self.fetching_read(addr);
        // we fetched the attribute, now we have to extract the correct 2 bits
        self.ppu.fetcher.attribute = self.palette_from_attribute(attr);
      }
      // https://www.nesdev.org/wiki/PPU_pattern_tables
      4 => {
        let addr = self.ppu.ctrl.bg_pttrntbl_addr 
          | ((self.ppu.fetcher.nametbl as u16) << 4)
          | self.ppu.v.fine_y() as u16;

        // TODO. optimize this read (ends always in chr)
        self.ppu.fetcher.pttrn_lo = self.fetching_read(addr);
      }
      6 => {
        let addr = self.ppu.ctrl.bg_pttrntbl_addr
          | ((self.ppu.fetcher.nametbl as u16) << 4)
          | 0x8
          | self.ppu.v.fine_y() as u16;

        // TODO. optimize this read (ends always in chr)
        self.ppu.fetcher.pttrn_hi = self.fetching_read(addr);
        
        // IMPORTANT: increase v by one
        // self.ppu.v.set_coarse_x(self.ppu.v.coarse_x().wrapping_add(1));
        self.ppu.scroll_x_inc();
      }

      _ => {}
    }
  }

  fn palette_from_attribute(&self, mut attr: u8) -> u8 {
    let v = &self.ppu.v;
    
    // can this be done without ifs?
    if v.coarse_y() & 0x2 != 0 { attr >>= 4; }
    if v.coarse_x() & 0x2 != 0 { attr >>= 2; }

    attr & 0b11
  }

  fn bg_color_from_palette(&mut self, palette: u8, pixel: u8) -> u8 {
    let addr = (palette << 2) | pixel;
    // TODO: we know we will fetch from palette, optimize this so we dont have to check for all addresses
    self.fetching_read(0x3f00 | addr as u16)
  }

  fn spr_color_from_palette(&mut self, palette: u8, pixel: u8) -> u8 {
    let addr = (palette << 2) | pixel;
    // TODO: we know we will fetch from palette, optimize this so we dont have to check for all addresses
    self.fetching_read(0x3f10 | addr as u16)
  }

  fn push_pixel(&mut self) {    
    // On every dot in these background fetch regions, a 4-bit pixel is selected by the fine x register from the low 8 bits of the pattern and attributes shift registers, which are then shifted. 
    let shift_mask = 0x8000 >> self.ppu.x;
    let shifter = &mut self.ppu.shifter;
    
    let pixel_lo = shifter.shift_ptrn_lo & shift_mask > 0;
    let pixel_hi = shifter.shift_ptrn_hi & shift_mask > 0;
    let pixel = ((pixel_hi as u8) << 1) | (pixel_lo as u8);

    let palette_lo = shifter.shift_attr_lo & shift_mask > 0;
    let palette_hi = shifter.shift_attr_hi & shift_mask > 0;
    let palette = ((palette_hi as u8) << 1) | (palette_lo as u8);

    let spr_pixel = &self.ppu.spr_scanline.0[self.ppu.cycle as usize - 1];
    let color_id = if spr_pixel.pixel() > 0 && spr_pixel.priority() {
      self.spr_color_from_palette(self.ppu.oam.0[spr_pixel.index() as usize], spr_pixel.pixel())
    } else {
      self.bg_color_from_palette(palette, pixel)
    };

    let pos = self.ppu.scanline as usize * 256 + (self.ppu.cycle as usize - 1);
    self.framebuf[pos] = color_id;
  }

  fn evaluate_sprites(&mut self) {
    let ppu = &mut self.ppu;
    ppu.oam_line.clear();
    ppu.spr_scanline.0.fill(SprScanlineData(0));

    let shifter = &mut ppu.shifter;
    shifter.spr_pttrn_hi.fill(0);
    shifter.spr_pttrn_lo.fill(0);

    let scanline = ppu.scanline;
    for (i, y) in ppu.oam.0.iter().copied().enumerate().step_by(4) {
      // Sprite data is delayed by one scanline; you must subtract 1 from the sprite's Y coordinate 
      // let y = y.wrapping_sub(1) as i16;

      let dist = ppu.scanline - y as i16;
      if dist >= 0 && dist < ppu.ctrl.spr_size as i16 {
        let sprite = Sprite::new(i as u8, &ppu.oam.0[i..i+4]);
        ppu.oam_line.push(sprite);
      }
    }

    ppu.stat.set(Status::SprOverflow, ppu.oam_line.len() > 8);
    ppu.oam_line.resize_with(8, || Sprite::empty());
  }

  fn compute_sprite_scanline(&mut self) {
    let ppu = &mut self.ppu;

    for (i, sprite) in ppu.oam_line.iter().enumerate() {
      for col in 0..8 {
        let mask = 0x80 >> col;
        let curr_pixel_lo = ppu.shifter.spr_pttrn_lo[i] & mask > 0;
        let curr_pixel_hi = ppu.shifter.spr_pttrn_hi[i] & mask > 0;
        let curr_pixel = ((curr_pixel_hi as u8) << 1) | curr_pixel_lo as u8;

        let x = sprite.x.wrapping_add(col) as usize;
        let scanline_pixel =  &ppu.spr_scanline.0[x];
        
        // if scanline_pixel.pixel() == 0 || scanline_pixel.index() > i as u8 {
        if scanline_pixel.pixel() == 0 {
          // pixel is transparent, draw curr pixel, or higher index wins
          let new_pixel = SprScanlineDataBuilder::new()
            .with_index(i as u8)
            .with_priority(sprite.priority)
            .with_pixel(curr_pixel)
            .build();

          ppu.spr_scanline.0[x] = new_pixel;
        }
      }
    }
  }

  fn fetch_step_spr(&mut self) {
    let ppu = &mut self.ppu;
    
    match (ppu.cycle - 257) % 8 {
      4 => {
        let spr_id = (ppu.cycle - 257) / 8;
        let sprite = &ppu.oam_line[spr_id as usize];
        let dist = ppu.scanline - sprite.y as i16;
        let fine_y = if sprite.flip_vert {
          7 - dist
        } else {
          dist
        };

        let pttrn_addr = ppu.ctrl.spr_pttrntbl_addr 
          | ((sprite.nametbl as u16) << 4)
          | (fine_y & 0b111) as u16;
        
        let flip_hori = sprite.flip_hori;

        // TODO. optimize this read (ends always in chr)
        let mut pttrn = self.fetching_read(pttrn_addr);
        if flip_hori {
          pttrn = pttrn.reverse_bits();
        }

        self.ppu.shifter.spr_pttrn_lo[spr_id as usize] = pttrn;
      }
      6 => {
        let spr_id = (ppu.cycle - 257) / 8;
        let sprite = &ppu.oam_line[spr_id as usize];
        let dist = ppu.scanline - sprite.y as i16;
        let fine_y = if sprite.flip_vert {
          7 - dist
        } else {
          dist
        };

        let pttrn_addr = ppu.ctrl.spr_pttrntbl_addr 
          | ((sprite.nametbl as u16) << 4)
          | 0x8
          | (fine_y & 0b111) as u16;

        let flip_hori = sprite.flip_hori;

        // TODO. optimize this read (ends always in chr)
        let mut pttrn = self.fetching_read(pttrn_addr);
        if flip_hori {
          pttrn = pttrn.reverse_bits();
        }

        self.ppu.shifter.spr_pttrn_hi[spr_id as usize] = pttrn;
      }
      _ => {}
    }
  }
}