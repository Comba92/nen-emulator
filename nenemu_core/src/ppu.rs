use std::array;

use crate::{
    emu::{NesEmulator, Region},
    utils::{byte_set_hi, byte_set_lo},
};
use bitflags::Flags;

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
  #[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
  struct Status: u8 {
    const SprOvfl = 1 << 5;
    const Spr0Hit = 1 << 6;
    const Vblank  = 1 << 7;
  }

  #[derive(Default, Debug)]
  #[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
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

#[derive(Debug)]
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
pub struct CtrlStrut {
    pub vram_addr_inc: u16,
    pub spr_pttrntbl_addr: u16,
    pub bg_pttrntbl_addr: u16,
    pub spr_size: u16,
    pub nmi_enabled: bool,
}
impl Default for CtrlStrut {
    fn default() -> Self {
        Self {
            vram_addr_inc: 1,
            spr_pttrntbl_addr: 0,
            bg_pttrntbl_addr: 0,
            spr_size: 8,
            nmi_enabled: false,
        }
    }
}

#[bitfields::bitfield(u16)]
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
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
    _unused: u16,
}

#[derive(Default)]
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
struct Fetcher {
    nametbl: u8,
    attribute: u8,
    pttrn_lo: u8,
    pttrn_hi: u8,
}

#[derive(Default)]
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
struct Shifter {
    shift_ptrn_lo: u16,
    shift_ptrn_hi: u16,
    // we simulate the 1bit latch by always pushing 8 bytes (thus having a 16bit shifter)
    shift_attr_lo: u16,
    shift_attr_hi: u16,
}

#[derive(Debug, Clone)]
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
struct Sprite {
    y: u8,
    tile: u8,
    attr: u8,
    x: u8,
}
impl Default for Sprite {
    fn default() -> Self {
        Self {
            y: 0xff,
            tile: 0xff,
            attr: 0xff,
            x: 0xff,
        }
    }
}
impl Sprite {
    pub fn new(bytes: &[u8], idx: usize) -> Self {
        let mut attr = bytes[2];
        // little trick: we store if its sprite 0 in attribute unused bits
        attr = (attr & !0x04) | (((idx == 0) as u8) << 2);

        Self {
            y: bytes[0],
            tile: bytes[1],
            // bits 2-4 of attribute are empty, we can use them to store spr0hit and visibility
            attr,
            x: bytes[3],
        }
    }

    pub fn palette(&self) -> u8 {
        self.attr & 0b11
    }
    // we stored spr0 in unused bits of attribute to save memory
    pub fn is_spr0(&self) -> bool {
        self.attr & 0x4 != 0
    }

    // 0: in front of background; 1: behind background
    pub fn priority(&self) -> bool {
        self.attr & 0x20 == 0
    }
    pub fn flip_hori(&self) -> bool {
        self.attr & 0x40 != 0
    }
    pub fn flip_vert(&self) -> bool {
        self.attr & 0x80 != 0
    }
}

