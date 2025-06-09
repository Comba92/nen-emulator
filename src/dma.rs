use std::u16;

pub trait Dma: Default {
  fn current(&mut self) -> u16;
  fn is_transfering(&self) -> bool;
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Default)]
pub struct OamDma {
  pub start: u16,
  pub offset: u16,
}

impl OamDma {
  pub fn init(&mut self, start: u8) {
    self.start = (start as u16) << 8;
    self.offset = 256;
  }
}

impl Dma for OamDma {
  fn current(&mut self) -> u16 {
    let res = self.start.wrapping_add(256 - self.offset);
    self.offset -= 1;
    res
  }

  fn is_transfering(&self) -> bool {
    self.offset > 0
  }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Default)]
pub struct DmcDma {
  pub addr: u16,
  pub remaining: u16,
}

impl DmcDma {
  pub fn init(&mut self, addr: u16, length: u16) {
    self.addr = addr;
    self.remaining = length;
  }
}

impl Dma for DmcDma {
  fn current(&mut self) -> u16 {
    let res = self.addr;
    let (addr, overflow) = self.addr.overflowing_add(1);
    self.addr = if overflow { 0x8000 } else { addr };
    self.remaining -= 1;
    res
  }

  fn is_transfering(&self) -> bool {
    self.remaining > 0
  }
}
