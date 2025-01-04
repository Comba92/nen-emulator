
use crate::{apu::Apu, bus::Bus, cart::{Cart, CartHeader}, cpu::Cpu, frame::FrameBuffer, joypad::{Joypad, JoypadButton}, ppu::Ppu};

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Nes {
  cpu: Cpu<Bus>,
}

impl Nes {
  pub fn boot_from_bytes(rom: &[u8]) -> Result<Self, String> {
    let cart = Cart::new(rom)?;
    Ok(Nes::boot_from_cart(cart))
  }

  pub fn boot_empty() -> Self {
    Self {
      cpu: Cpu::with_cart(Cart::empty()),
    }
  }

  pub fn step(&mut self) {
    self.get_cpu().step();
  }

  pub fn step_until_vblank(&mut self) {
    loop {
      if self.get_bus().poll_vblank() { break; }
      self.step();
    }
  }

  pub fn reset(&mut self) {
    self.get_cpu().reset();
    self.get_ppu().reset();
    self.get_apu().reset();
  }

  pub fn get_raw_screen(&self) -> *const u8 {
    self.cpu.bus.ppu.screen.0.buffer.as_ptr()
  }

  pub fn button_pressed(&mut self, button: u8) {
    self.get_joypad().buttons1.insert(JoypadButton::from_bits_retain(button));
  }

  pub fn button_released(&mut self, button: u8) {
    self.get_joypad().buttons1.remove(JoypadButton::from_bits_retain(button));
  }
}

impl Nes {
  pub fn boot_from_cart(cart: Cart) -> Self {
    Self {
      cpu: Cpu::with_cart(cart),
    }
  }

  pub fn load_rom_only(&mut self, cart: &Cart) {
    let mut curr_cart = self.get_bus().cart.borrow_mut();
    curr_cart.prg = cart.prg.clone();
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

  pub fn get_cart(&self) -> CartHeader {
    self.cpu.bus.cart.borrow().header.clone()
  }

  pub fn get_fps(&self) -> f32 {
    self.get_cart().timing.fps()
  }

  pub fn get_resolution(&mut self) -> (usize, usize) { (32*8, 30*8) }

  pub fn get_screen(&self) -> &FrameBuffer {
    &self.cpu.bus.ppu.screen.0
  }

  pub fn get_samples(&mut self) -> Vec<f32> {
    self.get_apu().get_samples()
  }

  pub fn get_joypad(&mut self) -> &mut Joypad {
    &mut self.cpu.bus.joypad
  }
}