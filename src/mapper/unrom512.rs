use crate::{
  banks::MemConfig,
  cart::{CartHeader, Mirroring},
};

use super::{Banking, Mapper};

// Mapper 30
// https://www.nesdev.org/wiki/UNROM_512
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct UNROM512;

#[cfg_attr(feature = "serde", typetag::serde)]
impl Mapper for UNROM512 {
  fn new(header: &CartHeader, banks: &mut MemConfig) -> Box<Self> {
    banks.prg = Banking::new_prg(header, 2);
    banks.prg.set_page_to_last_bank(1);
    banks.chr = Banking::new_chr(header, 1);

    Box::new(Self)
  }

  fn prg_write(&mut self, banks: &mut MemConfig, _: usize, val: u8) {
    banks.prg.set_page(0, val as usize & 0b1_1111);
    banks.chr.set_page(0, (val >> 5) as usize & 0b11);
    let mirroring = match (val >> 7) != 0 {
      false => Mirroring::SingleScreenA,
      true => Mirroring::SingleScreenB,
    };
    banks.vram.update(mirroring);
  }
}