#[cfg(feature = "savestates")]
use serde_big_array::BigArray;
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
pub struct Oam(#[cfg_attr(feature = "savestates", serde(with = "BigArray"))] pub [u8; 256]);
impl Default for Oam {
    fn default() -> Self {
        Self([0; 256])
    }
}

#[bitfields::bitfield(u8)]
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

pub struct SprScanline([SprScanlineData; 256]);
impl Default for SprScanline {
    fn default() -> Self {
        Self([0.into(); 256])
    }
}
impl SprScanline {
    fn spr_push(&mut self, sprite: &Sprite, mut pttrn_lo: u8, mut pttrn_hi: u8) {
        // flip here
        if sprite.flip_hori() {
            pttrn_lo = pttrn_lo.reverse_bits();
            pttrn_hi = pttrn_hi.reverse_bits();
        }

        // check if any pixel is out of visible screen
        let mx = 8.min(255 - sprite.x);

        // for every pixel of sprite
        for x in 0..mx {
            // we skip this because the pixel pushed earlier has priority
            if self.0[(sprite.x + x) as usize].pixel() > 0 {
                continue;
            }

            // get the pixel
            let mask = 0x80 >> x;
            let pixel_lo = (pttrn_lo & mask) > 0;
            let pixel_hi = (pttrn_hi & mask) > 0;
            let pixel = ((pixel_hi as u8) << 1) | (pixel_lo as u8);
            // pixel is transparent and shouldnt be drawn
            // if pixel == 0 {
            //     continue;
            // }

            let sprite_entry = SprScanlineDataBuilder::new()
                .with_pixel(pixel)
                .with_palette(sprite.palette())
                .with_priority(sprite.priority())
                .with_spr0(sprite.is_spr0());

            // push to scanline cache
            self.0[(sprite.x + x) as usize] = sprite_entry.build();
        }
    }
}

#[derive(Debug, Default)]
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
enum RenderState {
    PreRender,
    Rendering,
    Vblank,
    #[default]
    Disabled,
}

#[derive(Debug)]
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
enum RenderCmd {
    Idle,
    OamClear,
    SpriteEval,
    NtFetch,
    AtFetch,
    BgLoFetch,
    BgHiFetch,
    BgHiFetchFirst,
    BgHiFetchLast,
    SprLoFetch,
    SprHiFetch,
    ResetVert,
    ResetHori,
    LastDotInLine,
    StatClear,
    LastDotInFrame,
}

// Clear explanation of how PPU works in Rust
// https://docs.rs/nes-ppu/latest/src/nes_ppu
#[derive(Default)]
#[cfg_attr(feature = "savestates", derive(serde::Serialize, serde::Deserialize))]
pub struct Ppu2C02 {
    pub ctrl: CtrlStrut,
    mask: Mask,
    mask_write_delay: u8,
    stat: Status,

    render_state: RenderState,

    pub v: LoopyReg,
    t: LoopyReg,
    x: u8,
    w: bool,

    oam_addr: u8,
    ppu_data: u8,
    open_bus: u8,

    fetcher: Fetcher,
    shifter: Shifter,

    pub oam: Oam,
    oam_tmp: [Sprite; 8],
    oam_tmp_count: u8,

    #[cfg_attr(feature = "savestates", serde(skip))]
    spr_extra: Vec<Sprite>,

    #[cfg_attr(feature = "savestates", serde(skip))]
    pub spr_scanline: SprScanline,

    pub dma: Option<u16>,

    pub palettes: [u8; 32],

    pub dot: i16,
    pub line: i16,
    prerender_line: i16,
    pub pixel_idx: usize,

    // https://www.nesdev.org/wiki/PPU_frame_timing
    odd_frame: bool,
    vblank_suppress: bool,
    nmi_suppress: bool,
}

impl Ppu2C02 {
    pub fn new(region: &Region) -> Self {
        // pre render scanline not counted (its treated as -1)
        let prerender_line = match region {
            Region::NTSC => 261,
            // PAL NES PPUs render 70 vblank scanlines instead of 20
            Region::PAL => 311,
        };

        Self {
            line: prerender_line,
            prerender_line,
            oam_tmp: array::from_fn(|_| Sprite::default()),
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

    pub fn palettes_read(&self, addr: u16) -> u8 {
        let pal = addr as usize % 32;
        let res = if pal >= 16 && pal % 4 == 0 {
            self.palettes[pal & !0x10]
        } else {
            self.palettes[pal]
        };

        if self.mask.contains(Mask::GreyScale) {
            res & 0x30
        } else {
            res
        }
    }

    pub fn palettes_write(&mut self, addr: u16, val: u8) {
        let pal = addr as usize % 32;
        let val = val & 0x3f;

        // if we're writing a transparent color
        if pal % 4 == 0 {
            // write both backdrop colors
            self.palettes[pal & 0xf] = val;
            self.palettes[(pal & 0xf) | 0x10] = val;
        } else {
            // write palette color as is
            self.palettes[pal] = val;
        }
    }

    fn oam_read(&mut self, enable: bool) -> u8 {
        if enable && self.is_in_visible_scanline() {
            // https://www.nesdev.org/wiki/PPU_sprite_evaluation#Details
            // https://forums.nesdev.org/viewtopic.php?p=141975#p141975
            if self.dot < 64 {
                0xff
            } else if self.dot < 256 {
                0
            } else if self.dot < 320 {
                0xff
            } else {
                self.oam_tmp[0].y
            }
        } else {
            self.oam.0[self.oam_addr as usize]
        }
    }

    pub fn oam_write(&mut self, val: u8) {
        // Writes to OAMDATA during rendering (on the pre-render line and the visible lines 0–239, provided either sprite or background rendering is enabled) do not modify values in OAM, but do perform a glitchy increment of OAMADDR, bumping only the high 6 bits
        self.oam.0[self.oam_addr as usize] = val;
        self.oam_addr = self.oam_addr.wrapping_add(1);
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
        self.ctrl.bg_pttrntbl_addr | ((self.fetcher.nametbl as u16) << 4) | self.v.fine_y() as u16

        // self.ctrl.bg_pttrntbl_addr
        //   + ((self.fetcher.nametbl as u16) * 16)
        //   + self.v.fine_y() as u16
    }

    fn spr_pttrn_addr(&self, sprite: &Sprite) -> u16 {
        // after evaluation, we are 100% sure scanline is always bigger than y
        let dist = self.line - sprite.y as i16;

        let fine_y = if sprite.flip_vert() { 7 - dist } else { dist } as u16;

        if self.ctrl.spr_size == 8 {
            // 8x8 sprites
            self.ctrl.spr_pttrntbl_addr | ((sprite.tile as u16) << 4) | fine_y & 0x7
        } else {
            // 8x16 sprites
            let mut bottom_tile = dist >= 8;
            // In 8x16 mode, vertical flip flips each of the subtiles and also exchanges their position; the odd-numbered tile of a vertically flipped sprite is drawn on top.
            if sprite.flip_vert() {
                bottom_tile = !bottom_tile;
            }

            // For 8x16 sprites (bit 5 of PPUCTRL set), the PPU ignores the pattern table selection and selects a pattern table from bit 0 of this number.
            (sprite.tile as u16 & 1) << 12
                | (((sprite.tile & !1) as u16 | bottom_tile as u16) << 4)
                | fine_y & 0x7
        }
    }

    fn is_rendering_enabled(&self) -> bool {
        self.mask.contains(Mask::BgEnable) || self.mask.contains(Mask::SprEnable)
    }

    fn is_in_visible_scanline(&self) -> bool {
        self.line < 240 || self.line == self.prerender_line
    }

    fn is_in_vblank(&self) -> bool {
        self.line >= 240 && self.line != self.prerender_line
    }

    fn toggle_rendering(&mut self) {
        match self.render_state {
            RenderState::Disabled => {
                if self.is_rendering_enabled() {
                    self.render_state = if self.line < 240 {
                        RenderState::Rendering
                    } else if self.line == self.prerender_line {
                        RenderState::PreRender
                    } else {
                        RenderState::Vblank
                    };
                }
            }

            _ => {
                if !self.is_rendering_enabled() {
                    self.render_state = RenderState::Disabled
                }
            }
        }
    }

    fn handle_mask_write(&mut self) {
        if self.mask_write_delay > 0 {
            self.mask_write_delay -= 1;
            if self.mask_write_delay == 0 {
                self.toggle_rendering();
            }
        }
    }

    fn shifter_load(&mut self) {
        // On every 8th dot in these background fetch regions (the same dot on which the coarse x component of v is incremented), the pattern and attributes data are transferred into registers used for producing pixel data.
        let fetcher = &self.fetcher;
        let shifter = &mut self.shifter;

        // For the pattern data, these transfers are into the LOW 8 bits of two 16-bit shift registers.
        // Wiki says high bits, but it is wrong (even the diagram in the same page shows it)
        shifter.shift_ptrn_lo = byte_set_lo(shifter.shift_ptrn_lo, fetcher.pttrn_lo);
        shifter.shift_ptrn_hi = byte_set_lo(shifter.shift_ptrn_hi, fetcher.pttrn_hi);

        // For the attributes data, only 2 bits are transferred and into two 1-bit latches that feed 8-bit shift registers.
        // Here for conveniece, we use 16-bit shift registers too.
        let attr_lo = if fetcher.attribute & 0b01 != 0 {
            0xff
        } else {
            0
        };
        shifter.shift_attr_lo = byte_set_lo(shifter.shift_attr_lo, attr_lo);

        let attr_hi = if fetcher.attribute & 0b10 != 0 {
            0xff
        } else {
            0
        };
        shifter.shift_attr_hi = byte_set_lo(shifter.shift_attr_hi, attr_hi);
    }

    fn shifter_update(&mut self, amount: u8) {
        self.shifter.shift_ptrn_lo <<= amount;
        self.shifter.shift_ptrn_hi <<= amount;
        self.shifter.shift_attr_lo <<= amount;
        self.shifter.shift_attr_hi <<= amount;
    }

    fn inc_scroll_x(&mut self) {
        // if self.ppu.rendering_enabled() {
        let v = &mut self.v;
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
        let v = &mut self.v;
        if v.fine_y() < 7 {
            // if fine Y < 7
            v.set_fine_y(v.fine_y() + 1); // increment fine Y
        } else {
            v.set_fine_y(0); // fine Y = 0
            let mut y = v.coarse_y(); // let y = coarse Y

            if y == 29 {
                y = 0; // coarse Y = 0
                v.set_nametbl_y(!v.nametbl_y()); // switch vertical nametable
            } else if y == 31 {
                y = 0; // coarse Y = 0, nametable not switched
            } else {
                y += 1;
            } // increment coarse Y
            v.set_coarse_y(y); // put coarse Y back into v
        }
        // }
    }

    fn restore_scroll_x(&mut self) {
        // if self.rendering_enabled() {
        self.v.set_nametbl_x(self.t.nametbl_x());
        self.v.set_coarse_x(self.t.coarse_x());
        // }
    }

    fn restore_scroll_y(&mut self) {
        // if self.rendering_enabled() {
        self.v.set_nametbl_y(self.t.nametbl_y());
        self.v.set_coarse_y(self.t.coarse_y());
        self.v.set_fine_y(self.t.fine_y());
        // }
    }

    fn increase_vram_addr(&mut self) {
        // if self.is_in_visible_scanline() && self.is_rendering_enabled() {
        // https://www.nesdev.org/wiki/PPU_scrolling#$2007_(PPUDATA)_reads_and_writes
        // self.inc_scroll_x();
        // self.inc_scroll_y();
        // } else {
        self.v.0 = (self.v.0 + self.ctrl.vram_addr_inc) & 0x3fff;
        // }
    }

    fn spr_evaluation(&mut self) {
        self.oam_tmp_count = 0;
        self.oam_tmp.fill(Sprite::default());
        self.spr_extra.clear();

        let mut spr_ovfl_idx = 0;
        for (i, y) in self.oam.0.iter().copied().enumerate().step_by(4) {
            // Sprite data is delayed by one scanline; you must subtract 1 from the sprite's Y coordinate
            // let y = y.wrapping_sub(1) as i16;

            // we are rendering the NEXT scanline, so we don't want to subtract 1 from y
            let dist = self.line - y as i16;
            if 0 <= dist && dist < self.ctrl.spr_size as i16 {
                let sprite = Sprite::new(&self.oam.0[i..i + 4], i);

                if self.oam_tmp_count < 8 {
                    self.oam_tmp[self.oam_tmp_count as usize] = sprite;
                } else {
                    // we push extra sprites here, so we can show them later if sprite limit is disabled in settings
                    self.spr_extra.push(sprite);
                }

                self.oam_tmp_count += 1;
            }

            if self.oam_tmp_count >= 8 {
                // sprite overflow bug
                // If the value is not in range, increment n and m (without carry). If n overflows to 0, go to 4; otherwise go to 3
                // The m increment is a hardware bug - if only n was incremented, the overflow flag would be set whenever more than 8 sprites were present on the same scanline, as expected.
                let corrupt_y = self.oam.0[i + spr_ovfl_idx];
                spr_ovfl_idx = (spr_ovfl_idx + 1) % 4;
                let dist = self.line - corrupt_y as i16;
                if 0 <= dist && dist < self.ctrl.spr_size as i16 {
                    self.stat.insert(Status::SprOvfl);
                }
            }
        }
    }

    fn bg_color_from_palette(&mut self, palette: u8, pixel: u8) -> u8 {
        let addr = (palette << 2) | pixel;
        self.palettes_read(0x3f00 | addr as u16)
    }

    fn spr_color_from_palette(&mut self, palette: u8, pixel: u8) -> u8 {
        let addr = (palette << 2) | pixel;
        self.palettes_read(0x3f10 | addr as u16)
    }

    pub fn reset(&mut self) {
        *self = Ppu2C02 {
            line: self.prerender_line,
            prerender_line: self.prerender_line,
            // v: self.v,
            oam_tmp: array::from_fn(|_| Sprite::default()),
            ..Default::default()
        }
    }
}

impl NesEmulator {
    // https://www.nesdev.org/wiki/PPU_registers
    pub fn ppu_reg_read(&mut self, addr: u16) -> u8 {
        let ppu = &mut self.ppu;

        let res = match addr & 0x2007 {
            // Status
            0x2002 => {
                // Reading this register has the side effect of clearing the PPU's internal w register.
                ppu.w = false;

                // Reading $2002 within a few PPU clocks of when VBL is set results in special-case behavior.
                // Reading one PPU clock before reads it as clear and never sets the flag or generates NMI for that frame.
                // Reading on the same PPU clock or one later reads it as set, clears it, and suppresses the NMI for that frame.
                // Reading two or more PPU clocks before/after it's set behaves normally (reads flag's value, clears it, and doesn't affect NMI operation).

                if ppu.line == 241 && ppu.dot <= 2 {
                    if ppu.dot == 0 {
                        ppu.stat.remove(Status::Vblank);
                        ppu.vblank_suppress = true;
                    } else if ppu.dot <= 2 {
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
            0x2004 => self.ppu.oam_read(self.settings.enable_oam_read),
            // PpuData
            0x2007 => {
                // This read buffer is updated on every PPUDATA read, but only after the previous contents have been returned to the CPU, effectively delaying PPUDATA reads by one.

                let res = if self.ppu.v.0 >= 0x3f00 {
                    // https://www.nesdev.org/wiki/PPU_registers#Reading_palette_RAM
                    // The value on the nametable at $2700 through $27FF should be put in the buffer when reading from palette RAM at $3F00 through $3FFF.
                    self.ppu.ppu_data = self.ppu_dispatch_read(self.ppu.v.0 & 0x27ff);
                    self.ppu.palettes_read(self.ppu.v.0) | self.ppu.open_bus & 0xc0
                } else {
                    let val = self.ppu.ppu_data;
                    self.ppu.ppu_data = self.ppu_dispatch_read(self.ppu.v.0);
                    val
                };
                self.ppu.increase_vram_addr();
                res
            }

            _ => ppu.open_bus,
        };

        self.ppu.open_bus = res;
        res
    }

    // https://www.nesdev.org/wiki/PPU_registers
    pub fn ppu_reg_write(&mut self, addr: u16, val: u8) {
        let ppu = &mut self.ppu;

        match addr & 0x2007 {
            // Ctrl
            0x2000 => {
                let old_nmi_enabled = ppu.ctrl.nmi_enabled;
                let new_nmi_enabled = val & 0x80 != 0;

                if !old_nmi_enabled && new_nmi_enabled && ppu.stat.contains(Status::Vblank) {
                    // Changing NMI enable from 0 to 1 while the vblank flag in PPUSTATUS is 1 will immediately trigger an NMI.
                    self.mem.nmi = true;
                } else if old_nmi_enabled && !new_nmi_enabled {
                    // NMI shouldn't occur when disabled 0-1-2 PPU clock after VBL
                    if ppu.line == 241 && ppu.dot <= 2 {
                        self.mem.nmi = false
                    }
                }
                ppu.ctrl.nmi_enabled = new_nmi_enabled;

                ppu.t.set_nametbl_x(val & 1);
                ppu.t.set_nametbl_y((val >> 1) & 1);

                ppu.ctrl.vram_addr_inc = if val & 0x4 == 0 { 1 } else { 32 };
                ppu.ctrl.spr_pttrntbl_addr = if val & 0x8 == 0 { 0 } else { 0x1000 };
                ppu.ctrl.bg_pttrntbl_addr = if val & 0x10 == 0 { 0 } else { 0x1000 };
                ppu.ctrl.spr_size = if val & 0x20 == 0 { 8 } else { 16 };
            }
            // Mask
            0x2001 => {
                ppu.mask = Mask::from_bits_retain(val);
                ppu.toggle_rendering();
            }
            // OamAddr
            0x2003 => ppu.oam_addr = val,
            // OamData
            0x2004 => ppu.oam_write(val),
            // Scroll
            0x2005 => {
                match ppu.w {
                    false => {
                        // Scroll X
                        ppu.t.set_coarse_x(val >> 3);
                        // coarse x
                        ppu.x = val & 0x7;
                    }
                    true => {
                        // Scroll Y
                        ppu.t.set_coarse_y(val >> 3);
                        ppu.t.set_fine_y(val & 0x7);
                    }
                }
                ppu.w = !ppu.w;
            }
            // PpuAddr
            0x2006 => {
                match ppu.w {
                    false => {
                        // The 16-bit address is written to PPUADDR one byte at a time, high byte first.
                        ppu.t.0 = byte_set_hi(ppu.t.0, val);
                        // bit 14 of the internal t register that holds the data written to PPUADDR is forced to 0 when writing the PPUADDR high byte.
                        ppu.t.0 &= 0x3fff;
                    }
                    true => {
                        ppu.t.0 = byte_set_lo(ppu.t.0, val);
                        ppu.v.0 = ppu.t.0;
                    }
                }
                self.ppu.w = !self.ppu.w;
            }
            // PpuData
            0x2007 => {
                self.ppu_dispatch_write(self.ppu.v.0, val);
                self.ppu.increase_vram_addr();
            }
            _ => {}
        }

        self.ppu.open_bus = val;
    }

    fn videobuf_push(&mut self, color_id: u8) {
        let color = self.palette.0[color_id as usize];
        self.output.videobuf_back.0[self.ppu.pixel_idx + 0] = color.0;
        self.output.videobuf_back.0[self.ppu.pixel_idx + 1] = color.1;
        self.output.videobuf_back.0[self.ppu.pixel_idx + 2] = color.2;
        self.ppu.pixel_idx += 4;
    }

    fn render_pixel(&mut self) {
        let ppu = &mut self.ppu;
        let pixel_x = ppu.dot as usize - 1;

        // TODO: should be moved to different function for first 8 pixels?
        let in_lstrip = pixel_x < 8;
        let bg_visible = (!in_lstrip || ppu.mask.contains(Mask::ShowBgLeft))
            && ppu.mask.contains(Mask::BgEnable);
        let spr_visible = (!in_lstrip || ppu.mask.contains(Mask::ShowSprLeft))
            && ppu.mask.contains(Mask::SprEnable);

        // On every dot in these background fetch regions, a 4-bit pixel is selected by the fine x register from the low 8 bits of the pattern and attributes shift registers, which are then shifted.
        let shift_mask = 0x8000 >> ppu.x;

        let pixel_lo = ppu.shifter.shift_ptrn_lo & shift_mask > 0;
        let pixel_hi = ppu.shifter.shift_ptrn_hi & shift_mask > 0;
        let mut bg_pixel = ((pixel_hi as u8) << 1) | (pixel_lo as u8);
        bg_pixel *= bg_visible as u8;

        let spr_data = ppu.spr_scanline.0[pixel_x];
        let spr_pixel = spr_data.pixel() * spr_visible as u8;

        // TODO: can do this without ifs?
        let color_id = if !self.settings.disable_sprites
            && spr_pixel > 0
            && (spr_data.priority() || bg_pixel == 0)
        {
            ppu.spr_color_from_palette(spr_data.palette(), spr_pixel)
        } else if !self.settings.disable_background && bg_pixel > 0 {
            let palette_lo = ppu.shifter.shift_attr_lo & shift_mask > 0;
            let palette_hi = ppu.shifter.shift_attr_hi & shift_mask > 0;
            let bg_palette = ((palette_hi as u8) << 1) | (palette_lo as u8);

            ppu.bg_color_from_palette(bg_palette, bg_pixel)
        } else {
            ppu.bg_color_from_palette(0, 0)
        };

        ppu.shifter_update(1);
        if !ppu.stat.contains(Status::Spr0Hit) {
            // https://www.nesdev.org/wiki/PPU_OAM#Sprite_0_hits
            // https://www.nesdev.org/wiki/PPU_registers#Sprite_0_hit_flag
            let spr0_hit = spr_data.spr0() && spr_pixel > 0 && bg_pixel > 0 && pixel_x != 255;
            ppu.stat.set(Status::Spr0Hit, spr0_hit);
        }

        self.videobuf_push(color_id);
    }

    fn start_vblank(&mut self) {
        self.ppu.stat.set(Status::Vblank, !self.ppu.vblank_suppress);
        self.mem.nmi = self.ppu.ctrl.nmi_enabled && !self.ppu.nmi_suppress;

        std::mem::swap(
            &mut self.output.videobuf_view.0,
            &mut self.output.videobuf_back.0,
        );

        self.output.frame_ready = true;
    }

    fn end_frame(&mut self) {
        self.ppu.line = 0;
        self.ppu.pixel_idx = 0;
        self.ppu.odd_frame = !self.ppu.odd_frame;
        self.ppu.nmi_suppress = false;
        self.ppu.vblank_suppress = false;

        self.output.frame_ready = false;
    }

    // https://forums.nesdev.org/viewtopic.php?t=8066
    // https://forums.nesdev.org/viewtopic.php?t=10348
    // https://forums.nesdev.org/viewtopic.php?t=25833

    // https://www.nesdev.org/wiki/PPU_rendering
    pub fn ppu_step(&mut self) {
        // self.ppu.handle_mask_write();

        match self.ppu.render_state {
            RenderState::PreRender => self.ppu_render_step(&PRERENDER_LUT),

            RenderState::Rendering => {
                if 1 <= self.ppu.dot && self.ppu.dot <= 256 {
                    self.render_pixel();
                }
                self.ppu_render_step(&RENDER_LUT);
            }

            RenderState::Vblank => {
                if self.ppu.line == 241 && self.ppu.dot == 0 {
                    self.start_vblank();
                }

                let ppu = &mut self.ppu;
                ppu.dot += 1;
                if ppu.dot >= 341 {
                    ppu.dot = 0;
                    ppu.line += 1;
                    if ppu.line == ppu.prerender_line {
                        ppu.render_state = RenderState::PreRender;
                    }
                }
            }

            RenderState::Disabled => {
                let ppu = &mut self.ppu;

                if ppu.line < 240 && 1 <= ppu.dot && ppu.dot <= 256 {
                    let color_id = ppu.bg_color_from_palette(0, 0);
                    self.videobuf_push(color_id);
                } else if ppu.line == ppu.prerender_line && ppu.dot == 0 {
                    ppu.stat.clear();
                } else if ppu.line == 241 && ppu.dot == 0 {
                    self.start_vblank();
                }

                let ppu = &mut self.ppu;
                ppu.dot += 1;
                if ppu.dot >= 341 {
                    ppu.dot = 0;
                    ppu.line += 1;
                    if ppu.line > ppu.prerender_line {
                        self.end_frame();
                    }
                }
            }
        }
    }

    fn ppu_render_step(&mut self, lut: &[RenderCmd]) {
        let cmd = &lut[self.ppu.dot as usize];
        let dot = self.ppu.dot;
        self.ppu.dot += 1;

        match cmd {
            RenderCmd::Idle => {}
            RenderCmd::NtFetch => {
                self.ppu.fetcher.nametbl = self.ppu_dispatch_read(self.ppu.nametbl_addr());
            }
            RenderCmd::AtFetch => {
                let attr = self.ppu_dispatch_read(self.ppu.attribute_addr());
                // we fetched the attribute, now we have to extract the correct 2 bits
                self.ppu.fetcher.attribute = self.ppu.palette_from_attribute(attr);
            }
            // https://www.nesdev.org/wiki/PPU_pattern_tables
            RenderCmd::BgLoFetch => {
                self.ppu.fetcher.pttrn_lo = self.ppu_dispatch_read(self.ppu.bg_pttrn_addr())
            }
            RenderCmd::BgHiFetch => {
                self.ppu.fetcher.pttrn_hi = self.ppu_dispatch_read(8 + self.ppu.bg_pttrn_addr());
                self.ppu.inc_scroll_x();
                self.ppu.shifter_load();
            }
            RenderCmd::BgHiFetchLast => {
                self.ppu.fetcher.pttrn_hi = self.ppu_dispatch_read(8 + self.ppu.bg_pttrn_addr());
                self.ppu.inc_scroll_x();
                self.ppu.inc_scroll_y();
                self.ppu.shifter_load();
            }
            RenderCmd::BgHiFetchFirst => {
                self.ppu.fetcher.pttrn_hi = self.ppu_dispatch_read(8 + self.ppu.bg_pttrn_addr());
                self.ppu.inc_scroll_x();
                self.ppu.shifter_load();
                // this is the first load, so it has to load to the upper bits, not the lower bits
                self.ppu.shifter_update(8);
            }
            RenderCmd::ResetHori => {
                self.ppu.restore_scroll_x();
                self.ppu.oam_addr = 0;

                // clear scanline sprite cache here
                self.ppu.spr_scanline.0.fill(0.into());
            }
            RenderCmd::OamClear => {}
            RenderCmd::SpriteEval => self.ppu.spr_evaluation(),
            RenderCmd::SprLoFetch => {
                let spr_idx = (dot - 257) / 8;
                let sprite = &self.ppu.oam_tmp[spr_idx as usize];
                self.ppu.fetcher.pttrn_lo = self.ppu_dispatch_read(self.ppu.spr_pttrn_addr(sprite));
            }
            RenderCmd::SprHiFetch => {
                let spr_idx = (dot - 257) / 8;
                let sprite = &self.ppu.oam_tmp[spr_idx as usize];

                self.ppu.fetcher.pttrn_hi =
                    self.ppu_dispatch_read(8 + self.ppu.spr_pttrn_addr(sprite));

                if spr_idx < self.ppu.oam_tmp_count as i16 {
                    self.ppu.spr_scanline.spr_push(
                        &self.ppu.oam_tmp[spr_idx as usize],
                        self.ppu.fetcher.pttrn_lo,
                        self.ppu.fetcher.pttrn_hi,
                    );
                }
            }
            RenderCmd::LastDotInLine => {
                self.ppu_dispatch_read(self.ppu.nametbl_addr());

                // push extra sprites here
                self.spr_push_extra();

                self.ppu.dot = 0;
                self.ppu.line += 1;
                if self.ppu.line == 240 {
                    self.ppu.render_state = RenderState::Vblank;
                }
            }

            RenderCmd::StatClear => self.ppu.stat.clear(),
            RenderCmd::ResetVert => self.ppu.restore_scroll_y(),
            RenderCmd::LastDotInFrame => {
                self.ppu_dispatch_read(self.ppu.nametbl_addr());

                self.ppu.dot = 0;
                if self.ppu.odd_frame && self.ppu.mask.contains(Mask::BgEnable) {
                    self.ppu.dot = 1;
                }

                self.end_frame();
                self.ppu.render_state = RenderState::Rendering;

                // no sprites should be visible on the first scanline
                self.ppu.spr_scanline.0.fill(0.into());
            }
        }
    }

    fn spr_push_extra(&mut self) {
        if !self.settings.disable_sprite_limit {
            return;
        }

        for sprite in &self.ppu.spr_extra {
            let pttrn_addr = self.ppu.spr_pttrn_addr(&sprite);

            let pttrn_lo = self.ppu_debug_read(pttrn_addr);
            let pttrn_hi = self.ppu_debug_read(pttrn_addr + 8);

            self.ppu.spr_scanline.spr_push(sprite, pttrn_lo, pttrn_hi);
        }
    }
}

const fn ppu_lut_build() -> [RenderCmd; 341] {
    let mut render_lut = [const { RenderCmd::Idle }; _];

    // https://www.nesdev.org/w/images/default/4/4f/Ppu.svg
    let mut i = 2;
    while i < 256 {
        render_lut[i + 0] = RenderCmd::NtFetch;
        render_lut[i + 2] = RenderCmd::AtFetch;
        render_lut[i + 4] = RenderCmd::BgLoFetch;
        render_lut[i + 6] = RenderCmd::BgHiFetch;
        i += 8;
    }

    render_lut[256] = RenderCmd::BgHiFetchLast;
    render_lut[257] = RenderCmd::ResetHori;

    i = 258;
    while i < 320 {
        render_lut[i + 0] = RenderCmd::NtFetch;
        render_lut[i + 2] = RenderCmd::AtFetch;
        render_lut[i + 4] = RenderCmd::SprLoFetch;
        render_lut[i + 6] = RenderCmd::SprHiFetch;
        i += 8;
    }

    i = 322;
    while i < 336 {
        render_lut[i + 0] = RenderCmd::NtFetch;
        render_lut[i + 2] = RenderCmd::AtFetch;
        render_lut[i + 4] = RenderCmd::BgLoFetch;
        render_lut[i + 6] = RenderCmd::BgHiFetch;
        i += 8;
    }

    render_lut[328] = RenderCmd::BgHiFetchFirst;

    render_lut[338] = RenderCmd::NtFetch;
    render_lut[340] = RenderCmd::LastDotInLine;

    render_lut
}

const RENDER_LUT: [RenderCmd; 341] = const {
    let mut res = ppu_lut_build();
    res[1] = RenderCmd::OamClear;
    res[65] = RenderCmd::SpriteEval;
    res
};

const PRERENDER_LUT: [RenderCmd; 341] = const {
    let mut res = ppu_lut_build();
    res[0] = RenderCmd::StatClear;

    let mut i = 280;
    while i < 305 {
        res[i] = RenderCmd::ResetVert;
        i += 1;
    }

    res[340] = RenderCmd::LastDotInFrame;

    res
};
