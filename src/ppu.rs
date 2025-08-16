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
    const GreyScale = 1 << 0;
    const LstripBg  = 1 << 1;
    const LstripSpr = 1 << 2;
    const BgEnable  = 1 << 3;
    const SprEnable = 1 << 4;
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
  pttrn_addr: u16,
  attribute: u8,
  pttrn_lo: u8,
  pttrn_hi: u8,

  spr_pttrn_lo: [u8; 8],
  spr_pttrn_hi: [u8; 8],
}

#[derive(Default)]
struct ShifterData {
  shift_ptrn_lo: u16,
  shift_ptrn_hi: u16,
  // we simulate the 1bit latch by always pushing 8 bytes (thus having a 16bit shifter)
  shift_attr_lo: u16,
  shift_attr_hi: u16,
}

#[derive(Default, Clone)]
struct Sprite {
  y: u8,
  nametbl: u8,
  attr: u8,
  x: u8,
}
impl Sprite {
  pub fn new(index: u8, bytes: &[u8]) -> Self {
    Self {
      y: bytes[0],
      nametbl: bytes[1],
      // bits 2-4 of attribute are empty, we can use them to store spr0hit
      attr: bytes[2] | (((index == 0) as u8) << 2),
      x: bytes[3],
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

  open_bus: u8,
  oam_addr: u8,
  ppu_data: u8,

  pub oam: Oam,
  oam_tmp: Vec<Sprite>,
  oam_tmp_count: u8,
  spr_scanline: SprScanline,

  pub cycle: i16,
  pub scanline: i16,
  pub pixel: usize,

  // https://www.nesdev.org/wiki/PPU_frame_timing
  // TODO: odd frame handling
  odd_frame: bool,

  fetcher: FetcherData,
  shifter: ShifterData,
}

impl Ppu2C02 {
  pub fn new() -> Self {
    Self {
      scanline: 261,
      ..Default::default()
    }
  }

  fn increase_addr(&mut self) {
    // TODO: increase during rendering bug
    self.v.0 = (self.v.0 + self.ctrl.vram_addr_inc) & 0x7fff;
  }

  fn inc_scroll_x(&mut self) {
    if self.rendering_enabled() {
      if self.v.coarse_x() == 31 {
        self.v.set_coarse_x(0);
        self.v.set_nametbl_x(!self.v.nametbl_x());
      } else {
        self.v.set_coarse_x(self.v.coarse_x() + 1);
      }
    }
  }

  fn inc_scroll_y(&mut self) {
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

  fn restore_scroll_x(&mut self) {
    if self.rendering_enabled() {
      self.v.set_nametbl_x(self.t.nametbl_x());
      self.v.set_coarse_x(self.t.coarse_x());
    }
  }

  fn restore_scroll_y(&mut self) {
    if self.rendering_enabled() {
      self.v.set_nametbl_y(self.t.nametbl_y());
      self.v.set_coarse_y(self.t.coarse_y());
      self.v.set_fine_y(self.t.fine_y());
    }
  }

  fn rendering_enabled(&self) -> bool {
    self.mask.contains(Mask::BgEnable) || self.mask.contains(Mask::SprEnable)  
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
  // https://www.nesdev.org/wiki/PPU_registers
  pub fn ppu_reg_read(&mut self, addr: u16) -> u8 {
    let ppu = &mut self.ppu;
    
    let res = match addr {
      // Status
      0x2002 => {
        // Reading this register has the side effect of clearing the PPU's internal w register.
        ppu.w = WriteToggle::First;

        let res = ppu.stat.bits();
        // Reading PPUSTATUS will return the current state of this flag and then clear it.
        
        // TODO: reading edge cases
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
      0x2004 => ppu.oam_write(val),
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

  // do not use, prefer direct access instead
  #[deprecated]
  fn fetching_read(&mut self, addr: u16) -> u8 {
    self.ppu_dispatch_read(addr)
  }

  fn render_vram_read(&mut self, addr: u16) -> u8 {
    todo!()
  }

  fn render_chr_read(&mut self, addr: u16) -> u8 {
    todo!()
  }

  fn ppu_read8(&mut self) -> u8 {
    // This read buffer is updated on every PPUDATA read, but only after the previous contents have been returned to the CPU, effectively delaying PPUDATA reads by one. 
    let res = self.ppu.ppu_data;
    self.ppu.ppu_data = self.ppu_dispatch_read(self.ppu.v.0);
    self.ppu.increase_addr();

    // TODO: palette ram read
    // https://www.nesdev.org/wiki/PPU_registers#Reading_palette_RAM

    res
  }

  fn ppu_write8(&mut self, val: u8) {
    self.ppu_dispatch_write(self.ppu.v.0, val);
    self.ppu.increase_addr();
  }

  fn palette_from_attribute(&self, attr: u8) -> u8 {
    let v = self.ppu.v.0;
    
    // if v.coarse_y() & 0x2 != 0 { attr >>= 4; }
    // if v.coarse_x() & 0x2 != 0 { attr >>= 2; }

    let shift = ((v & 0x40) >> 4) | (v & 0x02);
    (attr >> shift) & 0b11
  }

  fn bg_color_from_palette(&mut self, palette: u8, pixel: u8) -> u8 {
    let addr = (palette << 2) | pixel;
    self.ppu_palette_read(0x3f00 | addr as u16)
  }

  fn spr_color_from_palette(&mut self, palette: u8, pixel: u8) -> u8 {
    let addr = (palette << 2) | pixel;
    self.ppu_palette_read(0x3f10 | addr as u16)
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

  fn fetch_step_bg(&mut self) {
    self.ppu.shifter_update();

    // we do cycle - 1 as we skip the idle cycle to be aligned to 8
    match (self.ppu.cycle - 1) % 8 {
      // https://www.nesdev.org/wiki/PPU_scrolling#Tile_and_attribute_fetching
      0 => {
        self.ppu.shifter_load_from_fetcher();
        self.ppu.fetcher.nametbl = self.ppu_vram_read(0x2000 | (self.ppu.v.0 & 0x0fff));
      }
      2 => {
        let v = &self.ppu.v.0;
        let addr = 0x23c0 | (v & 0xc00) | ((v >> 4) & 0x38) | ((v >> 2) & 0x7);
        // let addr = 0x23c0 
        //   | ((v.nametbl_y() as u16) << 11)
        //   | ((v.nametbl_x() as u16) << 10)
        //   | (((v.coarse_y() as u16) >> 2) << 3)
        //   | ((v.coarse_x() as u16) >> 2);
        
        let attr = self.ppu_vram_read(addr);

        // we fetched the attribute, now we have to extract the correct 2 bits
        self.ppu.fetcher.attribute = self.palette_from_attribute(attr);
      }
      // https://www.nesdev.org/wiki/PPU_pattern_tables
      4 => {
        let addr = self.ppu.ctrl.bg_pttrntbl_addr 
          | ((self.ppu.fetcher.nametbl as u16) << 4)
          | self.ppu.v.fine_y() as u16;

        self.ppu.fetcher.pttrn_addr = addr;
        self.ppu.fetcher.pttrn_lo = self.ppu_chr_read(addr);
      }
      6 => {
        self.ppu.fetcher.pttrn_hi = self
          .ppu_chr_read(self.ppu.fetcher.pttrn_addr + 8);

        // IMPORTANT: increase v by one
        self.ppu.inc_scroll_x();
      }

      _ => {}
    }
  }

  fn evaluate_sprites(&mut self) {
    let ppu = &mut self.ppu;
    ppu.oam_tmp.clear();
    ppu.spr_scanline.0.fill(SprScanlineData(0));

    let fetcher = &mut ppu.fetcher;
    fetcher.spr_pttrn_hi.fill(0);
    fetcher.spr_pttrn_lo.fill(0);

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

    ppu.stat.set(Status::SprOverflow, ppu.oam_tmp.len() > 8);
    ppu.oam_tmp_count = ppu.oam_tmp.len() as u8;
    ppu.oam_tmp.resize_with(8, || Sprite::empty());
  }

  // pre-computes a scanline of sprite data for later, so that we don't need any check when mixing with background
  fn compute_sprite_scanline(&mut self) {
    // TODO: option to draw over 8 sprites limit

    let ppu = &mut self.ppu;

    for (i, sprite) in ppu.oam_tmp.iter().enumerate().take(ppu.oam_tmp_count as usize) {
      for col in 0..8 {
        // X-scroll values of $F9-FF results in parts of the sprite to be past the right edge of the screen, thus invisible. It is not possible to have a sprite partially visible on the left edge.
        let x = sprite.x.saturating_add(col) as usize;
        let scanline_pixel =  &ppu.spr_scanline.0[x];
        
        if scanline_pixel.pixel() == 0 {
          let mask = 0x80 >> col;
          let curr_pixel_lo = ppu.fetcher.spr_pttrn_lo[i] & mask > 0;
          let curr_pixel_hi = ppu.fetcher.spr_pttrn_hi[i] & mask > 0;
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

  fn fetch_step_spr(&mut self) {
    let ppu = &mut self.ppu;
    
    // these are still aligned by 8
    match (ppu.cycle - 1) % 8 {
      0 => {
        self.ppu.shifter_load_from_fetcher();
        self.ppu.fetcher.nametbl = self.ppu_vram_read(0x2000 | (self.ppu.v.0 & 0x0fff));
      }
      2 => {
        let v = &self.ppu.v.0;
        let addr = 0x23c0 | (v & 0xc00) | ((v >> 4) & 0x38) | ((v >> 2) & 0x7);
        let attr = self.ppu_vram_read(addr);
        self.ppu.fetcher.attribute = self.palette_from_attribute(attr);
      }
      4 => {
        let spr_id = (ppu.cycle - 257) / 8;
        let sprite = &ppu.oam_tmp[spr_id as usize];

        // after evaluation, we are 100% sure scanline is always bigger than y
        let dist = ppu.scanline - sprite.y as i16;
        let fine_y = if sprite.flip_vert() {
          7 - dist
        } else {
          dist
        } as u16;

        let pttrn_addr = if ppu.ctrl.spr_size == 8 {
          // 8x8 sprites
          ppu.ctrl.spr_pttrntbl_addr 
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
        };

        ppu.fetcher.pttrn_addr = pttrn_addr;
        
        let flip_hori = sprite.flip_hori();

        let mut pttrn = self.ppu_chr_read(pttrn_addr);
        if flip_hori { pttrn = pttrn.reverse_bits(); }

        self.ppu.fetcher.spr_pttrn_lo[spr_id as usize] = pttrn;
      }
      6 => {
        let spr_id = (ppu.cycle - 257) / 8;
        let sprite = &ppu.oam_tmp[spr_id as usize];

        let pttrn_addr = ppu.fetcher.pttrn_addr;
        let flip_hori = sprite.flip_hori();

        let mut pttrn = self.ppu_chr_read(pttrn_addr + 8);
        if flip_hori { pttrn = pttrn.reverse_bits(); }

        self.ppu.fetcher.spr_pttrn_hi[spr_id as usize] = pttrn;
      }
      _ => {}
    }
  }

  fn push_pixel(&mut self) {
    let pixel_col = self.ppu.cycle as usize - 1;

    // On every dot in these background fetch regions, a 4-bit pixel is selected by the fine x register from the low 8 bits of the pattern and attributes shift registers, which are then shifted. 
    let (bg_pixel, bg_palette) = self.ppu.shifter_get_pixel_n_palette();
    let spr_pixel = &self.ppu.spr_scanline.0[pixel_col];

    let spr0_hit = self.ppu.rendering_enabled() && spr_pixel.pixel() > 0 && bg_pixel > 0 && spr_pixel.spr0() && pixel_col != 255;
    self.ppu.stat = Status::from_bits_retain(self.ppu.stat.bits() | ((spr0_hit as u8) << 6));

    let color_id = if self.ppu.mask.contains(Mask::SprEnable) && spr_pixel.pixel() > 0 && (spr_pixel.priority() || bg_pixel == 0) {
      self.spr_color_from_palette(spr_pixel.palette(), spr_pixel.pixel())
    } else if self.ppu.mask.contains(Mask::BgEnable) && bg_pixel > 0 {
      self.bg_color_from_palette(bg_palette, bg_pixel)
    } else {
      self.bg_color_from_palette(0, 0)
    };

    // TODO: mask greyscale and color emphasis
    self.videobuf[self.ppu.pixel] = color_id;
    self.ppu.pixel += 1;
  }


  // pub fn ppu_step(&mut self) {
  //   let ppu = &mut self.ppu;

  //   if ppu.scanline == 261 || ppu.scanline <= 239 {
  //     if ppu.scanline == 261 && ppu.cycle == 1 {
  //       ppu.stat.clear();
  //     } else if ppu.scanline == 261 && (ppu.cycle >= 280 && ppu.cycle <= 304) {
  //       ppu.scroll_y_tx();
  //     } else if ppu.cycle == 0 {
  //       self.compute_sprite_scanline();
  //     } else if (ppu.cycle >= 1 && ppu.cycle < 256) || (ppu.cycle >= 321 && ppu.cycle <= 336) {
  //       self.fetch_step_bg();
  //     } else if ppu.cycle == 256 {
  //       ppu.scroll_y_inc();
  //       self.evaluate_sprites();
  //     } else if ppu.cycle == 257 {
  //       ppu.scroll_x_tx();
  //       self.fetch_step_spr();
  //     } else if ppu.cycle >= 257 && ppu.cycle <= 320 {
  //       self.fetch_step_spr();
  //     }

  //     if self.ppu.scanline != 261 && self.ppu.rendering_enabled() && self.ppu.cycle >= 1 && self.ppu.cycle < 257 { 
  //       self.push_pixel();
  //     }
  //   } else if ppu.scanline >= 241 && ppu.scanline < 261 {
  //     if ppu.scanline == 241 && ppu.cycle == 1 {
  //       ppu.stat.insert(Status::Vblank);
        
  //       if ppu.ctrl.vblank_nmi_enabled {
  //         self.events.insert(emu::Events::NMI);
  //       }
  //     }
  //   }

  //   let ppu = &mut self.ppu;
  //   ppu.cycle += 1;
  //   if ppu.cycle > 340 {
  //     ppu.cycle = 0;
  //     ppu.scanline += 1;
  //     if ppu.scanline > 261 {
  //       ppu.scanline = 0;
  //       self.events.insert(emu::Events::FRAME);
  //     }
  //   }
  // }

  // TODO: can be done better?
  pub fn ppu_step(&mut self) {
    // https://forums.nesdev.org/viewtopic.php?t=8066
    // https://forums.nesdev.org/viewtopic.php?t=10348
    // https://forums.nesdev.org/viewtopic.php?t=25833


    // TODO: if on a visible scanline, and rendering is disabled, do nothing
    
    match (self.ppu.scanline, self.ppu.cycle) {
      // no sprites on first scanline
      (1..=239, 0) => self.compute_sprite_scanline(),
      (0..=239, 1..=256) => {
        self.fetch_step_bg();
        self.push_pixel();
      }
      (0..=239, 257) => {
        self.ppu.inc_scroll_y();
        self.ppu.restore_scroll_x();
        self.evaluate_sprites();
      }
      (0..=239, 257..=320) => self.fetch_step_spr(),
      (0..=239, 321..=336) => self.fetch_step_bg(),
      (241, 1) => {
        self.ppu.stat.insert(Status::Vblank);
        self.events.insert(emu::Events::PPU_FRAME);
        self.ppu.pixel = 0;

        if self.ppu.ctrl.vblank_nmi_enabled {
          self.events.insert(emu::Events::NMI);
        }
      }
      (261, 1) => {
        self.fetch_step_bg();
        self.ppu.stat.clear();
      }
      (261, 1..=256 | 321..=336) => self.fetch_step_bg(),
      (261, 257) => {
        self.ppu.inc_scroll_y();
        self.ppu.restore_scroll_x();
      }
      (261, 280) => self.ppu.restore_scroll_y(),
      _ => {}
    }

    self.ppu.cycle += 1;
    if self.ppu.cycle > 340 {
      self.ppu.cycle = 0;
      self.ppu.scanline += 1;
      if self.ppu.scanline > 261 {
        self.ppu.scanline = 0;
      }
    }
  }
}