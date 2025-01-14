use crate::cart::{CartBanking, CartHeader, Mirroring, PrgTarget, PpuTarget};

use super::{Banking, Mapper};

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Sunsoft4 {
  sram_enabled: bool,
  chrrom_banked: bool,
  mirroring: Mirroring,
  nametbl0: usize,
  nametbl1: usize,
  timer: usize,
}

impl Sunsoft4 {
  pub fn update_ciram_banks(&mut self, banks: &mut CartBanking) {
    const PAGE: usize = 8;
    match self.mirroring {
      Mirroring::Horizontal => {
        banks.chr.set_page(PAGE + 0, self.nametbl0);
        banks.chr.set_page(PAGE + 1, self.nametbl0);
        banks.chr.set_page(PAGE + 2, self.nametbl1);
        banks.chr.set_page(PAGE + 3, self.nametbl1);
      }
      Mirroring::Vertical => {
        banks.chr.set_page(PAGE + 0, self.nametbl0);
        banks.chr.set_page(PAGE + 1, self.nametbl1);
        banks.chr.set_page(PAGE + 2, self.nametbl0);
        banks.chr.set_page(PAGE + 3, self.nametbl1);
      }
      Mirroring::SingleScreenA => for i in 0..4 {
        banks.chr.set_page(PAGE + i, self.nametbl0);
      }
      Mirroring::SingleScreenB => for i in 0..4 {
        banks.chr.set_page(PAGE + i, self.nametbl1);
      }
      _ => {}
    }
  }
}

#[typetag::serde]
impl Mapper for Sunsoft4 {
  fn new(header: &CartHeader, banks: &mut CartBanking) -> Box<Self> {
    banks.prg = Banking::new_prg(header, 2);
    banks.prg.set_page_to_last_bank(1);

    banks.chr = Banking::new_chr(header, 12);

    Box::new(Self{ 
      sram_enabled: false,
      chrrom_banked: false,
      mirroring: header.mirroring,
      nametbl0: 0,
      nametbl1: 0,
      timer: 0,
    })
  }

  fn prg_write(&mut self, banks: &mut CartBanking ,addr: usize, val: u8) {
    match addr {
      0x8000..=0xBFFF => {
        let page = 2*((addr - 0x8000) / 0x800);
        let bank = val as usize & !1;
        banks.chr.set_page(page, bank);
        banks.chr.set_page(page+1, bank | 1);
      }
      0xC000..=0xCFFF => {
        if self.chrrom_banked {
          self.nametbl0 = val as usize & 0b0111_1111;
          self.update_ciram_banks(banks);
        }
      }
      0xD000..=0xDFFF => {
        if self.chrrom_banked {
          self.nametbl1 = val as usize & 0b0111_1111;
          self.update_ciram_banks(banks);
        }
      }
      0xE000..=0xEFFF => {
        self.mirroring = match val & 0b11 {
          0 => Mirroring::Vertical,
          1 => Mirroring::Horizontal,
          2 => Mirroring::SingleScreenA,
          _ => Mirroring::SingleScreenB,
        };
        banks.ciram.update_mirroring(self.mirroring);

        self.chrrom_banked = val >> 4 != 0;
      }
      0xF000..=0xFFFF => {
        banks.prg.set_page(0, val as usize & 0b1111);
        self.sram_enabled = (val >> 4) & 1 != 0;
      }
      _ => {}
    }
  }

  fn map_prg_addr(&self, banks: &mut CartBanking, addr: usize) -> PrgTarget {
    match addr {
      0x6000..=0x7FFF => PrgTarget::SRam(self.sram_enabled, banks.sram.translate(addr)),
      0x8000..=0xFFFF => PrgTarget::Prg(banks.prg.translate(addr)),
      _ => unreachable!()
    }
  }

  fn map_ppu_addr(&mut self, banks: &mut CartBanking, addr: usize) -> PpuTarget {
    match addr {
      0x0000..=0x1FFF => PpuTarget::Chr(banks.chr.translate(addr)),
      0x2000..=0x2FFF => {
        if self.chrrom_banked {
          PpuTarget::Chr(banks.chr.translate(addr))
        } else {
          PpuTarget::CiRam(banks.ciram.translate(addr))
        }
      }
      _ => unreachable!()
    }
  }
}