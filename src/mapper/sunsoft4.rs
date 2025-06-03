use crate::{banks::{MemConfig, VramBanking}, cart::{CartHeader, Mirroring}, mem};

use super::{Banking, Mapper};

// Mapper 68
// https://www.nesdev.org/wiki/INES_Mapper_068
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Default)]
pub struct Sunsoft4 {
  sram_enabled: bool,
  chrrom_nametbls: bool,
  
  // TODO: we probably dont need these, just use the ciram banking in cfg
  vram_ciram_banks:  Banking<VramBanking>,
  vram_chrrom_banks: Banking<VramBanking>,

  mirroring: Mirroring,
  nametbl0: usize,
  nametbl1: usize,
}

impl Sunsoft4 {
  pub fn update_chrrom_banks(&mut self) {
    match self.mirroring {
      Mirroring::Horizontal => {
        self.vram_chrrom_banks.set_page(0, self.nametbl0);
        self.vram_chrrom_banks.set_page(1, self.nametbl0);
        self.vram_chrrom_banks.set_page(2, self.nametbl1);
        self.vram_chrrom_banks.set_page(3, self.nametbl1);
      }
      Mirroring::Vertical => {
        self.vram_chrrom_banks.set_page(0, self.nametbl0);
        self.vram_chrrom_banks.set_page(1, self.nametbl1);
        self.vram_chrrom_banks.set_page(2, self.nametbl0);
        self.vram_chrrom_banks.set_page(3, self.nametbl1);
      }
      Mirroring::SingleScreenA => for i in 0..4 {
        self.vram_chrrom_banks.set_page(i, self.nametbl0);
      }
      Mirroring::SingleScreenB => for i in 0..4 {
        self.vram_chrrom_banks.set_page(i, self.nametbl1);
      }
      _ => {}
    }
  }
}

#[cfg_attr(feature = "serde", typetag::serde)]
impl Mapper for Sunsoft4 {
  fn new(header: &CartHeader, cfg: &mut MemConfig) -> Box<Self> {
    cfg.prg = Banking::new_prg(header, 2);
    cfg.prg.set_page_to_last_bank(1);

    cfg.chr = Banking::new_chr(header, 4);
    let vram_chrrom_banks = Banking::new(header.chr_real_size(), 0x2000, 1024, 4);
    let vram_ciram_banks  = Banking::new_vram(header);

    Box::new(Self{ 
      vram_chrrom_banks,
      vram_ciram_banks,

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
        self.update_chrrom_banks();
        if self.chrrom_nametbls {
          banks.vram = self.vram_chrrom_banks.clone();
        }
      }
      0xD000..=0xDFFF => {
        self.nametbl1 = val as usize | 0b1000_0000;
        self.update_chrrom_banks();
        if self.chrrom_nametbls {
          banks.vram = self.vram_chrrom_banks.clone();
        }
      }
      0xE000..=0xEFFF => {
        self.mirroring = match val & 0b11 {
          0 => Mirroring::Vertical,
          1 => Mirroring::Horizontal,
          2 => Mirroring::SingleScreenA,
          _ => Mirroring::SingleScreenB,
        };
        self.vram_ciram_banks.update(self.mirroring);

        self.chrrom_nametbls = val >> 4 != 0;

        if self.chrrom_nametbls {
          banks.vram = self.vram_chrrom_banks.clone();
          banks.mapping.set_vram_handlers(mem::chr_from_vram_read, mem::chr_from_vram_write);
        } else {
          banks.vram = self.vram_ciram_banks.clone();
          banks.mapping.set_vram_handlers(mem::vram_read, mem::vram_write);
        }
      }
      0xF000..=0xFFFF => {
        banks.prg.set_page(0, val as usize & 0b1111);
        self.sram_enabled = (val >> 4) & 1 != 0;
      }
      _ => {}
    }
  }

  // fn map_ppu_addr_branching(&mut self, banks: &mut MemConfig, addr: usize) -> PpuTarget {
  //   match addr {
  //     0x0000..=0x1FFF => PpuTarget::Chr(banks.chr.translate(addr)),
  //     0x2000..=0x2FFF => {
  //       if self.chrrom_nametbls {
  //         PpuTarget::Chr(self.vram_chrrom_banks.translate(addr))
  //       } else {
  //         PpuTarget::CiRam(banks.ciram.translate(addr))
  //       }
  //     }
  //     _ => unreachable!()
  //   }
  // }
}