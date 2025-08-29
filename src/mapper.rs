
use std::ops::Neg;

use crate::{apu, bus::{Banking, Bus, ChrBank, CpuHandler, IrqFlags, PpuHandler}, emu::Mirroring, utils::{byte_set_hi, byte_set_lo}};

// https://www.nesdev.org/wiki/Mapper
pub trait Mapper {
  fn new(mem: &mut Bus) -> Box<Self> where Self: Sized;
  // 0x8000..=0xffff
  // TODO: turn back val to u8... ?
  fn prg_write(&mut self, mem: &mut Bus, addr: u16, val: u16);
  
  // 0x4020..=0x5fff
  fn cart_read(&mut self, mem: &mut Bus, _addr: u16) -> u8 { mem.cpu_data_bus }
  // TODO: consider getting rid of this and use handlers
  fn cart_write(&mut self, _mem: &mut Bus, _addr: u16, _val: u16) {}
  fn step(&mut self, _mem: &mut Bus, _cycles: usize) {}

  fn notify_ppu_addr(&mut self, _mem: &mut Bus, _cycles: usize) {}
  fn notify_cpu_addr(&mut self, _mem: &mut Bus, _addr: u16, _val: Option<u8>) {}

  fn sample(&self) -> f32 { 0.0 }
}

pub fn from_header(mem: &mut Bus) -> Result<Box<dyn Mapper>, String> {
  let mapper: Box<dyn Mapper> = match mem.header.mapper {
    0 => NROM::new(mem),
    1 => MMC1::new(mem),
    2 | 94 | 180 => UxROM::new(mem),
    3 | 185 => CNROM::new(mem),
    4 => MMC3::new(mem),
    5 => MMC5::new(mem),
    7 => AxROM::new(mem),
    9 | 10 => MMC2::new(mem),
    11 => ColorDreams::new(mem),
    13 => CPROM::new(mem),
    16 | 153 | 157 | 159 => BandaiFCG::new(mem),
    19 | 210 => Namco129_163::new(mem),
    20 => FDS::new(mem),
    21 | 22 | 23 | 25 => VRC2_4::new(mem),
    24 | 26 => VRC6::new(mem),
    31 => NSF::new(mem),
    34 | 177 | 241 => NINA00x_BNROM::new(mem),
    // 32 => IremG101::new(mem),
    // 65 => IremH3001::new(mem),
    66 => GxROM::new(mem),
    67 => Sunsoft3::new(mem),
    68 => Sunsoft4::new(mem),
    69 => SunsoftFME7::new(mem),
    70 | 152 => Bandai74::new(mem),
    71 | 232 => Codemasters::new(mem),
    73 => VRC3::new(mem),
    75 => VRC1::new(mem),
    77 => NapoleonSenki::new(mem),
    78 => Irem74HCx::new(mem),
    79 => NINA003_006::new(mem),
    85 => VRC7::new(mem),
    87 | 101 => J87::new(mem),
    89 => Sunsoft89::new(mem),
    93 => Sunsoft93::new(mem),
    97 => IremTAMS1::new(mem),
    184 => Sunsoft1::new(mem),
    206 | 154 | 95 | 88 | 76 => DxROM::new(mem),
    _ => return Err(format!("mapper {} not implemented", mem.header.mapper)),
  };

  Ok(mapper)
}

// https://www.nesdev.org/wiki/NROM
pub struct NROM;
impl Mapper for NROM {
  fn new(mem: &mut Bus) -> Box<Self> {    
    if mem.header.prg_size > 16 * 1024 {
      // if we have 32 kb, no mirroring, we have mirroring by default
      mem.banks.prg = Banking::new_prg(&mem.header, 1);
    }

    Box::new(Self)
  }

  fn prg_write(&mut self, _: &mut Bus, _: u16, _: u16) {}
}

// https://www.nesdev.org/wiki/UxROM
struct UxROM {
  bank: u8,
  shift: u8,
}
impl Mapper for UxROM {
  fn new(mem: &mut Bus) -> Box<Self> {
    let shift = if mem.header.mapper == 94 { 2 } else { 0 };
    let (swapped, fixed) = if mem.header.mapper == 180 { (1, 0) } else { (0, 1) };
    mem.banks.prg.set_page_to_last_bank(fixed);

    Box::new(Self {
      bank: swapped, shift,
    })
  }

  fn prg_write(&mut self, mem: &mut Bus, _: u16, val: u16) {
    mem.banks.prg.set_page(self.bank, val >> self.shift);
  }
}

// https://www.nesdev.org/wiki/CNROM
// https://www.nesdev.org/wiki/CNROM#Mapper_185
// TODO: mapper 185
struct CNROM;
impl Mapper for CNROM {
  fn new(mem: &mut Bus) -> Box<Self> {
    if mem.header.prg_size <= 16 * 1024 {
      mem.banks.prg.set_page_to_last_bank(1);
    } else {
      mem.banks.prg = Banking::new_prg(&mem.header, 1);
    }

    mem.banks.chr = Banking::new_chr(&mem.header, 1);
    // The Namco game Hayauchi Super Igo adds 2 KiB of PRG-RAM, denoted using mapper 3 and the appropriate value in the header's PRG-RAM size field.
    mem.banks.wram = Banking::new(2 * 1024, 2 * 1024, 4);
    Box::new(Self)
  }

  fn prg_write(&mut self, mem: &mut Bus, _: u16, val: u16) {
    mem.banks.chr.set_page(0, val & 0xf);
  }
}


// https://www.nesdev.org/wiki/GxROM
struct GxROM;
impl Mapper for GxROM {
  fn new(mem: &mut Bus) -> Box<Self> {
    mem.banks.prg = Banking::new_prg(&mem.header, 1);

    Box::new(Self)
  }

  fn prg_write(&mut self, mem: &mut Bus, _: u16, val: u16) {
    mem.banks.prg.set_page(0, (val >> 4) & 0b11);
    mem.banks.chr.set_page(0, val & 0b1111);
  }
}

// https://www.nesdev.org/wiki/AxROM
struct AxROM;
impl Mapper for AxROM {
  fn new(mem: &mut Bus) -> Box<Self> where Self: Sized {
    mem.banks.prg = Banking::new_prg(&mem.header, 1);
    Box::new(Self)
  }

  fn prg_write(&mut self, mem: &mut Bus, _: u16, val: u16) {
    mem.banks.prg.set_page(0, val & 0b111);
    
    let mirroring = if val & 0x10 == 0 {
      Mirroring::LowTable
    } else {
      Mirroring::HighTable
    };
    mem.banks.vram.mirror(&mirroring);
  }
}

// https://www.nesdev.org/wiki/Color_Dreams
struct ColorDreams;
impl Mapper for ColorDreams {
  fn new(mem: &mut Bus) -> Box<Self> {
    mem.banks.prg = Banking::new_prg(&mem.header, 1);
    Box::new(Self)
  }

  fn prg_write(&mut self, mem: &mut Bus, _: u16, val: u16) {
    mem.banks.prg.set_page(0, val & 0b11);
    mem.banks.chr.set_page(0, val >> 4);
  }
}

// https://www.nesdev.org/wiki/INES_Mapper_071
// https://www.nesdev.org/wiki/INES_Mapper_232
#[derive(Default)]
struct Codemasters {
  mapper: u16,
  prg_block: u16,
  prg_bank: u16,
}
impl Mapper for Codemasters {
  fn new(mem: &mut Bus) -> Box<Self> {
    mem.banks.prg = Banking::new_prg(&mem.header, 2);
    // this starts at last bank for some reason
    mem.banks.prg.set_page_to_last_bank(1);

    Box::new(Self {
      mapper: mem.header.mapper,
      ..Default::default()
    })
  }

  fn prg_write(&mut self, mem: &mut Bus, addr: u16, val: u16) {
    match (addr & 0xf000, self.mapper) {
      (0x8000..=0xb000, 232) => {
        self.prg_block = (val >> 3) & 0b11;
        self.prg_bank = (self.prg_block << 2) | (self.prg_bank & 0x3);
        mem.banks.prg.set_page(0, self.prg_bank);
        // CAREFUL: last page should be relative to current block
        mem.banks.prg.set_page(1, (self.prg_block << 2) | 0x3);
      }
      // For compatibility without using a submapper, FCEUX begins all games with fixed mirroring, and applies single screen mirroring only once $9000-9FFF is written, ignoring writes to $8000-8FFF.
      (0x9000, _) => if val & 0x10 == 0 {
        mem.banks.vram.mirror(&Mirroring::LowTable);
      } else {
        mem.banks.vram.mirror(&Mirroring::HighTable);
      }
      (0xc000..=0xf000, 71) => mem.banks.prg.set_page(0, val & 0b1111),
      (0xc000..=0xf000, 232) => {
        self.prg_bank = (self.prg_bank & 0xc) | (val & 0b11);
        mem.banks.prg.set_page(0, self.prg_bank);
      }
      _ => {}
    }
  }
}

// https://www.nesdev.org/wiki/CPROM
struct CPROM;
impl Mapper for CPROM {
  fn new(mem: &mut Bus) -> Box<Self> {
    mem.banks.prg = Banking::new_prg(&mem.header, 1);
    mem.banks.chr = Banking::new_chr(&mem.header, 2);
    Box::new(Self)
  }

  fn prg_write(&mut self, mem: &mut Bus, _: u16, val: u16) {
    mem.banks.chr.set_page(1, val & 0b11);
  }
}

// https://www.nesdev.org/wiki/INES_Mapper_031
struct NSF;
impl Mapper for NSF {
  fn new(mem: &mut Bus) -> Box<Self> {
    mem.banks.prg = Banking::new_prg(&mem.header, 8);
    mem.banks.prg.set_page_to_last_bank(7);
    Box::new(Self)
  }

