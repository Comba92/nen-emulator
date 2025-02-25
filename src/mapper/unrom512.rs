use crate::cart::{CartBanking, CartHeader, Mirroring};

use super::{Banking, Mapper};


// Mapper 30
// https://www.nesdev.org/wiki/UNROM_512
#[derive(serde::Serialize, serde::Deserialize)]
pub struct UNROM512;

#[typetag::serde]
impl Mapper for UNROM512 {
  fn new(header: &CartHeader, banks: &mut CartBanking) -> Box<Self> {
    banks.prg = Banking::new_prg(header, 2);
    banks.prg.set_page_to_last_bank(1);
    banks.chr = Banking::new_chr(header, 1);

    Box::new(Self)
  }

  fn prg_write(&mut self, banks: &mut CartBanking, _: usize, val: u8) {
    banks.prg.set_page(0, val as usize & 0b1_1111);
    banks.chr.set_page(0, (val >> 5) as usize & 0b11);
    let mirroring = match (val >> 7) != 0 {
      false => Mirroring::SingleScreenA,
      true  => Mirroring::SingleScreenB,
    };
    banks.ciram.update(mirroring);
  }
}
