use bitflags::Flags;
use crate::{dma::Dma, emu::Emu, utils::{byte_set_hi, byte_set_lo}};

bitflags::bitflags! {
  #[derive(Default, Debug)]
  struct Ctrl: u8 {
    const AddrIncr  = 1 << 2;
    const SprTable  = 1 << 3;
    const BgTable   = 1 << 4;
    const SprSize   = 1 << 5;
    const NmiEnable = 1 << 7;
  }

  #[derive(Default, Debug)]
  struct Status: u8 {
    const SprOvfl = 1 << 5;
    const Spr0Hit = 1 << 6;
    const Vblank  = 1 << 7;
  }

  #[derive(Default, Debug)]
  struct Mask: u8 {
    const GreyScale   = 1 << 0;
    const ShowBgLeft  = 1 << 1;
    const ShowSprLeft = 1 << 2;
    const BgEnable    = 1 << 3;
    const SprEnable   = 1 << 4;
    const RedEmphasis   = 1 << 5;
    const GreenEmphasis = 1 << 6;
    const BlueEmphasis  = 1 << 7;
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

#[repr(u8)]
#[derive(Default)]
enum WriteToggle {
  #[default] First, Second
}

#[derive(Default)]
struct FetcherData {
  nametbl: u8,
  pttrn_addr: u16,
  attribute: u8,
  pttrn_lo: u8,
  pttrn_hi: u8,
}

#[derive(Default)]
struct ShifterData {
  shift_ptrn_lo: u16,
  shift_ptrn_hi: u16,
  // we simulate the 1bit latch by always pushing 8 bytes (thus having a 16bit shifter)
  shift_attr_lo: u16,
  shift_attr_hi: u16,
}

#[derive(Default, Debug, Clone)]
struct Sprite {
  y: u8,
  nametbl: u8,
  attr: u8,
  x: u8,
  pttrn_lo: u8,
  pttrn_hi: u8,
}
impl Sprite {
  pub fn new(index: u8, bytes: &[u8]) -> Self {
    Self {
      y: bytes[0],
      nametbl: bytes[1],
      // bits 2-4 of attribute are empty, we can use them to store spr0hit
      attr: (bytes[2] & !0x1c) | (((index == 0) as u8) << 2),
      x: bytes[3],
      ..Default::default()
    }
  }

  pub fn empty() -> Self {
    Self {
      nametbl: 0xff,
      ..Default::default()
    }
  }

  pub fn palette(&self) -> u8 { self.attr & 0b11 }
  // we stored spr0 in unused bits of attribute to save memory
  pub fn spr0(&self) -> bool { self.attr & 0x4 != 0 }
  // 0: in front of background; 1: behind background
  pub fn priority(&self) -> bool { self.attr & 0x20 == 0 }
  pub fn flip_hori(&self) -> bool { self.attr & 0x40 != 0 }
  pub fn flip_vert(&self) -> bool { self.attr & 0x80 != 0 }
}

pub struct Oam(pub [u8; 256]);
impl Default for Oam {
  fn default() -> Self { Self([0; 256]) }
}

#[bitfields::bitfield(u8)]
#[derive(Clone, Copy)]
struct SprScanlineData {
  #[bits(2)]
  pixel: u8,
  #[bits(2)]
  palette: u8,
  priority: bool,
  spr0: bool,
  #[bits(2)]
  _unused: u8,
}

struct SprScanline([SprScanlineData; 256]);
impl Default for SprScanline {
  fn default() -> Self { Self([0.into(); 256]) }
}

// Clear explanation of how PPU works in Rust
// https://docs.rs/nes-ppu/latest/src/nes_ppu
#[derive(Default)]
pub struct Ppu2C02 {

  ctrl: CtrlStrut,
  mask: Mask,
  stat: Status,
  
  v: LoopyReg,
  t: LoopyReg,
  x: u8,
  w: WriteToggle,

  open_bus: u8,
  oam_addr: u8,
  ppu_data: u8,

  pub oam: Oam,
  oam_tmp: Vec<Sprite>,
  oam_tmp_count: u8,
  spr_scanline: SprScanline,
  pub dma: Dma,

  pub cycle: i16,
  pub scanline: i16,
  pub pixel: usize,
  
  // https://www.nesdev.org/wiki/PPU_frame_timing
  odd_frame: bool,
  vblank_suppress: bool,
  nmi_suppress: bool,

  fetcher: FetcherData,
  shifter: ShifterData,
}

// TODO: rendering_enabled checks

impl Ppu2C02 {
  pub fn new() -> Self {
    Self {
      scanline: -1,
      oam_tmp: vec![Sprite::empty(); 8],
      ..Default::default()
    }
  }

  fn palette_from_attribute(&self, attr: u8) -> u8 {
    let v = self.v.0;
    
    // if v.coarse_y() & 0x2 != 0 { attr >>= 4; }
    // if v.coarse_x() & 0x2 != 0 { attr >>= 2; }

    let shift = ((v & 0x40) >> 4) | (v & 0x02);
    (attr >> shift) & 0b11
  }

  // https://www.nesdev.org/wiki/PPU_scrolling#Tile_and_attribute_fetching
  fn nametbl_addr(&self) -> u16 {
    0x2000 | (self.v.0 & 0x0fff)
  }

  fn attribute_addr(&self) -> u16 {
    let v = self.v.0;
    0x23c0 | (v & 0xc00) | ((v >> 4) & 0x38) | ((v >> 2) & 0x7)
    // 0x23c0 
    //   | ((v.nametbl_y() as u16) << 11)
    //   | ((v.nametbl_x() as u16) << 10)
    //   | (((v.coarse_y() as u16) >> 2) << 3)
    //   | ((v.coarse_x() as u16) >> 2)
  }

  fn bg_pttrn_addr(&self) -> u16 {
    self.ctrl.bg_pttrntbl_addr 
      | ((self.fetcher.nametbl as u16) << 4)
      | self.v.fine_y() as u16

    // self.ctrl.bg_pttrntbl_addr 
    //   + ((self.fetcher.nametbl as u16) * 16)
    //   + self.v.fine_y() as u16
  }

  fn spr_pttrn_addr(&self, sprite: &Sprite) -> u16 {
    // after evaluation, we are 100% sure scanline is always bigger than y
    let dist = self.scanline - sprite.y as i16;
    
    let fine_y = if sprite.flip_vert() {
      7 - dist
    } else {
      dist
    } as u16;

    if self.ctrl.spr_size == 8 {
      // 8x8 sprites
      self.ctrl.spr_pttrntbl_addr 
      | ((sprite.nametbl as u16) << 4)
      | fine_y & 0b111
    } else {
      // 8x16 sprites
      let mut bottom_tile = dist >= 8;
      // In 8x16 mode, vertical flip flips each of the subtiles and also exchanges their position; the odd-numbered tile of a vertically flipped sprite is drawn on top.
      if sprite.flip_vert() { bottom_tile = !bottom_tile; }
      
      // For 8x16 sprites (bit 5 of PPUCTRL set), the PPU ignores the pattern table selection and selects a pattern table from bit 0 of this number. 
      (sprite.nametbl as u16 & 1) << 12
      | (((sprite.nametbl & !1) as u16 | bottom_tile as u16) << 4)
      | fine_y & 0b111
    }
  }

  fn rendering_enabled(&self) -> bool {
    self.mask.contains(Mask::BgEnable) || self.mask.contains(Mask::SprEnable)  
  }

  fn is_rendering(&self) -> bool {
    self.rendering_enabled() && self.scanline < 240 
  }

  fn shifter_load_from_fetcher(&mut self) {
    // On every 8th dot in these background fetch regions (the same dot on which the coarse x component of v is incremented), the pattern and attributes data are transferred into registers used for producing pixel data. 
    let fetcher = &self.fetcher;
    let shifter = &mut self.shifter;

    // For the pattern data, these transfers are into the LOW 8 bits of two 16-bit shift registers.
    // Wiki says high bits, but it is wrong (even the diagram in the same page shows it)
    shifter.shift_ptrn_lo = byte_set_lo(shifter.shift_ptrn_lo, fetcher.pttrn_lo);
    shifter.shift_ptrn_hi = byte_set_lo(shifter.shift_ptrn_hi, fetcher.pttrn_hi);
    
    // For the attributes data, only 2 bits are transferred and into two 1-bit latches that feed 8-bit shift registers. 
    // Here for conveniece, we use 16-bit shift registers too. 
    let attr_lo = if fetcher.attribute & 0b01 != 0 { 0xff } else { 0 };
    shifter.shift_attr_lo = byte_set_lo(shifter.shift_attr_lo, attr_lo);
    
    let attr_hi = if fetcher.attribute & 0b10 != 0 { 0xff } else { 0 };
    shifter.shift_attr_hi = byte_set_lo(shifter.shift_attr_hi, attr_hi);
  }

  fn shifter_update(&mut self) {
    if self.rendering_enabled() {
      self.shifter.shift_ptrn_lo <<= 1;
      self.shifter.shift_ptrn_hi <<= 1;
      self.shifter.shift_attr_lo <<= 1;
      self.shifter.shift_attr_hi <<= 1;
    }
  }

  fn shifter_get_pixel_n_palette(&mut self) -> (u8, u8) {
    let shifter = &self.shifter;
    let shift_mask = 0x8000 >> self.x;

    let pixel_lo = shifter.shift_ptrn_lo & shift_mask > 0;
    let pixel_hi = shifter.shift_ptrn_hi & shift_mask > 0;
    let bg_pixel = ((pixel_hi as u8) << 1) | (pixel_lo as u8);

    let palette_lo = shifter.shift_attr_lo & shift_mask > 0;
    let palette_hi = shifter.shift_attr_hi & shift_mask > 0;
    let palette = ((palette_hi as u8) << 1) | (palette_lo as u8);

    (bg_pixel, palette)
  }

  pub fn oam_write(&mut self, val: u8) {
    self.oam.0[self.oam_addr as usize] = val;
    self.oam_addr = self.oam_addr.wrapping_add(1);
  }
}

impl Emu {
  pub fn ppu_reset(&mut self) {
    self.ppu = Ppu2C02::new();
  }

  fn increase_addr(&mut self) {
    let ppu = &mut self.ppu;
    // if !ppu.is_rendering() {
      ppu.v.0 = (ppu.v.0 + ppu.ctrl.vram_addr_inc) & 0x3fff;
      self.update_ppu_bus(self.ppu.v.0);
    // } else {
      // https://www.nesdev.org/wiki/PPU_scrolling#$2007_(PPUDATA)_reads_and_writes
      //   self.inc_scroll_x();
      //   self.inc_scroll_y();
    // }
  }

  fn inc_scroll_x(&mut self) {
    // if self.ppu.rendering_enabled() {
      let v = &mut self.ppu.v;
      if v.coarse_x() == 31 {
        v.set_coarse_x(0);
        v.set_nametbl_x(!v.nametbl_x());
      } else {
        v.set_coarse_x(v.coarse_x() + 1);
      }
    // }
  }

  fn inc_scroll_y(&mut self) {
    // if self.ppu.rendering_enabled() {
      let v = &mut self.ppu.v;
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
    // }
  }

  fn restore_scroll_x(&mut self) {
    // if self.ppu.rendering_enabled() {
      self.ppu.v.set_nametbl_x(self.ppu.t.nametbl_x());
      self.ppu.v.set_coarse_x(self.ppu.t.coarse_x());
    // }
  }

  fn restore_scroll_y(&mut self) {
    // if self.ppu.rendering_enabled() {
      self.ppu.v.set_nametbl_y(self.ppu.t.nametbl_y());
      self.ppu.v.set_coarse_y(self.ppu.t.coarse_y());
      self.ppu.v.set_fine_y(self.ppu.t.fine_y());
    // }
  }

  // https://www.nesdev.org/wiki/PPU_registers
  pub fn ppu_reg_read(&mut self, addr: u16) -> u8 {
    let ppu = &mut self.ppu;
    
    let res = match addr {
      // Status
      0x2002 => {
        // Reading this register has the side effect of clearing the PPU's internal w register.
        ppu.w = WriteToggle::First;
        
      // Reading $2002 within a few PPU clocks of when VBL is set results in special-case behavior. 
      // Reading one PPU clock before reads it as clear and never sets the flag or generates NMI for that frame. 
      // Reading on the same PPU clock or one later reads it as set, clears it, and suppresses the NMI for that frame.
      //  Reading two or more PPU clocks before/after it's set behaves normally (reads flag's value, clears it, and doesn't affect NMI operation). 

        if ppu.scanline == 241 && ppu.cycle <= 2 {
          if ppu.cycle == 0 {
            ppu.stat.remove(Status::Vblank);
            ppu.vblank_suppress = true;
          } else if ppu.cycle <= 2 {
            ppu.stat.insert(Status::Vblank);
          }
          
          self.mem.nmi = false;
          ppu.nmi_suppress = true;
        }

        let res = ppu.stat.bits();
        // Reading PPUSTATUS will return the current state of this flag and then clear it.

        ppu.stat.remove(Status::Vblank);
        res | (ppu.open_bus & 0x1f)
      }
      // OamData
      0x2004 => ppu.oam.0[ppu.oam_addr as usize],
      // PpuData
      0x2007 => self.ppu_read8(),

      _ => ppu.open_bus,
    };

    self.ppu.open_bus = res;
    res
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
          self.mem.nmi = true;
        }
        ppu.ctrl.vblank_nmi_enabled = new_nmi_enabled;

        ppu.t.set_nametbl_x(val & 1);
        ppu.t.set_nametbl_y((val >> 1) & 1);

        ppu.ctrl.vram_addr_inc = if val & 0x4 == 0 { 1 } else { 32 };
        ppu.ctrl.spr_pttrntbl_addr = if val & 0x8 == 0 { 0 } else { 0x1000 };
        ppu.ctrl.bg_pttrntbl_addr = if val & 0x10 == 0 { 0 } else { 0x1000 };
        ppu.ctrl.spr_size = if val & 0x20 == 0 { 8 } else { 16 };

        // self.mapper.notify_ppu_ctrl(val);
      }
      // Mask
      0x2001 => {
        ppu.mask = Mask::from_bits_retain(val);
        if !ppu.rendering_enabled() {
          // During VBlank and when rendering is disabled, the value on the PPU address bus is the current value of the v register. 
          self.update_ppu_bus(self.ppu.v.0);
        }
        // self.mapper.notify_ppu_mask(val);
      }
      // OamAddr
      // TODO: behaviour during rendering: https://www.nesdev.org/wiki/PPU_registers#OAMADDR_-_Sprite_RAM_address_($2003_write)
      0x2003 => ppu.oam_addr = val,
      // OamData
      // TODO: behaviour during rendering: https://www.nesdev.org/wiki/PPU_registers#OAMDATA_-_Sprite_RAM_data_($2004_read/write)
      0x2004 => ppu.oam_write(val),
      // Scroll
      0x2005 => {
        match ppu.w {
          WriteToggle::First  => {
            // Scroll X
            ppu.t.set_coarse_x(val >> 3);
            // coarse x
            ppu.x = val & 0x7;
            ppu.w = WriteToggle::Second;
          }
          WriteToggle::Second => {
            // Scroll Y
            ppu.t.set_coarse_y(val >> 3);
            ppu.t.set_fine_y(val & 0x7);
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

            self.update_ppu_bus(self.ppu.v.0);
          }
        };
      }
      // PpuData
      0x2007 => self.ppu_write8(val),
      _ => {}
    }

    self.ppu.open_bus = val;
  }


  fn ppu_read8(&mut self) -> u8 {
    // This read buffer is updated on every PPUDATA read, but only after the previous contents have been returned to the CPU, effectively delaying PPUDATA reads by one. 
    
    let res = if self.ppu.v.0 >= 0x3f00 {
    // https://www.nesdev.org/wiki/PPU_registers#Reading_palette_RAM
    // WARNING: older PPUs did not have this problem
      self.ppu_palette_read(self.ppu.v.0) | self.ppu.open_bus & 0xc0
    } else {
      self.ppu.ppu_data
    };
    
    self.ppu.ppu_data = self.ppu_dispatch_read(self.ppu.v.0);
    self.increase_addr();

    res
  }

  fn ppu_write8(&mut self, val: u8) {
    self.ppu_dispatch_write(self.ppu.v.0, val);
    self.increase_addr();
  }

  fn bg_fetch_step(&mut self) {
    self.ppu.shifter_update();
    
    // Each memory fetch takes 2 dots: on the 1st, the full address is placed onto the PPU's address bus and the low 8 bits are stored into an external address latch, and on the 2nd, the read is performed.
    // we dont care, and do evrything on the first dot

    // we do cycle - 1 as we skip the idle cycle to be aligned to 8
    match (self.ppu.cycle - 1) % 8 {
      // https://www.nesdev.org/wiki/PPU_scrolling#Tile_and_attribute_fetching
      0 => {
        self.ppu.shifter_load_from_fetcher();
        self.ppu.fetcher.nametbl = self.ppu_dispatch_read(self.ppu.nametbl_addr());
      }
      2 => {        
        let attr = self.ppu_dispatch_read(self.ppu.attribute_addr());
        // we fetched the attribute, now we have to extract the correct 2 bits
        self.ppu.fetcher.attribute = self.ppu.palette_from_attribute(attr);
      }
      // https://www.nesdev.org/wiki/PPU_pattern_tables
      4 => {
        let addr = self.ppu.bg_pttrn_addr();
        self.ppu.fetcher.pttrn_addr = addr;
        self.ppu.fetcher.pttrn_lo = self.ppu_dispatch_read(addr);
      }
      6 => {
        self.ppu.fetcher.pttrn_hi = self
          .ppu_dispatch_read(self.ppu.fetcher.pttrn_addr + 8);

        // IMPORTANT: increase v by one
        self.inc_scroll_x();
      }

      _ => {}
    }
  }

  fn spr_fetch_step(&mut self) {
    let ppu = &mut self.ppu;
    
    // these are still aligned by 8
    match (ppu.cycle - 1) % 8 {
      0 => {
        // unused nt fetch
        self.ppu_dispatch_read(self.ppu.nametbl_addr());
      }
      2 => {
        // ignored nt fetch
        self.ppu_dispatch_read(self.ppu.nametbl_addr());
      }
      4 => {
        let spr_id = (ppu.cycle - 257) / 8;
        let sprite = &ppu.oam_tmp[spr_id as usize];

        let pttrn_addr = ppu.spr_pttrn_addr(sprite);
        ppu.fetcher.pttrn_addr = pttrn_addr;
        let flip_hori = sprite.flip_hori();

        let mut pttrn = self.ppu_dispatch_read(pttrn_addr);
        if flip_hori { pttrn = pttrn.reverse_bits(); }

        self.ppu.oam_tmp[spr_id as usize].pttrn_lo = pttrn;
      }
      6 => {
        let spr_id = (ppu.cycle - 257) / 8;
        let sprite = &ppu.oam_tmp[spr_id as usize];

        let pttrn_addr = ppu.fetcher.pttrn_addr;
        let flip_hori = sprite.flip_hori();

        let mut pttrn = self.ppu_dispatch_read(pttrn_addr + 8);
        if flip_hori { pttrn = pttrn.reverse_bits(); }

        self.ppu.oam_tmp[spr_id as usize].pttrn_hi = pttrn;
      }
      _ => {}
    }
  }

  fn spr_evaluation(&mut self) {
    let ppu = &mut self.ppu;
    ppu.oam_tmp.clear();

    for (i, y) in ppu.oam.0.iter().copied().enumerate().step_by(4) {
      // Sprite data is delayed by one scanline; you must subtract 1 from the sprite's Y coordinate 
      // let y = y.wrapping_sub(1) as i16;

      // we are rendering the NEXT scanline, so we don't want to subtract 1 from y
      let dist = ppu.scanline - y as i16;
      if 0 <= dist && dist < ppu.ctrl.spr_size as i16 {
        let sprite = Sprite::new(i as u8, &ppu.oam.0[i..i+4]);
        ppu.oam_tmp.push(sprite);
      }
    }

    ppu.stat.set(Status::SprOvfl, ppu.rendering_enabled() && ppu.oam_tmp.len() > 8);
    ppu.oam_tmp_count = ppu.oam_tmp.len().min(8) as u8;

    // we always make secondary oam have 8 sprites, so sprite fetching works
    if ppu.oam_tmp.len() < 8 {
      ppu.oam_tmp.resize_with(8, || Sprite::empty());
    }
  }

  // pre-computes a scanline of sprite data for later, so that we don't need any check when mixing with background
  fn spr_compute_next_scanline(&mut self) {
    self.ppu.spr_scanline.0.fill(0.into());

    // fetch sprites over sprite limit
    if self.settings.no_sprite_limit {
      self.ppu.oam_tmp_count = self.ppu.oam_tmp.len() as u8;

      for i in 8..self.ppu.oam_tmp.len() {
        let sprite = &self.ppu.oam_tmp[i as usize];
        let pttrn_addr = self.ppu.spr_pttrn_addr(sprite);

        let flip_hori = sprite.flip_hori();

        let mut pttrn_lo = self.ppu_dispatch_read(pttrn_addr);
        let mut pttrn_hi = self.ppu_dispatch_read(pttrn_addr + 8);
        
        if flip_hori {
          pttrn_lo = pttrn_lo.reverse_bits(); 
          pttrn_hi = pttrn_hi.reverse_bits(); 
        }

        let sprite = &mut self.ppu.oam_tmp[i as usize];
        sprite.pttrn_lo = pttrn_lo;
        sprite.pttrn_hi = pttrn_hi;
      }
    }
    

    let ppu = &mut self.ppu;
    for sprite in ppu.oam_tmp.iter().take(ppu.oam_tmp_count as usize) {
      for col in 0..8 {
        // X-scroll values of $F9-FF results in parts of the sprite to be past the right edge of the screen, thus invisible. It is not possible to have a sprite partially visible on the left edge.
        let x = sprite.x.saturating_add(col) as usize;
        let scanline_pixel =  &ppu.spr_scanline.0[x];
        
        if scanline_pixel.pixel() == 0 {
          let mask = 0x80 >> col;
          let curr_pixel_lo = sprite.pttrn_lo & mask > 0;
          let curr_pixel_hi = sprite.pttrn_hi & mask > 0;
          let curr_pixel = ((curr_pixel_hi as u8) << 1) | curr_pixel_lo as u8;

          // pixel is transparent, draw curr pixel
          let new_pixel = SprScanlineDataBuilder::new()
            .with_pixel(curr_pixel)
            .with_palette(sprite.palette())
            .with_priority(sprite.priority())
            .with_spr0(sprite.spr0())
            .build();

          ppu.spr_scanline.0[x] = new_pixel;
        }
      }
    }
  }

  fn bg_color_from_palette(&mut self, palette: u8, pixel: u8) -> u8 {
    let addr = (palette << 2) | pixel;
    self.ppu_palette_read(0x3f00 | addr as u16)
  }

  fn spr_color_from_palette(&mut self, palette: u8, pixel: u8) -> u8 {
    let addr = (palette << 2) | pixel;
    self.ppu_palette_read(0x3f10 | addr as u16)
  }

  fn render_pixel(&mut self) {
    let ppu = &mut self.ppu;
    let pixel_x = ppu.cycle as usize - 1;
    
    let in_lstrip  = pixel_x < 8;
    let bg_visible  = !in_lstrip || ppu.mask.contains(Mask::ShowBgLeft);
    let spr_visible = !in_lstrip || ppu.mask.contains(Mask::ShowSprLeft);

    let lstrip_bg_mask = ((bg_visible as u8) << 1) | bg_visible as u8;
    let lstrip_spr_mask = ((spr_visible as u8) << 1) | spr_visible as u8;
    
    // On every dot in these background fetch regions, a 4-bit pixel is selected by the fine x register from the low 8 bits of the pattern and attributes shift registers, which are then shifted. 
    let (mut bg_pixel, bg_palette) = ppu.shifter_get_pixel_n_palette();
    bg_pixel &= lstrip_bg_mask;

    let spr_data = ppu.spr_scanline.0[pixel_x];
    let spr_pixel = spr_data.pixel() & lstrip_spr_mask;

    if !ppu.stat.contains(Status::Spr0Hit) {
      // https://www.nesdev.org/wiki/PPU_OAM#Sprite_0_hits
      // https://www.nesdev.org/wiki/PPU_registers#Sprite_0_hit_flag
      let spr0_hit = 
        spr_data.spr0() &&
        ppu.mask.contains(Mask::BgEnable) &&
        ppu.mask.contains(Mask::SprEnable) &&
        spr_pixel > 0 && bg_pixel > 0 &&
        pixel_x != 255;

      ppu.stat.set(Status::Spr0Hit, spr0_hit);
    }

    // TODO: can do this without ifs?
    // TODO: something is off with sprite priority in front of backgrounds
    let color_id = if ppu.mask.contains(Mask::SprEnable) && spr_pixel > 0 && (spr_data.priority() || bg_pixel == 0) {
      self.spr_color_from_palette(spr_data.palette(), spr_pixel)
    } else if ppu.mask.contains(Mask::BgEnable) && bg_pixel > 0 {
      self.bg_color_from_palette(bg_palette, bg_pixel)
    } else {
      self.bg_color_from_palette(0, 0)
    };

    // TODO: mask greyscale and color emphasis
    self.videobuf[self.ppu.pixel] = color_id;
    self.ppu.pixel += 1;
  }

  // https://forums.nesdev.org/viewtopic.php?t=8066
  // https://forums.nesdev.org/viewtopic.php?t=10348
  // https://forums.nesdev.org/viewtopic.php?t=25833

  // https://www.nesdev.org/wiki/PPU_rendering

  pub fn ppu_step(&mut self) {
    match self.ppu.scanline {
      -1 => self.prerender_step(),
      0..=239 => self.render_step(),
      
      // During VBlank and when rendering is disabled, the value on the PPU address bus is the current value of the v register. 
      240 => if self.ppu.cycle == 1 && !self.ppu.rendering_enabled() {
        self.update_ppu_bus(self.ppu.v.0);
      }
      241 => if self.ppu.cycle == 1 {
        self.ppu.stat.set(Status::Vblank, !self.ppu.vblank_suppress);
        self.mem.nmi = self.ppu.ctrl.vblank_nmi_enabled && !self.ppu.nmi_suppress;

        self.frame_ready = true;
      }
      _ => {}
    }

    self.mem.ppu_cycle = self.ppu.cycle;
    let ppu = &mut self.ppu;

    ppu.cycle += 1;
    if ppu.cycle > 340 {
      ppu.cycle = 0;
      ppu.scanline += 1;

      self.mem.ppu_scanline = self.ppu.scanline;
      let ppu = &mut self.ppu;
      
      if ppu.scanline >= 261 {
        ppu.scanline = -1;
        
        self.mem.ppu_scanline = self.ppu.scanline;
        let ppu = &mut self.ppu;
        
        ppu.pixel = 0;
        ppu.odd_frame = !ppu.odd_frame;
        ppu.vblank_suppress = false;
        ppu.nmi_suppress = false;

        // first scanline shouldn't render any sprite
        ppu.spr_scanline.0.fill(0.into());
      }
    }
  }

  fn render_step(&mut self) {
    let ppu = &mut self.ppu;

    if !ppu.rendering_enabled() {
      if matches!(ppu.cycle, 1..=256) {
        self.videobuf[self.ppu.pixel] = self.bg_color_from_palette(0, 0);
        self.ppu.pixel += 1;
      }
      return;
    }

    match ppu.cycle {
      1..=255 => {
        // render pixel goes after the fetch, because at this time, the new tile hasnt been loaded into the shifters yet
        self.bg_fetch_step();
        self.render_pixel();
      }
      256 => {
        self.bg_fetch_step();
        self.render_pixel();
        self.inc_scroll_y();
        self.spr_evaluation();
      }
      257 => {
        self.restore_scroll_x();
        self.spr_fetch_step();
      }
      257..=320 => self.spr_fetch_step(),
      321..=336 => self.bg_fetch_step(),
      337 | 339 => { self.ppu_dispatch_read(self.ppu.nametbl_addr()); }
      340 => self.spr_compute_next_scanline(),
      _ => {}
    }
  }

  fn prerender_step(&mut self) {
    let ppu = &mut self.ppu;

    if !ppu.rendering_enabled() {
      if ppu.cycle == 1 {
        ppu.stat.clear();
      }

      return;
    }
    
    match ppu.cycle {
      1 => {
        ppu.stat.clear();
        self.bg_fetch_step();
      }
      1..=255 | 321..=336 => self.bg_fetch_step(),
      256 => {
        self.bg_fetch_step();
        self.inc_scroll_y();
      }
      257 => {
        self.restore_scroll_x();
        self.spr_fetch_step();
      }
      257..=279 | 305..=320 => self.spr_fetch_step(),
      280..=304 => {
        self.restore_scroll_y();
        self.spr_fetch_step();
      }
      337 => { self.ppu_dispatch_read(self.ppu.nametbl_addr()); }
      339 => {
        if ppu.odd_frame && ppu.mask.contains(Mask::BgEnable) {
          ppu.cycle += 1;
        }
        self.ppu_dispatch_read(self.ppu.nametbl_addr());
      }
      _ => {}
    }
  }
}