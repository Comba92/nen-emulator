use crate::cart::Mirroring;

use super::{Bank, Mapper};

// Mapper 71
// https://www.nesdev.org/wiki/INES_Mapper_071

#[derive(Default, serde::Serialize, serde::Deserialize)]
pub struct INesMapper071 {
  prg_bank_select: Bank,
  // https://www.nesdev.org/wiki/INES_Mapper_071#Mirroring_($8000-$9FFF)
  mirroring: Option<Mirroring>,
}

#[typetag::serde]
impl Mapper for INesMapper071 {
  fn prg_write(&mut self, _prg: &mut[u8], addr: usize, val: u8) {
    match addr {
      0x9000..=0x9FFF => self.mirroring = match (val >> 4) & 1 != 0 {
        false => Some(Mirroring::SingleScreenA),
        true  => Some(Mirroring::SingleScreenB),
      },
      0xC000..=0xFFFF => self.prg_bank_select = val as usize & 0b1111,
      _ => {}
    }
  }

  fn prg_addr(&self, prg: &[u8], addr: usize) -> usize {
    match addr {
      0x8000..=0xBFFF => self.prg_bank_addr(prg, self.prg_bank_select, addr),
      0xC000..=0xFFFF => self.prg_bank_addr(prg, self.prg_last_bank(prg), addr),
      _ => unreachable!()
    }
  }

  fn mirroring(&self) -> Option<Mirroring> {
    self.mirroring
  }
}