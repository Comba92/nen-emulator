use crate::{bus::{self, Banking, ChrBank, MemHandler}, cart::CartHeader, emu::Mirroring};

// https://www.nesdev.org/wiki/Mapper
pub trait Mapper {
  fn new(header: &CartHeader, mem: &mut MemHandler) -> Box<Self> where Self: Sized;
  fn prg_write(&mut self, mem: &mut MemHandler, addr: u16, val: u8);
  fn step(&mut self, _mem: &mut MemHandler) {}

  // TODO: temporary solution for MMC2
  fn notify_mmc2(&mut self, _addr: u16, _mem: &mut MemHandler) {}
  fn notify_mmc3(&mut self, _mem: &mut MemHandler) {}
}

pub fn mapper_from_header(header: &CartHeader, mem: &mut MemHandler) -> Result<Box<dyn Mapper>, String> {
  let mapper: Box<dyn Mapper> = match header.mapper {
    0 => NROM::new(header, mem),
    1 => MMC1::new(header, mem),
    2 => UxROM::new(header, mem),
    3 => CNROM::new(header, mem),
    4 => MMC3::new(header, mem),
    7 => AxROM::new(header, mem),
    9 => MMC2::new(header, mem),
    11 => ColorDreams::new(header, mem),
    66 => GxROM::new(header, mem),
    71 => Codemasters::new(header, mem),
    _ => return Err(format!("mapper {} not implemented", header.mapper)),
  };

  Ok(mapper)
}

// https://www.nesdev.org/wiki/NROM
pub struct NROM;
impl Mapper for NROM {
  fn new(header: &CartHeader, mem: &mut MemHandler) -> Box<Self> {    
    if header.prg_size <= 16 * 1024 {
      // Mirror of $8000-$BFFF (NROM-128).
      mem.banks.prg.set_page(1, 0);
    } else {
      // Last 16 KB of ROM (NROM-256)
      mem.banks.prg.set_page(1, 1);
    }

    Box::new(Self)
  }

  fn prg_write(&mut self, _: &mut MemHandler, _: u16, _: u8) {}
}

// https://www.nesdev.org/wiki/UxROM
struct UxROM; 
impl Mapper for UxROM {
  fn new(_: &CartHeader, mem: &mut MemHandler) -> Box<Self> {
    mem.banks.prg.set_page_to_last_bank(1);
    Box::new(Self)
  }

  fn prg_write(&mut self, mem: &mut MemHandler, _: u16, val: u8) {
    mem.banks.prg.set_page(0, val & 0b111);
  }
}

// https://www.nesdev.org/wiki/CNROM
struct CNROM;
impl Mapper for CNROM {
  fn new(header: &CartHeader, mem: &mut MemHandler) -> Box<Self> {
    if header.prg_size <= 16 * 1024 {
      mem.banks.prg.set_page(1, 0);
    } else {
      mem.banks.prg.set_page(1, 1);
    }
    
    mem.banks.sram = Banking::new(2 * 1024, 0x6000, 2 * 1024, 4);
    Box::new(Self)
  }

  fn prg_write(&mut self, mem: &mut MemHandler, _: u16, val: u8) {
    mem.banks.chr.set_page(0, val & 0b11);
  }
}

// https://www.nesdev.org/wiki/GxROM
struct GxROM;
impl Mapper for GxROM {
  fn new(header: &CartHeader, mem: &mut MemHandler) -> Box<Self> {
    mem.banks.prg = Banking::new_prg(header, 1);
    Box::new(Self)
  }

  fn prg_write(&mut self, mem: &mut MemHandler, _: u16, val: u8) {
    mem.banks.prg.set_page(0, (val >> 4) & 0b11);
    mem.banks.chr.set_page(0, val & 0b11);
  }
}

// https://www.nesdev.org/wiki/AxROM
struct AxROM;
impl Mapper for AxROM {
  fn new(header: &CartHeader, mem: &mut MemHandler) -> Box<Self> where Self: Sized {
    mem.banks.prg = Banking::new_prg(header, 1);
    Box::new(Self)
  }

