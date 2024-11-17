use std::path::Path;

use crate::{bus::Bus, cart::Cart, cpu::Cpu, render::FrameBuffer};

pub struct Nes {
  pub cpu: Cpu<Bus>,
  pub paused: bool,
}

impl Nes {
  pub fn new(cart: Cart) -> Self {
    Self {
      cpu: Cpu::new(cart),
      paused: false,
    }
  }

  pub fn empty() -> Self {
    Self {
      cpu: Cpu::new(Cart::empty()),
      paused: true,
    }
  }

  pub fn from_rom_path(rom_path: &Path) -> Result<Self, String> {
    let cart = Cart::new(rom_path);
    match cart {
      Ok(cart) => Ok(Nes::new(cart)),
      Err(msg) => Err(msg.to_string())
    }
  }

  pub fn step(&mut self) {
    let cycles_at_start = self.cpu.cycles;
    self.cpu.step();
    self.cpu.bus.step(self.cpu.cycles - cycles_at_start);
  }

  pub fn step_until_vblank(&mut self) {
    loop {
      if self.paused { break; }
      if self.cpu.bus.peek_vblank() { break; }
      self.step();
    }
  }

  pub fn reset(&mut self) {
    self.cpu.reset();
    self.cpu.bus.ppu.reset();
  }

  pub fn get_screen(&self) -> &FrameBuffer {
    &self.cpu.bus.ppu.screen.0
  }
}
