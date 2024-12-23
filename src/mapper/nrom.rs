use super::{Mapper, ROM_START};

// Mapper 0 https://www.nesdev.org/wiki/NROM
pub struct NRom;
impl Mapper for NRom {
  fn prg_addr(&self, prg: &[u8], addr: usize) -> usize {
    // if it only has 16KiB, then mirror to first bank
    if prg.len() == self.prg_bank_size() { 
      self.prg_bank_addr(prg, 0, addr)
    }
    else { addr - ROM_START }
  }

  fn prg_write(&mut self, _prg: &mut[u8], _addr: usize, _val: u8) {}
}