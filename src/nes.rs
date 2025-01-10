use crate::{apu::Apu, bus::Bus, cart::{Cart, CartHeader}, cpu::Cpu, frame::FrameBuffer, joypad::{Joypad, JoypadButton}, ppu::Ppu};
use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct Nes {
  cpu: Cpu<Bus>,
}

#[wasm_bindgen]
impl Nes {
  pub fn boot_from_bytes(rom: &[u8]) -> Result<Self, String> {
    let cart = Cart::new(rom)?;
    Ok(Nes::boot_from_cart(cart))
  }

  pub fn boot_empty() -> Self {
    Self {
      cpu: Cpu::with_cart(Cart::default()),
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

  pub fn get_raw_samples(&mut self) -> *const f32 {
    self.get_apu().get_samples().as_ptr()
  }

  pub fn button_pressed(&mut self, button: u8) {
    self.get_joypad().buttons1.insert(JoypadButton::from_bits_retain(button));
  }

  pub fn button_released(&mut self, button: u8) {
    self.get_joypad().buttons1.remove(JoypadButton::from_bits_retain(button));
  }
  
  pub fn get_fps(&self) -> f32 {
    self.get_cart().timing.fps()
  }

  pub fn save_sram(&self) -> Option<Vec<u8>> {
    self.cpu.bus.cart.borrow().get_sram()
  }

  pub fn load_sram(&mut self, data: Vec<u8>) {
    self.get_bus().cart.borrow_mut().set_sram(data);
  }

  pub fn load_from_emu(&mut self, other: Nes) {
    // save prg and chr in temp values
    let mut self_cart = self.get_bus().cart.borrow_mut();
    let prg = core::mem::take(&mut self_cart.prg);
    let chr = core::mem::take(&mut self_cart.chr);
    drop(self_cart);
    
    // copy the new emulator
    *self = other;

    // the new emulator is missing prg and chr; we take the temp ones
    let mut self_cart = self.get_bus().cart.borrow_mut();
    self_cart.prg = prg;
    // we only copy the temp chr if it is not chr ram, as that has already been deserialized by serde
    if !self_cart.header.uses_chr_ram {
      self_cart.chr = chr;
    }
    drop(self_cart);

    // As the cart is an rc, serde makes a new copy (so distinct rcs) for each referece.
    // When loading a savestate, we have to clone again the new cart, 
    // and re-wire it to the relative devices.
    let ppu_cart = self.cpu.bus.cart.clone();
    self.get_ppu().wire_cart(ppu_cart);
    let apu_cart = self.cpu.bus.cart.clone();
    self.get_apu().wire_cart(apu_cart);
  }
}

impl Nes {
  pub fn boot_from_cart(cart: Cart) -> Self {
    Self {
      cpu: Cpu::with_cart(cart),
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

  pub fn get_cart(&self) -> CartHeader {
    self.cpu.bus.cart.borrow().header.clone()
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