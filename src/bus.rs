use log::debug;
use crate::{cart::{Cart, CartHeader}, joypad::Joypad, mapper::CartMapper, mem::Memory, ppu::Ppu};

#[derive(Debug)]
enum BusDst {
  Ram, Ppu, Dma, SRam, Prg, Joypad1, Joypad2, NoImpl
}

pub struct Bus {
  ram: [u8; 0x800],
  sram: [u8; 0x2000],
  prg: Vec<u8>,
  pub cart: CartHeader,
  pub mapper: CartMapper,
  pub ppu: Ppu,
  pub joypad: Joypad,
}

impl Memory for Bus {
  fn read(&mut self, addr: u16) -> u8 {
    let (dst, addr) = self.map_address(addr);
    match dst {
      BusDst::Ram => self.ram[addr],
      BusDst::Ppu => self.ppu.reg_read(addr as u16),
      BusDst::Joypad1 => self.joypad.read(),
      BusDst::SRam => self.sram[addr],
      BusDst::Prg => self.mapper.borrow().read_prg(&self.prg, addr),
      _ => { debug!("Read to {addr:04X} not implemented"); 0 }
    }
  }

  fn write(&mut self, addr: u16, val: u8) {
    let (dst, addr) = self.map_address(addr);
    match dst {
      BusDst::Ram => self.ram[addr] = val,
      BusDst::Ppu => self.ppu.reg_write(addr as u16, val),
      BusDst::Dma => {
        let mut page = [0; 256];
        let start = (val as u16) << 8;
        for offset in 0..256 {
          page[offset] = self.read(
            start.wrapping_add(offset as u16)
          );
        }
        self.ppu.oam_dma(&page);

        // TODO: write to OAM_DATA instead of manually writing oam
        // TODO: this takes 513 CPU cycles, CPU is stalled during the transfer
      }
      BusDst::Joypad1 => self.joypad.write(val),
      BusDst::Joypad2 => {} // TODO: second joypad
      BusDst::SRam => self.sram[addr] = val,
      BusDst::Prg => self.mapper.borrow_mut().write_prg(addr, val),
      BusDst::NoImpl => debug!("Write to {addr:04X} not implemented")
    }
  }

  fn poll_nmi(&mut self) -> bool {
    self.ppu.nmi_requested.take().is_some()
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
      cart: cart.header,
      prg: cart.prg_rom,
      mapper: cart.mapper,
      joypad: Joypad::new(),
    }
  }

  fn map_address(&self, addr: u16) -> (BusDst, usize) {
    match addr {
      0x0000..=0x1FFF => {
        let ram_addr = addr & 0x07FF;
        (BusDst::Ram, ram_addr as usize)
      }
      0x2000..=0x3FFF => {
        let ppu_addr = addr & 0x2007;
        (BusDst::Ppu, ppu_addr as usize)
      }
  
      0x4014 => (BusDst::Dma, addr as usize),
      0x4016 => (BusDst::Joypad1, addr as usize),
      0x4017 => (BusDst::Joypad2, addr as usize),
  
      0x6000..=0x7FFF => (BusDst::SRam, addr as usize - 0x6000),
      0x8000..=0xFFFF => (BusDst::Prg, addr as usize - 0x8000),
      _ => (BusDst::NoImpl, addr as usize)
    }
  }

  pub fn step(&mut self, cycles: usize) {
    for _ in 0..cycles*3 { self.ppu.step_accurate(); }
  }
  
  pub fn peek_vblank(&mut self) -> bool {
    self.ppu.vblank_started.take().is_some()
  }
}

#[cfg(test)]
mod bus_tests {
  use std::path::Path;

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
    let mut bus = Bus::new(Cart::new(&Path::new("./tests/nestest.nes")).unwrap());
    
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