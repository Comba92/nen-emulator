use crate::{bus::{self, Banking, Bus, ChrBank, CpuHandler, IrqFlags, PpuHandler, VramBank}, cart::CartHeader, emu::Mirroring, utils::{byte_set_hi, byte_set_lo}};

// https://www.nesdev.org/wiki/Mapper
pub trait Mapper {
  fn new(header: &CartHeader, mem: &mut Bus) -> Box<Self> where Self: Sized;
  // 0x8000..=0xffff
  fn prg_write(&mut self, mem: &mut Bus, addr: u16, val: u8);
  // 0x4020..=0x5fff
  fn cart_read(&mut self, _mem: &mut Bus, _addr: u16) -> u8 { 0 }
  fn cart_write(&mut self, _mem: &mut Bus, _addr: u16, _val: u8) {}
  fn step(&mut self, _mem: &mut Bus) {}

  fn notify_ppu_addr(&mut self, _mem: &mut Bus, _cycles: usize) {}
  fn sample(&self) -> f32 { 0.0 }
}

pub fn mapper_from_header(header: &CartHeader, mem: &mut Bus) -> Result<Box<dyn Mapper>, String> {
  let mapper: Box<dyn Mapper> = match header.mapper {
    0 => NROM::new(header, mem),
    1 => MMC1::new(header, mem),
    2 | 94 | 180 => UxROM::new(header, mem),
    3 | 185 => CNROM::new(header, mem),
    4 => MMC3::new(header, mem),
    // 5 => MMC5::new(header, mem),
    7 => AxROM::new(header, mem),
    9 => MMC2::new(header, mem),
    11 => ColorDreams::new(header, mem),
    13 => CPROM::new(header, mem),
    // 19 => Namco129_163::new(header, mem),
    24 | 26 => VRC6::new(header, mem),
    31 => NSF::new(header, mem),
    66 | 140 => GxROM::new(header, mem),
    68 => Sunsoft4::new(header, mem),
    69 => SunsoftFME7::new(header, mem),
    70 | 152 => Bandai74::new(header, mem),
    71 | 232 => Codemasters::new(header, mem),
    73 => VRC3::new(header, mem),
    75 => VRC1::new(header, mem),
    78 => Irem74HCx::new(header, mem),
    87 | 101 => J87::new(header, mem),
    89 => Sunsoft89::new(header, mem),
    93 => Sunsoft93::new(header, mem),
    97 => IremTAMS1::new(header, mem),
    184 => Sunsoft1::new(header, mem),
    // TODO: mapper 34
    // TODO: mapper 72 / 92
    // TODO: mapper 206 (mmc3 prototype)
    _ => return Err(format!("mapper {} not implemented", header.mapper)),
  };

  Ok(mapper)
}

// https://www.nesdev.org/wiki/NROM
pub struct NROM;
impl Mapper for NROM {
  fn new(header: &CartHeader, mem: &mut Bus) -> Box<Self> {    
    // if header.prg_size <= 16 * 1024 {
    //   // Mirror of $8000-$BFFF (NROM-128).
    //   mem.banks.prg.set_page(1, 0);
    // } else {
    //   // Last 16 KB of ROM (NROM-256)
    //   mem.banks.prg = Banking::new_prg(header, 1);
    // }
    if header.prg_size > 16 * 1024 {
      mem.banks.prg = Banking::new_prg(header, 1);
    }

    Box::new(Self)
  }

  fn prg_write(&mut self, _: &mut Bus, _: u16, _: u8) {}
}

// https://www.nesdev.org/wiki/UxROM
struct UxROM {
  bank: u8,
  shift: u8,
}
impl Mapper for UxROM {
  fn new(header: &CartHeader, mem: &mut Bus) -> Box<Self> {
    let shift = if header.mapper == 94 { 2 } else { 0 };
    let (swapped, fixed) = if header.mapper == 180 { (1, 0) } else { (0, 1) };
    mem.banks.prg.set_page_to_last_bank(fixed);

    Box::new(Self {
      bank: swapped, shift,
    })
  }

  fn prg_write(&mut self, mem: &mut Bus, _: u16, val: u8) {
    mem.banks.prg.set_page(self.bank, val >> self.shift);
  }
}

// https://www.nesdev.org/wiki/CNROM
struct CNROM;
impl Mapper for CNROM {
  fn new(_: &CartHeader, mem: &mut Bus) -> Box<Self> {
    // The Namco game Hayauchi Super Igo adds 2 KiB of PRG-RAM, denoted using mapper 3 and the appropriate value in the header's PRG-RAM size field.
    mem.banks.wram = Banking::new(2 * 1024, 2 * 1024, 4);
    Box::new(Self)
  }

  fn prg_write(&mut self, mem: &mut Bus, _: u16, val: u8) {
    mem.banks.chr.set_page(0, val);
  }
}

// https://www.nesdev.org/wiki/GxROM
struct GxROM;
impl Mapper for GxROM {
  fn new(header: &CartHeader, mem: &mut Bus) -> Box<Self> {
    if header.mapper == 140 {
      todo!("mapper 140");
    } else {
      mem.banks.prg = Banking::new_prg(header, 1);
    }

    Box::new(Self)
  }

  fn prg_write(&mut self, mem: &mut Bus, _: u16, val: u8) {
    mem.banks.prg.set_page(0, (val >> 4) & 0b11);
    mem.banks.chr.set_page(0, val & 0b1111);
  }
}

// https://www.nesdev.org/wiki/AxROM
struct AxROM;
impl Mapper for AxROM {
  fn new(header: &CartHeader, mem: &mut Bus) -> Box<Self> where Self: Sized {
    mem.banks.prg = Banking::new_prg(header, 1);
    Box::new(Self)
  }

