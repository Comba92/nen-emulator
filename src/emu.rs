use std::path::Path;

use crate::{apu::Apu, bus::Bus, cart::{Cart, INesHeader}, cpu::Cpu, joypad::{Joypad, JoypadButton}, ppu::Ppu, frame::FrameBuffer};
use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen]
pub struct Emu {
  cpu: Cpu<Bus>,
  pub is_paused: bool,
}

#[wasm_bindgen]
impl Emu {
  pub fn from_bytes(rom: &[u8]) -> Result<Self, String> {
    let cart = Cart::new(rom)?;
    Ok(Emu::with_cart(cart))
  }

  pub fn empty() -> Self {
    Self {
      cpu: Cpu::with_cart(Cart::empty()),
      is_paused: true,
    }
  }

  pub fn load_from_bytes(&mut self, bytes: &[u8]) -> Result<(), String> {
    let cart = Cart::new(bytes)?;
    self.load_cart(cart);
    Ok(())
  }

  pub fn step(&mut self) {
    self.get_cpu().step();
  }

  pub fn step_until_vblank(&mut self) {
    loop {
      if self.is_paused { return; }
      if self.get_bus().poll_vblank() { break; }
      self.step();
    }
  }

  pub fn step_until_sample(&mut self) -> i16 {
    loop {
      if self.is_paused { return 0; }
      if let Some(sample) = self.get_bus().poll_sample() {
        return sample;
      }
      self.step();
    }
  }

  pub fn reset(&mut self) {
    self.cpu.reset();
    self.cpu.bus.ppu.reset();
    self.cpu.bus.apu.reset();
    self.is_paused = false;
  }

  pub fn get_raw_screen(&self) -> *const u8 {
    self.cpu.bus.ppu.screen.0.buffer.as_ptr()
  }

  pub fn button_pressed(&mut self, button: u8) {
    self.get_joypad().buttons.insert(JoypadButton::from_bits_retain(button));
  }

  pub fn button_released(&mut self, button: u8) {
    self.get_joypad().buttons.remove(JoypadButton::from_bits_retain(button));
  }
}

impl Emu {
  pub fn with_cart(cart: Cart) -> Self {
    Self {
      cpu: Cpu::with_cart(cart),
      is_paused: false,
    }
  }

  pub fn load_cart(&mut self, cart: Cart) {
    self.cpu.load_cart(cart);
    self.is_paused = false;
  }

  pub fn from_file(rom_path: &Path) -> Result<Self, String> {
    let cart = Cart::from_file(rom_path);
    match cart {
      Ok(cart) => Ok(Emu::with_cart(cart)),
      Err(msg) => Err(msg.to_string())
    }
  }

  pub fn get_bus(&mut self) -> &mut Bus {
    &mut self.cpu.bus
  }

  pub fn get_cpu(&mut self) -> &mut Cpu<Bus> {
    &mut self.cpu
  }

  pub fn get_ppu(&mut self) -> &mut Ppu {
    &mut self.cpu.bus.ppu
  }

  pub fn get_apu(&mut self) -> &mut Apu {
    &mut self.cpu.bus.apu
  }

  pub fn get_cart(&self) -> &INesHeader {
    &self.cpu.bus.cart
  }

  pub fn get_screen(&self) -> &FrameBuffer {
    &self.cpu.bus.ppu.screen.0
  }

  pub fn get_joypad(&mut self) -> &mut Joypad {
    &mut self.cpu.bus.joypad
  }
}