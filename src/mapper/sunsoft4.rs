use crate::{cart::{CartHeader, MemConfig, Mirroring, PpuTarget, PrgTarget}, ppu::Ppu};

use super::{Banking, Mapper};

// Mapper 68
// https://www.nesdev.org/wiki/INES_Mapper_068
#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct Sunsoft4 {
  sram_enabled: bool,
  chrrom_nametbls: bool,
  
  // TODO: we probably dont need these, just use the ciram banking in cfg
  ciram_banks:  Banking,
  chrrom_banks: Banking,

  mirroring: Mirroring,
  nametbl0: usize,
  nametbl1: usize,
  timer: usize,
}

impl Sunsoft4 {
  pub fn update_chrrom_banks(&mut self) {
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
  fn new(header: &CartHeader, cfg: &mut MemConfig) -> Box<Self> {
    cfg.prg = Banking::new_prg(header, 2);
    cfg.prg.set_page_to_last_bank(1);

    cfg.chr = Banking::new_chr(header, 4);
    let chrrom_banks = Banking::new(header.chr_real_size(), 0x2000, 1024, 4);
    let ciram_banks  = Banking::new_ciram(header);

    Box::new(Self{ 
      chrrom_banks,
      ciram_banks,

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
          banks.ciram = self.chrrom_banks.clone();
        }
      }
      0xD000..=0xDFFF => {
        self.nametbl1 = val as usize | 0b1000_0000;
        self.update_chrrom_banks();
        if self.chrrom_nametbls {
          banks.ciram = self.chrrom_banks.clone();
        }
      }
      0xE000..=0xEFFF => {
        self.mirroring = match val & 0b11 {
          0 => Mirroring::Vertical,
          1 => Mirroring::Horizontal,
          2 => Mirroring::SingleScreenA,
          _ => Mirroring::SingleScreenB,
        };
        self.ciram_banks.update(self.mirroring);

        self.chrrom_nametbls = val >> 4 != 0;

        if self.chrrom_nametbls {
          banks.ciram = self.chrrom_banks.clone();

          banks.mapping.ppu_reads[2]  = |ppu: &Ppu, addr: u16| {
            let cart = ppu.cart.as_mut();
            cart.chr[cart.mapper.ciram_translate(&mut cart.cfg, addr)]
          };
          banks.mapping.ppu_writes[2] = |ppu: &mut Ppu, addr: u16, val: u8| {
            let cart = ppu.cart.as_mut();
            cart.chr[cart.mapper.ciram_translate(&mut cart.cfg, addr)] = val;
          };
        } else {
          banks.ciram = self.ciram_banks.clone();

          banks.mapping.ppu_reads[2]  = Ppu::ciram_read;
          banks.mapping.ppu_writes[2] = Ppu::ciram_write;
        }
      }
      0xF000..=0xFFFF => {
        banks.prg.set_page(0, val as usize & 0b1111);
        self.sram_enabled = (val >> 4) & 1 != 0;
      }
      _ => {}
    }
  }

  fn map_prg_addr_branching(&mut self, banks: &mut MemConfig, addr: usize) -> PrgTarget {
    match addr {
      0x6000..=0x7FFF => PrgTarget::SRam(self.sram_enabled, banks.sram.translate(addr)),
      0x8000..=0xFFFF => PrgTarget::Prg(banks.prg.translate(addr)),
      _ => unreachable!()
    }
  }

  fn map_ppu_addr_branching(&mut self, banks: &mut MemConfig, addr: usize) -> PpuTarget {
    match addr {
      0x0000..=0x1FFF => PpuTarget::Chr(banks.chr.translate(addr)),
      0x2000..=0x2FFF => {
        if self.chrrom_nametbls {
          PpuTarget::Chr(self.chrrom_banks.translate(addr))
        } else {
          PpuTarget::CiRam(banks.ciram.translate(addr))
        }
      }
      _ => unreachable!()
    }
  }
}