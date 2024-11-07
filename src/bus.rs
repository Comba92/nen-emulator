#[allow(unused_imports)]
use log::{debug, info, trace, warn};

use crate::{cart::Cart, ppu::Ppu};

#[derive(Debug)]
pub enum BusDst {
  Ram, Ppu, SRam, Prg, NoImpl
}

pub struct Bus {
  pub mem: [u8; 0x10000],
  pub cart: Cart,
  pub ppu: Ppu,

  pub ppu_enabled: bool,
}

impl Bus {
  pub fn new(cart: Cart) -> Self {
    let mut bus = Self {
      mem: [0; 0x10000], ppu: Ppu::new(&cart), cart,
      ppu_enabled: true,
    };
    bus.write_data(0x8000,&bus.cart.prg_rom.clone());
    bus.write_data(0xC000,&bus.cart.prg_rom.clone());

    bus
  }

  pub fn step(&mut self, cycles: usize) {
    if self.ppu_enabled {
      self.ppu.step(cycles * 3);
    }
  }

  pub fn poll_nmi(&mut self) -> bool {
    let nmi = self.ppu.nmi_requested;
    self.ppu.nmi_requested = false;
    nmi
  }
  pub fn poll_irq(&mut self) -> bool { false }

  pub fn map(&self, addr: u16) -> (BusDst, u16) {
    match addr {
      0..=0x1FFF => {
        let ram_addr = addr & 0x07FF;
        (BusDst::Ram, ram_addr)
      }
      0x2000..=0x3FFF => {
        let ppu_addr = addr & 0x2007;
        (BusDst::Ppu, ppu_addr)
      }
      0x6000..=0x7FFF => (BusDst::SRam, addr),
      0x8000..=0xFFFF => (BusDst::Prg, addr),
      _ => (BusDst::NoImpl, 0)
    }
  }

  pub fn __read(&mut self, addr: u16) -> u8 {
    self.mem[addr as usize]  
  }

  pub fn __write(&mut self, addr: u16, val: u8) {
    self.mem[addr as usize] = val;
  }

  pub fn read(&mut self, addr: u16) -> u8 {
    trace!("READ {:?}: {:04X} -> {:04X}", self.map(addr).0, addr, self.map(addr).1);
  
    if (0..=0x1FFF).contains(&addr) {
      let ram_addr = addr & 0x07FF;
      self.mem[ram_addr as usize]
    } else if (0x2000..=0x3FFF).contains(&addr) {
      info!("READ {:?}: {:04X} -> {:04X}", self.map(addr).0, addr, self.map(addr).1);
      let ppu_addr = addr & 0x2007;
      self.ppu.reg_read(ppu_addr)
    } else if (0x6000..=0x7FFF).contains(&addr) {
      self.mem[addr as usize]
    } else if (0x8000..=0xFFFF).contains(&addr) {
      self.mem[addr as usize]
    } else { debug!("Unimplemented memory read at {addr:04X}"); 0 }
  }

  pub fn read16(&mut self, addr: u16) -> u16 {
    let low = self.read(addr);
    let high = self.read(addr.wrapping_add(1));
    u16::from_le_bytes([low, high])
  }

  pub fn write(&mut self, addr: u16, val: u8) {
    trace!("WRITE {:?}: {:04X} -> {:04X}", self.map(addr).0, addr, self.map(addr).1);
    if (0..=0x1FFF).contains(&addr) {
      let ram_addr = addr & 0x07FF;
      self.mem[ram_addr as usize] =  val;
    } else if (0x2000..=0x3FFF).contains(&addr) {
      let ppu_addr = addr & 0x2007;
      self.ppu.reg_write(ppu_addr, val);
    } else if addr == 0x4104 {
      info!("PPU_OAM_DMA WRITE {addr:04X} = {val:02X}");
      let start = (val as u16) << 8;
      for i in 0..256 {
        self.ppu.oam[i] = self.read(start + i as u16);
      }
    } else if (0x6000..=0x7FFF).contains(&addr) {
      self.mem[addr as usize] = val;
    } else if (0x8000..=0xFFFF).contains(&addr) {
      warn!("Can't write to PRG")
      // self.mem[addr as usize] = val;
    }
    else { debug!("Unimplemented memory write at {addr:04X}"); self.mem[addr as usize] = val; }
  }

  pub fn write16(&mut self, addr: u16, val: u16) {
    let [low, high] = val.to_le_bytes();
    self.write(addr, low);
    self.write(addr.wrapping_add(1), high);
  }

  pub fn write_data(&mut self, start: u16, data: &[u8]) {
    // for (i , byte) in data.iter().enumerate() {
    //   self.write(start + i as u16, *byte);
    // }
    let (left, _) = self.mem[start as usize..].split_at_mut(data.len());
    left.copy_from_slice(data);
  }
}

#[cfg(test)]
mod bus_tests {
  use std::path::Path;

use super::*;

  #[test]
  fn ram_read() {
    let mut bus = Bus::new(Cart::empty());
    bus.mem[0..0x800].fill(0xFF);
    bus.mem[0x6000..0x8000].fill(0xFF);

    for i in 0..0x2000 {
      assert_eq!(0xFF, bus.read(i), 
        "RAM Read from {i:04X} mirrored to {:04X}", i & 0x1FFF
      );
    }

    for i in 0x6000..0x8000 {
      assert_eq!(0xFF, bus.read(i), 
        "SRAM Read from {i:04X}"
      );
    }
  }

  #[test]
  fn ram_write() {
    let mut bus = Bus::new(Cart::empty());

    for i in 0..0x2000 {
      bus.write(i, 0xFF);
    }
    for i in 0x6000..0x8000 {
      bus.write(i, 0xFF);
    }

    for i in 0..0x1FFF {
      assert_eq!(0xFF, bus.read(i), 
        "Read from {i:04X} mirrored to {:04X}", i & 0x1FFF
      );
    }

    for i in 0x6000..0x8000 {
      assert_eq!(0xFF, bus.read(i), 
        "SRAM Read from {i:04X}"
      );
    }
  }

  #[test]
  fn prg_read() {
    let mut bus = Bus::new(Cart::new(&Path::new("./tests/nestest.nes")));
    
    let mut empty_bytes = 0;
    for i in 0x8000..0x8000+bus.cart.prg_rom.len() {
      empty_bytes += if bus.read(i as u16) != 0 { 1 } else { 0 }
    }
    assert_ne!(0, empty_bytes, "PRG ROM is empty")
  }

  #[test]
  fn ppu_regs() {
    let mut bus = Bus::new(Cart::empty());
    bus.ppu.vram[0x2000..=0x2FFF].fill(0xFF);

    colog::init();

    bus.write(0x2BFE, 0x20);
    bus.write(0x2BFE, 0x25);
    assert_eq!(0, bus.read(0x2BFF));
    assert_eq!(0xFF, bus.read(0x2BFF));
  }
}