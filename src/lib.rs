use crate::{apu::Apu, bus::Bus, cart::{Cart, CartHeader}, cpu::Cpu, frame::FrameBuffer, joypad::{Joypad, JoypadButton}, ppu::Ppu};
use wasm_bindgen::prelude::wasm_bindgen;

pub mod cpu;
pub mod addr;

pub mod mem;
pub mod bus;
pub mod dma;
pub mod mapper;

pub mod ppu;
pub mod frame;

pub mod apu;
pub mod joypad;

pub mod cart;


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
    self.cpu.bus.ppu.screen.buffer.as_ptr()
  }

  pub fn get_raw_samples(&self) -> *const f32 {
    self.cpu.bus.apu.samples.as_ptr()
  }

  pub fn get_samples_count(&self) -> f32 {
    self.cpu.bus.apu.samples.len() as f32
  }
  
  pub fn consume_samples(&mut self) {
    self.get_apu().samples.clear();
    self.get_apu().samples.reserve(800);
  }

  pub fn button_pressed(&mut self, button: u8) {
    self.get_joypad().buttons1.insert(JoypadButton::from_bits_retain(button));
  }

  pub fn button_released(&mut self, button: u8) {
    self.get_joypad().buttons1.remove(JoypadButton::from_bits_retain(button));
  }
  
  pub fn get_fps(&self) -> f32 {
    self.get_cart_header().timing.fps()
  }

  pub fn save_sram(&self) -> Option<Vec<u8>> {
    self.cpu.bus.cart.as_ref().get_sram()
  }

  pub fn load_sram(&mut self, data: Vec<u8>) {
    self.get_bus().cart.as_mut().set_sram(data);
  }

  pub fn toggle_sprite_limit(&mut self) {
    let limit = &mut self.get_ppu().oam_sprite_limit;
    if *limit == 8 {
      *limit = u8::MAX;
    } else {
      *limit = 8;
    }
  }

  pub fn load_from_emu(&mut self, other: Nes) {
    // save prg and chr in temp values
    let old_cart = self.get_bus().cart.as_mut();
    let prg = core::mem::take(&mut old_cart.prg);
    let chr = core::mem::take(&mut old_cart.chr);

    // copy the new emulator
    *self = other;

    // the new emulator is missing prg and chr; we take the temp ones
    let new_cart = self.get_bus().cart.as_mut();
    new_cart.prg = prg;
    // we only copy the temp chr if it is not chr ram, as that has already been deserialized by serde
    if !new_cart.header.uses_chr_ram {
      new_cart.chr = chr;
    }

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

  pub fn get_cart_header(&self) -> &CartHeader {
    &self.cpu.bus.cart.as_ref().header
  }

  pub fn get_cart(&mut self) -> &mut Cart {
    self.get_bus().cart.as_mut()
  }

  pub fn get_resolution(&mut self) -> (usize, usize) { (32*8, 30*8) }

  pub fn get_screen(&self) -> &FrameBuffer {
    &self.cpu.bus.ppu.screen
  }

  pub fn get_samples(&mut self) -> Vec<f32> {
    self.get_apu().consume_samples()
  }

  pub fn get_joypad(&mut self) -> &mut Joypad {
    &mut self.cpu.bus.joypad
  }
}