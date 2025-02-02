use crate::cart::{CartBanking, CartHeader, PpuTarget, PrgTarget};

use super::{Banking, Mapper};

// Mapper 111
// https://www.nesdev.org/wiki/GTROM
#[derive(serde::Serialize, serde::Deserialize)]
pub struct GTROM;
impl GTROM {
  fn write(&mut self, banks: &mut CartBanking, val: u8) {
    banks.prg.set_page(0, val as usize & 0b1111);
    banks.chr.set_page(0, (val >> 4) as usize & 1);
    // The nametables can select between the last two 8KiB of the PPU RAM
    banks.ciram.set_page(0, ((val >> 5) as usize & 1) + 2);
  }
}

#[typetag::serde]
impl Mapper for GTROM {
  fn new(header: &CartHeader, banks: &mut CartBanking) -> Box<Self> {
    banks.prg = Banking::new_prg(header, 1);
    banks.chr = Banking::new_chr(header, 1);
    banks.ciram = Banking::new(header.chr_real_size(), 0x2000, 8*1024, 1);

    Box::new(Self)
  }

  fn prg_write(&mut self, banks: &mut CartBanking, addr: usize, val: u8) {
    if (0x7000..=0x7FFF).contains(&addr) {
      self.write(banks, val);
    }
  }
  fn cart_write(&mut self, banks: &mut CartBanking, addr: usize, val:u8) {
    if (0x5000..=0x5FFF).contains(&addr) {
      self.write(banks, val);
    }
  }

  fn map_prg_addr(&mut self, banks: &mut CartBanking, addr: usize) -> PrgTarget {
    match addr {
      0x6000..=0x7FFF => PrgTarget::Prg(addr),
      0x8000..=0xFFFF => PrgTarget::Prg(banks.prg.translate(addr)),
      _ => unreachable!()
    }
  }

  fn map_ppu_addr(&mut self, banks: &mut CartBanking, addr: usize) -> PpuTarget {
    match addr {
      0x0000..=0x1FFF => PpuTarget::Chr(banks.chr.translate(addr)),
      // this thing uses the vram mirrors as additional ram
      0x2000..=0x3FFF => PpuTarget::Chr(banks.ciram.translate(addr)),
      _ => unreachable!()
    }
  }
}