  fn prg_write(&mut self, mem: &mut MemHandler, _: u16, val: u8) {
    mem.banks.prg.set_page(0, val & 0b111);
    
    let mirroring = if val & 0x10 == 0 {
      Mirroring::SingleScreenA
    } else {
      Mirroring::SingleScreenB
    };
    mem.banks.vram.mirror(&mirroring);
  }
}

struct ColorDreams;
impl Mapper for ColorDreams {
  fn new(header: &CartHeader, mem: &mut MemHandler) -> Box<Self> {
    mem.banks.prg = Banking::new_prg(header, 1);
    Box::new(Self)
  }

  fn prg_write(&mut self, mem: &mut MemHandler, _: u16, val: u8) {
    mem.banks.prg.set_page(0, val & 0b11);
    mem.banks.chr.set_page(0, val >> 4);
  }
}

// TODO: not fully implemented
struct Codemasters;
impl Mapper for Codemasters {
  fn new(header: &CartHeader, mem: &mut MemHandler) -> Box<Self> {
    mem.banks.prg = Banking::new_prg(header, 2);
    mem.banks.prg.set_page_to_last_bank(1);

    Box::new(Self)
  }

  fn prg_write(&mut self, mem: &mut MemHandler, addr: u16, val: u8) {
    match addr {
      0xc000..=0xffff => mem.banks.prg.set_page(0, val & 0b1111),
      _ => {}
    }
  }
}

// TODO: SxROM support
// Needs NES2.0 / db support for SRAM
// TODO: prg rom write delay
#[derive(Default)]
struct MMC1 {
  shift_reg: u8,
  shift_count: u8,

  prg_swapped: u8,
  prg_bank: u8,
  prg_bank_mask: u8,

  chr_bank_mask: u8,
}
impl Mapper for MMC1 {
  fn new(header: &CartHeader, mem: &mut MemHandler) -> Box<Self> {
    mem.banks.chr = Banking::new_chr(header, 2);
    
    // starts in mode3 by default
    mem.banks.prg.set_page(0, 0);
    mem.banks.prg.set_page_to_last_bank(1);

    Box::new(Self::default())
  }

  fn prg_write(&mut self, mem: &mut MemHandler, addr: u16, val: u8) {
    if val & 0x80 != 0 {
      self.shift_reg = 0;
      self.shift_count = 0;

      // back to mode3
      mem.banks.prg.change_mode(2);
      mem.banks.prg.set_page(0, self.prg_bank);
      self.prg_bank_mask = 0;
      
      return;
    }

    self.shift_reg |= (val & 1) << self.shift_count;
    self.shift_count += 1;

    if self.shift_count < 5 { return; }

    let shift_val = self.shift_reg;
    self.shift_reg = 0;
    self.shift_count = 0;

    // TODO: only higher bits of address are needed
    match addr {
      // 0x8000..0x9ffff
      0x8000..=0x9fff => {
        let mirroring = match shift_val & 0b11 {
          0 => Mirroring::SingleScreenA,
          1 => Mirroring::SingleScreenB,
          2 => Mirroring::Vertical,
          _ => Mirroring::Horizontal
        };
        mem.banks.vram.mirror(&mirroring);
        
        let prg_mode = (shift_val >> 2) & 0b11;
        (self.prg_swapped, self.prg_bank_mask) = match prg_mode {
          2 => {
            // 2: fix first bank at $8000 and switch 16 KB bank at $C000
            mem.banks.prg.change_mode(2);
            mem.banks.prg.set_page(0, 0);
            mem.banks.prg.set_page(1, self.prg_bank);
            (1, 0)
          }
          3 => {
            // 3: fix last bank at $C000 and switch 16 KB bank at $8000)
            mem.banks.prg.change_mode(2);
            mem.banks.prg.set_page(0, self.prg_bank);
            mem.banks.prg.set_page_to_last_bank(1);
            (0, 0)
          }
          _ => {
            // 0, 1: switch 32 KB at $8000, ignoring low bit of bank number;
            mem.banks.prg.change_mode(1);
            mem.banks.prg.set_page(0, self.prg_bank & !1);
            (0, 1)
          }
        };

        let chr_mode = shift_val & 0x80;
        if chr_mode == 0 {
          mem.banks.chr.change_mode(1);
          self.chr_bank_mask = 1;
        } else {
          mem.banks.chr.change_mode(2);
          self.chr_bank_mask = 0;
        };
      }
      // 0xa000..0xbfff
      0xa000..=0xbfff => mem.banks.chr.set_page(0, shift_val & !self.chr_bank_mask),
      // 0xc000..0xdfff
      0xc000..=0xdfff => mem.banks.chr.set_page(1, shift_val),
      // 0xe000..0xffff
      0xe000..=0xffff => {
        self.prg_bank = shift_val;
        mem.banks.prg.set_page(self.prg_swapped, shift_val & !self.prg_bank_mask);
      }
      _ => {}
    } 
  }
}

