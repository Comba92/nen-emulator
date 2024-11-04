#![allow(dead_code)]

use std::fmt;

use bitflags::bitflags;
use log::{info, warn};

use super::cart::{Cart, NametblMirroring};

// +--------------------+ $10000
// |     Mirrors        |
// |  $0000 ~ $3fff     |
// +--------------------+ $4000
// |     Mirrors        |
// |  $3f00 ~ $3fff     |
// +--------------------+ $3f20
// | sprite palette     |
// +--------------------+ $3f10
// |  image palette     |
// +--------------------+ $3f00
// |     Mirrors        |
// |  $2000 ~ $2eff     |
// +--------------------+ $3000
// |  attribute table 3 |
// +--------------------+ $2fc0
// |      name  table 3 |
// +--------------------+ $2c00
// |  attribute table 2 |
// +--------------------+ $2bc0
// |      name  table 2 |
// +--------------------+ $2800
// |  attribute table 1 |
// +--------------------+ $27c0
// |      name  table 1 |
// +--------------------+ $2400
// |  attribute table 0 |
// +--------------------+ $23c0
// |      name  table 0 |
// +--------------------+ $2000
// |   pattern table 1  |
// +--------------------+ $1000
// |   pattern table 0  |
// +--------------------+ $0000


const VRAM_SIZE: usize = 0x0800; // 2KB
const OAM_SIZE: usize = 0x100; // 256 bytes

const PATTERNS_START: u16 = 0x0000;
const PATTERNS_SIZE: usize = 0x2000;
const PATTERNS_END: u16 = 0x1FFF;

const VRAM_START: u16 = NAMETBLS_START;
const NAMETBLS_START: u16 = 0x2000;
const NAMETBLS_END: u16 = 0x2FFF;

const PALETTES_START: u16 = 0x3F00;
const PALETTES_SIZE: usize = 0x20; // 32 bytes
const PALETTES_END: u16 = PALETTES_START + PALETTES_SIZE as u16;
const PALETTES_MIRRORS_END: u16 = 0x3FFF;
const VRAM_END: u16 = PALETTES_MIRRORS_END;

const PPU_MIRRORS_START: u16 = 0x4000;

pub const PPU_CTRL: u16 = 0x2000;
pub const PPU_MASK: u16 = 0x2001;
pub const PPU_STAT: u16 = 0x2002;
pub const OAM_ADDR: u16 = 0x2003;
pub const OAM_DATA: u16 = 0x2004;
pub const PPU_SCROLL: u16 = 0x2005;
pub const PPU_ADDR: u16 = 0x2006;
pub const PPU_DATA: u16 = 0x2007;
pub const OAM_DMA: u16 = 0x4014;

const VISIBLE_SCANLINES: usize = 240;
const VERTICAL_OVERSCAN: usize = 261;
const SCANLINE_PIXELS: usize = 256;
const HORIZONTAL_OVERSCAN: usize = 341;

const NAMETBL_WIDTH: usize = 32;
const NAMETBL_HEIGHT: usize = 30;
const NAMETBL_SIZE: u16 = 0x400; // 1 KB

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
    NAMETBLS_START + NAMETBL_SIZE*nametbl_idx as u16
  }
  pub fn vramm_addr_incr(&self) -> u16 {
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

struct OAMEntry {
  y: u8,
  tile: u8,
  attribute: u8,
  x: u8,
}

pub struct Ppu {
  pub vram: [u8; 0x4000],
  pub palette: [u8; PALETTES_SIZE],
  pub oam: [u8; OAM_SIZE],
  pub chr_rom: [u8; PATTERNS_SIZE],

  v: u16,
  t: u16,
  x: u8,
  w: u8,
  pub cycles: usize,
  pub scanline: usize,

  pub ctrl: PpuCtrl,
  pub mask: PpuMask,
  pub stat: PpuStat,
  pub oam_addr: u8,

  pub scroll_state: RequestScroll,
  pub scroll_x: u8,
  pub scroll_y: u8,

  pub req_state: RequestAddr,
  pub req_addr: u16,
  pub req_buf: u8,

  pub mirroring: NametblMirroring,
  pub nmi_requested: bool,
  pub cpu_cycles: usize,
}

#[derive(Debug)]
pub enum RequestAddr {
  Start, AddrHigh(u8)
}
pub enum RequestScroll {
  Start, ScrollX
}

impl fmt::Debug for Ppu {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Ppu").field("v", &self.v).field("t", &self.t).field("x", &self.x).field("w", &self.w).field("cycles", &self.cycles).field("scanline", &self.scanline).field("ctrl", &self.ctrl).field("mask", &self.mask).field("stat", &self.stat).field("req_state", &self.req_state).field("req_addr", &self.req_addr).field("req_buf", &self.req_buf).finish()
    }
}

