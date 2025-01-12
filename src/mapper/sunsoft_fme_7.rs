use crate::cart::{CartBanking, CartHeader, Mirroring, PrgTarget};

use super::{Banking, Mapper};

#[derive(serde::Serialize, serde::Deserialize)]
enum Command { Chr(u8), Prg0, Prg1(u8), Nametbl, IrqCtrl, IrqLo, IrqHi }

// Mapper 69
// https://www.nesdev.org/wiki/Sunsoft_FME-7
#[derive(serde::Serialize, serde::Deserialize)]
pub struct SunsoftFME7 {
  command: Command,

  sram_banked: bool,
  sram_enabled: bool,

  irq_enabled: bool,
  irq_counter_enabled: bool,
  irq_requested: Option<()>,
  irq_count: u16,
}

#[typetag::serde]
impl Mapper for SunsoftFME7 {
  fn new(header: &CartHeader, banks: &mut CartBanking) -> Box<Self> {
    banks.prg = Banking
      ::new(header.prg_size, 0x6000, 8*1024, 5);
    banks.chr = Banking::new_chr(header, 8);
    
    banks.prg.set_page_to_last_bank(4);

    let mapper = Self {
      command: Command::Chr(0),
      sram_banked: false,
      sram_enabled: false,
      irq_enabled: false,
      irq_counter_enabled: false,
      irq_requested: None,
      irq_count: 0,
    };
    Box::new(mapper)
  }

  fn write(&mut self, banks: &mut CartBanking, addr: usize, val: u8) {
    match addr {
      0x8000..=0x9FFF => {
        self.command = match val & 0b1111 {
          0x8 => Command::Prg0,
          0x9..=0xB => Command::Prg1(val & 0b11_1111),
          0xC => Command::Nametbl,
          0xD => Command::IrqCtrl,
          0xE => Command::IrqLo,
          0xF => Command::IrqHi,
          _ => Command::Chr(val & 0b1111),
        }
      }
      0xA000..=0xBFFF => {
        match self.command {
          Command::Chr(page) => banks.chr.set(page as usize, val as usize),
          Command::Prg0 => {
            self.sram_banked = (val >> 6) & 1 != 0;
            self.sram_enabled = val >> 7 != 0;

            let bank = val as usize & 0b11_1111;

            if self.sram_banked {
              banks.sram.set(0, bank);
            } else {
              banks.prg.set(0, bank);
            }
          }
          Command::Prg1(page) => 
            // remeber the first page might be sram, hence the + 1
            banks.prg.set(page as usize - 0x9 + 1, val as usize & 0b11_1111),
          Command::Nametbl => {
            let mirroring = match val & 0b11 {
              0 => Mirroring::Vertical,
              1 => Mirroring::Horizontal,
              2 => Mirroring::SingleScreenA,
              _ => Mirroring::SingleScreenB
            };
            banks.vram.update(mirroring);
          }
          Command::IrqCtrl => {
            self.irq_enabled = val & 1 != 0;
            self.irq_counter_enabled = val >> 7 != 0;
            self.irq_requested = None;
          }
          Command::IrqLo => self.irq_count = (self.irq_count & 0xFF00) | val as u16,
          Command::IrqHi => self.irq_count = (self.irq_count & 0x00FF) | ((val as u16) << 8),
        }
      }
      _ => {}
    }
  }

  fn map_prg_addr(&self, banks: &mut CartBanking, addr: usize) -> PrgTarget {
    match addr {
      0x4020..=0x5FFF => PrgTarget::Cart,
      0x6000..=0x7FFF => {
        if self.sram_banked {
          PrgTarget::SRam(self.sram_enabled, banks.sram.addr(addr))
        } else {
          PrgTarget::Prg(banks.prg.addr(addr))
        }
      }
      0x8000..=0xFFFF => PrgTarget::Prg(banks.prg.addr(addr)),
      _ => unreachable!()
    }
  }

  fn notify_cpu_cycle(&mut self) {
    if !self.irq_counter_enabled { return; }

    self.irq_count = self.irq_count.wrapping_sub(1);
    if self.irq_count == 0xFFFF && self.irq_enabled {
      self.irq_requested = Some(());
    }
  }
}