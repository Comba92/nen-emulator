use crate::cart::{CartHeader, Mirroring, PrgTarget};

use super::{Banking, ChrBanking, Mapper, PrgBanking, SRamBanking};

#[derive(serde::Serialize, serde::Deserialize)]
enum Command { Chr(u8), Prg0, Prg1(u8), Nametbl, IrqCtrl, IrqLo, IrqHi }

// Mapper 69
// https://www.nesdev.org/wiki/Sunsoft_FME-7
#[derive(serde::Serialize, serde::Deserialize)]
pub struct SunsoftFME7 {
  prg_banks: Banking<PrgBanking>,
  chr_banks: Banking<ChrBanking>,
  sram_banks: Banking<SRamBanking>,
  mirroring: Mirroring,
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
  fn new(header: &CartHeader) -> Box<Self> {
    let mut prg_banks = Banking
      ::new(header.prg_size, 0x6000, 8*1024, 5);
    let chr_banks = Banking::new_chr(header, 8);
    let sram_banks = Banking::new_sram(header);
    let mirroring = header.mirroring;
    
    prg_banks.set_page_to_last_bank(4);

    let mapper = Self {
      prg_banks,chr_banks,sram_banks,
      mirroring,
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

  fn write(&mut self, addr: usize, val: u8) {
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
          Command::Chr(page) => self.chr_banks.set(page as usize, val as usize),
          Command::Prg0 => {
            self.sram_banked = (val >> 6) & 1 != 0;
            self.sram_enabled = val >> 7 != 0;

            let bank = val as usize & 0b11_1111;

            if self.sram_banked {
              self.sram_banks.set(0, bank);
            } else {
              self.prg_banks.set(0, bank);
            }
          }
          Command::Prg1(page) => 
            // remeber the first page might be sram, hence the + 1
            self.prg_banks.set(page as usize - 0x9 + 1, val as usize & 0b11_1111),
          Command::Nametbl => {
            self.mirroring = match val & 0b11 {
              0 => Mirroring::Vertical,
              1 => Mirroring::Horizontal,
              2 => Mirroring::SingleScreenA,
              _ => Mirroring::SingleScreenB
            };
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

  fn map_addr(&mut self, addr: usize) -> PrgTarget {
    match addr {
      0x4020..=0x5FFF => PrgTarget::Cart,
      0x6000..=0x7FFF => {
        if self.sram_banked {
          PrgTarget::SRam(self.sram_enabled, self.sram_addr(addr))
        } else {
          PrgTarget::Prg(self.prg_addr(addr))
        }
      }
      0x8000..=0xFFFF => PrgTarget::Prg(self.prg_addr(addr)),
      _ => unreachable!()
    }
  }

  fn prg_addr(&mut self, addr: usize) -> usize {
    self.prg_banks.addr(addr)
  }

  fn chr_addr(&mut self, addr: usize) -> usize {
    self.chr_banks.addr(addr)
  }

  fn sram_addr(&mut self, addr: usize) -> usize {
    self.sram_banks.addr(addr)
  }

  fn notify_cpu_cycle(&mut self) {
    if !self.irq_counter_enabled { return; }

    self.irq_count = self.irq_count.wrapping_sub(1);
    if self.irq_count == 0xFFFF && self.irq_enabled {
      self.irq_requested = Some(());
    }
  }

  fn mirroring(&self) -> Mirroring { self.mirroring }
}