impl Ppu {
  pub fn new(cart: &Cart) -> Self {
    let mut ppu = Self {
      vram: [0; 0x4000],
      oam: [0; OAM_SIZE], 
      palette: [0; PALETTES_SIZE], 
      chr_rom: [0; PATTERNS_SIZE],
      v: 0, t:0, x: 0, w: 0, 
      cycles: 21, scanline: 0,
      ctrl: PpuCtrl::empty(), 
      stat: PpuStat::empty(),
      mask: PpuMask::empty(),
      oam_addr: 0,
      req_state: RequestAddr::Start, req_addr: 0, req_buf: 0,
      scroll_state: RequestScroll::Start, scroll_x: 0, scroll_y: 0,
      mirroring: cart.header.nametbl_mirroring,
      nmi_requested: false,
      cpu_cycles: 0,
    };

    let (left, _) = ppu.vram.split_at_mut(cart.chr_rom.len());
    left.copy_from_slice(&cart.chr_rom);
    ppu
  }

  pub fn send_nmi(&mut self) {
    //if self.ctrl.contains(PpuCtrl::vblank_nmi_on) {
      self.nmi_requested = true;
    //}
  }

  pub fn step(&mut self, cycles: usize, cpu_cycles: usize) {
    self.cpu_cycles = cpu_cycles;
    self.cycles = self.cycles.wrapping_add(cycles);

    if self.cycles >= HORIZONTAL_OVERSCAN {
      self.cycles -= HORIZONTAL_OVERSCAN;
      self.scanline += 1;
    }

    if (0..VISIBLE_SCANLINES).contains(&self.scanline) {
      // drawing here
    } else if self.scanline == VISIBLE_SCANLINES+1 {
        self.stat.insert(PpuStat::vblank);
        self.send_nmi();
    } else if self.scanline >= VERTICAL_OVERSCAN {
      self.scanline = 0;
      self.stat.remove(PpuStat::vblank);
    }
  }

  fn next_req_addr(&mut self) {
    self.req_addr = (self.req_addr + self.ctrl.vramm_addr_incr()) & VRAM_END;
  }

  pub fn reg_read(&mut self, addr: u16) -> u8 {
    if [PPU_CTRL, PPU_MASK, OAM_ADDR, PPU_SCROLL, PPU_ADDR, OAM_DMA].contains(&addr) {
        info!("Invalid READ to write-only PPU register ${addr:04X}");
        return 0;
    }

    match addr {
      PPU_STAT => {
        let res = self.stat.bits();
        self.req_state = RequestAddr::Start;
        self.scroll_state = RequestScroll::Start;
        self.stat.remove(PpuStat::vblank);
        res
      }
      OAM_DATA => self.oam[self.oam_addr as usize],
      PPU_DATA => self.mem_read(self.req_addr),
      _ => {
        info!("Read to PPU REG ${addr:04X} at cycle {} not implemented", self.cpu_cycles);
        0
      }
    }
  }

  pub fn reg_write(&mut self, addr: u16, val: u8) {
      match addr {
        PPU_CTRL => self.ctrl = PpuCtrl::from_bits_retain(val),
        PPU_MASK => self.mask = PpuMask::from_bits_retain(val),
        PPU_STAT => info!("Invalid write to read-only PPUSTAT register ${addr:04X} at cycle {}", self.cpu_cycles),
        OAM_ADDR => self.oam_addr = val,
        OAM_DATA => {
          self.oam[self.oam_addr as usize] = val;
          self.oam_addr = self.oam_addr.wrapping_add(1);
        }
        PPU_SCROLL => {
          match self.scroll_state {
            RequestScroll::Start => {
              self.scroll_state = RequestScroll::ScrollX;
              self.scroll_x = val;
            }
            RequestScroll::ScrollX => {
              self.scroll_state = RequestScroll::Start;
              self.scroll_y = val;
            }
        }
        }
        PPU_ADDR => {
          match self.req_state {
              RequestAddr::Start => self.req_state = RequestAddr::AddrHigh(val),
              RequestAddr::AddrHigh(high) => {
                self.req_state = RequestAddr::Start;
                self.req_addr = u16::from_le_bytes([val, high]) & VRAM_END;
              }
          }
        }
        PPU_DATA => self.mem_write(self.req_addr, val),
        _ => info!("Write to PPU REG ${addr:04X} not implemented at cycle {}", self.cpu_cycles)
      };
  }