mod mmc2 {
  pub enum Latch {
    FD, FE
  }
}

struct MMC2 {
  bank_fd: Banking<ChrBank>,
  bank_fe: Banking<ChrBank>,
  latch0: mmc2::Latch,
  latch1: mmc2::Latch,
}

impl Mapper for MMC2 {
  fn new(header: &CartHeader, mem: &mut MemHandler) -> Box<Self> where Self: Sized {
    mem.banks.prg = Banking::new_prg(header, 4);
    let last_bank = (mem.banks.prg.banks_count - 1) as u8;
    mem.banks.prg.set_page(1, last_bank-2);
    mem.banks.prg.set_page(2, last_bank-1);
    mem.banks.prg.set_page(3, last_bank);

    mem.banks.chr = Banking::new_chr(header, 2);

    Box::new(Self {
      bank_fd: Banking::new_chr(header, 2),
      bank_fe: Banking::new_chr(header, 2),
      latch0: mmc2::Latch::FD,
      latch1: mmc2::Latch::FD,
    })
  }

  fn prg_write(&mut self, mem: &mut MemHandler, addr: u16, val: u8) {
    // TODO: only high bits needed
    match addr {
      0xa000..=0xafff => mem.banks.prg.set_page(0, val & 0xf),
      0xb000..=0xbfff => self.bank_fd.set_page(0, val & 0x1f),
      0xc000..=0xcfff => self.bank_fe.set_page(0, val & 0x1f),
      0xd000..=0xdfff => self.bank_fd.set_page(1, val & 0x1f),
      0xe000..=0xefff => self.bank_fe.set_page(1, val & 0x1f),
      0xf000..=0xffff => {
        let mirroring = match val & 1 {
          0 => Mirroring::Vertical,
          _ => Mirroring::Horizontal
        };

        mem.banks.vram.mirror(&mirroring);
      }
      _ => {}
    }
  }

  // TODO: temporary solution
  fn notify_mmc2(&mut self, addr: u16, mem: &mut MemHandler) {
    use mmc2::Latch;
    let banks = &mut mem.banks;

    match addr {
      0x0fd8 => self.latch0 = Latch::FD,
      0x0fe8 => self.latch0 = Latch::FE,
      0x1fd8..=0x1fdf => self.latch1 = Latch::FD, 
      0x1fe8..=0x1fef => self.latch1 = Latch::FE,
      _ => {}
    }

    match self.latch0 {
      Latch::FD => banks.chr.bankings[0] = self.bank_fd.bankings[0],
      Latch::FE => banks.chr.bankings[0] = self.bank_fe.bankings[0],
    }

    match self.latch1 {
      Latch::FD => banks.chr.bankings[1] = self.bank_fd.bankings[1],
      Latch::FE => banks.chr.bankings[1] = self.bank_fe.bankings[1],
    }
  }
}

#[derive(Default)]
struct MMC3 {
  bank_select: u8,

  chr_invert: bool,

  prg_mode: u8,
  prg_swapped: u8,

  irq_count: u8,
  irq_latch: u8,
  irq_reload: bool,
  irq_enabled: bool,

  a12_low_count: isize,
  clock_count: usize,
}
// https://forums.nesdev.org/viewtopic.php?t=14056
impl Mapper for MMC3 {
  fn new(header: &CartHeader, mem: &mut MemHandler) -> Box<Self> {
    mem.banks.prg = Banking::new_prg(header, 4);
    // start with prg mode0
    mem.banks.prg.set_page(2, mem.banks.prg.banks_count as u8 - 2);
    mem.banks.prg.set_page_to_last_bank(3);

    mem.banks.chr = Banking::new_chr(header, 8);
    mem.banks.chr.set_page2(0, 0);
    mem.banks.chr.set_page2(2, 0);

    Box::new(Self::default())
  }

