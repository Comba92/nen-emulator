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
  fn sample(&self) -> u8 { 0 }
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
    31 => NSF::new(header, mem),
    66 | 140 => GxROM::new(header, mem),
    68 => Sunsoft4::new(header, mem),
    69 => SunsoftFME7::new(header, mem),
    71 | 232 => Codemasters::new(header, mem),
    78 => Irem74HCx::new(header, mem),
    _ => return Err(format!("mapper {} not implemented", header.mapper)),
  };

  Ok(mapper)
}

// https://www.nesdev.org/wiki/NROM
pub struct NROM;
impl Mapper for NROM {
  fn new(header: &CartHeader, mem: &mut Bus) -> Box<Self> {    
    if header.prg_size <= 16 * 1024 {
      // Mirror of $8000-$BFFF (NROM-128).
      mem.banks.prg.set_page(1, 0);
    } else {
      // Last 16 KB of ROM (NROM-256)
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
    // TODO: use mask
    match (addr, self.mapper) {
      (0x8000..=0xbfff, 232) => {
        self.prg_block = (val >> 3) & 0b11;
        mem.banks.prg.set_page(0, (self.prg_block << 4) | self.prg_bank);
      }
      // For compatibility without using a submapper, FCEUX begins all games with fixed mirroring, and applies single screen mirroring only once $9000-9FFF is written, ignoring writes to $8000-8FFF.
      (0x9000..=0x9fff, _) => if val & 0x10 == 0 {
        mem.banks.vram.mirror(&Mirroring::SingleScreenA);
      } else {
        mem.banks.vram.mirror(&Mirroring::SingleScreenB);
      }
      (0xc000..=0xffff, 71) => mem.banks.prg.set_page(0, val & 0b1111),
      (0xc000..=0xffff, 232) => {
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
    // TODO: use mask
    if matches!(addr, 0x5000..0x5fff) {
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
  // TODO: do we really need a banking object here? probably just four registers
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
    mem.banks.prg = Banking::new_prg(header, 4);
    // start with prg mode0
    mem.banks.prg.set_page(2, mem.banks.prg.banks_count as u8 - 2);
    mem.banks.prg.set_page_to_last_bank(3);

    mem.banks.chr = Banking::new_chr(header, 8);
    mem.banks.chr.set_page2(0, 0);
    mem.banks.chr.set_page2(2, 0);

    Box::new(Self::default())
  }

  fn prg_write(&mut self, mem: &mut Bus, addr: u16, val: u8) {
    // TODO: only higher bits and first bit matters
    
    match (addr, addr % 2 == 0) {
      (0x8000..=0x9fff, true) => {
        // TODO: something is off with chr banks
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

#[derive(Default)]
struct Sunsoft4 {
  use_chr_rom: bool,
  mirroring: Mirroring,
  chr_table0: u8,
  chr_table1: u8,
}
impl Sunsoft4 {
  fn update_chr_banks(&mut self, mem: &mut Bus) {
    let vram = &mut mem.banks.vram;
    
    if !self.use_chr_rom {
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
    // TODO: use mask
    
    match addr {
      0x6000..=0x7fff => {
        // TODO: Licensing IC
      }

      0x8000..=0x8fff => mem.banks.chr.set_page(0, val),
      0x9000..=0x9fff => mem.banks.chr.set_page(1, val),
      0xa000..=0xafff => mem.banks.chr.set_page(2, val),
      0xb000..=0xbfff => mem.banks.chr.set_page(3, val),

      0xc000..=0xcfff => {
        self.chr_table0 = 0x80 | val;
        if self.use_chr_rom {
          self.update_chr_banks(mem);
        }
      }

      0xd000..=0xdfff => {
        self.chr_table1 = 0x80 | val;
        if self.use_chr_rom {
          self.update_chr_banks(mem);
        }
      }

      0xe000..=0xefff => {
        self.mirroring = match val & 0b11 {
          0 => Mirroring::Vertical,
          1 => Mirroring::Horizontal,
          2 => Mirroring::SingleScreenA,
          _ => Mirroring::SingleScreenB,
        };

        let mode = val & 0x10 > 0;
        if mode != self.use_chr_rom {
          let new_size = if mode { mem.chr.len() } else { 2 * 1024 };
          mem.banks.vram.change_size(new_size);

          let handler = if mode { PpuHandler::ChrInVram } else { PpuHandler::Vram };
          mem.set_vram_handlers(handler);

          self.use_chr_rom = mode;
          self.update_chr_banks(mem);
        }
      }
      0xf000..=0xffff => {
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
    match addr {
      0x8000..=0x9fff => self.command = val & 0b1111,
      0xa000..=0xbfff => {
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