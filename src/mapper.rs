use crate::{bus::BankingHandler, cart::CartHeader};

pub trait Mapper {
  fn new(header: &CartHeader, banks: &mut BankingHandler) -> Box<Self> where Self: Sized;
  fn prg_write(&mut self, banks: &mut BankingHandler, addr: u16, val: u8);
  fn step(&mut self) {}
}

pub fn mapper_from_header(header: &CartHeader, banks: &mut BankingHandler) -> Result<Box<dyn Mapper>, String> {
  let mapper: Box<dyn Mapper> = match header.mapper {
    0 => NROM::new(header, banks),
    2 => UxROM::new(header, banks),
    3 => CNROM::new(header, banks),
    _ => return Err(format!("Mapper {} not implemented", header.mapper)),
  };

  Ok(mapper)
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

pub struct UxROM; 
impl Mapper for UxROM {
  fn new(_: &CartHeader, banks: &mut BankingHandler) -> Box<Self> {
    banks.prg.set_page_to_last_bank(1);
    Box::new(Self)
  }

  fn prg_write(&mut self, banks: &mut BankingHandler, addr: u16, val: u8) {
    banks.prg.set_page(0, val as usize);
  }
}

pub struct CNROM;
impl Mapper for CNROM {
  fn new(header: &CartHeader, banks: &mut BankingHandler) -> Box<Self> {
    Box::new(Self)    
  }

  fn prg_write(&mut self, banks: &mut BankingHandler, addr: u16, val: u8) {
    banks.chr.set_page(0, val as usize);
  }
}