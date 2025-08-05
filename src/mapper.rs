use crate::{bus::BankingHandler, cart::CartHeader};

pub trait Mapper {
  fn new(header: &CartHeader, banks: &mut BankingHandler) -> Box<Self> where Self: Sized;
  fn prg_write(&mut self, banks: &mut BankingHandler, addr: u16, val: u8);
  fn cpu_step(&mut self) {}
}

pub struct NROM;
impl Mapper for NROM {
  fn new(header: &CartHeader, banks: &mut BankingHandler) -> Box<Self> {    
    if header.prg_size <= 16 * 1024 {
      banks.prg.set_page(1, 0);
    } else {
      banks.prg.set_page(1, 1);
    }

    Box::new(Self)
  }

  fn prg_write(&mut self, _: &mut BankingHandler, _: u16, _: u8) {}
}