  pub fn mem_read(&mut self, addr: u16) -> u8 {
    let res = self.req_buf;
    let addr = addr & VRAM_END;
    self.req_buf = self.vram[addr as usize];
    self.next_req_addr();
    warn!("reading ppu at ${addr:04X} at cycle {}", self.cpu_cycles);
    info!("PPU read got value {:02X} from read buf", res);
    res
  }

  pub fn mem_read_old(&mut self, addr: u16) -> u8 {
    let res = self.req_buf;
    warn!("reading ppu at ${addr:04X} at cycle {}", self.cpu_cycles);

    let addr = addr & VRAM_END;
    let data = match addr {
      0..=PATTERNS_END => {
        warn!("reading chr_rom at ${addr:04X} at cycle {}", self.cpu_cycles);
        self.chr_rom[addr as usize]
      },
      NAMETBLS_START..=NAMETBLS_END => {
        let mirrored = self.mirror_nametbl(addr);
        warn!("reading vram at ${mirrored:04X} at cycle {}", self.cpu_cycles);
        self.vram[mirrored as usize]
      },
      PALETTES_START..=PALETTES_MIRRORS_END => {
        warn!("reading palettes at ${addr:04X} at cycle {}", self.cpu_cycles);
        let _palette = (addr & PALETTES_END) - PALETTES_START;
        // self.palette[palette as usize]
        0
      }
      _ => {
        info!("read to unused vram address ${addr:04X} at cycle {}", self.cpu_cycles);
        0
      }
    };

    self.req_buf = data;
    self.next_req_addr();
    info!("PPU read got value {:02X} from read buf", res);
    res
  }

  pub fn mem_write(&mut self, addr: u16, val: u8) {
    let addr = addr & VRAM_END;
    warn!("writing ppu at ${addr:04X} at cycle {}", self.cpu_cycles);
    self.vram[addr as usize] = val;
    self.next_req_addr();
  }

  pub fn mem_write_old(&mut self, addr: u16, val: u8) {
    warn!("writing ppu at ${addr:04X} at cycle {}", self.cpu_cycles);
    
    let addr = addr & VRAM_END;
    match addr {
      0..=PATTERNS_END => {
        info!("illegal write on chr_rom at ${addr:04X} at cycle {}", self.cpu_cycles);
        self.chr_rom[addr as usize] = val;
      },
      NAMETBLS_START..=NAMETBLS_END => {
        let mirrored = self.mirror_nametbl(addr);
        warn!("writing vram at ${mirrored:04X} at cycle {}", self.cpu_cycles);
        self.vram[mirrored as usize] = val;
      },
      PALETTES_START..=PALETTES_MIRRORS_END => {
        let palette = (addr & PALETTES_END) - PALETTES_START;
        warn!("writing palettes at ${palette:04X} (original: ${addr:04X}) at cycle {}", self.cpu_cycles);
        // self.palette[palette as usize] = val;
      }
      _ => {
        info!("write to unused vram address ${addr:04X} at cycle {}", self.cpu_cycles);
      }
    };

    self.next_req_addr();
  }

  // Horizontal:
  // 0x0800 [ B ]  [ A ] [ a ]
  // 0x0400 [ A ]  [ B ] [ b ]
 
  // Vertical:
  // 0x0800 [ B ]  [ A ] [ B ]
  // 0x0400 [ A ]  [ a ] [ b ]
  pub fn mirror_nametbl(&self, addr: u16) -> u16 {
    let addr = (addr & NAMETBLS_END) - NAMETBLS_START;
    let nametbl_idx = addr / NAMETBL_SIZE;
    
    use NametblMirroring::*;
    match (self.mirroring, nametbl_idx) {
      (Horizontally, 1) | (Horizontally, 2) => addr - NAMETBL_SIZE,
      (Horizontally, 3) => addr - NAMETBL_SIZE*2,
      (Vertically, 2) | (Vertically, 3) => addr - NAMETBL_SIZE*2,
      (_, _) => addr,
    }
  }
}

#[cfg(test)]
mod ppu_tests {
  use super::*;

  fn new_empty_ppu() -> Ppu {
    Ppu::new(&Cart::empty())
  }

  #[test]
  fn test_ppu_vram_writes() {
      let mut ppu = new_empty_ppu();
      ppu.reg_write(PPU_ADDR, 0x23);
      ppu.reg_write(PPU_ADDR, 0x05);
      ppu.reg_write(PPU_DATA, 0x66);

      assert_eq!(ppu.vram[0x0305], 0x66);
  }

