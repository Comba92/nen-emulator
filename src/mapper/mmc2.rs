use crate::{banks::{ChrBanking, MemConfig}, cart::{CartHeader, Mirroring}};

use super::{Banking, Mapper};

// Mapper 09 / 10
// https://www.nesdev.org/wiki/MMC2
// https://www.nesdev.org/wiki/MMC4 
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Clone, Copy, Default)]
enum Mmc2Latch { FD, #[default] FE }
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MMC2 {
  mapper: u16,
  chr_banks0: Banking<ChrBanking>,
  chr_banks1: Banking<ChrBanking>,
  latch0: Mmc2Latch,
  latch1: Mmc2Latch,
}

impl MMC2 {
  fn update_latches(&mut self, addr: u16) {
    match (addr, self.mapper) {
      (0x0FD8, 9) | (0x0FD8..=0x0FDF, 10) => self.latch0 = Mmc2Latch::FD,
      (0x0FE8, 9) | (0x0FE8..=0x0FEF, 10) => self.latch0 = Mmc2Latch::FE,
      (0x1FD8..=0x1FDF, _) => self.latch1 = Mmc2Latch::FD,
      (0x1FE8..=0x1FEF, _) => self.latch1 = Mmc2Latch::FE,
      _ => {}
    };
  }
}

#[cfg_attr(feature = "serde", typetag::serde)]
impl Mapper for MMC2 {
  fn new(header: &CartHeader, banks: &mut MemConfig)-> Box<Self> {
    let chr_banks0 = Banking::new_chr(header, 2);
    let chr_banks1 = Banking::new_chr(header, 2);
    
    match header.mapper {
      9 => {
        // MMC2 - Three 8 KB PRG ROM banks, fixed to the last three banks
        banks.prg = Banking::new_prg(header, 4);
        banks.prg.set_page(1, banks.prg.banks_count-3);
        banks.prg.set_page(2, banks.prg.banks_count-2);
        banks.prg.set_page(3, banks.prg.banks_count-1);
      }
      10 => {
        // MMC4 - 16 KB PRG ROM bank, fixed to the last bank
        banks.prg = Banking::new_prg(header, 2);
        banks.prg.set_page_to_last_bank(1);
      }
      _ => unreachable!(),
    };

    Box::new(Self{
      mapper: header.mapper,
      chr_banks0, chr_banks1,
      latch0: Mmc2Latch::FE, 
      latch1: Mmc2Latch::FE,
    })
  }

  fn prg_write(&mut self, banks: &mut MemConfig, addr: usize, val: u8) {
    let val = val as usize & 0b1_1111;
    
    match addr {
      0xA000..=0xAFFF => banks.prg.set_page(0, val & 0b1111),
      0xB000..=0xBFFF => self.chr_banks0.set_page(0, val),
      0xC000..=0xCFFF => self.chr_banks0.set_page(1, val),
      0xD000..=0xDFFF => self.chr_banks1.set_page(0, val),
      0xE000..=0xEFFF => self.chr_banks1.set_page(1, val),
      0xF000..=0xFFFF => {
          let mirroring = match val & 1 {
              0 => Mirroring::Vertical,
              _ => Mirroring::Horizontal,
          };
          banks.vram.update(mirroring);
      }
      _ => {}
    }
  }

  // fn map_ppu_addr_branching(&mut self, banks: &mut MemConfig, addr: usize) -> PpuTarget {
  //   let res = match addr {
  //     0x0000..=0x0FFF => PpuTarget::Chr(self.chr_banks0.page_to_bank_addr(self.latch0 as usize, addr)),
  //     0x1000..=0x1FFF => PpuTarget::Chr(self.chr_banks1.page_to_bank_addr(self.latch1 as usize, addr)),
  //     0x2000..=0x2FFF =>  PpuTarget::CiRam(banks.ciram.translate(addr)),
  //     _ => unreachable!()
  //   };

  //   // https://www.nesdev.org/wiki/MMC2#CHR_banking
  //   // https://www.nesdev.org/wiki/MMC4#Banks
  //   match (addr, self.mapper) {
  //     (0x0FD8, 9) | (0x0FD8..=0x0FDF, 10) => self.latch0 = Mmc2Latch::FD,
  //     (0x0FE8, 9) | (0x0FE8..=0x0FEF, 10) => self.latch0 = Mmc2Latch::FE,
  //     (0x1FD8..=0x1FDF, _) => self.latch1 = Mmc2Latch::FD,
  //     (0x1FE8..=0x1FEF, _) => self.latch1 = Mmc2Latch::FE,
  //     _ => {}
  //   };

  //   res
  // }

  fn chr_translate(&mut self, _: &mut MemConfig, addr: u16) -> usize {
    let res = match addr {
      0x0000..=0x0FFF => self.chr_banks0.page_to_bank_addr(self.latch0 as usize, addr as usize),
      0x1000..=0x1FFF => self.chr_banks1.page_to_bank_addr(self.latch1 as usize, addr as usize),
      _ => unreachable!()
    };

    self.update_latches(addr);
    res
  }

  fn vram_translate(&mut self, banks: &mut MemConfig, addr: u16) -> usize {
    let res = banks.vram.translate(addr as usize);
    self.update_latches(addr);
    res
  }
}