  fn prg_write(&mut self, mem: &mut Bus, _: u16, val: u8) {
    mem.banks.prg.set_page(0, val & 0b111);
    
    let mirroring = if val & 0x10 == 0 {
      Mirroring::SingleScreenA
    } else {
      Mirroring::SingleScreenB
    };
    mem.banks.vram.mirror(&mirroring);
  }
}

// https://www.nesdev.org/wiki/Color_Dreams
struct ColorDreams;
impl Mapper for ColorDreams {
  fn new(header: &CartHeader, mem: &mut Bus) -> Box<Self> {
    mem.banks.prg = Banking::new_prg(header, 1);
    Box::new(Self)
  }

  fn prg_write(&mut self, mem: &mut Bus, _: u16, val: u8) {
    mem.banks.prg.set_page(0, val & 0b11);
    mem.banks.chr.set_page(0, val >> 4);
  }
}

// https://www.nesdev.org/wiki/INES_Mapper_071
#[derive(Default)]
struct Codemasters {
  mapper: u16,
  prg_block: u8,
  prg_bank: u8,
}
impl Mapper for Codemasters {
  fn new(header: &CartHeader, mem: &mut Bus) -> Box<Self> {
    mem.banks.prg = Banking::new_prg(header, 2);
    mem.banks.prg.set_page_to_last_bank(1);

    Box::new(Self {
      mapper: header.mapper,
      ..Default::default()
    })
  }

  fn prg_write(&mut self, mem: &mut Bus, addr: u16, val: u8) {
    match (addr >> 12, self.mapper) {
      (0x8..=0xb, 232) => {
        self.prg_block = (val >> 3) & 0b11;
        mem.banks.prg.set_page(0, (self.prg_block << 4) | self.prg_bank);
      }
      // For compatibility without using a submapper, FCEUX begins all games with fixed mirroring, and applies single screen mirroring only once $9000-9FFF is written, ignoring writes to $8000-8FFF.
      (0x9, _) => if val & 0x10 == 0 {
        mem.banks.vram.mirror(&Mirroring::SingleScreenA);
      } else {
        mem.banks.vram.mirror(&Mirroring::SingleScreenB);
      }
      (0xc..=0xf, 71) => mem.banks.prg.set_page(0, val & 0b1111),
      (0xc..=0xf, 232) => {
        self.prg_bank = val & 0b11;
        mem.banks.prg.set_page(0, (self.prg_block << 4) | self.prg_bank);
      }
      _ => {}
    }
  }
}

// https://www.nesdev.org/wiki/CPROM
struct CPROM;
impl Mapper for CPROM {
  fn new(header: &CartHeader, mem: &mut Bus) -> Box<Self> {
    mem.banks.chr = Banking::new_chr(header, 2);
    Box::new(Self)
  }

  fn prg_write(&mut self, mem: &mut Bus, _: u16, val: u8) {
    mem.banks.chr.set_page(1, val & 0b11);
  }
}

// https://www.nesdev.org/wiki/INES_Mapper_031
struct NSF;
impl Mapper for NSF {
  fn new(header: &CartHeader, mem: &mut Bus) -> Box<Self> {
    mem.banks.prg = Banking::new_prg(header, 8);
    mem.banks.prg.set_page_to_last_bank(7);
    Box::new(Self)
  }

  fn prg_write(&mut self, _: &mut Bus, _: u16, _: u8) {}
  fn cart_write(&mut self, mem: &mut Bus, addr: u16, val: u8) {
    if (addr >> 12) == 0x5 {
      mem.banks.prg.set_page(addr as u8 & 0b111, val);
    }
  }
}

// https://www.nesdev.org/wiki/INES_Mapper_078
struct Irem74HCx {
  is_holy_diver: bool,
}
impl Mapper for Irem74HCx {
  fn new(header: &CartHeader, mem: &mut Bus) -> Box<Self> {
    mem.banks.prg.set_page_to_last_bank(1);

    Box::new(Self {
      is_holy_diver: header.alt_mirroring
    })
  }

  fn prg_write(&mut self, mem: &mut Bus, _: u16, val: u8) {
    mem.banks.prg.set_page(0, val & 0b111);
    mem.banks.chr.set_page(0, val >> 4);

    let mirroring = match (self.is_holy_diver, val & 0x8) {
      (true, 0)  => Mirroring::Horizontal,
      (true, _)  => Mirroring::Vertical,
      (false, 0) => Mirroring::SingleScreenA,
      (false, _) => Mirroring::SingleScreenB
    };
    mem.banks.vram.mirror(&mirroring);
  }
}

// https://www.nesdev.org/wiki/INES_Mapper_152
// https://www.nesdev.org/wiki/INES_Mapper_070
// TODO: very similiar to Sunsoft89
struct Bandai74 {
  mapper: u16,
}
impl Mapper for Bandai74 {
  fn new(header: &CartHeader, mem: &mut Bus) -> Box<Self> {
    mem.banks.prg.set_page_to_last_bank(1);
    Box::new(Self {
      mapper: header.mapper,
    })
  }

  fn prg_write(&mut self, mem: &mut Bus, _: u16, val: u8) {
    mem.banks.chr.set_page(0, val & 0b1111);
    
    if self.mapper == 152 {
      mem.banks.prg.set_page(0, (val >> 4) & 0b111);
      let mirroring = if val & 0x8 == 0 { Mirroring::SingleScreenA } else { Mirroring::SingleScreenB };
      mem.banks.vram.mirror(&mirroring);
    } else {
      mem.banks.prg.set_page(0, (val >> 4) & 0b1111);
    }
  }
}

