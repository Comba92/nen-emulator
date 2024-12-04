use log::debug;
use crate::{apu::Apu, cart::{Cart, INesHeader}, joypad::Joypad, mapper::CartMapper, mem::Memory, ppu::Ppu};

#[derive(Debug)]
enum BusDst {
  Ram, Ppu, Apu, SRam, Prg, Joypad1, Joypad2, NoImpl
}

pub struct Bus {
  ram: [u8; 0x800],
  sram: [u8; 0x2000],
  prg: Vec<u8>,
  pub cart: INesHeader,
  pub mapper: CartMapper,
  pub ppu: Ppu,
  pub apu: Apu,
  pub joypad: Joypad,
}
// TODO: consider moving VRAM here

impl Memory for Bus {
  fn read(&mut self, addr: u16) -> u8 {
    let (dst, addr) = self.map_address(addr);
    match dst {
      BusDst::Ram => self.ram[addr],
      BusDst::Ppu => self.ppu.read_reg(addr as u16),
      BusDst::Apu => self.apu.read_reg(addr as u16),
      BusDst::Joypad1 => self.joypad.read(),
      BusDst::SRam => self.sram[addr],
      BusDst::Prg => self.mapper.borrow_mut().read_prg(&self.prg, addr),
      _ => { debug!("Read to {addr:04X} not implemented"); 0 }
    }
  }

  fn write(&mut self, addr: u16, val: u8) {
    let (dst, addr) = self.map_address(addr);
    match dst {
      BusDst::Ram => self.ram[addr] = val,
      BusDst::Ppu => self.ppu.write_reg(addr as u16, val),
      BusDst::Apu => self.apu.write_reg(addr as u16, val),
      BusDst::Joypad1 => self.joypad.write(val),
      BusDst::Joypad2 => {} // TODO: second joypad
      BusDst::SRam => self.sram[addr] = val,
      BusDst::Prg => self.mapper.borrow_mut().write_prg(&mut self.prg, addr, val),
      BusDst::NoImpl => debug!("Write to {addr:04X} not implemented")
    }
  }

  fn poll_nmi(&mut self) -> bool {
    self.ppu.nmi_requested.take().is_some()
  }

  fn poll_irq(&mut self) -> bool {
    self.mapper.borrow_mut().poll_irq()
    || self.apu.frame_irq_requested.take().is_some()
  }
  
  fn tick(&mut self) {
    for _ in 0..3 { self.ppu.step(); }
    self.apu.step();
  }
}

impl Bus {
  pub fn new(cart: Cart) -> Self {
    let ppu = Ppu::new(
      cart.chr_rom,
      cart.mapper.clone(),
      cart.header.nametbl_mirroring
    );

    Self {
      ram: [0; 0x800], 
      sram: [0; 0x2000],
      ppu, 
      apu: Apu::new(),
      cart: cart.header,
      prg: cart.prg_rom,
      mapper: cart.mapper,
      joypad: Joypad::new(),
    }
  }

  fn map_address(&self, addr: u16) -> (BusDst, usize) {
    match addr {
      0x0000..=0x1FFF => (BusDst::Ram, addr as usize & 0x07FF),
      0x2000..=0x3FFF => (BusDst::Ppu, addr as usize & 0x2007),
      0x4000..=0x4013 | 0x4015 | 0x4017 => (BusDst::Apu, addr as usize),
      0x4016 => (BusDst::Joypad1, addr as usize),
      // 0x4017 => (BusDst::Joypad2, addr as usize),
      0x6000..=0x7FFF => (BusDst::SRam, addr as usize - 0x6000),
      // We pass it as is to the mapper, for convenience
      0x8000..=0xFFFF => (BusDst::Prg, addr as usize),
      _ => (BusDst::NoImpl, addr as usize)
    }
  }
    
  pub fn poll_vblank(&mut self) -> bool {
    self.ppu.vblank_started.take().is_some()
  }

  pub fn poll_sample(&mut self) -> Option<i16> {
    self.apu.current_sample.take()
  }
}

#[cfg(test)]
mod bus_tests {
  use std::fs;
  use super::*;

  #[test]
  fn ram_read() {
    let mut bus = Bus::new(Cart::empty());
    bus.ram[0..0x800].fill(0xFF);
    bus.sram[0x6000..0x8000].fill(0xFF);

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
    let rom = fs::read("./tests/nestest.nes").unwrap();
    let mut bus = Bus::new(Cart::new(&rom).unwrap());
    
    let mut empty_bytes = 0;
    for i in 0x8000..0x8000+bus.cart.prg_size {
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