use crate::cart::{CartBanking, CartHeader, PpuTarget};

use super::{set_byte_hi, set_byte_lo, Banking, Mapper};

#[derive(Default, Clone, Copy, serde::Serialize, serde::Deserialize)]
enum ChrTarget { #[default] Chr, Ciram0, Ciram1 }

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Namco129_163 {
  irq_value: u16,
  irq_enabled: bool,
  irq_requested: Option<()>,

  chr_selects: [ChrTarget; 12],
  chrram0_enabled: bool,
  chrram1_enabled: bool,
  exram_write_enabled: [bool; 4],

  apu_enabled: bool,
}

#[typetag::serde]
impl Mapper for Namco129_163 {
  fn new(header: &CartHeader, banks: &mut CartBanking) -> Box<Self> {
    banks.prg = Banking::new_prg(header, 4);
    banks.prg.set_page_to_last_bank(3);
    
    banks.chr = Banking::new(header.chr_real_size(), 0, 1024, 12);
    let chr_selects = [Default::default(); 12];

    Box::new(Self{
      chr_selects,
      irq_enabled: false,
      irq_value: 0,
      irq_requested: None,
      chrram0_enabled: false,
      chrram1_enabled: false,
      exram_write_enabled: [false; 4],
      apu_enabled: false,
    })
  }

  fn cart_read(&mut self, addr: usize) -> u8 {
    match addr {
      0x5000..=0x57FFF => self.irq_value as u8,
      0x5800..=0x5FFFF => {
        let mut res = 0;
        res |= (self.irq_value >> 8) as u8;
        res |= (self.irq_enabled as u8) << 7;
        res
      }
      _ => 0xFF,
    }
  }

  fn cart_write(&mut self, _: &mut CartBanking, addr: usize, val: u8) {
    match addr {
      0x5000..=0x57FFF => {
        self.irq_value = set_byte_lo(self.irq_value, val);
        self.irq_requested = None;
      }
      0x5800..=0x5FFFF => {
        self.irq_value = 
          set_byte_hi(self.irq_value, val) & 0b0111_1111;
        self.irq_enabled = val >> 7 != 0;
        self.irq_requested = None;
      }
      _ => {}
    }
  }

  fn prg_write(&mut self, banks: &mut CartBanking, addr: usize, val: u8) {    
    match addr {
      0x8000..=0x9FFF => {
        let page = (addr as usize - 0x8000) / 0x800;

        if val >= 0xE0 && self.chrram0_enabled {
          self.chr_selects[page] = ChrTarget::Ciram0;
        } else {
          self.chr_selects[page] = ChrTarget::Chr;
        }

        banks.chr.set_page(page, val as usize);
      }
      0xA000..=0xBFFF => {
        let page = (addr as usize - 0x8000) / 0x800;

        if val >= 0xE0 && self.chrram1_enabled {
          self.chr_selects[page] = ChrTarget::Ciram1;
        } else {
          self.chr_selects[page] = ChrTarget::Chr;
        }

        banks.chr.set_page(page, val as usize);
      }
      0xC000..=0xDFFF => {
        let page = (addr as usize - 0x8000) / 0x800;

        if val >= 0xE0 {
          if val % 2 == 0 {
            self.chr_selects[page] = ChrTarget::Ciram0;
          } else {
            self.chr_selects[page] = ChrTarget::Ciram1;
          }
        } else {
          self.chr_selects[page] = ChrTarget::Chr;
        }

        banks.chr.set_page(page, val as usize);
      }
      0xE000..=0xE7FF => {
        let bank = val as usize & 0b11_1111;
        banks.prg.set_page(0, bank);
        self.apu_enabled = (val >> 6) & 1 == 0;
      }
      0xE800..=0xEFFF => {
        let bank = val as usize & 0b11_1111;
        banks.prg.set_page(1, bank);

        self.chrram0_enabled = (val >> 6) & 1 == 0;
        self.chrram1_enabled = (val >> 7) & 1 == 0;
      }
      0xF000..=0xF7FF => {
        let bank = val as usize & 0b11_1111;
        banks.prg.set_page(2, bank);
      }
      0xF800..=0xFFFF => {
        if val >> 6 == 0 {
          self.exram_write_enabled.fill(false);
        } else {
          for i in 0..self.exram_write_enabled.len() {
            self.exram_write_enabled[i] = val as usize >> i == 0; 
          }
        }
      }
      _ => {}
    }
  }

  fn map_ppu_addr(&mut self, banks: &mut CartBanking, addr: usize) -> PpuTarget {
    let page = addr / 0x400;

    match self.chr_selects[page] {
      ChrTarget::Chr => PpuTarget::Chr(banks.chr.translate(addr)),
      ChrTarget::Ciram0 => PpuTarget::CiRam(addr % 0x400),
      ChrTarget::Ciram1 => PpuTarget::CiRam((addr % 0x400) + 0x400),
    }
  }

  fn notify_cpu_cycle(&mut self) {
    if self.irq_requested.is_some() { return; }

    self.irq_value += 1;
    if self.irq_value >= 0x7FFF {
      self.irq_requested = Some(());
    }
  }

  fn poll_irq(&mut self) -> bool {
    self.irq_requested.is_some()
  }
}