use crate::cart::{MemConfig, CartHeader, Mirroring, PrgTarget, PpuTarget};

use super::{Banking, CiramBanking, Mapper};

#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct Sunsoft4 {
  sram_enabled: bool,
  chrrom_banked: bool,
  chrrom_banks: Banking<CiramBanking>,
  mirroring: Mirroring,
  nametbl0: usize,
  nametbl1: usize,
  timer: usize,
}

impl Sunsoft4 {
  pub fn update_ciram_banks(&mut self) {
    match self.mirroring {
      Mirroring::Horizontal => {
        self.chrrom_banks.set_page(0, self.nametbl0);
        self.chrrom_banks.set_page(1, self.nametbl0);
        self.chrrom_banks.set_page(2, self.nametbl1);
        self.chrrom_banks.set_page(3, self.nametbl1);
      }
      Mirroring::Vertical => {
        self.chrrom_banks.set_page(0, self.nametbl0);
        self.chrrom_banks.set_page(1, self.nametbl1);
        self.chrrom_banks.set_page(2, self.nametbl0);
        self.chrrom_banks.set_page(3, self.nametbl1);
      }
      Mirroring::SingleScreenA => for i in 0..4 {
        self.chrrom_banks.set_page(i, self.nametbl0);
      }
      Mirroring::SingleScreenB => for i in 0..4 {
        self.chrrom_banks.set_page(i, self.nametbl1);
      }
      _ => {}
    }
  }
}

#[typetag::serde]
impl Mapper for Sunsoft4 {
  fn new(header: &CartHeader, banks: &mut MemConfig) -> Box<Self> {
    banks.prg = Banking::new_prg(header, 2);
    banks.prg.set_page_to_last_bank(1);

    banks.chr = Banking::new_chr(header, 4);
    let chrrom_banks = Banking::new(header.chr_real_size(), 0, 1024, 4);

    Box::new(Self{ 
      chrrom_banks,
      mirroring: header.mirroring,
      ..Default::default()
    })
  }

  fn prg_write(&mut self, banks: &mut MemConfig, addr: usize, val: u8) {
    match addr {
      0x8000..=0xBFFF => {
        let page = (addr - 0x8000) / 0x1000;
        banks.chr.set_page(page, val as usize);
      }
      0xC000..=0xCFFF => {
        // Only D6-D0 are used; D7 is ignored and treated as 1, 
        // so nametables must be in the last 128 KiB of CHR ROM.
        self.nametbl0 = val as usize | 0b1000_0000;
        self.update_ciram_banks();
      }
      0xD000..=0xDFFF => {
        self.nametbl1 = val as usize | 0b1000_0000;
        self.update_ciram_banks();
      }
      0xE000..=0xEFFF => {
        self.mirroring = match val & 0b11 {
          0 => Mirroring::Vertical,
          1 => Mirroring::Horizontal,
          2 => Mirroring::SingleScreenA,
          _ => Mirroring::SingleScreenB,
        };
        banks.ciram.update(self.mirroring);

        self.chrrom_banked = val >> 4 != 0;
      }
      0xF000..=0xFFFF => {
        banks.prg.set_page(0, val as usize & 0b1111);
        self.sram_enabled = (val >> 4) & 1 != 0;
      }
      _ => {}
    }
  }

  fn map_prg_addr(&mut self, banks: &mut MemConfig, addr: usize) -> PrgTarget {
    match addr {
      0x6000..=0x7FFF => PrgTarget::SRam(self.sram_enabled, banks.sram.translate(addr)),
      0x8000..=0xFFFF => PrgTarget::Prg(banks.prg.translate(addr)),
      _ => unreachable!()
    }
  }

  fn map_ppu_addr(&mut self, banks: &mut MemConfig, addr: usize) -> PpuTarget {
    match addr {
      0x0000..=0x1FFF => PpuTarget::Chr(banks.chr.translate(addr)),
      0x2000..=0x2FFF => {
        if self.chrrom_banked {
          PpuTarget::Chr(self.chrrom_banks.translate(addr))
        } else {
          PpuTarget::CiRam(banks.ciram.translate(addr))
        }
      }
      _ => unreachable!()
    }
  }
}