  fn prg_write(&mut self, _: &mut Bus, _: u16, _: u16) {}
  fn cart_write(&mut self, mem: &mut Bus, addr: u16, val: u16) {
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
  fn new(mem: &mut Bus) -> Box<Self> {
    mem.banks.prg.set_page_to_last_bank(1);

    Box::new(Self {
      is_holy_diver: mem.header.submapper == 3 || mem.header.alt_mirroring
    })
  }

  fn prg_write(&mut self, mem: &mut Bus, _: u16, val: u16) {
    mem.banks.prg.set_page(0, val & 0b111);
    mem.banks.chr.set_page(0, val >> 4);

    let mirroring = match (self.is_holy_diver, val & 0x8) {
      (true, 0)  => Mirroring::Horizontal,
      (true, _)  => Mirroring::Vertical,
      (false, 0) => Mirroring::LowTable,
      (false, _) => Mirroring::HighTable
    };
    mem.banks.vram.mirror(&mirroring);
  }
}

// https://www.nesdev.org/wiki/INES_Mapper_016
// https://www.nesdev.org/wiki/INES_Mapper_153
// https://www.nesdev.org/wiki/INES_Mapper_157
// https://www.nesdev.org/wiki/INES_Mapper_159
// TODO: eeprom
#[derive(Default)]
struct BandaiFCG {
  mapper: u16,
  submapper: u8,
  prg_bank: u16,
  irq_enabled: bool,
  irq_latch: u16,
  irq_count: u16,
}
impl BandaiFCG {
  fn write(&mut self, mem: &mut Bus, addr: u16, val: u16) {
    match (addr & 0xf, self.mapper) {
      (0x0..=0x7, 16 | 159) => mem.banks.chr.set_page(addr as u8 & 0xf, val),
      (0x0..=0x3, 153) => {
        let prg_block = val & 1;
        self.prg_bank = (prg_block << 4) | (self.prg_bank & 0x0f);
        mem.banks.prg.set_page(0, self.prg_bank);

        let last_bank = if prg_block == 0 {
          mem.banks.prg.banks_count/2 - 1
        } else {
          mem.banks.prg.banks_count-1
        };
        mem.banks.prg.set_page(1, last_bank);
      }
      (0x0..=0x3, 157) => {
        // TODO: eeprom clock
      }
      (0x8, _) => {
        self.prg_bank = (self.prg_bank & 0xf0) | val;
        mem.banks.prg.set_page(0, self.prg_bank);
      }
      (0x9, _) => {
        let mirroring = match val & 0x3 {
          0 => Mirroring::Vertical,
          1 => Mirroring::Vertical,
          2 => Mirroring::LowTable,
          _ => Mirroring::HighTable,
        };
        mem.banks.vram.mirror(&mirroring);
      }
      (0xa, _) => {
        self.irq_enabled = val & 1 > 0;
        if self.irq_enabled && self.irq_count == 0 {
          mem.irq.insert(IrqFlags::MAPPER);
        } else {
          mem.irq.remove(IrqFlags::MAPPER);
        }

        if self.submapper == 5 {
          self.irq_count = self.irq_latch;
        }
      }
      (0xb, _) => if self.submapper == 4 {
        self.irq_count = byte_set_lo(self.irq_count, val as u8);
      } else if self.submapper == 5 {
        self.irq_latch = byte_set_lo(self.irq_latch, val as u8);
      }
      (0xc, _) => if self.submapper == 4 {
        self.irq_count = byte_set_hi(self.irq_count, val as u8);
      } else if self.submapper == 5 {
        self.irq_latch = byte_set_hi(self.irq_latch, val as u8);
      }
      (0xd, 16 | 159) => if self.submapper == 5 {
        // TODO: eeprom ctrl
      }
      (0xd, 157) => {
        // TODO: eeprom ctrl
      }
      (0xd, 153) => mem.wram_enable(val & 0x20 > 0),
      _ => {}
    }
  }
}
impl Mapper for BandaiFCG {
  fn new(mem: &mut Bus) -> Box<Self> {
    mem.banks.chr = Banking::new_chr(&mem.header, 8);
    
    if mem.header.mapper == 153 {
      // needed for Famicom Jump II
      _ = getrandom::fill(&mut mem.wram);

      // has two prg blocks, last bank should be mid
      mem.banks.prg.set_page(1, mem.banks.prg.banks_count/2-1);
    } else {
      // has eeprom
      mem.set_wram_handlers(CpuHandler::Mapper);

      // no prg blocks
      mem.banks.prg.set_page_to_last_bank(1);
    }

    if matches!(mem.header.mapper, 153 | 157) {
      // chr is unbanked
      for i in 0..8 {
        mem.banks.chr.set_page(i, i as u16);
      }
    }

    let submapper = if mem.header.mapper == 16 {
      mem.header.submapper
    } else {
      // all other work as submapper 5
      5
    };

    Box::new(Self { 
      mapper: mem.header.mapper,
      submapper,
      ..Default::default()
    })
  }

  fn cart_read(&mut self, _mem: &mut Bus, _addr: u16) -> u8 {
    // TODO: eeprom read for 16, 157, 159

    0
  }

  fn cart_write(&mut self, mem: &mut Bus, addr: u16, val: u16) {
    if self.submapper == 4 {
      self.write(mem, addr, val);
    }
  }

  fn prg_write(&mut self, mem: &mut Bus, addr: u16, val: u16) {
    if self.submapper == 5 {
      self.write(mem, addr, val);
    }
  }

  fn step(&mut self, mem: &mut Bus, _cycles: usize) {
    if self.irq_enabled {
      if self.irq_count == 0 {
        if self.submapper == 5 {
          self.irq_count = self.irq_latch;
        }
        mem.irq.insert(IrqFlags::MAPPER);
      }
      self.irq_count -= 1;
    }
  }
}

// https://www.nesdev.org/wiki/INES_Mapper_152
// https://www.nesdev.org/wiki/INES_Mapper_070
// TODO: very similiar to Sunsoft89
struct Bandai74 {
  mapper: u16,
}
impl Mapper for Bandai74 {
  fn new(mem: &mut Bus) -> Box<Self> {
    mem.banks.prg.set_page_to_last_bank(1);
    Box::new(Self {
      mapper: mem.header.mapper,
    })
  }

  fn prg_write(&mut self, mem: &mut Bus, _: u16, val: u16) {
    mem.banks.chr.set_page(0, val & 0xf);
    
    if self.mapper == 152 {
      mem.banks.prg.set_page(0, (val >> 4) & 0b111);
      let mirroring = if val & 0x80 == 0 { Mirroring::LowTable } else { Mirroring::HighTable };
      mem.banks.vram.mirror(&mirroring);
    } else {
      mem.banks.prg.set_page(0, val >> 4);
    }
  }
}

// https://www.nesdev.org/wiki/INES_Mapper_097
struct IremTAMS1;
impl Mapper for IremTAMS1 {
  fn new(mem: &mut Bus) -> Box<Self> {
    mem.banks.prg.set_page_to_last_bank(0);
    Box::new(Self)
  }

  fn prg_write(&mut self, mem: &mut Bus, _: u16, val: u16) {
    mem.banks.prg.set_page(1, val & 0x1f);
    let mirroring = if val & 0x80 == 0 { Mirroring::Horizontal } else { Mirroring::Vertical }; 
    mem.banks.vram.mirror(&mirroring);
  }
}

// TODO
// https://www.nesdev.org/wiki/INES_Mapper_032
// struct IremG101;

// https://www.nesdev.org/wiki/INES_Mapper_065
// struct IremH3001;

mod mmc1 {
  #[derive(Default, Debug)]
  pub enum WramKind {
    Bank32, Bank16, #[default] Bank8
  }
}

// Needs NES2.0 / db support for WRAM (NEW FINDING: only SOROM games have 2 different kind of RAM))
// TODO: prg rom write delay
#[derive(Default, Debug)]
struct MMC1 {
  shift_reg: u16,
  shift_count: u8,

  prg_mode: u8,
  prg_bank: u16,
  prg_hi_bank: u16,
  
  // 512kb of prg
  has_big_prg: bool,
  last_bank: u16,
  wram_kind: mmc1::WramKind,

  chr_mode: bool,
  chr_bank0: u16,
  chr_bank1: u16,
}
impl MMC1 {
  fn update_all_banks(&mut self, mem: &mut Bus, val: u16) {
    if self.has_big_prg {
      self.prg_hi_bank = val & 0x10;

      if self.prg_hi_bank > 0 {
        // last bank is the real last
        self.last_bank = mem.banks.prg.banks_count-1;
      } else {
        // last bank is the mid one
        self.last_bank = mem.banks.prg.banks_count/2-1;
      }
    }

    use mmc1::WramKind;
    let wram = &mut mem.banks.wram;
    match self.wram_kind {
      WramKind::Bank16 => wram.set_page(0, (val >> 3) & 0x1),
      WramKind::Bank32 => wram.set_page(0, (val >> 2) & 0x3),
      _ => {}
    }

    self.update_prg_banks(mem);
    self.update_chr_banks(mem);
  }

  fn update_prg_banks(&mut self, mem: &mut Bus) {
    let bank = self.prg_hi_bank | self.prg_bank;
    match self.prg_mode {
      2 => {
        // 2: fix first bank at $8000 and switch 16 KB bank at $C000 
        mem.banks.prg.set_page(0, 0);
        mem.banks.prg.set_page(1, bank);
      }
      3 => {
        // 3: fix last bank at $C000 and switch 16 KB bank at $8000)
        mem.banks.prg.set_page(0, bank);
        // CAREFUL HERE: if we have 512kb, this has still the be the last 256kb bank of the current block 
        mem.banks.prg.set_page(1, self.last_bank);
      }
      _ => {
        // 0, 1: switch 32 KB at $8000, ignoring low bit of bank number;
        mem.banks.prg.set_pages_aligned2(0, bank);
      }
    }
  }

  fn update_chr_banks(&mut self, mem: &mut Bus) {
    if self.chr_mode {
      mem.banks.chr.set_page(0, self.chr_bank0);
      mem.banks.chr.set_page(1, self.chr_bank1);
    } else {
      mem.banks.chr.set_pages_aligned2(0, self.chr_bank0 << 0);
    }
  }
}
impl Mapper for MMC1 {
  fn new(mem: &mut Bus) -> Box<Self> {
    mem.banks.chr = Banking::new_chr(&mem.header, 2);

    let has_big_prg = mem.header.prg_size >= 512 * 1024;
    let last_bank = if has_big_prg {
      // start with mid bank
      mem.banks.prg.banks_count/2-1
    } else {
      // will always be real last
      mem.banks.prg.banks_count-1
    };

    let wram_kind = if mem.header.wram_size >= 32 * 1024 {
      mmc1::WramKind::Bank32
    } else if mem.header.wram_size >= 16 * 1024 {
      mmc1::WramKind::Bank16
    } else {
      mmc1::WramKind::Bank8
    };

    let mut res = Self {
      has_big_prg,
      wram_kind,
      last_bank,
      prg_mode: 3,
      ..Default::default()
    };

    res.update_prg_banks(mem);
    res.update_chr_banks(mem);

    Box::new(res)
  }

