use wasm_bindgen::prelude::wasm_bindgen;

use crate::{bus::Bus, cart::Cart, cpu::Cpu};

#[wasm_bindgen]
pub struct JSEmu {
  cpu: Cpu<Bus>,
}

#[wasm_bindgen]
impl JSEmu {
  pub fn new() -> Self {
    Self {cpu: Cpu::new(Cart::empty())}
  }

  pub fn test(&self) -> String {
    format!("{:?}", self.cpu)
  }

  pub fn screen(&self) -> *const u8 {
    self.cpu.bus.ppu.screen.0.buffer.as_ptr()
  }
}