struct IremTAMS1;
impl Mapper for IremTAMS1 {
  fn new(_: &CartHeader, _: &mut Bus) -> Box<Self> {
    Box::new(Self)
  }

  fn prg_write(&mut self, mem: &mut Bus, _: u16, val: u8) {
    mem.banks.prg.set_page(1, val & 0b11111);
    let mirroring = if val & 0x80 == 0 { Mirroring::Horizontal } else { Mirroring::Vertical }; 
    mem.banks.vram.mirror(&mirroring);
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
  fn new(header: &CartHeader, mem: &mut Bus) -> Box<Self> {
    mem.banks.chr = Banking::new_chr(header, 2);
    
    // starts in mode3 by default
    mem.banks.prg.set_page(0, 0);
    mem.banks.prg.set_page_to_last_bank(1);

    Box::new(Self::default())
  }

  fn prg_write(&mut self, mem: &mut Bus, addr: u16, val: u8) {
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

    match addr & 0xe000 {
      // 0x8000..0x9ffff
      0x8000 => {
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
      0xa000 => mem.banks.chr.set_page(0, shift_val & !self.chr_bank_mask),
      // 0xc000..0xdfff
      0xc000 => mem.banks.chr.set_page(1, shift_val),
      // 0xe000..0xffff
      0xe000 => {
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
  // TODO: do we really need a banking object here? probably just four registers
  // we can do that (tested) but we'd like to precompute the set_page() on prg_write
  bank_fd: Banking<ChrBank>,
  bank_fe: Banking<ChrBank>,
  latch0: mmc2::Latch,
  latch1: mmc2::Latch,
}

impl Mapper for MMC2 {
  fn new(header: &CartHeader, mem: &mut Bus) -> Box<Self> where Self: Sized {
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

  fn prg_write(&mut self, mem: &mut Bus, addr: u16, val: u8) {
    match addr >> 12 {
      0xa => mem.banks.prg.set_page(0, val & 0xf),
      0xb => self.bank_fd.set_page(0, val & 0x1f),
      0xc => self.bank_fe.set_page(0, val & 0x1f),
      0xd => self.bank_fd.set_page(1, val & 0x1f),
      0xe => self.bank_fe.set_page(1, val & 0x1f),
      0xf => {
        let mirroring = match val & 1 {
          0 => Mirroring::Vertical,
          _ => Mirroring::Horizontal
        };

        mem.banks.vram.mirror(&mirroring);
      }
      _ => {}
    }
  }

  fn notify_ppu_addr(&mut self, mem: &mut Bus, _: usize) {
    use mmc2::Latch;
    let banks = &mut mem.banks;

    match mem.ppu_addr_bus {
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

  a12_low_count: usize,
}
// https://forums.nesdev.org/viewtopic.php?t=14056
impl Mapper for MMC3 {
  fn new(header: &CartHeader, mem: &mut Bus) -> Box<Self> {
    if header.alt_mirroring {
      // MMC3 can have 4 screen mirroring
      mem.banks.vram.change_size(4 * 1024);
      mem.banks.vram.mirror(&Mirroring::FourScreens);
      mem.vram.resize(4 * 1024, 0);
    }

    mem.banks.prg = Banking::new_prg(header, 4);
    // start with prg mode0
    mem.banks.prg.set_page(2, mem.banks.prg.banks_count as u8 - 2);
    mem.banks.prg.set_page_to_last_bank(3);

    mem.banks.chr = Banking::new_chr(header, 8);
    mem.banks.chr.set_page2x(0, 0);
    mem.banks.chr.set_page2x(2, 0);

    Box::new(Self::default())
  }

  fn prg_write(&mut self, mem: &mut Bus, addr: u16, val: u8) {
    // TODO: only higher bits and first bit matters
  
    match addr & 0xe001 {
      // (0x8000..=0x9fff, true)
      0x8000 => {
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

      // (0x8000..=0x9fff, false)
      0x8001 => {
        match (self.bank_select, self.chr_invert) {
          (6, _) => mem.banks.prg.set_page(self.prg_swapped, val & 0x3f),
          (7, _) => mem.banks.prg.set_page(1, val & 0x3f),
          (0 | 1, false) => mem.banks.chr.set_page2x(self.bank_select * 2, val),
          (0 | 1, true)  => mem.banks.chr.set_page2x(self.bank_select * 2 + 4, val),
          // cases 2..=5
          (_ , false)    => mem.banks.chr.set_page((self.bank_select - 2) + 4, val),
          (_, true)      => mem.banks.chr.set_page(self.bank_select - 2, val),
        }
      }

      // (0xa000..=0xbfff, true)
      0xa000 => {
        // This bit has no effect on cartridges with hardwired 4-screen VRAM.
        if mem.vram.len() > 2 * 1024 { return; }

        // inverted from what wiki says...
        let mirroring = match val & 1 {
          0 => Mirroring::Vertical,
          _ => Mirroring::Horizontal,
        };
        mem.banks.vram.mirror(&mirroring);
      }
      
      // (0xa000..=0xbfff, false)
      0xa001 => {
        // TODO: ram protect
      }

      // (0xc000..=0xdfff, true)
      0xc000 => self.irq_latch = val,
      
      // (0xc000..=0xdfff, false)
      0xc001 => {
        self.irq_reload = true;
        self.irq_count = 0;
      }

      // (0xe000..=0xffff, true)
      0xe000 => {
        self.irq_enabled = false;
        mem.irq.remove(bus::IrqFlags::MAPPER);
      }
      // (0xe000..=0xffff, false)
      0xe001 => self.irq_enabled = true, 
      _ => {}
    }
  }

  fn notify_ppu_addr(&mut self, mem: &mut Bus, cycles: usize) {
    let a12_low = mem.ppu_addr_bus & 0x1000 == 0;

    let rising_edge = if !a12_low {
      let res = self.a12_low_count > 0 && cycles - self.a12_low_count >= 3;
      self.a12_low_count = 0;
      res
    } else if self.a12_low_count == 0 {
      self.a12_low_count = cycles;
      false
    } else { false };

    if rising_edge {
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
}

// https://www.nesdev.org/wiki/INES_Mapper_019
// TODO: audio
// TODO: mapper 210 is similiar (but simpler) to this
// struct Namco129_163 {
//   exram: [u8; 128],
//   irq_count: u16,
//   irq_enabled: bool,

//   chr_ram0: bool,
//   chr_ram1: bool,
// }
// impl Mapper for Namco129_163 {
//   fn new(header: &CartHeader, mem: &mut Bus) -> Box<Self> {
//     mem.banks.prg = Banking::new_prg(header, 4);
//     mem.banks.prg.set_page_to_last_bank(3);

//     mem.banks.chr = Banking::new_chr(header, 12);
//     mem.banks.vram = Banking::new(2 * 1024, 4 * 1024, 12);

//     Box::new(Self {
//       exram: [0; 128],
//       irq_count: 0x7fff,
//       irq_enabled: false,

//       chr_ram0: false,
//       chr_ram1: false,
//     })
//   }

//   fn cart_read(&mut self, _mem: &mut Bus, addr: u16) -> u8 {
//     // TODO: use mask
//     println!("READ CART");

//     match addr {
//       0x5000..=0x57ff => self.irq_count as u8,
//       0x5800..=0x5fff => ((self.irq_enabled as u8) << 7) | (self.irq_count >> 8) as u8,
//       _ => 0
//     }
//   }

//   fn cart_write(&mut self, mem: &mut Bus, addr: u16, val: u8) {
//     // TODO: use mask
//     println!("WROTE CART");
//     match addr {
//       0x5000..=0x57ff => {
//         self.irq_count = byte_set_lo(self.irq_count, val);
//         mem.irq.remove(bus::IrqFlags::MAPPER);
//       }
//       0x5800..=0x5fff => {
//         self.irq_count = byte_set_hi(self.irq_count, val & 0x7f);
//         self.irq_enabled = val & 0x7f > 0;
//         mem.irq.remove(bus::IrqFlags::MAPPER);
//       }

//       _ => {}
//     }
//   }

//   fn prg_write(&mut self, mem: &mut Bus, addr: u16, val: u8) {
//     // TODO: use mask
//     match addr {
//       0x8000..=0xdfff => {
//         let page = ((addr - 0x8000) / 0x800) as u8; 

//         if val >= 0xe0 {
//           // use internal nametables
//           let table = (val % 2 == 0) as u8;
//           match addr {
//             0x8000..=0x9fff => {
//               if !self.chr_ram0 {
//                 mem.banks.vram.set_page(page, table);
//                 mem.ppu_handlers_1kb[page as usize] = bus::PpuHandler::VramAsChr;
//               } else {
//                 mem.banks.vram.set_page(page, table);
//                 mem.ppu_handlers_1kb[page as usize] = bus::PpuHandler::VramAsChr;
//               }
//             }
//             0xa000..=0xbfff => {
//               if !self.chr_ram1 {
//                 mem.banks.vram.set_page(page, table);
//                 mem.ppu_handlers_1kb[page as usize] = bus::PpuHandler::VramAsChr;
//               } else {
//                 mem.banks.vram.set_page(page, table);
//                 mem.ppu_handlers_1kb[page as usize] = bus::PpuHandler::VramAsChr;
//               }
//             }
//             0xc000..=0xdfff => {
//               mem.banks.vram.set_page(page, table);
//               mem.ppu_handlers_1kb[page as usize] = bus::PpuHandler::VramAsChr;
//             }
//             _ => {}
//           } 
//         } else {
//           // use chr rom
//           mem.banks.chr.set_page(page, val);
//           mem.ppu_handlers_1kb[page as usize] = bus::PpuHandler::Chr;
//         }
//       }

//       0xe000..=0xe7ff => {
//         mem.banks.prg.set_page(0, val & 0x3f);
//         // TODO: disable sound
//       }
//       0xe800..=0xefff => {
//         mem.banks.prg.set_page(1, val & 0x3f);
//         self.chr_ram0 = val & 0x40 > 0;
//         self.chr_ram1 = val & 0x80 > 0;
//       }
//       0xf000..=0xf7ff => mem.banks.prg.set_page(2, val & 0x3f),
//       0xf800..=0xffff => {
//         // TODO: write protect for exram
//       }
//       _ => {}
//     }
//   }

//   fn step(&mut self, mem: &mut Bus) {
//     if self.irq_enabled && self.irq_count < 0x7fff {
//       self.irq_count += 1;
//       if self.irq_count >= 0x7fff {
//         println!("IRQ CLOCKED");
//         mem.irq.insert(bus::IrqFlags::MAPPER);
//       }
//     }
//   }
// }


// https://www.nesdev.org/wiki/INES_Mapper_184
struct Sunsoft1;
impl Mapper for Sunsoft1 {
  fn new(header: &CartHeader, mem: &mut Bus) -> Box<Self> {
    mem.banks.chr = Banking::new_chr(header, 2);
    mem.set_wram_handlers(CpuHandler::Mapper);
    Box::new(Self)
  }

  fn prg_write(&mut self, _: &mut Bus, _: u16, _: u8) {}

  fn cart_write(&mut self, mem: &mut Bus, _: u16, val: u8) {
    mem.banks.chr.set_page(0, val & 0b111);
    mem.banks.chr.set_page(1, (val >> 4) & 0b111);
  }
}

// https://www.nesdev.org/wiki/INES_Mapper_093
struct Sunsoft93;
impl Mapper for Sunsoft93 {
  fn new(_: &CartHeader, mem: &mut Bus) -> Box<Self> {
    mem.banks.prg.set_page_to_last_bank(1);
    Box::new(Self)
  }

  fn prg_write(&mut self, mem: &mut Bus, _: u16, val: u8) {
    mem.banks.prg.set_page(0, (val >> 4) & 0b111);
    // TODO: ram enable
  }
}

// https://www.nesdev.org/wiki/INES_Mapper_089
struct Sunsoft89;
impl Mapper for Sunsoft89 {
  fn new(_: &CartHeader, mem: &mut Bus) -> Box<Self> {
    mem.banks.prg.set_page_to_last_bank(1);
    Box::new(Self)
  }

  fn prg_write(&mut self, mem: &mut Bus, _: u16, val: u8) {
    mem.banks.prg.set_page(0, (val >> 4) & 0b111);
    mem.banks.chr.set_page(0, ((val & 0x80) >> 4) | (val & 0b111));

    let mirroring = if val & 0x8 == 0 { Mirroring::SingleScreenA } else { Mirroring::SingleScreenB };
    mem.banks.vram.mirror(&mirroring);
  }
}

#[derive(Default)]
struct Sunsoft4 {
  uses_chr_rom: bool,
  mirroring: Mirroring,
  chr_table0: u8,
  chr_table1: u8,
}
impl Sunsoft4 {
  fn update_chr_banks(&mut self, mem: &mut Bus) {
    let vram = &mut mem.banks.vram;
    
    if !self.uses_chr_rom {
      vram.mirror(&self.mirroring);
      return;
    }

    match &self.mirroring {
      Mirroring::Vertical => {
        vram.set_page(0, self.chr_table0);
        vram.set_page(1, self.chr_table1);
        vram.set_page(2, self.chr_table0);
        vram.set_page(3, self.chr_table1);
      },
      Mirroring::Horizontal => {
        vram.set_page(0, self.chr_table0);
        vram.set_page(1, self.chr_table0);
        vram.set_page(2, self.chr_table1);
        vram.set_page(3, self.chr_table1);
      }
      Mirroring::SingleScreenA => for i in 0..4 {
        vram.set_page(i, self.chr_table0);
      }
      Mirroring::SingleScreenB => for i in 0..4 {
        vram.set_page(i, self.chr_table1);
      },
      // shouldn't have 4 screens mirroring
      _ => {}
    }
  }
}
impl Mapper for Sunsoft4 {
  fn new(header: &CartHeader, mem: &mut Bus) -> Box<Self> {
    mem.banks.prg.set_page_to_last_bank(1);
    mem.banks.chr = Banking::new_chr(header, 4);

    Box::new(Self {
      mirroring: header.mirroring.clone(),
      ..Default::default()
    })
  }

  fn prg_write(&mut self, mem: &mut Bus, addr: u16, val: u8) {
    match addr >> 12 {
      0x6 | 0x7 => {
        // TODO: Licensing IC
      }

      0x8 => mem.banks.chr.set_page(0, val),
      0x9 => mem.banks.chr.set_page(1, val),
      0xa => mem.banks.chr.set_page(2, val),
      0xb => mem.banks.chr.set_page(3, val),

      0xc => {
        self.chr_table0 = 0x80 | val;
        if self.uses_chr_rom {
          self.update_chr_banks(mem);
        }
      }

      0xd => {
        self.chr_table1 = 0x80 | val;
        if self.uses_chr_rom {
          self.update_chr_banks(mem);
        }
      }

      0xe => {
        self.mirroring = match val & 0b11 {
          0 => Mirroring::Vertical,
          1 => Mirroring::Horizontal,
          2 => Mirroring::SingleScreenA,
          _ => Mirroring::SingleScreenB,
        };

        let mode = val & 0x10 > 0;
        if mode != self.uses_chr_rom {
          let new_size = if mode { mem.chr.len() } else { 2 * 1024 };
          mem.banks.vram.change_size(new_size);

          let handler = if mode { PpuHandler::ChrInVram } else { PpuHandler::Vram };
          mem.set_vram_handlers(handler);

          self.uses_chr_rom = mode;
          self.update_chr_banks(mem);
        }
      }
      0xf => {
        // TODO: prg ram enable
        // TODO: nantettatte stuff
        mem.banks.prg.set_page(0, val & 0b1111);
      }
      _ => {}
    }
  }
}

#[derive(Default)]
struct SunsoftFME7 {
  uses_wram: bool,
  command: u8,
  irq_enabled: bool,
  irq_count_enabled: bool,
  irq_count: u16,
}
impl Mapper for SunsoftFME7 {
  fn new(header: &CartHeader, mem: &mut Bus) -> Box<Self> {
    mem.banks.prg = Banking::new_prg(header, 4);
    mem.banks.prg.set_page_to_last_bank(3);
    mem.banks.chr = Banking::new_chr(header, 8);

    Box::new(Self {
      uses_wram: true,
      ..Default::default()
    })
  }

  fn prg_write(&mut self, mem: &mut Bus, addr: u16, val: u8) {
    match addr & 0xe000 {
      // 0x8000..=0x9fff
      0x8000 => self.command = val & 0b1111,
      // 0xa000..=0xbfff
      0xa000 => {
        match self.command {
          0..=7 => mem.banks.chr.set_page(self.command, val),
          8 => {
            let mode = val & 0x40 > 0;

            if mode != self.uses_wram {
              let new_size = if mode { mem.wram.len() } else { mem.prg.len() };
              mem.banks.wram.change_size(new_size);

              let handler = if mode { CpuHandler::Wram } else { CpuHandler::PrgInWram };
              mem.set_wram_handlers(handler);

              mem.banks.wram.set_page(0, val & 0x3f);
              self.uses_wram = mode;
            }

            // TODO: ram enable bit
          }
          0x9..=0xb => mem.banks.prg.set_page(self.command - 9, val),
          0xc => {
            let mirroring = match val & 0b11 {
                0 => Mirroring::Vertical,
                1 => Mirroring::Horizontal,
                2 => Mirroring::SingleScreenA,
                _ => Mirroring::SingleScreenB
            };
            mem.banks.vram.mirror(&mirroring);
          }
          0xd => {
            self.irq_enabled = val & 1 > 0;
            self.irq_count_enabled = val & 0x80 > 0;
            mem.irq.remove(bus::IrqFlags::MAPPER);
          }
          0xe => self.irq_count = byte_set_lo(self.irq_count, val),
          0xf => self.irq_count = byte_set_hi(self.irq_count, val),
          _ => {}
        }
      }

      _ => {}
    }
  }

  fn step(&mut self, mem: &mut Bus) {
    if self.irq_count_enabled {
      self.irq_count = self.irq_count.wrapping_sub(1);
      
      if self.irq_count == 0xffff && self.irq_enabled {
        mem.irq.insert(bus::IrqFlags::MAPPER);
      }
    }
  }
}

// https://www.nesdev.org/wiki/INES_Mapper_087
// TODO: mapper 101
struct J87 {
  shift: u8,
}
impl Mapper for J87 {
  fn new(header: &CartHeader, mem: &mut Bus) -> Box<Self> {
    if header.prg_size > 16 * 1024 {
      mem.banks.prg = Banking::new_prg(header, 1);
    }
    mem.set_wram_handlers(CpuHandler::Mapper);
    let shift = if header.mapper == 87 { 1 } else { 0 };
    Box::new(Self {
      shift
    })
  }

  fn cart_write(&mut self, mem: &mut Bus, _: u16, val: u8) {
    let bank = ((val & 0x1) << self.shift) | ((val & 0x2) >> self.shift);
    mem.banks.chr.set_page(0, bank);
  }

  fn prg_write(&mut self, _: &mut Bus, _: u16, _: u8) {}
}

// https://www.nesdev.org/wiki/VRC1
// TODO: 4 screen vram
#[derive(Default)]
struct VRC1 {
  chr_hi0: u8,
  chr_hi1: u8,
}
impl Mapper for VRC1 {
  fn new(header: &CartHeader, mem: &mut Bus) -> Box<Self> {
    mem.banks.prg = Banking::new_prg(header, 4);
    mem.banks.prg.set_page_to_last_bank(3);
    mem.banks.chr = Banking::new_chr(header, 2);

    Box::new(Self::default()) 
  }

  fn prg_write(&mut self, mem: &mut Bus, addr: u16, val: u8) {
    match addr >> 12 {
      0x8 => mem.banks.prg.set_page(0, val),
      0xa => mem.banks.prg.set_page(1, val),
      0xc => mem.banks.prg.set_page(2, val),
      0x9 => {
        let mirroring = match val & 1 {
          0 => Mirroring::Vertical,
          _ => Mirroring::Horizontal,
        };
        mem.banks.vram.mirror(&mirroring);

        self.chr_hi0 = (val >> 1) & 1;
        self.chr_hi1 = (val >> 2) & 1;
      }
      0xe => mem.banks.chr.set_page(0, (self.chr_hi0 << 5) | val),
      0xf => mem.banks.chr.set_page(1, (self.chr_hi1 << 5) | val),
      _ => {}
    }
  }
}

// https://www.nesdev.org/wiki/VRC3
#[derive(Default)]
struct VRC3 {
  irq_count: u16,
  irq_latch: u16,
  irq_enabled: bool,
  irq_enable_on_ack: bool,
  irq_8bit_mode: bool,
}
impl Mapper for VRC3 {
  fn new(_: &CartHeader, mem: &mut Bus) -> Box<Self> {
    mem.banks.prg.set_page_to_last_bank(1);
    Box::new(Self::default())
  }

  fn prg_write(&mut self, mem: &mut Bus, addr: u16, val: u8) {
    match addr >> 12 {
      0x8 => self.irq_latch = (self.irq_latch & !0x000f) | (((val & 0xf) as u16) << 0),
      0x9 => self.irq_latch = (self.irq_latch & !0x00f0) | (((val & 0xf) as u16) << 4),
      0xa => self.irq_latch = (self.irq_latch & !0x0f00) | (((val & 0xf) as u16) << 8),
      0xb => self.irq_latch = (self.irq_latch & !0xf000) | (((val & 0xf) as u16) << 12),
      0xc => {
        self.irq_enable_on_ack = val & 0x1 > 0;
        self.irq_enabled = val & 0x2 > 0;
        if self.irq_enabled {
          self.irq_count = self.irq_latch;
        }

        self.irq_8bit_mode = val & 0x4 > 0;
        mem.irq.remove(bus::IrqFlags::MAPPER);
      }
      0xd => {
        self.irq_enabled = self.irq_enable_on_ack;
        mem.irq.remove(bus::IrqFlags::MAPPER);
      }
      
      0xf => mem.banks.prg.set_page(0, val),
      _ => {}
    }
  }

  fn step(&mut self, mem: &mut Bus) {
    if self.irq_enabled {      
      if self.irq_8bit_mode {
        let next = (self.irq_count & 0xff) + 1;
        if next > 0xff {
          self.irq_count = byte_set_lo(self.irq_count, self.irq_latch as u8);
          mem.irq.insert(bus::IrqFlags::MAPPER);
        }
      } else {
        self.irq_count = self.irq_count.wrapping_add(1);
        if self.irq_count == 0 {
          self.irq_count = self.irq_latch;
          mem.irq.insert(bus::IrqFlags::MAPPER);
        }
      }
    }
  }
}

mod konami {
  use crate::bus;

  // https://www.nesdev.org/wiki/VRC_IRQ
  pub struct Irq {
    prescaler: i16,
    pub count: u8,
    pub latch: u8,
    pub enable_after_ack: bool,
    pub enabled: bool,
    pub mode_scanline: bool,
  }

  impl Default for Irq {
    fn default() -> Self {
      Self {
        prescaler: 341,
        count: 0,
        latch: 0,
        enable_after_ack: false,
        enabled: false,
        mode_scanline: false,
      }
    }
  }

  impl Irq {
    pub fn write_ctrl(&mut self, val: u8, mem: &mut bus::Bus) {
      self.enable_after_ack = val & 0x1 > 0;
      self.enabled = val & 0x2 > 0;
      self.mode_scanline = val & 0x4 == 0;

      if self.enabled {
        self.count = self.latch;
      }
      mem.irq.remove(bus::IrqFlags::MAPPER)
    }

    pub fn write_ack(&mut self, mem: &mut bus::Bus) {
      self.enabled = self.enable_after_ack;
      mem.irq.remove(bus::IrqFlags::MAPPER);
    }

    pub fn step(&mut self, mem: &mut bus::Bus) {
      if !self.enabled { return; } 

      self.prescaler -= 3;
      if !self.mode_scanline || (self.mode_scanline && self.prescaler <= 0) {
        if self.count == 0xff {
          self.count = self.latch;
          mem.irq.insert(bus::IrqFlags::MAPPER);
        } else {
          self.count = self.count.wrapping_add(1);
        }
        self.prescaler += 341;
      }
    }
  }
}

mod vrc6 {
  use crate::{apu, utils::{byte_set_hi, byte_set_lo}};

  #[derive(Default)]
  pub struct Pulse {
    enabled: bool,
    div: apu::DividerCounter,
    volume: u8,
    duty: u8,
    step: u8,
    ignore_duty: bool,
  }

  impl Pulse {
    pub fn write_ctrl(&mut self, val: u8) {
      self.volume = val & 0xf;
      self.duty = (val >> 4) & 0x7;
      self.ignore_duty = val & 0x80 > 0;
    }

    pub fn write_freq_lo(&mut self, val: u8) {
      self.div.period = byte_set_lo(self.div.period, val);
    }

    pub fn write_freq_hi(&mut self, val: u8, shift: u8) {
      self.div.period = byte_set_hi(self.div.period, val & 0xf);
      self.div.period >>= shift;

      self.enabled = val & 0x80 > 0;

      if !self.enabled {
        self.step = 0;
      }
    }

    pub fn step(&mut self) {
      self.div.step(|| {
        self.step = (self.step + 1) % 16;
      });
    }

    pub fn sample(&self) -> u8 {
      if self.enabled && (self.ignore_duty || self.step <= self.duty) {
        self.volume
      } else { 0 }
    }
  }

  #[derive(Default)]
  pub struct Saw {
    enabled: bool,
    rate: u8,
    acc: u8,
    count: u8,
    div: apu::DividerCounter,
  }
  impl Saw {
    pub fn write_ctrl(&mut self, val: u8) {
      self.rate = val & 0x3f;
    }

    pub fn write_freq_lo(&mut self, val: u8) {
      self.div.period = byte_set_lo(self.div.period, val);
    }

    pub fn write_freq_hi(&mut self, val: u8, shift: u8) {
      self.div.period = byte_set_hi(self.div.period, val & 0xf);
      self.div.period >>= shift;

      self.enabled = val & 0x80 > 0;
      if !self.enabled {
        self.acc = 0;
        self.count = 0;
      }
    }

    pub fn step(&mut self) {
      self.div.step(|| {
        self.count = (self.count + 1) % 14;
        
        if self.count == 0 {
          self.acc = 0;
        } else if self.count % 2 == 0 {
          // If A is more than 42 the accumulator will wrap, resulting in distorted sound. 
          self.acc = (self.acc + self.rate) % 42;
        }
      });
    }

    pub fn sample(&self) -> u8 {
      if self.enabled { self.acc >> 3 } else { 0 }
    }
  }
}

// https://www.nesdev.org/wiki/VRC6
#[derive(Default)]
struct VRC6 {
  mapper: u16,
  regs: [u8; 8],
  mode: u8,
  mirroring: u8,
  uses_chr_rom: bool,

  irq: konami::Irq,

  audio_halt: bool,
  audio_freq_shift: u8,

  p0: vrc6::Pulse,
  p1: vrc6::Pulse,
  saw: vrc6::Saw,
}
impl VRC6 {
  fn update_chr_banks(&mut self, mem: &mut Bus) {
    let chr = &mut mem.banks.chr;

    // When bit 5 of $B003 is set, 2 KiB pattern table banks pass PPU A10 through (ignoring the LSB of the register).
    // So, mode 1, 2, 3 2kb banks should be contiguos.

    // When bit 5 of $B003 is clear, CHR/CIRAM A10 will be controlled directly by the register LSB, causing 2 KiB banks to have duplicate 1 KiB halves.
    // Existing Konami games did not use this configuration. 
    // This means 2kb map to the same bank. We are not emulating it.

    match self.mode {
      0 => for i in 0..8 {
        chr.set_page(i as u8, self.regs[i]);
      }

      // each register sets two pages
      1 => for i in 0..4 {
        chr.set_page2x(2 * i as u8, self.regs[i]);
      }

      _ => {
        for i in 0..4 {
          chr.set_page(i as u8, self.regs[i]);
        }
        // only r4 and r5 set two pages each
        chr.set_page2x(4, self.regs[4]);
        chr.set_page2x(6, self.regs[5]);
      }
    }
  }

  fn update_vram_banks(&mut self, mem: &mut Bus) {
    let vram = &mut mem.banks.vram;

    // When bit 5 of $B003 is set, 2 KiB pattern table banks pass PPU A10 through (ignoring the LSB of the register).
    // Nametables apply different rules at the same time: see below. 

    // Only mode 0 was used by Konami's commercial games.
    match self.mode {
      // This mode was not intended for use with ROM nametables ($B003:4 set), because it overrides the LSB of the nametable registers with the signal intended for CIRAM A10. 
      // Because R6 and R7 are already in use to control the pattern banks, this is not very suitable if combined with ROM nametables (Mode 3 is designed for that instead). 
      0 => {
        match self.mirroring {
          // Vertical
          0 => {
            vram.set_page(0, self.regs[6] & !1);
            vram.set_page(1, self.regs[6] | 1);
            vram.set_page(2, self.regs[7] & !1);
            vram.set_page(3, self.regs[7] | 1);
          }
          // Horizontal
          1 => {
            vram.set_page(0, self.regs[6] & !1);
            vram.set_page(1, self.regs[7] & !1);
            vram.set_page(2, self.regs[6] | 1);
            vram.set_page(3, self.regs[7] | 1);
          }
          // SingleScreenA
          2 => {
            vram.set_page(0, self.regs[6] & !1);
            vram.set_page(1, self.regs[6] & !1);
            vram.set_page(2, self.regs[7] & !1);
            vram.set_page(3, self.regs[7] & !1);
          }
          // SingleScreenB
          _ => {
            vram.set_page(0, self.regs[6] | 1);
            vram.set_page(1, self.regs[7] | 1);
            vram.set_page(2, self.regs[6] | 1);
            vram.set_page(3, self.regs[7] | 1);
          }
        }
      }
      _ => todo!("VRC6 modes 1, 2, 3")
    }
  }

  fn update_all_banks(&mut self, mem: &mut Bus) {
    self.update_chr_banks(mem);
    self.update_vram_banks(mem);
  }
}
impl Mapper for VRC6 {
  fn new(header: &CartHeader, mem: &mut Bus) -> Box<Self> {
    mem.banks.prg = Banking::new_prg(header, 4);
    mem.banks.prg.set_page2x(0, 0);
    mem.banks.prg.set_page_to_last_bank(3);

    mem.banks.chr = Banking::new_chr(header, 8);

    Box::new(Self {
      mapper: header.mapper,
      ..Default::default()
    })
  }

  fn prg_write(&mut self, mem: &mut Bus, mut addr: u16, val: u8) {
    if self.mapper == 26 {
      addr = (addr & 0xffc) | ((addr & 0x01) << 1) | ((addr & 0x02) >> 1);
    }

    match addr & 0xf003 {
      // be careful here: value passed here is missing lsb bit, so we have to shift it right
      0x8000..=0x8003 => mem.banks.prg.set_page2x(0, val << 1),
      0xc000..=0xc003 => mem.banks.prg.set_page(2, val),
      
      0xb003 => {
        self.mode = val & 0x3;
        self.mirroring = (val >> 2) & 0x3;

        // The VRC6 supports the use of a larger RAM to provide more nametables.
        // However, the three commercial VRC6 games neither provided extra nametable RAM, nor used ROM nametables.
        self.uses_chr_rom = val & 0x10 > 0;
        
        // The commercial games always left bit 5 set.

        self.update_all_banks(mem);

        // TODO: prg ram enable
      }
      0xd000..=0xd003 => {
        self.regs[addr as usize - 0xd000] = val;
        self.update_all_banks(mem);
      }
      0xe000..=0xe003 => {
        self.regs[addr as usize - 0xe000 + 4] = val;
        self.update_all_banks(mem);
      }

      0xf000 => self.irq.latch = val,
      0xf001 => self.irq.write_ctrl(val, mem),
      0xf002 => self.irq.write_ack(mem),

      0x9003 => {
        self.audio_halt = val & 0x1 > 0;
        let audio_16x = val & 0x2 > 0;
        let audio_256x = val & 0x4 > 0;

        if !self.audio_halt {
          self.audio_freq_shift = 0;
        } else if audio_256x {
          self.audio_freq_shift = 8;
        } else if audio_16x {
          self.audio_freq_shift = 4;
        }
      }

      0x9000 => self.p0.write_ctrl(val),
      0x9001 => self.p0.write_freq_lo(val),
      0x9002 => self.p0.write_freq_hi(val, self.audio_freq_shift),

      0xa000 => self.p1.write_ctrl(val),
      0xa001 => self.p1.write_freq_lo(val),
      0xa002 => self.p1.write_freq_hi(val, self.audio_freq_shift),

      0xb000 => self.saw.write_ctrl(val),
      0xb001 => self.saw.write_freq_lo(val),
      0xb002 => self.saw.write_freq_hi(val, self.audio_freq_shift),
      _ => {}
    }
  }

  fn step(&mut self, mem: &mut Bus) {
    self.irq.step(mem);

    if !self.audio_halt {
      self.p0.step();
      self.p1.step();
      self.saw.step();
    }
  }

  fn sample(&self) -> f32 {
    (self.p0.sample() + self.p1.sample() + self.saw.sample()) as f32
  }
}

struct MMC5 {

}
impl MMC5 {

}
