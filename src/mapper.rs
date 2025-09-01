use crate::{bus::{Banking, Bus, ChrBank, CpuHandler, IrqFlags, PpuHandler}, emu::Mirroring, utils::{byte_set_hi, byte_set_lo}};

mod konami;
use konami::*;

mod mmc5;
use mmc5::*;

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

  fn special_read(&mut self, _mem: &mut Bus, _addr: u16) -> u8 { 0 }

  fn sample(&self) -> f32 { 0.0 }
}

pub fn new(mem: &mut Bus) -> Result<Box<dyn Mapper>, String> {
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
    40 => NTDEC2722::new(mem),
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
          2 => CpuHandler::Wram,
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

  fn cart_read(&mut self, mem: &mut Bus, _addr: u16) -> u8 {
    // TODO: eeprom read for 16, 157, 159
    mem.cpu_data_bus
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

// https://www.nesdev.org/wiki/INES_Mapper_040
#[derive(Default)]
struct NTDEC2722 {
  irq_enabled: bool,
  irq_count: u16,
  submapper: u8,
}
impl Mapper for NTDEC2722 {
  fn new(mem: &mut Bus) -> Box<Self> {
    mem.banks.prg = Banking::new_prg(&mem.header, 4);
    mem.set_wram_handlers(CpuHandler::PrgInWram);

    mem.banks.wram.set_page(0, 6);
    mem.banks.prg.set_page(0, 4);
    mem.banks.prg.set_page(1, 5);
    mem.banks.prg.set_page(3, 7);

    Box::new(Self::default())
  }

  fn prg_write(&mut self, mem: &mut Bus, addr: u16, val: u16) {
    match addr & 0xe000 {
      0x8000 => {
        self.irq_enabled = false;
        self.irq_count = 0;
        mem.irq.remove(IrqFlags::MAPPER);
      }
      0xa000 => self.irq_enabled = true,

      0xc000 => if self.submapper == 1 {
        // TODO: submapper 1 stuff
      }
      0xe000 => mem.banks.prg.set_page(2, val),
      _ => {}
    }
  }

  fn step(&mut self, mem: &mut Bus, _cycles: usize) {
    if self.irq_enabled {
      self.irq_count = self.irq_count.wrapping_add(1);
      if self.irq_count == 0x1000 {
        mem.irq.insert(IrqFlags::MAPPER)
      } else if self.irq_count == 0x2000 {
        // if the software doesn't acknowledge the interrupt for another 4096 cycles it will self-acknowledge.
        mem.irq.remove(IrqFlags::MAPPER);
      }
    }
  }
}

// TODO
// https://www.nesdev.org/wiki/INES_Mapper_032
// struct IremG101;

// https://www.nesdev.org/wiki/INES_Mapper_065
// struct IremH3001;

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
    if self.mapper != 19 { return mem.cpu_data_bus }

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

    Box::new(Self)
  }

  fn prg_write(&mut self, mem: &mut Bus, _: u16, val: u16) {
    mem.banks.prg.set_page(0, val & 0xf);
    mem.banks.chr.set_page(0, val >> 4);
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
    // TODO: set this to 5 banks and addressable size of 40kb
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
            let handler = if mode { CpuHandler::Wram } else { CpuHandler::PrgInWram };
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


// https://www.nesdev.org/wiki/Family_Computer_Disk_System
// https://www.nesdev.org/wiki/FDS_RAM_adaptor_cable_pinout
#[derive(Default)]
pub struct FDS {
  pub disks: Vec<Vec<u8>>,
  disk_current: u8,
  disk_position: usize,

  timer_count: u16,
  timer_reload: u16,
  timer_repeat: bool,
  timer_enabled: bool,

  disk_enabled: bool,
  audio_enabled: bool,

  reset_transfer: bool,
  motor_enabled: bool,
  reading: bool,
  crc_ctrl: bool,
  crc_enabled: bool,
  disk_irq_enabled: bool,

  crc_acc: u16,
  crc_bad: bool,
  disk_at_end: bool,
  disk_scanning: bool,
  disk_ready: bool,
  disk_in_gap: bool,

  mirroring: bool,

  read_buf: u8,
  write_buf: u8,

  byte_transferred: bool,
}
impl FDS {
  fn disk_read(&self) -> u8 {
    self.disks[self.disk_current as usize][self.disk_position as usize]
  }

  fn disk_write(&mut self, val: u8) {
    self.disks[self.disk_current as usize][self.disk_position as usize] = val;
  }

  fn update_crc(&mut self, val: u8) {
    self.crc_acc ^= val as u16;
    for _ in 0..8 {
      let carry = self.crc_acc & 1;
      self.crc_acc >>= 1;
      self.crc_acc ^= 0x8408 * carry;
    }
  }
}
impl Mapper for FDS {
  fn new(_: &mut Bus) -> Box<Self> {
    // everything is initialized in the bus constructor
    Box::new(Self::default())
  }

  fn cart_read(&mut self, mem: &mut Bus, addr: u16) -> u8 {
    match addr {
      0x4030 => {
        let mut res = 0;
        res |= mem.irq.contains(IrqFlags::MAPPER) as u8;
        res |= (self.byte_transferred as u8) << 1;
        res |= (self.mirroring as u8) << 3;
        res |= (self.crc_bad as u8) << 4;
        // res |= (self.byte_transferred as u8) << 7;
        
        self.byte_transferred = false;
        mem.irq.remove(IrqFlags::MAPPER);
        mem.irq.remove(IrqFlags::DISK);

        res | (mem.cpu_data_bus & 0x24)
      }
      0x4031 => {
        self.byte_transferred = false;
        mem.irq.remove(IrqFlags::DISK);
        self.read_buf
      }
      0x4032 => {
        let mut res = 0;
        // TODO: disk inserted
        res |= (!self.disk_scanning as u8) << 1;
        // TODO: disk write protect

        mem.irq.remove(IrqFlags::DISK);

        res | (mem.cpu_data_bus & 0xf8)
      }
      0x4033 => 0x80,
      _ => 0xff 
    }
  }

  fn cart_write(&mut self, mem: &mut Bus, addr: u16, val: u16) {
    match addr {
      0x4020 => self.timer_reload = byte_set_lo(self.timer_reload, val as u8),
      0x4021 => self.timer_reload = byte_set_hi(self.timer_reload, val as u8),
    
      0x4022 => {
        self.timer_repeat = val & 0x1 > 0;
        self.timer_enabled = val & 0x2 > 0 && self.disk_enabled;

        if self.timer_enabled {
          self.timer_count = self.timer_reload;
        } else {
          mem.irq.remove(IrqFlags::MAPPER);
        }
      }

      0x4023 => {
        self.disk_enabled = val & 0x1 > 0;
        self.audio_enabled = val & 0x2 > 0;

        // Clearing $4023.0 will immediately stop the IRQ counter and acknowledge any pending timer IRQs.
        if !self.disk_enabled {
          self.timer_enabled = false;
          mem.irq.remove(IrqFlags::MAPPER);
          mem.irq.remove(IrqFlags::DISK);
        }
      }

      0x4024 => if self.disk_enabled {
        self.write_buf = val as u8;
        self.byte_transferred = false;
      }

      0x4025 => if self.disk_enabled {
        // while high, this instructs the storage media pointer to be reset (and stay reset) at the beginning of the media
        // while low, the media pointer is to be advanced at a constant rate, and data progressively transferred to/from the media
        self.motor_enabled = val & 0x1 > 0;
        // the falling edge of this signal would instruct the drive to stop its motor (and therefore end the current scan of the disk)
        self.reset_transfer = val & 0x2 > 0;
        // while low, this signal indicates that data appearing on the "write data" signal pin is to be written to the storage media.
        self.reading = val & 0x4 > 0;

        let mirroring = if val & 0x8 > 0 {
          Mirroring::Horizontal
        } else {
          Mirroring::Vertical
        };
        mem.banks.vram.mirror(&mirroring);
        self.mirroring = val & 0x8 > 0;

        self.crc_ctrl = 0x10 > 0;
        self.disk_ready = 0x40 > 0;
        self.disk_irq_enabled = val & 0x80 > 0;

        mem.irq.remove(IrqFlags::DISK);
      }
      _ => {}
    }
  }

  fn prg_write(&mut self, _mem: &mut Bus, _addr: u16, _val: u16) {}

  fn step(&mut self, mem: &mut Bus, _cycles: usize) {
    if self.timer_enabled {
      if self.timer_count > 0 {
        self.timer_count -= 1;
      } else {
        mem.irq.insert(IrqFlags::MAPPER);
        self.timer_count = self.timer_reload;
        self.timer_enabled = self.timer_repeat;
      }
    }

    if !self.motor_enabled {
      self.disk_at_end = true;
      self.disk_scanning = false;
      return;
    }

    if self.reset_transfer && !self.disk_scanning { return; }

    if self.disk_at_end {
      self.disk_at_end = false;
      self.disk_position = 0;
      self.disk_in_gap = true;
    }

    self.disk_scanning = true;
    let mut should_irq = self.disk_irq_enabled;
    if self.reading {
      let data = self.disk_read();

      if !self.crc_ctrl {
        self.update_crc(data);
      }

      if !self.disk_ready {
        self.disk_in_gap = true;
        self.crc_acc = 0;
        self.crc_bad = false;
      } else if self.disk_in_gap && data > 0 {
        self.disk_in_gap = false;
        should_irq = false;
      }

      if !self.disk_in_gap {
        self.byte_transferred = true;
        self.read_buf = data;
        if should_irq {
          mem.irq.insert(IrqFlags::DISK);
        }
      }

      if self.crc_ctrl {
        self.crc_bad = self.crc_acc > 0;
      }
    } else {
      let mut data = 0;

      if !self.crc_ctrl {
        self.byte_transferred = true;
        data = self.write_buf;
        if should_irq {
          mem.irq.insert(IrqFlags::DISK);
        }
      }

      if !self.disk_ready {
        data = 0;
        self.crc_acc = 0;
      }

      if !self.crc_ctrl {
        self.update_crc(data);
      } else {
        data = self.crc_acc as u8;
        self.crc_acc >>= 8;
      }

      self.disk_write(data);
      self.disk_in_gap = true;
      self.crc_bad = false;
    }

    self.disk_position += 1;
    if self.disk_position >= self.disks[self.disk_current as usize].len() {
      self.motor_enabled = false;

      if self.disk_irq_enabled {
        mem.irq.insert(IrqFlags::DISK);
      }
    }
  }
}