  #[test]
    fn test_ppu_vram_reads() {
        let mut ppu = new_empty_ppu();
        ppu.reg_write(PPU_CTRL, 0);
        ppu.vram[0x0305] = 0x66;

        ppu.reg_write(PPU_ADDR, 0x23);
        ppu.reg_write(PPU_ADDR, 0x05);

        ppu.reg_read(PPU_DATA); //load_into_buffer
        assert_eq!(ppu.req_addr, 0x2306);
        assert_eq!(ppu.reg_read(PPU_DATA), 0x66);
    }

    #[test]
    fn test_ppu_vram_reads_cross_page() {
        let mut ppu = new_empty_ppu();
        ppu.reg_write(PPU_CTRL, 0);
        ppu.vram[0x01ff] = 0x66;
        ppu.vram[0x0200] = 0x77;

        ppu.reg_write(PPU_ADDR, 0x21);
        ppu.reg_write(PPU_ADDR, 0xff);

        ppu.reg_read(PPU_DATA); //load_into_buffer
        assert_eq!(ppu.reg_read(PPU_DATA), 0x66);
        assert_eq!(ppu.reg_read(PPU_DATA), 0x77);
    }

    #[test]
    fn test_ppu_vram_reads_step_32() {
        let mut ppu = new_empty_ppu();
        ppu.reg_write(PPU_CTRL, 0b100);
        ppu.vram[0x01ff] = 0x66;
        ppu.vram[0x01ff + 32] = 0x77;
        ppu.vram[0x01ff + 64] = 0x88;

        ppu.reg_write(PPU_ADDR, 0x21);
        ppu.reg_write(PPU_ADDR, 0xff);

        ppu.reg_read(PPU_DATA); //load_into_buffer
        assert_eq!(ppu.reg_read(PPU_DATA), 0x66);
        assert_eq!(ppu.reg_read(PPU_DATA), 0x77);
        assert_eq!(ppu.reg_read(PPU_DATA), 0x88);
    }

    #[test]
    fn test_vram_horizontal_mirror() {
        let mut ppu = new_empty_ppu();
        ppu.mirroring = NametblMirroring::Horizontally;

        ppu.reg_write(PPU_ADDR, 0x24);
        ppu.reg_write(PPU_ADDR, 0x05);

        ppu.reg_write(PPU_DATA, 0x66); //write to a

        ppu.reg_write(PPU_ADDR, 0x28);
        ppu.reg_write(PPU_ADDR, 0x05);

        ppu.reg_write(PPU_DATA, 0x77); //write to B

        ppu.reg_write(PPU_ADDR, 0x20);
        ppu.reg_write(PPU_ADDR, 0x05);

        ppu.reg_read(PPU_DATA); //load into buffer
        assert_eq!(ppu.reg_read(PPU_DATA), 0x66); //read from A

        ppu.reg_write(PPU_ADDR, 0x2C);
        ppu.reg_write(PPU_ADDR, 0x05);

        ppu.reg_read(PPU_DATA); //load into buffer
        assert_eq!(ppu.reg_read(PPU_DATA), 0x77); //read from b
    }

    #[test]
    fn test_vram_vertical_mirror() {
        let mut ppu = new_empty_ppu();
        ppu.mirroring = NametblMirroring::Vertically;

        ppu.reg_write(PPU_ADDR, 0x20);
        ppu.reg_write(PPU_ADDR, 0x05);

        ppu.reg_write(PPU_DATA, 0x66); //write to A

        ppu.reg_write(PPU_ADDR, 0x2C);
        ppu.reg_write(PPU_ADDR, 0x05);

        ppu.reg_write(PPU_DATA, 0x77); //write to b

        ppu.reg_write(PPU_ADDR, 0x28);
        ppu.reg_write(PPU_ADDR, 0x05);

        ppu.reg_read(PPU_DATA); //load into buffer
        assert_eq!(ppu.reg_read(PPU_DATA), 0x66); //read from a

        ppu.reg_write(PPU_ADDR, 0x24);
        ppu.reg_write(PPU_ADDR, 0x05);

        ppu.reg_read(PPU_DATA); //load into buffer
        assert_eq!(ppu.reg_read(PPU_DATA), 0x77); //read from B
    }

    #[test]
    fn test_ppu_vram_mirroring() {
        let mut ppu = new_empty_ppu();
        ppu.reg_write(PPU_CTRL, 0);
        ppu.vram[0x0305] = 0x66;

        ppu.reg_write(PPU_ADDR, 0x63); //0x6305 -> 0x2305
        ppu.reg_write(PPU_ADDR, 0x05);

        ppu.reg_read(PPU_DATA); //load into_buffer
        assert_eq!(ppu.reg_read(PPU_DATA), 0x66);
        // assert_eq!(ppu.addr.read(), 0x0306)
    }
}