  fn prg_write(&mut self, mem: &mut Bus, addr: u16, val: u16) {
    if val & 0x80 != 0 {
      self.shift_reg = 0;
      self.shift_count = 0;

      // back to mode3
      self.prg_mode = 3;
      self.update_prg_banks(mem);
      
      return;
    }

    self.shift_reg |= (val & 1) << self.shift_count;
    self.shift_count += 1;

    if self.shift_count < 5 { return; }

    let val = self.shift_reg;
    self.shift_reg = 0;
    self.shift_count = 0;

    match addr & 0xe000 {
      // 0x8000..=0x9fff => {
      0x8000 => {
        let mirroring = match val & 0x3 {
          0 => Mirroring::LowTable,
          1 => Mirroring::HighTable,
          2 => Mirroring::Vertical,
          _ => Mirroring::Horizontal
        };
        mem.banks.vram.mirror(&mirroring);
        
        self.prg_mode = (val as u8 >> 2) & 0x3;
        self.update_prg_banks(mem);

        self.chr_mode = val & 0x10 > 0;
        self.update_chr_banks(mem);
      }
      0xa000..=0xbfff => {
      // 0xa000 => {
        self.chr_bank0 = val;
        self.update_all_banks(mem, val);
      }
      // 0xc000..=0xdfff => {
      0xc000 => {
        self.chr_bank1 = val;
        if self.chr_mode {
          self.update_all_banks(mem, val);
        }
      }
      // 0xe000..=0xffff => {
      0xe000 => {
        self.prg_bank = val & 0xf;
        self.update_prg_banks(mem);

        mem.wram_enable(val & 0x10 == 0);
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

// https://www.nesdev.org/wiki/MMC2
// https://www.nesdev.org/wiki/MMC4
struct MMC2 {
  // TODO: do we really need a banking object here? probably just four registers
  // we can do that (tested) but we'd like to precompute the set_page() on prg_write
  bank_fd: Banking<ChrBank>,
  bank_fe: Banking<ChrBank>,
  latch0: mmc2::Latch,
  latch1: mmc2::Latch,
  mapper: u16,
}

impl Mapper for MMC2 {
  fn new(mem: &mut Bus) -> Box<Self> where Self: Sized {
    if mem.header.mapper == 9 {
      // MMC2
      mem.banks.prg = Banking::new_prg(&mem.header, 4);
      let last_bank = mem.banks.prg.banks_count - 1;
      mem.banks.prg.set_page(1, last_bank-2);
      mem.banks.prg.set_page(2, last_bank-1);
      mem.banks.prg.set_page(3, last_bank);
    } else if mem.header.mapper == 10 {
      // MMC4
      // only two 16 kb pages
      mem.banks.prg.set_page_to_last_bank(1);
    }

    mem.banks.chr = Banking::new_chr(&mem.header, 2);

    Box::new(Self {
      bank_fd: Banking::new_chr(&mem.header, 2),
      bank_fe: Banking::new_chr(&mem.header, 2),
      latch0: mmc2::Latch::FD,
      latch1: mmc2::Latch::FD,
      mapper: mem.header.mapper,
    })
  }

  fn prg_write(&mut self, mem: &mut Bus, addr: u16, val: u16) {
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

    match (mem.ppu_addr_bus, self.mapper) {
      (0x0fd8, 9) | (0xfd8..=0xfdf, 10) => self.latch0 = Latch::FD,
      (0x0fe8, 9) | (0xfe8..=0xfef, 10) => self.latch0 = Latch::FE,
      (0x1fd8..=0x1fdf, _) => self.latch1 = Latch::FD, 
      (0x1fe8..=0x1fef, _) => self.latch1 = Latch::FE,
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


// https://www.nesdev.org/wiki/MMC3
// https://www.nesdev.org/wiki/MMC6
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

  is_mmc6: bool,
}
// https://forums.nesdev.org/viewtopic.php?t=14056
impl Mapper for MMC3 {
  fn new(mem: &mut Bus) -> Box<Self> {
    if mem.header.alt_mirroring || mem.header.mirroring == Mirroring::FourScreens {
      // MMC3 can have 4 screen mirroring
      mem.set_4screen_mirroring();
    }

    mem.banks.prg = Banking::new_prg(&mem.header, 4);
    // start with prg mode0
    mem.banks.prg.set_page(2, mem.banks.prg.banks_count - 2);
    mem.banks.prg.set_page_to_last_bank(3);

    mem.banks.chr = Banking::new_chr(&mem.header, 8);
    mem.banks.chr.set_pages_aligned2(0, 0);
    mem.banks.chr.set_pages_aligned2(2, 0);

    let is_mmc6 = mem.header.submapper == 1;

    if is_mmc6 {
      mem.banks.wram = Banking::new_wram(&mem.header, 8);
    }

    Box::new(Self {
      is_mmc6,
      ..Default::default()
    })
  }

  fn prg_write(&mut self, mem: &mut Bus, addr: u16, val: u16) {
    match addr & 0xe001 {
      // (0x8000..=0x9fff, true)
      0x8000 => {
        self.bank_select = val as u8 & 0x7;
        
        let chr_invert = val & 0x80 > 0;
        if self.chr_invert != chr_invert {
          for i in 0..4 {
            mem.banks.chr.swap_pages(i, i+4);
          }

          self.chr_invert = chr_invert;
        }

        let prg_mode = val as u8 & 0x40;
        if self.prg_mode != prg_mode {
          mem.banks.prg.swap_pages(0, 2);

          self.prg_swapped = if prg_mode == 0 { 0 } else { 2 };
          self.prg_mode = prg_mode;
        }

        if self.is_mmc6 { mem.wram_enable(val & 0x20 > 0); }
      }

      // (0x8000..=0x9fff, false)
      0x8001 => {
        match (self.bank_select, self.chr_invert) {
          (6, _) => mem.banks.prg.set_page(self.prg_swapped, val & 0x3f),
          (7, _) => mem.banks.prg.set_page(1, val & 0x3f),
          (0 | 1, false) => mem.banks.chr.set_pages_aligned2(self.bank_select * 2, val),
          (0 | 1, true)  => mem.banks.chr.set_pages_aligned2(self.bank_select * 2 + 4, val),
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
        let mode = val >> 6;
        let handler = match mode {
          // 0b10, enabled, allow writes
          2 => CpuHandler::WramRW,
          // 0b11, enabled, deny writes
          3 => CpuHandler::WramReadOnly,
          _ => CpuHandler::Mapper,
        };

        if self.is_mmc6 {
          // TODO: sets writing protection for 512 byte blocks of wram!! we can't do that...
        } else {
          mem.set_wram_handlers(handler);
        }
      }

      // (0xc000..=0xdfff, true)
      0xc000 => self.irq_latch = val as u8,
      
      // (0xc000..=0xdfff, false)
      0xc001 => {
        self.irq_reload = true;
        self.irq_count = 0;
      }

      // (0xe000..=0xffff, true)
      0xe000 => {
        self.irq_enabled = false;
        mem.irq.remove(IrqFlags::MAPPER);
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
        mem.irq.insert(IrqFlags::MAPPER);
      }
    }
  }
}

// https://www.nesdev.org/wiki/INES_Mapper_019
// https://www.nesdev.org/wiki/INES_Mapper_210
// TODO: audio
// TODO: exram save data
struct Namco129_163 {
  // exram: [u8; 128],
  irq_count: u16,
  irq_enabled: bool,

  chr_ram0: bool,
  chr_ram1: bool,

  mapper: u16,
  submapper: u8,
}
impl Mapper for Namco129_163 {
  fn new(mem: &mut Bus) -> Box<Self> {
    mem.banks.prg = Banking::new_prg(&mem.header, 4);
    mem.banks.prg.set_page_to_last_bank(3);
    mem.banks.wram = Banking::new_wram(&mem.header, 4);

    if mem.header.mapper == 19 {
      // namco 129/163
      mem.banks.chr = Banking::new(mem.header.chr_size, 12 * 1024, 12);
      mem.banks.vram = Banking::new(2 * 1024, 12 * 1024, 12);
      mem.set_vram_handlers(PpuHandler::VramInChr);  
    } else if mem.header.mapper == 210 {
      // namco 175/340
      mem.banks.chr = Banking::new_chr(&mem.header, 8);
    }
  

    Box::new(Self {
      // exram: [0; 128],
      irq_count: 0,
      irq_enabled: false,

      chr_ram0: false,
      chr_ram1: false,

      mapper: mem.header.mapper,
      submapper: mem.header.submapper,
    })
  }

  fn cart_read(&mut self, mem: &mut Bus, addr: u16) -> u8 {
    // TODO: use mask
    if self.mapper != 19 { return 0 }

    match addr {
      0x5000..=0x57ff => self.irq_count as u8,
      0x5800..=0x5fff => ((self.irq_enabled as u8) << 7) | (self.irq_count >> 8) as u8,
      _ => mem.cpu_data_bus
    }
  }

  fn cart_write(&mut self, mem: &mut Bus, addr: u16, val: u16) {
    if self.mapper != 19 { return; }

    let val = val as u8;
    // TODO: use mask
    match addr {
      0x5000..=0x57ff => {
        self.irq_count = byte_set_lo(self.irq_count, val);
        mem.irq.remove(IrqFlags::MAPPER);
      }
      0x5800..=0x5fff => {
        self.irq_count = byte_set_hi(self.irq_count, val & 0x7f);
        self.irq_enabled = val & 0x7f > 0;
        mem.irq.remove(IrqFlags::MAPPER);
      }

      _ => {}
    }
  }

  fn prg_write(&mut self, mem: &mut Bus, addr: u16, val: u16) {
    // TODO: use mask
    match (addr, self.mapper) {
      (0x8000..=0xdfff, 19) => {
        let page = ((addr - 0x8000) / 0x800) as u8;

        let nametbl_enabled = (page >= 8) || (page < 4 && !self.chr_ram0) || (page >= 4 && !self.chr_ram1);

        if val >= 0xe0 && nametbl_enabled {
          // use nametables
          mem.banks.vram.set_page(page, val & 1);
          mem.ppu_handlers_1kb[page as usize] = PpuHandler::VramInChr;
        } else {
          // use chr
          mem.banks.chr.set_page(page, val);
          // All commercial-era titles only come with CHR-ROM.
          mem.ppu_handlers_1kb[page as usize] = PpuHandler::ChrRom;
        }
      }
      
      (0x8000..=0xbfff, 210) => {
        let page = ((addr - 0x8000) / 0x800) as u8;
        mem.banks.chr.set_page(page, val);
      }

      (0xc000..=0xc7ff, 210) => {
        // namco 175 only
        if self.submapper == 1 {
          mem.wram_enable(val & 1 > 0);
        }
      }

      (0xe000..=0xe7ff, _) => {
        mem.banks.prg.set_page(0, val & 0x3f);
        // TODO: disable sound

        // namco 340 only
        if self.mapper == 210 && self.submapper == 2 {
          let mirroring = match val & 0xc0 {
            0 => Mirroring::LowTable,
            1 => Mirroring::Vertical,
            2 => Mirroring::HighTable,
            _ => Mirroring::Horizontal
          };
          mem.banks.vram.mirror(&mirroring);
        }
      }

      (0xe800..=0xefff, _) => {
        mem.banks.prg.set_page(1, val & 0x3f);
        self.chr_ram0 = val & 0x40 > 0;
        self.chr_ram1 = val & 0x80 > 0;
      }
      (0xf000..=0xf7ff, _) => {
        mem.banks.prg.set_page(2, val & 0x3f);
      }
      (0xf800..=0xffff, 19) => {
        // TODO: write protect for exram for mapper 19
        // TODO: this works with 2kb windows, we cant really do it with 8kb handlers...
      }
      _ => {}
    }
  }

  fn step(&mut self, mem: &mut Bus, _: usize) {
    if self.mapper == 19 {
      if self.irq_enabled && self.irq_count < 0x7fff {
        self.irq_count += 1;
        if self.irq_count >= 0x7fff {
          mem.irq.insert(IrqFlags::MAPPER);
        }
      }
    }
  }
}


// https://www.nesdev.org/wiki/INES_Mapper_184
struct Sunsoft1;
impl Mapper for Sunsoft1 {
  fn new(mem: &mut Bus) -> Box<Self> {
    mem.banks.chr = Banking::new_chr(&mem.header, 2);
    mem.set_wram_handlers(CpuHandler::Mapper);
    Box::new(Self)
  }

  fn prg_write(&mut self, _: &mut Bus, _: u16, _: u16) {}

  fn cart_write(&mut self, mem: &mut Bus, _: u16, val: u16) {
    mem.banks.chr.set_page(0, val & 0b111);
    mem.banks.chr.set_page(1, (val >> 4) & 0b111);
  }
}

// https://www.nesdev.org/wiki/INES_Mapper_093
struct Sunsoft93;
impl Mapper for Sunsoft93 {
  fn new(mem: &mut Bus) -> Box<Self> {
    mem.banks.prg.set_page_to_last_bank(1);
    Box::new(Self)
  }

  fn prg_write(&mut self, mem: &mut Bus, _: u16, val: u16) {
    mem.banks.prg.set_page(0, (val >> 4) & 0b111);
  }
}

// https://www.nesdev.org/wiki/INES_Mapper_089
struct Sunsoft89;
impl Mapper for Sunsoft89 {
  fn new(mem: &mut Bus) -> Box<Self> {
    mem.banks.prg.set_page_to_last_bank(1);
    Box::new(Self)
  }

  fn prg_write(&mut self, mem: &mut Bus, _: u16, val: u16) {
    mem.banks.prg.set_page(0, (val >> 4) & 0b111);
    mem.banks.chr.set_page(0, ((val & 0x80) >> 4) | (val & 0b111));

    let mirroring = if val & 0x8 == 0 { Mirroring::LowTable } else { Mirroring::HighTable };
    mem.banks.vram.mirror(&mirroring);
  }
}

// https://www.nesdev.org/wiki/INES_Mapper_067
#[derive(Default)]
struct Sunsoft3 {
  irq_write: bool,
  irq_count: u16,
  irq_enabled: bool,
}
impl Mapper for Sunsoft3 {
  fn new(mem: &mut Bus) -> Box<Self> where Self: Sized {
    mem.banks.prg.set_page_to_last_bank(1);
    mem.banks.chr = Banking::new_chr(&mem.header, 4);

    Box::new(Self::default())
  }

  fn prg_write(&mut self, mem: &mut Bus, addr: u16, val: u16) {
    if addr & 0x8800 == 0x8000 {
      mem.irq.remove(IrqFlags::MAPPER);
    }

    match addr & 0xf800 {
      0x8800 => mem.banks.chr.set_page(0, val),
      0x9800 => mem.banks.chr.set_page(1, val),
      0xa800 => mem.banks.chr.set_page(2, val),
      0xb800 => mem.banks.chr.set_page(3, val),
      
      0xc800 => {
        self.irq_count = if !self.irq_write {
          byte_set_hi(self.irq_count, val as u8)
        } else {
          byte_set_lo(self.irq_count, val as u8)
        };
        self.irq_write = !self.irq_write;
      }

      0xd800 => {
        self.irq_enabled = val & 0x10 > 0;
        self.irq_write = false;
      }
      0xe800 => {
        let mirroring = match val & 0x3 {
          0 => Mirroring::Vertical,
          1 => Mirroring::Horizontal,
          2 => Mirroring::LowTable,
          _ => Mirroring::HighTable,
        };
        mem.banks.vram.mirror(&mirroring);
      }

      0xf800 => mem.banks.prg.set_page(0, val & 0xf),
      _ => {}
    }
  }

  fn step(&mut self, mem: &mut Bus, _cycles: usize) {
    if self.irq_enabled {
      if self.irq_count == 0 {
        mem.irq.insert(IrqFlags::MAPPER);
        self.irq_enabled = false;
      } else {
        self.irq_count -= 1;
      }
    }
  }
}

// https://www.nesdev.org/wiki/INES_Mapper_068
#[derive(Default)]
struct Sunsoft4 {
  uses_chr_rom: bool,
  mirroring: Mirroring,
  chr_table0: u16,
  chr_table1: u16,
}
impl Sunsoft4 {
  fn update_chr_banks(&mut self, mem: &mut Bus) {
    if !self.uses_chr_rom {
      mem.banks.vram.mirror(&self.mirroring);
      return;
    }
    
    let chr = &mut mem.banks.chr;
    match &self.mirroring {
      Mirroring::Vertical => {
        chr.set_page(8 + 0, self.chr_table0);
        chr.set_page(8 + 1, self.chr_table1);
        chr.set_page(8 + 2, self.chr_table0);
        chr.set_page(8 + 3, self.chr_table1);
      },
      Mirroring::Horizontal => {
        chr.set_page(8 + 0, self.chr_table0);
        chr.set_page(8 + 1, self.chr_table0);
        chr.set_page(8 + 2, self.chr_table1);
        chr.set_page(8 + 3, self.chr_table1);
      }
      Mirroring::LowTable => for i in 8..12 {
        chr.set_page(i, self.chr_table0);
      }
      Mirroring::HighTable => for i in 8..12 {
        chr.set_page(i, self.chr_table1);
      },
      // shouldn't have 4 screens mirroring
      _ => {}
    }
  }
}
impl Mapper for Sunsoft4 {
  fn new(mem: &mut Bus) -> Box<Self> {
    mem.banks.prg.set_page_to_last_bank(1);
    mem.banks.chr = Banking::new(mem.header.chr_size, 12 * 1024, 12);
    mem.banks.chr.set_pages_aligned2(0, 0);
    mem.banks.chr.set_pages_aligned2(2, 2);
    mem.banks.chr.set_pages_aligned2(4, 4);
    mem.banks.chr.set_pages_aligned2(6, 6);

    Box::new(Self {
      mirroring: mem.header.mirroring.clone(),
      ..Default::default()
    })
  }

  fn cart_write(&mut self, _mem: &mut Bus, addr: u16, _val: u16) {
    // TODO: licensing IC
    match addr >> 12 {
      0x6 | 0x7 => {
        // TODO: Licensing IC Nantettatte Baseball
      }

      _ => {}
    }
  }

  fn prg_write(&mut self, mem: &mut Bus, addr: u16, val: u16) {
    match addr >> 12 {
      // mapper expects 2kb banks number, but we have 1kb bank slots, we need to shift
      0x8 => mem.banks.chr.set_pages_aligned2(0, val << 1),
      0x9 => mem.banks.chr.set_pages_aligned2(2, val << 1),
      0xa => mem.banks.chr.set_pages_aligned2(4, val << 1),
      0xb => mem.banks.chr.set_pages_aligned2(6, val << 1),

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
          2 => Mirroring::LowTable,
          _ => Mirroring::HighTable,
        };

        let mode = val & 0x10 > 0;
        if mode != self.uses_chr_rom {
          let handler = if mode { PpuHandler::ChrRom } else { PpuHandler::Vram };
          mem.set_vram_handlers(handler);
          self.uses_chr_rom = mode;
        }

        self.update_chr_banks(mem);
      }
      0xf => {
        mem.banks.prg.set_page(0, val & 0b1111);
        mem.wram_enable(val & 0x10 > 0);
      }
      _ => {}
    }
  }
}


mod sunsoft_fme7 {
  use crate::apu::{self, DividerCounter};

  // https://www.nesdev.org/wiki/Sunsoft_5B_audio
  // TODO: incomplete
  pub struct Tone {
    pub enabled: bool,
    div: apu::DividerCounter,
    pub volume: u8,
    pub period: u16,
    step: u16,
    tone_high: bool,
  }
  impl Default for Tone {
    fn default() -> Self {
      let mut res = Self {
        div: DividerCounter::default(),
        enabled: false,
        volume: 0,
        period: 0,
        step: 0,
        tone_high: false,
      };

      res.div.period = 15;
      res
    }
  }
  impl Tone {
    pub fn step(&mut self) {
      // Unlike the 2A03 and VRC6 pulse channels' frequency formulas, the formula for 5B does not add 1 to the period.
      // A period value of 0 appears to produce the same result as a period value of 1, for tone[1], noise and envelope[2]. 
      
      // Correct behaviour can be implemented as a counter that counts up on every 16th clock cycle until it is equal to or greater than the period register,
      // at which point the output flips and the counter resets to 0. 
      self.div.step(|| {
        self.step += 1;
        if self.step >= self.period {
          self.step = 0;
          self.tone_high = !self.tone_high;
        }
      });
    }

    pub fn sample(&self) -> u8 {
      if self.enabled && self.tone_high { self.volume } else { 0 }
    }
  }
}

// https://www.nesdev.org/wiki/Sunsoft_FME-7
#[derive(Default)]
struct SunsoftFME7 {
  uses_wram: bool,
  cpu_command: u8,

  irq_enabled: bool,
  irq_count_enabled: bool,
  irq_count: u16,

  audio_command: u8,
  audio_enabled: bool,

  // TODO: put these fuckers in an array
  ta: sunsoft_fme7::Tone,
  tb: sunsoft_fme7::Tone,
  tc: sunsoft_fme7::Tone,
}
impl Mapper for SunsoftFME7 {
  fn new(mem: &mut Bus) -> Box<Self> {
    mem.banks.prg = Banking::new_prg(&mem.header, 4);
    mem.banks.prg.set_page_to_last_bank(3);
    mem.banks.chr = Banking::new_chr(&mem.header, 8);

    Box::new(Self {
      uses_wram: true,
      ..Default::default()
    })
  }

  fn prg_write(&mut self, mem: &mut Bus, addr: u16, val: u16) {
    match addr & 0xe000 {
      // 0x8000..=0x9fff
      0x8000 => self.cpu_command = val as u8 & 0b1111,
      // 0xa000..=0xbfff
      0xa000 => match self.cpu_command {
        0..=7 => mem.banks.chr.set_page(self.cpu_command, val),
        8 => {
          let mode = val & 0x40 > 0;

          if mode != self.uses_wram {
            let handler = if mode { CpuHandler::WramRW } else { CpuHandler::PrgInWram };
            mem.set_wram_handlers(handler);

            mem.banks.wram.set_page(0, val & 0x3f);
            self.uses_wram = mode;
          }

          mem.wram_enable(val & 0x80 > 0);
        }
        0x9..=0xb => mem.banks.prg.set_page(self.cpu_command - 9, val),
        0xc => {
          let mirroring = match val & 0b11 {
              0 => Mirroring::Vertical,
              1 => Mirroring::Horizontal,
              2 => Mirroring::LowTable,
              _ => Mirroring::HighTable
          };
          mem.banks.vram.mirror(&mirroring);
        }
        0xd => {
          self.irq_enabled = val & 1 > 0;
          self.irq_count_enabled = val & 0x80 > 0;
          mem.irq.remove(IrqFlags::MAPPER);
        }
        0xe => self.irq_count = byte_set_lo(self.irq_count, val as u8),
        0xf => self.irq_count = byte_set_hi(self.irq_count, val as u8),
        _ => {}
      }

      // 0xc000..=0xdfff
      0xc000 => {
        self.audio_command = val as u8 & 0x0f;
        self.audio_enabled = val & 0xf0 == 0;
      }
      // TODO: audio partially implemented, not working
      // 0xe000..=0xffff
      0xe000 => {
        if !self.audio_enabled { return; }

        match self.audio_command {
          0x0 => self.ta.period = byte_set_lo(self.ta.period, val as u8),
          0x1 => self.ta.period = byte_set_hi(self.ta.period, val as u8 & 0xf),

          0x2 => self.tb.period = byte_set_lo(self.ta.period, val as u8),
          0x3 => self.tb.period = byte_set_hi(self.ta.period, val as u8 & 0xf),

          0x4 => self.tc.period = byte_set_lo(self.ta.period, val as u8),
          0x5 => self.tc.period = byte_set_hi(self.ta.period, val as u8 & 0xf),

          0x6 => {
            // Noise period
          }
          0x7 => {
            self.ta.enabled = val & 0x1 > 0;
            self.tb.enabled = val & 0x2 > 0;
            self.tc.enabled = val & 0x4 > 0;
          }

          0x8 => {
            self.ta.volume = val as u8 & 0xf;
          }
          0x9 => {
            self.tb.volume = val as u8 & 0xf;
          }
          0xa => {
            self.tc.volume = val as u8 & 0xf;
          }

          // This audio hardware was only used in one game, Gimmick!
          // Because this game did not use many features of the chip (e.g. noise, envelope), its features are often only partially implemented by emulators. 
          _ => {}
        }
      }
      _ => {}
    }
  }

  fn step(&mut self, mem: &mut Bus, _cycles: usize) {
    if self.irq_count_enabled {
      self.irq_count = self.irq_count.wrapping_sub(1);
      
      if self.irq_count == 0xffff && self.irq_enabled {
        mem.irq.insert(IrqFlags::MAPPER);
      }
    }

    self.ta.step();
    self.tb.step();
    self.tc.step();
  }

  fn sample(&self) -> f32 {
    (self.ta.sample() + self.tb.sample() + self.tc.sample()) as f32
  }
}

// https://www.nesdev.org/wiki/INES_Mapper_087
// https://www.nesdev.org/wiki/INES_Mapper_101
struct J87 {
  shift: u8,
}
impl Mapper for J87 {
  fn new(mem: &mut Bus) -> Box<Self> {
    if mem.header.prg_size > 16 * 1024 {
      mem.banks.prg = Banking::new_prg(&mem.header, 1);
    }
    mem.set_wram_handlers(CpuHandler::Mapper);
    let shift = if mem.header.mapper == 87 { 1 } else { 0 };
    Box::new(Self {
      shift
    })
  }

  fn cart_write(&mut self, mem: &mut Bus, _: u16, val: u16) {
    let bank = ((val & 0x1) << self.shift) | ((val & 0x2) >> self.shift);
    mem.banks.chr.set_page(0, bank);
  }

  fn prg_write(&mut self, _: &mut Bus, _: u16, _: u16) {}
}

// https://www.nesdev.org/wiki/INES_Mapper_034
// https://www.nesdev.org/wiki/INES_Mapper_177
// https://www.nesdev.org/wiki/INES_Mapper_241
#[allow(non_camel_case_types)]
struct NINA00x_BNROM {
  mapper: u16,
  submapper: u8,
}
impl Mapper for NINA00x_BNROM {
  fn new(mem: &mut Bus) -> Box<Self> where Self: Sized {
    // should be considered BNROM when the CHR-ROM size is 0-8 KiB, and NINA-001/NINA-002 when the CHR-ROM size is above 8 KiB. 
    if mem.header.submapper == 1 || mem.header.chr_size > 8 * 1024 {
      mem.banks.chr = Banking::new_chr(&mem.header, 2);
    } else if mem.header.submapper == 2 || mem.header.chr_size <= 8 * 1024  {
      mem.banks.chr = Banking::new_chr(&mem.header, 1);
    }
    mem.banks.prg = Banking::new_prg(&mem.header, 1);
    
    let submapper = if mem.header.mapper == 34 { mem.header.submapper } else { 2 };

    Box::new(Self {
      mapper: mem.header.mapper,
      submapper,
    })
  }

  fn prg_write(&mut self, mem: &mut Bus, addr: u16, val: u16) {
    match (addr, self.submapper) {
      (0x7ffd, 1) | (0x8000..=0xffff, 2) => {
        mem.banks.prg.set_page(0, val);
        if self.mapper == 177 {
          if val & 0x20 > 0 {
            mem.banks.vram.mirror(&Mirroring::Vertical);
          } else {
            mem.banks.vram.mirror(&Mirroring::Horizontal);
          }
        }
      }
      (0x7ffe, 1) => mem.banks.chr.set_page(0, val),
      (0x7fff, 1) => mem.banks.chr.set_page(1, val),
      _ => {}
    }
  }
}

// https://www.nesdev.org/wiki/INES_Mapper_034
struct NINA003_006;
impl Mapper for NINA003_006 {
  fn new(mem: &mut Bus) -> Box<Self> where Self: Sized {
    mem.banks.prg = Banking::new_prg(&mem.header, 1);
    Box::new(Self)
  }

  fn prg_write(&mut self, mem: &mut Bus, addr: u16, val: u16) {
    if addr & 0xe100 == 0x4100 {
      mem.banks.prg.set_page(0, (val >> 3) & 1);
      mem.banks.chr.set_page(0, val & 0x7);
    }
  }
}

// https://www.nesdev.org/wiki/INES_Mapper_206
// https://www.nesdev.org/wiki/INES_Mapper_088
// https://www.nesdev.org/wiki/INES_Mapper_095
// https://www.nesdev.org/wiki/INES_Mapper_154
// https://www.nesdev.org/wiki/INES_Mapper_076
struct DxROM {
  select: u8,
  mapper: u16,
}
impl Mapper for DxROM {
  fn new(mem: &mut Bus) -> Box<Self> {
    if mem.header.alt_mirroring || mem.header.mirroring == Mirroring::FourScreens {
      mem.set_4screen_mirroring();
    }

    // same as MMC3
    mem.banks.prg = Banking::new_prg(&mem.header, 4);
    mem.banks.prg.set_page(2, mem.banks.prg.banks_count - 2);
    mem.banks.prg.set_page_to_last_bank(3);

    mem.banks.chr = Banking::new_chr(&mem.header, 8);
    mem.banks.chr.set_pages_aligned2(0, 0);
    mem.banks.chr.set_pages_aligned2(2, 0);

    if mem.header.mapper == 76 {
      mem.banks.chr = Banking::new_chr(&mem.header, 4);
    }

    Box::new(Self {
      select: 0,
      mapper: mem.header.mapper,
    })
  }

  fn prg_write(&mut self, mem: &mut Bus, addr: u16, val: u16) {
    if self.mapper == 154 {
      // Note that this bit is present over the entire 32kB range; it is not present in only odd or even addresses unlike the associated Namcot 108. 
      if val & 0x40 > 0 {
        mem.banks.vram.mirror(&Mirroring::HighTable);
      } else {
        mem.banks.vram.mirror(&Mirroring::LowTable);
      }
    }
    
    match addr & 0xe001 {
      // (0x8000..=0x9fff, true)
      0x8000 => {
        self.select = val as u8 & 0x7;
      }

      // (0x8000..=0x9fff, false)
      0x8001 => {
        let mut val = val;
        if matches!(self.mapper, 88 | 154) {
          // A possible way to implement this would be to mask the CHR ROM 1K bank output from the mapper by ANDing with $3F, and then OR it with $40 for N108 registers 2, 3, 4, and 5. 
          // https://github.com/SourMesen/Mesen2/blob/master/Core/NES/Mappers/Namco/Namco108_88.h
          match self.select {
            0 | 1 => val &= 0x3f,
            6 | 7 => {}
            _ => val |= 0x40
          }
        } else if self.mapper == 95 {
          if self.select == 0 {
            mem.banks.vram.set_page(0, (val & 0x20) >> 5);
            mem.banks.vram.set_page(1, (val & 0x20) >> 5);
          } else if self.select == 1 {
            mem.banks.vram.set_page(2, (val & 0x20) >> 5);
            mem.banks.vram.set_page(3, (val & 0x20) >> 5);
          }
        }

        match self.select {
          6 | 7 => mem.banks.prg.set_page(self.select - 6, val & 0x3f),
          0 | 1 => mem.banks.chr.set_pages_aligned2(2 * self.select, val),
          // cases 2..=5
          _     => if self.mapper == 76 {
            mem.banks.chr.set_page(self.select - 2, val);
          } else {
            mem.banks.chr.set_page((self.select - 2) + 4, val)
          }
        }
      }

        _ => {}
    }
  }
}

// https://www.nesdev.org/wiki/INES_Mapper_077 
struct NapoleonSenki;
impl Mapper for NapoleonSenki {
  fn new(mem: &mut Bus) -> Box<Self> {
    mem.banks.prg = Banking::new_prg(&mem.header, 1);
    mem.banks.chr = Banking::new(mem.header.chr_size, 2 * 1024, 1);
    
    // this games provides 8kb of chr ram + 2kb of vram
    // we simulate chr ram by extending our vram from 0x0000 to 0x2fff, even if we dont use 0x0000..=0x07ff
    mem.vram.resize(12 * 1024, 0);
    mem.banks.vram = Banking::new(12 * 1024, 12 * 1024, 6);
    
    for i in 1..6 {
      mem.banks.vram.bankings[i] = i * 2048;
      // I HAVE NO CLUE WHAT THIS DOENST WORK and have to manually set the pages..
      // mem.banks.vram.set_page(i as u8, i);
    }

    for i in 2..12 {
      mem.ppu_handlers_1kb[i] = PpuHandler::VramInChr;
    }
    println!("{:?}", mem.banks.vram);
    println!("{:?}", mem.ppu_handlers_1kb);

    Box::new(Self)
  }

  fn prg_write(&mut self, mem: &mut Bus, _: u16, val: u16) {
    mem.banks.prg.set_page(0, val & 0xf);
    mem.banks.chr.set_page(0, val >> 4);
  }
}

// https://www.nesdev.org/wiki/VRC1
#[derive(Default)]
struct VRC1 {
  chr_bank0: u16,
  chr_bank1: u16,
}
impl Mapper for VRC1 {
  fn new(mem: &mut Bus) -> Box<Self> {
    mem.banks.prg = Banking::new_prg(&mem.header, 4);
    mem.banks.prg.set_page_to_last_bank(3);
    mem.banks.chr = Banking::new_chr(&mem.header, 2);

    Box::new(Self::default()) 
  }

  fn prg_write(&mut self, mem: &mut Bus, addr: u16, val: u16) {
    match addr & 0xf000 {
      0x8000 => mem.banks.prg.set_page(0, val),
      0xa000 => mem.banks.prg.set_page(1, val),
      0xc000 => mem.banks.prg.set_page(2, val),
      0x9000 => {
        let mirroring = match val & 1 {
          0 => Mirroring::Vertical,
          _ => Mirroring::Horizontal,
        };
        mem.banks.vram.mirror(&mirroring);

        self.chr_bank0 = (self.chr_bank0 & 0xf) | ((val & 0x2) << 3);
        self.chr_bank1 = (self.chr_bank1 & 0xf) | ((val & 0x4) << 2);
        mem.banks.chr.set_page(0, self.chr_bank0);
        mem.banks.chr.set_page(1, self.chr_bank1);
      }
      0xe000 => {
        self.chr_bank0 = (self.chr_bank0 & 0x10) | (val & 0xf);
        mem.banks.chr.set_page(0, self.chr_bank0);
      }
      0xf000 => {
        self.chr_bank1 = (self.chr_bank1 & 0x10) | (val & 0xf);
        mem.banks.chr.set_page(1, self.chr_bank1);
      }
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
  fn new(mem: &mut Bus) -> Box<Self> {
    mem.banks.prg.set_page_to_last_bank(1);
    Box::new(Self::default())
  }

  fn prg_write(&mut self, mem: &mut Bus, addr: u16, val: u16) {
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
        mem.irq.remove(IrqFlags::MAPPER);
      }
      0xd => {
        self.irq_enabled = self.irq_enable_on_ack;
        mem.irq.remove(IrqFlags::MAPPER);
      }
      
      0xf => mem.banks.prg.set_page(0, val),
      _ => {}
    }
  }

  fn step(&mut self, mem: &mut Bus, _cycles: usize) {
    if self.irq_enabled {      
      if self.irq_8bit_mode {
        let next = (self.irq_count & 0xff) + 1;
        if next > 0xff {
          self.irq_count = byte_set_lo(self.irq_count, self.irq_latch as u8);
          mem.irq.insert(IrqFlags::MAPPER);
        }
        self.irq_count = byte_set_lo(self.irq_count, next as u8);
      } else {
        self.irq_count = self.irq_count.wrapping_add(1);
        if self.irq_count == 0 {
          self.irq_count = self.irq_latch;
          mem.irq.insert(IrqFlags::MAPPER);
        }
      }
    }
  }
}

mod vrc {
  use crate::bus::{self, IrqFlags};

  #[derive(Default)]
  // https://www.nesdev.org/wiki/VRC_IRQ
  pub struct Irq {
    prescaler: i16,
    pub count: u8,
    pub latch: u8,
    pub enable_after_ack: bool,
    pub enabled: bool,
    pub mode_scanline: bool,
  }

  impl Irq {
    pub fn write_ctrl(&mut self, val: u8, mem: &mut bus::Bus) {
      self.enable_after_ack = val & 0x1 > 0;
      self.enabled = val & 0x2 > 0;
      self.mode_scanline = val & 0x4 == 0;

      if self.enabled {
        self.count = self.latch;
        self.prescaler = 341;
      }
      
      // Any write to this register will acknowledge the pending IRQ and reset the prescaler.
      mem.irq.remove(IrqFlags::MAPPER)
    }

    pub fn write_ack(&mut self, mem: &mut bus::Bus) {
      self.enabled = self.enable_after_ack;
      mem.irq.remove(IrqFlags::MAPPER);
    }

    pub fn step(&mut self, mem: &mut bus::Bus) {
      if !self.enabled { return; } 

      self.prescaler -= 3;
      if !self.mode_scanline || (self.mode_scanline && self.prescaler <= 0) {
        if self.count == 0xff {
          self.count = self.latch;
          mem.irq.insert(IrqFlags::MAPPER);
        } else {
          self.count += 1;
        }
        self.prescaler += 341;
      }
    }

    //   let mut clock = || {
    //     if self.count >= 0xff {
    //       self.count = self.latch;
    //       mem.irq.insert(IrqFlags::MAPPER);
    //     } else {
    //       self.count += 1;
    //     }
    //   };

    //   if self.mode_scanline {
    //     self.prescaler -= 3;
    //     if self.prescaler <= 0 {
    //       self.prescaler += 341;
    //       clock();
    //     }
    //   } else {
    //     clock();
    //   }
    // }
  }
}


// https://www.nesdev.org/wiki/VRC2_and_VRC4
#[derive(Default)]
struct VRC2_4 {
  irq: vrc::Irq,
  mapper: u16,
  submapper: u8,
  is_vrc2: bool,

  prg_swapped: u8,
  prg_bank: u16,
  chr_regs: [u16; 8],

  latch: u8,
}
impl VRC2_4 {
  fn translate_address(&self, addr: u16) -> u16 {
    // The primary difference between them was having the mapper address lines connected in different ways. In particular, two lines chosen from A0-A7 will be used to select registers. 

    let take_bits = |a0: u8, a1: u8| {
      ((addr >> a0) & 1, (addr >> a1) & 1)
    };

    let (a0, a1) = match (self.mapper, self.submapper) {
      (23, 1 | 3) => take_bits(0, 1),
      (22, 0) | (25, 1 | 3) => take_bits(1, 0),
      (21, 1) => take_bits(1, 2),
      (21, 2) => take_bits(6, 7),
      (25, 2) => take_bits(3, 2),
      (23, 2) => take_bits(2, 3),

      _ => unreachable!()
    };

    addr & 0xff00 | (a1 << 1) | a0
  }

  fn update_chr_banks(&mut self, mem: &mut Bus, addr: u16, val: u16) {
    let reg_pair = (addr >> 12) - 0xb;
    // we can tell if it is low or high nibble by second bit
    let low_or_high = (addr >> 1) & 1;
    // multiply reg pair by two, add low or high
    let page = ((reg_pair) << 1) | low_or_high;

    let reg = &mut self.chr_regs[page as usize]; 
    
    if addr & 1 == 0 {
      // low
      *reg = (*reg & 0x1f0) | (val & 0xf);
    } else {
      // high
      let val = if self.is_vrc2 {
        // VRC2 only has 4 high bits of CHR select. $B003 bit 4 is ignored. 
        val & 0xf
      } else { val & 0x1f };

      *reg = (*reg & 0xf) | (val << 4);
    }

    if self.mapper == 22 {
      // On VRC2a (mapper 22), the low bit is ignored (right shift value by 1). 
      mem.banks.chr.set_page(page as u8, *reg >> 1);
    } else {
      mem.banks.chr.set_page(page as u8, *reg);
    }
  }
}
impl Mapper for VRC2_4 {
  fn new(mem: &mut Bus) -> Box<Self> {
    mem.banks.prg = Banking::new_prg(&mem.header, 4);
    let last_bank = mem.banks.prg.banks_count-1;
    mem.banks.prg.set_page(2, last_bank-1);
    mem.banks.prg.set_page(3, last_bank);

    mem.banks.chr = Banking::new_chr(&mem.header, 8);

    let is_vrc2 = matches!(
      (mem.header.mapper, mem.header.submapper),
      (22, 0) | (23, 3) | (25, 3)
    );

    if is_vrc2 && mem.wram.is_empty() {
      mem.set_wram_handlers(CpuHandler::Mapper);
    }

    // TODO: might have 2kb wram mirrored, we cant do that with 8kb handlers..

    Box::new(Self {
      mapper: mem.header.mapper,
      submapper: mem.header.submapper,
      is_vrc2,
      ..Default::default()
    })
  }

  fn cart_read(&mut self, mem: &mut Bus, addr: u16) -> u8 {
    if self.is_vrc2 && matches!(addr, 0x6000..=0x6fff) {
      self.latch
    } else { mem.cpu_data_bus }
  }

  fn cart_write(&mut self, _: &mut Bus, addr: u16, val: u16) {
    if self.is_vrc2 && matches!(addr, 0x6000..=0x6fff) {
      self.latch = val as u8 & 1;
    }
  }

  fn prg_write(&mut self, mem: &mut Bus, addr: u16, val: u16) {
    let addr = self.translate_address(addr);
    match (addr & 0xf00f, self.is_vrc2) {
      (0x9002, false) => {
        mem.wram_enable(val & 0x1 > 0);

        let swap_mode = val & 0x2 > 0;
        let second_last_bank = mem.banks.prg.banks_count-2;
        if swap_mode {
          // the 8 KiB page at $8000 is fixed to the second-to-last 8 KiB in the ROM
          // the 8 KiB page at $C000 is controlled by the $800x register
          mem.banks.prg.set_page(0, second_last_bank);
          self.prg_swapped = 2;
        } else {
          // the 8 KiB page at $8000 is controlled by the $800x register
          // the 8 KiB page at $C000 is fixed to the second-to-last 8 KiB in the ROM
          self.prg_swapped = 0;
          mem.banks.prg.set_page(2, second_last_bank);
        }
        mem.banks.prg.set_page(self.prg_swapped, self.prg_bank);
      }

      (0x8000..=0x8003, _) => {
        self.prg_bank = val;
        mem.banks.prg.set_page(self.prg_swapped, val)
      }
      (0xa000..=0xa003, _) => mem.banks.prg.set_page(1, val),
      (0x9000..=0x9003, true) | (0x9000, false) => {
        let val = if self.is_vrc2 { val & 0b01 } else { val & 0b11};

        let mirroring = match val {
          0 => Mirroring::Vertical,
          1 => Mirroring::Horizontal,
          2 => Mirroring::LowTable,
          _ => Mirroring::HighTable,
        };
        mem.banks.vram.mirror(&mirroring);
      }

      (0xb000..=0xe003, _) => self.update_chr_banks(mem, addr, val),

      (0xf000, false) => self.irq.latch = (self.irq.latch & 0xf0) | (val as u8 & 0xf),
      (0xf001, false) => self.irq.latch = (self.irq.latch & 0x0f) | ((val as u8 & 0xf) << 4),
      (0xf002, false) => self.irq.write_ctrl(val as u8, mem),
      (0xf003, false) => self.irq.write_ack(mem),
      _ => {}
    }
  }

  fn step(&mut self, mem: &mut Bus, _cycles: usize) {
    if !self.is_vrc2 {
      self.irq.step(mem);
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
  regs: [u16; 8],
  mode: u8,
  mirroring: u8,
  uses_chr_rom: bool,

  irq: vrc::Irq,

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
        chr.set_pages_aligned2(2 * i as u8, self.regs[i]);
      }

      _ => {
        for i in 0..4 {
          chr.set_page(i as u8, self.regs[i]);
        }
        // only r4 and r5 set two pages each
        chr.set_pages_aligned2(4, self.regs[4]);
        chr.set_pages_aligned2(6, self.regs[5]);
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
  fn new(mem: &mut Bus) -> Box<Self> {
    mem.banks.prg = Banking::new_prg(&mem.header, 4);
    mem.banks.prg.set_pages_aligned2(0, 0);
    mem.banks.prg.set_page_to_last_bank(3);

    mem.banks.chr = Banking::new_chr(&mem.header, 8);

    Box::new(Self {
      mapper: mem.header.mapper,
      ..Default::default()
    })
  }

  fn prg_write(&mut self, mem: &mut Bus, mut addr: u16, val: u16) {
    if self.mapper == 26 {
      addr = (addr & 0xffc) | ((addr & 0x01) << 1) | ((addr & 0x02) >> 1);
    }

    let val = val as u8;
    match addr & 0xf003 {
      // be careful here: value passed here is missing lsb bit, so we have to shift it right
      0x8000..=0x8003 => mem.banks.prg.set_pages_aligned2(0, (val as u16) << 1),
      0xc000..=0xc003 => mem.banks.prg.set_page(2, val as u16),
      
      0xb003 => {
        self.mode = val & 0x3;
        self.mirroring = (val >> 2) & 0x3;

        // The VRC6 supports the use of a larger RAM to provide more nametables.
        // However, the three commercial VRC6 games neither provided extra nametable RAM, nor used ROM nametables.
        self.uses_chr_rom = val & 0x10 > 0;
        
        // The commercial games always left bit 5 set.

        self.update_all_banks(mem);

        mem.wram_enable(val & 0x80 > 0);
      }
      0xd000..=0xd003 => {
        self.regs[addr as usize - 0xd000] = val as u16;
        self.update_all_banks(mem);
      }
      0xe000..=0xe003 => {
        self.regs[addr as usize - 0xe000 + 4] = val as u16;
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

  fn step(&mut self, mem: &mut Bus, _cycles: usize) {
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

// https://www.nesdev.org/wiki/VRC7
// TODO: incomplete
#[derive(Default)]
struct VRC7 {
  irq: vrc::Irq,
}
impl Mapper for VRC7 {
  fn new(mem: &mut Bus) -> Box<Self> {
    mem.banks.prg = Banking::new_prg(&mem.header, 4);
    mem.banks.prg.set_page_to_last_bank(3);
    mem.banks.chr = Banking::new_chr(&mem.header, 8);

    Box::new(Self::default())
  }

  fn prg_write(&mut self, mem: &mut Bus, addr: u16, val: u16) {
    let addr = if addr & 0x10 > 0 {
      // if we have 0x10, clear it and insert 0x8
      addr & !0x10 | 0x8
    } else { addr };

    match addr & 0xf00f {
      // LOL
      0x8000 => mem.banks.prg.set_page(0, val),
      0x8008 => mem.banks.prg.set_page(1, val),
      0x9000 => mem.banks.prg.set_page(2, val),
      0xa000 => mem.banks.chr.set_page(0, val),
      0xa008 => mem.banks.chr.set_page(1, val),
      0xb000 => mem.banks.chr.set_page(2, val),
      0xb008 => mem.banks.chr.set_page(3, val),
      0xc000 => mem.banks.chr.set_page(4, val),
      0xc008 => mem.banks.chr.set_page(5, val),
      0xd000 => mem.banks.chr.set_page(6, val),
      0xd008 => mem.banks.chr.set_page(7, val),

      0xe000 => {
        let mirroring = match val & 0x03 {
          0 => Mirroring::Vertical,
          1 => Mirroring::Horizontal,
          2 => Mirroring::LowTable,
          _ => Mirroring::HighTable,
        };
        mem.banks.vram.mirror(&mirroring);

        // TODO: audio

        mem.wram_enable(val & 0x80 > 0);
      }
      0xe008 => self.irq.latch = val as u8,
      0xf000 => self.irq.write_ctrl(val as u8, mem),
      0xf008 => self.irq.write_ack(mem),
      _ => {}
    }
  }

  fn step(&mut self, mem: &mut Bus, _cycles: usize) {
    self.irq.step(mem);
  }
}

mod mmc5 {
  pub struct ExRam(pub [u8; 1024]);
  impl Default for ExRam {
    fn default() -> Self { Self([0; 1024]) }
  }

  pub enum ExRamMode {
    Vram, CpuRW, CpuReadOnly,
  }
}

#[derive(Default)]
struct MMC5 {
  exram: mmc5::ExRam,

  ppu_substituion: bool,
  ppu_big_sprites: bool,

  prg_mode: u8,
  prg_regs: [u16; 5],

  chr_mode: u8,
  chr_regs: [u16; 12],
  chr_hi: u8,
  last_chr_wrote: u16,

  tile_fetches_count: usize,

  wram_protect: u8,
  exram_mode: u8,

  irq_enabled: bool,
  irq_pending: bool,
  irq_cmp: u16,
  irq_count: u16,
  ppu_in_frame: bool,

  ppu_addr_count: usize,
  last_ppu_addr: Option<u16>,
  ppu_idle_countdown: usize,

  multiplicand: u8,
  multiplier: u8,
  product: u16,

  p0: apu::Pulse,
  p1: apu::Pulse,
  audio_cycles: usize,
}
impl MMC5 {
  fn update_prg_banks(&mut self, mem: &mut Bus) {
    let wram = &mut mem.banks.wram;
    let prg = &mut mem.banks.prg;
  
    // always on wram page 0
    wram.set_page(0, self.prg_regs[0] & 0x7f);
    // always on rom, so forcefully set high bit to 1
    self.prg_regs[4] |= 0x80;

    let mut set_bank = |page, bank| {
      if bank & 0x80 > 0 {
        // rom
        prg.set_page(page - 1, bank & 0x7f);
        mem.cpu_handlers_8kb[3 + page as usize] = CpuHandler::PrgMMC5;
      } else {
        // ram
        wram.set_page(page, bank & 0x7f);

        let handler = if mem.wram.is_empty() {
          CpuHandler::Mapper
        } else if self.wram_protect == 0x6 {
          CpuHandler::WramRW
        } else {
          CpuHandler::WramReadOnly
        };

        mem.cpu_handlers_8kb[3 + page as usize] = handler;
      }
    };

    // 5114 only in mode 3
    if self.prg_mode == 3 {
      set_bank(1, self.prg_regs[1]);
    }

    // 5115 in modes 1, 2, 3
    let reg5115 = self.prg_regs[2];
    if self.prg_mode == 3 {
      set_bank(2, reg5115);
    } else if matches!(self.prg_mode, 1 | 2) {
      set_bank(1, reg5115 & !1);
      set_bank(2, reg5115 | 1);
    }

    // 5116 in modes 2, 3
    if matches!(self.prg_mode, 2 | 3) {
      set_bank(3, self.prg_regs[3]);
    }

    // 5117 in all modes
    let reg5117 = self.prg_regs[4];
    if matches!(self.prg_mode, 2 | 3) {
      set_bank(4, reg5117);
    } else if self.prg_mode == 1 {
      set_bank(3, reg5117 & !1);
      set_bank(4, reg5117 | 1);
    } else if self.prg_mode == 0 {
      let reg5117 = reg5117 & !0x3;

      set_bank(1, reg5117 | 0);
      set_bank(2, reg5117 | 1);
      set_bank(3, reg5117 | 2);
      set_bank(4, reg5117 | 3);
    }
  }

  fn update_chr_banks(&mut self, mem: &mut Bus) {
    // in 8x8 sprites mode, in 16x8 sprites mode and rendering sprites, in vblank use last written low registers
    let use_low_regs = !self.ppu_big_sprites 
      || (self.tile_fetches_count >= 32 && self.tile_fetches_count < 48)
      || (!self.ppu_in_frame && self.last_chr_wrote <= 0x5127);

    if use_low_regs {
      self.update_chr_low_regs(mem);
    } else {
      self.update_chr_high_regs(mem);
    }
  }

  fn update_chr_low_regs(&mut self, mem: &mut Bus) {
    // Caution: Unlike the MMC1 and unlike PRG banking on the MMC5, the banks are always indexed by the currently selected size.
    // When using 2kb, 4kb or 8kb bank sizes, the registers hold bank index of that larger size, and lower bits are *not* ignored. 

    let chr = &mut mem.banks.chr;
    match self.chr_mode {
      // 8kb
      0 => chr.set_pages_unaligned(0, self.chr_regs[7], 8),
      // 4kb
      1 => {
        chr.set_pages_unaligned(0, self.chr_regs[3], 4);
        chr.set_pages_unaligned(4, self.chr_regs[7], 4);
      }
      // 2kb
      2 =>  for i in 0..4 {
        // only odds chr_regs
        chr.set_pages_unaligned(i, self.chr_regs[i as usize * 2 + 1], 2);
      }
      // 1kb
      _ => for i in 0..8 {
        chr.set_page(i, self.chr_regs[i as usize]);
      }
    }
  }

  fn update_chr_high_regs(&mut self, mem: &mut Bus) {
    // Caution: Unlike the MMC1 and unlike PRG banking on the MMC5, the banks are always indexed by the currently selected size.
    // When using 2kb, 4kb or 8kb bank sizes, the registers hold bank index of that larger size, and lower bits are *not* ignored. 
    // shifting is needed
    let chr = &mut mem.banks.chr;
    match self.chr_mode {
      // 8kb
      0 => chr.set_pages_unaligned(0, self.chr_regs[11], 8),
      // 4kb
      1 => {
        chr.set_pages_unaligned(0, self.chr_regs[11], 4);
        chr.set_pages_unaligned(4, self.chr_regs[11], 4);
      }
      // 2kb
      2 =>  {
        chr.set_pages_unaligned(0, self.chr_regs[9], 2);
        chr.set_pages_unaligned(2, self.chr_regs[11], 2);
        chr.set_pages_unaligned(4, self.chr_regs[9], 2);
        chr.set_pages_unaligned(6, self.chr_regs[11], 2);
      }
      // 1kb
      _ => for i in 0..4 {
        let bank = self.chr_regs[8 + i as usize];
        chr.set_page(i, bank);
        chr.set_page(4 + i, bank);
      }
    }
  }

  fn update_vram_banks(&mut self, mem: &mut Bus, val: u8) {
    for i in 0..4 {
      // let nametbl = (val >> (i * 2)) & 0x3;
      let nametbl = (val >> (i * 2)) & 1;
      // TODO: do not handle exram for now
      mem.banks.vram.set_page(i, nametbl as u16);
    }
  }

  fn reset_irq(&mut self, mem: &mut Bus) {
    self.ppu_in_frame = false;
    self.last_ppu_addr = None;
    self.irq_count = 0;
    mem.irq.remove(IrqFlags::MAPPER);
  }
}
impl Mapper for MMC5 {
  fn new(mem: &mut Bus) -> Box<Self> {
    mem.banks.prg = Banking::new_prg(&mem.header, 4);
    mem.banks.chr = Banking::new_chr(&mem.header, 8);

    // wram can be mapped in range 0x6000..=0xdfff (32kb)
    mem.banks.wram = Banking::new(mem.header.wram_size, 32 * 1024, 4);
    mem.set_prg_handlers(CpuHandler::PrgMMC5);
    mem.cpu_handlers_8kb[1] = CpuHandler::PpuMMC5;

    let mut res = Self::default();
    // The Koei games never write to this register, apparently relying on the MMC5 defaulting to mode 3 at power on. 
    res.prg_mode = 3;
    // All known games have their reset vector in the last bank of PRG ROM, and the vector points to an address greater than or equal to $E000.
    // This tells us that $5117 must have a reliable power-on value of $FF. 
    res.prg_regs[4] = 0xff;

    res.update_prg_banks(mem);
    res.update_chr_banks(mem);

    Box::new(res)
  }

  fn cart_read(&mut self, mem: &mut Bus, addr: u16) -> u8 {
    match addr {
      0x5015 => {
        let mut res = 0;
        res |= ((self.p0.len.count > 0) as u8) << 0;
        res |= ((self.p1.len.count > 0) as u8) << 1;
        res
      }

      0x5204 => {
        let mut res = 0;
        res |= (self.irq_pending as u8) << 7;
        res |= (self.ppu_in_frame as u8) << 6;

        self.irq_pending = false;
        mem.irq.remove(IrqFlags::MAPPER);
        res
      }

      0x5205 => self.product as u8,
      0x5206 => (self.product >> 8) as u8,

      0x5c00..=0x5fff => {
        // TODO: exram
        0
      }
      _ => mem.cpu_data_bus,
    }
  }

  fn cart_write(&mut self, mem: &mut Bus, addr: u16, val: u16) {
    let val = val as u8;
    match addr {
      0x5000 => self.p0.write_ctrl(val as u8),
      0x5002 => self.p0.write_timer_lo(val as u8),
      0x5003 => self.p0.write_timer_hi(val as u8),

      0x5004 => self.p1.write_ctrl(val as u8),
      0x5006 => self.p1.write_timer_lo(val as u8),
      0x5007 => self.p1.write_timer_hi(val as u8),

      0x5015 => {
        self.p0.len.enable(val & 0x1 > 0);
        self.p1.len.enable(val & 0x2 > 0);
      }

      0x5100 => {
        self.prg_mode = val & 0x3;
        self.update_prg_banks(mem);
      }
      0x5101 => {
        self.chr_mode = val & 0x3;
        self.update_chr_banks(mem);
      }

      0x5102 => {
        self.wram_protect = (self.wram_protect & 0xc) | (val & 0x3);
        // self.wram_protect == 0x6
        // TODO: wram rw
      }
      0x5103 => {
        self.wram_protect = (self.wram_protect & 0x3) | ((val & 0x3) << 2);
        // self.wram_protect == 0x6
        // TODO: wram rw
      }

      0x5104 => {
        self.exram_mode = val & 0x3;
      }

      0x5105 => self.update_vram_banks(mem, val),

      0x5113..=0x5117 => {
        let reg = addr as usize - 0x5113;
        self.prg_regs[reg] = val as u16;
        self.update_prg_banks(mem);
      }

      0x5120..=0x512b => {
        let reg = addr as usize - 0x5120;
        self.chr_regs[reg] = ((self.chr_hi as u16) << 8) | val as u16;
        self.last_chr_wrote = addr;

        self.update_chr_banks(mem);
      }

      // no official game relies on this register, and most don't even initialize it. 
      0x5130 => self.chr_hi = val & 0x3,

      0x5203 => self.irq_cmp = val as u16,
      0x5204 => {
        self.irq_enabled = val & 0x80 > 0;
      
        if self.irq_enabled && self.irq_pending {
          mem.irq.insert(IrqFlags::MAPPER);
        } else if !self.irq_enabled {
          mem.irq.remove(IrqFlags::MAPPER);
        }
      }

      0x5205 => {
        self.multiplicand = val;
        self.product = self.multiplicand as u16 * self.multiplier as u16;
      }
      0x5206 => {
        self.multiplier = val;
        self.product = self.multiplicand as u16 * self.multiplier as u16;
      }

      0x5c00..=0x5fff => {
        // TODO exram
      }
      _ => {}
    }
  }

  // https://www.nesdev.org/wiki/MMC5#Scanline_Detection_and_Scanline_IRQ
  fn notify_ppu_addr(&mut self, mem: &mut Bus, _cycles: usize) {
    // nametable tile fetch, not attribute
    if mem.ppu_addr_bus & 0x2000 > 0 && mem.ppu_addr_bus & 0x3ff < 0x3c0 {
      self.tile_fetches_count += 1;
      
      // there are 16 dummy nametables fetches during sprites rendering
      if self.ppu_in_frame && matches!(self.tile_fetches_count, 32 | 48) {
        self.update_chr_banks(mem);
      }
    }

    // The MMC5 detects scanlines by first looking for three consecutive PPU reads from the same nametable address in the range $2xxx. 
    // the scanline gets detected when the PPU does the attribute table byte read, which is at PPU cycle 4.
    if mem.ppu_addr_bus & 0x2000 > 0 && self.last_ppu_addr.is_some_and(|x| x == mem.ppu_addr_bus) {
      self.ppu_addr_count += 1;

      if self.ppu_addr_count >= 2 {
        // scanline just started
        self.tile_fetches_count = 0;

        if !self.ppu_in_frame {
          self.ppu_in_frame = true;
          self.irq_count = 0;
          self.update_chr_banks(mem);
        } else {
          self.irq_count += 1;
          // Value $00 is a special case that will not produce IRQ pending conditions
          if self.irq_count == self.irq_cmp {
            self.irq_pending = true;
            // The IRQ pending flag is raised when the desired scanline is reached regardless of whether or not the scanline IRQ is enabled, i.e. even after a 0 was written to the scanline IRQ enable flag. 
            // However, an actual IRQ is only sent to the CPU if both the scanline IRQ enable flag and IRQ pending flag are set. 
            // A $5203 value of $00 is a special case where the comparison is never true.
            if self.irq_enabled {
              mem.irq.insert(IrqFlags::MAPPER);
            }
          }
        }
      }
    } else {
      self.ppu_addr_count = 0;
    }

    self.last_ppu_addr = Some(mem.ppu_addr_bus);
    self.ppu_idle_countdown = 3;
  }

  fn step(&mut self, mem: &mut Bus, _cycles: usize) {
    if self.ppu_idle_countdown > 0 {
      self.ppu_idle_countdown -= 1;
      if self.ppu_idle_countdown == 0 {
        self.ppu_in_frame = false;
        self.last_ppu_addr = None;
        self.update_chr_banks(mem);
      }
    }

    if self.audio_cycles % 2 == 1 {
      self.p0.step_divider();
      self.p1.step_divider();
    }
    
    // envelope and length counter are fixed to a 240hz update rate.
    if self.audio_cycles > (1789773 / 240) {
      self.audio_cycles -= 1789773 / 240;
      self.p0.len.step();
      self.p0.env.step();
      self.p1.len.step();
      self.p1.env.step();
    }
    self.audio_cycles += 1;
  }

  fn notify_cpu_addr(&mut self, mem: &mut Bus, addr: u16, val: Option<u8>) {
    match (addr, val) {
      (0xfffa | 0xfffb, None) => {
        self.reset_irq(mem);
        self.update_chr_banks(mem);
      }

      (0x2000, Some(val)) => {
        self.ppu_big_sprites = val & 0x20 > 0;
        self.update_chr_banks(mem);
      }

      (0x2001, Some(val)) => {
        let ppu_sub = val & 0x18 > 0;
        // When the MMC5 sees $00 written to $2001, and then the PPU’s rendering gets enabled via a mirror of $2001, the MMC5 still counts scanlines and can generate scanline interrupts even though it thinks $2001 is still disabled.
        // The transition from disabled to enabled resets the scanline counter.
        if !self.ppu_substituion && ppu_sub {
          self.reset_irq(mem);
        } else if !ppu_sub {
          self.ppu_in_frame = false;
          self.last_ppu_addr = None;
        }
        
        self.ppu_substituion = ppu_sub;
        self.update_chr_banks(mem);
      }

      _ => {}
    }
  }

  fn prg_write(&mut self, _: &mut Bus, _: u16, _: u16) {}

  // The sound output of the square channels are equivalent in volume to the corresponding APU channels, but the polarity of all MMC5 channels is reversed compared to the APU. 
  fn sample(&self) -> f32 {
    ((self.p0.sample() + self.p1.sample()) as f32).neg()
  }
}

#[derive(Default)]
struct FDS {
  irq_reload: u16,
  irq_repeat: bool,
  irq_enabled: bool,
}
impl Mapper for FDS {
  fn new(mem: &mut Bus) -> Box<Self> {
    mem.banks.prg = Banking::new_prg(&mem.header, 1);
    mem.set_wram_handlers(CpuHandler::PrgInWram);
    mem.set_prg_handlers(CpuHandler::PrgInWram);

    // we put bios in wram so that it can't be written to
    mem.wram.resize(8 * 1024, 0);
    mem.wram.copy_from_slice(include_bytes!("../utils/disksys.rom"));
    mem.cpu_handlers_8kb[7] = CpuHandler::WramReadOnly;

    Box::new(Self::default())
  }

  fn cart_write(&mut self, mem: &mut Bus, addr: u16, val: u16) {
    match addr {
      0x4020 => self.irq_reload = byte_set_lo(self.irq_reload, val as u8),
      0x4021 => self.irq_reload = byte_set_hi(self.irq_reload, val as u8),
    
      0x4022 => {
        self.irq_repeat = val & 0x1 > 0;
        self.irq_enabled = val & 0x2 > 0;
      }

      0x4023 => {
        // TODO: master io enable
      }

      0x4024 => {
        // TODO: write data register
      }

      0x4025 => {
        // TODO: fds ctrl
      }

      0x4032 => {
        // TODO: disk status
      }
      _ => {}
    }
  }

  fn prg_write(&mut self, _mem: &mut Bus, _addr: u16, _val: u16) {}
}