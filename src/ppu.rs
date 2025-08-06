use std::ops::{Shl, Shr};

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
  spr_size: u8,
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
  pattern_lo: u8,
  pattern_hi: u8,
}

#[derive(Default)]
enum FetcherState {
  #[default]
  Nametable,
  Attribute,
  PatternLo,
  PatternHi,
}

#[derive(Default)]
struct ShifterData {
  shift_ptrn_lo: u16,
  shift_ptrn_hi: u16,
  // we simulate the 1bit latch by always pushing 8 bytes (thus having a 16bit shifter)
  shift_attr_lo: u16,
  shift_attr_hi: u16,
}

#[derive(Default)]
enum RenderState {
  #[default]
  PreRenderScanline,
  FirstIdleCycle,
  RenderingBg,
  RenderingUnusedBg,
  RenderingSpr,
  RenderingEnd,
  PostRenderScanline,
  Vblank,
}

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

  oam: [u8; 256],

  state: RenderState,
  pub cycle: i16,
  pub scanline: i16,
  pixels_count: usize,
  odd_frame: bool,

  fetcher: FetcherData,
  shifter: ShifterData,
}

impl Default for Ppu2C02 {
  fn default() -> Self {
    Self { ctrl: Default::default(), mask: Default::default(), stat: Default::default(), v: Default::default(), t: Default::default(), x: Default::default(), w: Default::default(), oam_addr: Default::default(), ppu_data: Default::default(), oam: [0; 256], state: Default::default(), cycle: Default::default(), scanline: Default::default(), pixels_count: Default::default(), odd_frame: Default::default(), fetcher: Default::default(), shifter: Default::default() }
  }
}

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
      if v.fine_y() < 7 {        // if fine Y < 7
        v.set_fine_y(v.fine_y() + 1);          // increment fine Y
      } else {                  
        v.set_fine_y(0);                    // fine Y = 0
        let mut y = v.coarse_y();        // let y = coarse Y
        if y == 29 {
          y = 0;                         // coarse Y = 0
          v.set_nametbl_y(!v.nametbl_y());    // switch vertical nametable
        } else if y == 31 {
          y = 0;                        // coarse Y = 0, nametable not switched
        } else {
          y += 1;
        }                         // increment coarse Y
        v.set_coarse_y(y);     // put coarse Y back into v
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
    shifter.shift_ptrn_lo = byte_set_hi(shifter.shift_ptrn_lo, fetcher.pattern_lo);
    shifter.shift_ptrn_hi = byte_set_hi(shifter.shift_ptrn_hi, fetcher.pattern_hi);
    
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

        0xff
      }
      // OamData
      0x2004 => ppu.oam[ppu.oam_addr as usize],
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
          self.interrupts.insert(emu::Interrupts::NMI);
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
        ppu.oam[ppu.oam_addr as usize] = val;
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

  pub fn ppu_step_simple(&mut self) {
    if self.ppu.scanline == 241 && self.ppu.cycle == 1 {
      if self.ppu.ctrl.vblank_nmi_enabled {
        self.interrupts.insert(emu::Interrupts::NMI);
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

  // pub fn ppu_step(&mut self) {
  //   // TODO: lookup table handlers?
  //   match self.ppu.state {
  //     RenderState::PreRenderScanline => {
  //       // no drawing occurs here
  //       self.fetch_step();

  //       let ppu = &mut self.ppu;
        
  //       if ppu.cycle == 304 {
  //         ppu.v.set_nametbl_y(ppu.t.nametbl_y());
  //         ppu.v.set_coarse_y(ppu.t.coarse_y());
  //         ppu.v.set_fine_y(ppu.t.fine_y());
  //       }

  //       else if (ppu.odd_frame && ppu.cycle == 339) 
  //         || (!ppu.odd_frame && ppu.cycle == 340) 
  //       {
  //         ppu.state = RenderState::FirstIdleCycle;
  //         ppu.scanline = 0;
  //         ppu.cycle = -1;
  //         ppu.odd_frame = !ppu.odd_frame;
  //         self.frame_ready = true;
  //       }
  //     }
  //     RenderState::FirstIdleCycle => self.ppu.state = RenderState::RenderingBg,
  //     RenderState::RenderingBg => {
  //       self.push_pixel();
  //       self.fetch_step();
        
  //       let ppu = &mut self.ppu;
  //       if ppu.cycle == 240 {
  //         // wrap coarse x here
  //         ppu.v.set_coarse_x(0);
  //         ppu.v.set_nametbl_x(ppu.v.nametbl_x() ^ 1);
          
  //         // unused tile fetch
  //         ppu.state = RenderState::RenderingUnusedBg;
  //       }
  //     }
  //     RenderState::RenderingUnusedBg => {
  //       let ppu = &mut self.ppu;
        
  //       if ppu.cycle == 256 {
  //         // y increment
  //         let v = &mut ppu.v;
  //         if v.fine_y() < 7 {        // if fine Y < 7
  //           v.set_fine_y(v.fine_y() + 1);          // increment fine Y
  //         } else {                  
  //           v.set_fine_y(0);                    // fine Y = 0
  //           let mut y = v.coarse_y();        // let y = coarse Y
  //           if y == 29 {
  //             y = 0;                         // coarse Y = 0
  //             v.set_nametbl_y(v.nametbl_y() ^ 1);    // switch vertical nametable
  //           } else if y == 31 {
  //             y = 0;                        // coarse Y = 0, nametable not switched
  //           } else {
  //             y += 1;
  //           }                         // increment coarse Y
  //           v.set_coarse_y(y);     // put coarse Y back into v
  //         }

  //         ppu.v.set_coarse_x(ppu.t.coarse_x());
  //         ppu.v.set_nametbl_x(ppu.t.nametbl_x());

  //         // TODO: sprite evaluation for next scanline
  //         ppu.state = RenderState::RenderingSpr;
  //       }
  //     }
  //     RenderState::RenderingSpr => {
  //       // TODO: sprites fetchin and drawing

  //       if self.ppu.cycle == 320 {
  //         self.ppu.state = RenderState::RenderingEnd;
  //       }
  //     }
  //     RenderState::RenderingEnd => {
  //       // no drawing here, only fetches to the first two tiles for next scanline
  //       self.fetch_step();

  //       let ppu= &mut self.ppu;
  //       if ppu.cycle == 340 {
  //         if ppu.scanline == 239 {
  //           ppu.state = RenderState::PostRenderScanline;
  //           ppu.pixels_count = 0;
  //         } else {
  //           ppu.state = RenderState::FirstIdleCycle;
  //         }
  //         ppu.cycle = -1;
  //         ppu.scanline += 1;
  //       }
  //     }
  //     RenderState::PostRenderScanline => {
  //       // do nothing

  //       let ppu = &mut self.ppu;
  //       if ppu.scanline == 241 && ppu.cycle == 1 {
  //         ppu.state = RenderState::Vblank;
          
  //         ppu.stat.insert(Status::Vblank);
  //         self.interrupts.insert(emu::Interrupts::NMI);
  //       }
  //     }
  //     RenderState::Vblank => {
  //       // do nothing

  //       let ppu = &mut self.ppu;
  //       if ppu.scanline == 261 && ppu.cycle == 1 {
  //         ppu.state = RenderState::PreRenderScanline;
  //         ppu.stat.clear();
  //         // first pre render line fetch
  //         self.fetch_step();
  //       }
  //     }
  //   }

  //   if self.ppu.cycle == 340 {
  //     self.ppu.scanline += 1;
  //     self.ppu.cycle = -1;
  //   }
  //   self.ppu.cycle += 1;
  // }

  pub fn ppu_step(&mut self) {
    let ppu = &mut self.ppu;

    if ppu.scanline == 261 || (ppu.scanline >= 0 && ppu.scanline < 240) {
      if ppu.scanline == 261 && ppu.cycle == 1 {
        ppu.stat.clear();
      } else if ppu.scanline == 261 && ppu.cycle == 280 {
        ppu.scroll_y_tx(); 
      } else if (ppu.cycle >= 1 && ppu.cycle < 256) || (ppu.cycle >= 321 && ppu.cycle <= 336) {
        self.fetch_step();
      } else if ppu.cycle == 256 {
        ppu.scroll_y_inc();
      } else if ppu.cycle == 257 {
        ppu.scroll_x_tx();
      }
    } else if ppu.scanline >= 241 && ppu.scanline < 261 {
      if ppu.scanline == 241 && ppu.cycle == 1 {
        ppu.stat.insert(Status::Vblank);
        
        if ppu.ctrl.vblank_nmi_enabled {
          self.interrupts.insert(emu::Interrupts::NMI);
        }
      }
    }

    if self.ppu.mask.contains(Mask::EnableBg) && self.ppu.cycle >= 1 && self.ppu.cycle < 257 && self.ppu.scanline < 240 { 
      self.push_pixel();
    }

    let ppu = &mut self.ppu;
    ppu.cycle += 1;
    if ppu.cycle >= 341 {
      ppu.cycle = 0;
      ppu.scanline += 1;
      if ppu.scanline > 261 {
        ppu.scanline = 0;
        self.frame_ready = true; 
      }
    }
  }

  fn fetch_step(&mut self) {
    self.ppu.shifter_update();
    
    // we do cycle - 1 as we skip the idle cycle to be aligned to 8
    match (self.ppu.cycle - 1) % 8 {
      // https://www.nesdev.org/wiki/PPU_scrolling#Tile_and_attribute_fetching
      0 => {
        self.ppu.shifter_load();
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
        
        let attr = self.fetching_read(addr);
        // we fetched the attribute, now we have to extract the correct 2 bits
        self.ppu.fetcher.attribute = self.palette_from_attribute(attr);
      }
      // https://www.nesdev.org/wiki/PPU_pattern_tables
      4 => {
        let addr = self.ppu.ctrl.bg_pttrntbl_addr 
          | ((self.ppu.fetcher.nametbl as u16) << 4)
          | self.ppu.v.fine_y() as u16;

        self.ppu.fetcher.pattern_lo = self.fetching_read(addr);
      }
      6 => {
        let addr = self.ppu.ctrl.bg_pttrntbl_addr
          | ((self.ppu.fetcher.nametbl as u16) << 4)
          | 0x8
          | self.ppu.v.fine_y() as u16;

        self.ppu.fetcher.pattern_hi = self.fetching_read(addr);
        
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

  fn push_pixel(&mut self) {
    // On every dot in these background fetch regions, a 4-bit pixel is selected by the fine x register from the low 8 bits of the pattern and attributes shift registers, which are then shifted. 
    let shift_mask = 0x8000 >> self.ppu.x;
    let shifter = &self.ppu.shifter;
    
    let pixel_lo = shifter.shift_ptrn_lo & shift_mask > 0;
    let pixel_hi = shifter.shift_ptrn_hi & shift_mask > 0;
    let pixel = ((pixel_hi as u8) << 1) | (pixel_lo as u8);

    let palette_lo = shifter.shift_attr_lo & shift_mask > 0;
    let palette_hi = shifter.shift_attr_hi & shift_mask > 0;
    let palette = ((palette_hi as u8) << 1) | (palette_lo as u8);

    let color_id = self.bg_color_from_palette(palette, pixel);

    let pos = self.ppu.scanline as usize * 256 + (self.ppu.cycle as usize - 1);
    self.framebuf[pos] = color_id;
    self.ppu.pixels_count = (self.ppu.pixels_count + 1) % (256 * 240);
  }

  pub fn render_nametbl0(&mut self) {
    self.framebuf.fill(0);

    for y in 0..240 {
      for x in 0..256 {
        let nametbl_id = y/8 * 32 + x/8;
        let nametbl = self.fetching_read(0x2000 + nametbl_id);
        let attr_id = 0x23c0 | (((y/4) & 0b111) << 3) | ((x/4) & 0b111);
        let mut attr = self.fetching_read(attr_id);

        // can this be done without ifs?
        if (x/8) & 0x2 != 0 { attr >>= 4; }
        if (x/8) & 0x2 != 0 { attr >>= 2; }

        let attr = attr & 0b11;
        let palette = self.palette_from_attribute(attr);

        let ptrn_id = (nametbl << 4) as u16 + (y%8);
        let ptrn_lo = self.fetching_read(ptrn_id as u16);
        let ptrn_hi = self.fetching_read(ptrn_id as u16 + 8);
        
        let pixel = (((ptrn_hi >> (x % 8)) & 1) << 1) | ((ptrn_lo >> (x % 8)) & 1);
        let pos =  y * 256 + x;
        self.framebuf[pos as usize] = self.bg_color_from_palette(palette, pixel);
      }
    }
  }
}