  fn prg_write(&mut self, mem: &mut MemHandler, addr: u16, val: u8) {
    // TODO: only higher bits and first bit matters
    
    
    match (addr, addr % 2 == 0) {
      (0x8000..=0x9fff, true) => {
        self.bank_select = val & 0x7;
        
        let chr_invert = val & 0x80 > 0;
        if self.chr_invert != chr_invert {
          for i in 0..4 {
            mem.banks.chr.swap_pages(i, i+4);
          }

          self.chr_invert = chr_invert;
        }

        let prg_mode = val & 0x40;
        if self.prg_mode != prg_mode {
          mem.banks.prg.swap_pages(0, 2);

          self.prg_swapped = if prg_mode == 0 { 0 } else { 2 };
          self.prg_mode = prg_mode;
        }
      }

      (0x8000..=0x9fff, false) => {
        match (self.bank_select, self.chr_invert) {
          (6, _) => mem.banks.prg.set_page(self.prg_swapped, val & 0x3f),
          (7, _) => mem.banks.prg.set_page(1, val & 0x3f),
          (0 | 1, false) => mem.banks.chr.set_page2(self.bank_select * 2, val),
          (0 | 1, true)  => mem.banks.chr.set_page2(self.bank_select * 2 + 4, val),
          // cases 2..=5
          (_ , false)    => mem.banks.chr.set_page((self.bank_select - 2) + 4, val),
          (_, true)      => mem.banks.chr.set_page(self.bank_select - 2, val),
        }
      }

      (0xa000..=0xbfff, true) => {
        // inverted from what wiki says...
        let mirroring = match val & 1 {
          0 => Mirroring::Vertical,
          _ => Mirroring::Horizontal,
        };
        mem.banks.vram.mirror(&mirroring);
      }

      (0xa000..=0xbfff, false) => {
        // TODO: ram protect
      }

      (0xc000..=0xdfff, true) => self.irq_latch = val,
      (0xc000..=0xdfff, false) => {
        self.irq_reload = true;
        self.irq_count = 0;
      }

      (0xe000..=0xffff, true) => {
        self.irq_enabled = false;
        mem.irq.remove(bus::IrqFlags::MAPPER);
      }
      (0xe000..=0xffff, false) => self.irq_enabled = true, 
      _ => {}
    }
  }

  // fn step(&mut self, mem: &mut MemHandler) {
  //   let a12_low = mem.ppu_addr_bus & 0x1000 == 0;

  //   if self.a12_low_count >= 3 && !a12_low {
  //     // println!("Decrementing IRQ at cycle {}", mem.ppu_cycle);
  //     // self.clock_count += 1;

  //     // if mem.ppu_scanline > 240 {
  //     //   println!("Clocked for {} scanlines", self.clock_count);
  //     //   self.clock_count = 0;
  //     // }

  //     if self.irq_reload || self.irq_count == 0 {
  //       self.irq_count = self.irq_latch;
  //       self.irq_reload = false;
  //     } else {
  //       self.irq_count -= 1;
  //     }

  //     if self.irq_enabled && self.irq_count == 0 {
  //       mem.irq.insert(bus::IrqFlags::MAPPER);
  //     }
  //   }

  //   if a12_low {
  //     self.a12_low_count += 1;
  //   } else {
  //     self.a12_low_count = 0;
  //   }
  // }

  fn notify_mmc3(&mut self, mem: &mut MemHandler) {
    // println!("Decrementing IRQ at cycle {}", mem.ppu_cycle);

    if self.irq_reload || self.irq_count == 0 {
      self.irq_count = self.irq_latch;
      self.irq_reload = false;
    } else {
      self.irq_count -= 1;
    }

    if self.irq_enabled && self.irq_count == 0 {
      mem.irq.insert(bus::IrqFlags::MAPPER);
    }
  }
}