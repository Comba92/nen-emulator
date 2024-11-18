use std::path::Path;
use crate::{bus::Bus, cart::Cart, cpu::Cpu, joypad::Joypad, render::FrameBuffer};
use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen]
pub struct Emu {
  cpu: Cpu<Bus>,
  pub paused: bool,
}

#[wasm_bindgen]
impl Emu {
  pub fn from_bytes(rom: &[u8]) -> Self {
    let cart = Cart::new(rom);
    match cart {
      Ok(cart) => Emu::with_cart(cart),
      Err(_) => Emu::empty(),
    }
  }

  pub fn empty() -> Self {
    Self {
      cpu: Cpu::with_cart(Cart::empty()),
      paused: true,
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
}

impl Emu {
  pub fn with_cart(cart: Cart) -> Self {
    Self {
      cpu: Cpu::with_cart(cart),
      paused: false,
    }
  }

  pub fn from_file(rom_path: &Path) -> Result<Self, String> {
    let cart = Cart::from_file(rom_path);
    match cart {
      Ok(cart) => Ok(Emu::with_cart(cart)),
      Err(msg) => Err(msg.to_string())
    }
  }

  pub fn get_cpu(&mut self) -> &mut Cpu<Bus> {
    &mut self.cpu
  }

  pub fn get_screen(&self) -> &FrameBuffer {
    &self.cpu.bus.ppu.screen.0
  }

  pub fn get_joypad(&mut self) -> &mut Joypad {
    &mut self.cpu.bus.joypad
  }
}