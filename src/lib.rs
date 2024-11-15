use std::path::Path;

use bus::Bus;
use cart::Cart;
use cpu::Cpu;
use dev::Joypad;
use renderer::FrameBuffer;

pub mod cpu;
pub mod instr;

pub mod mem;
pub mod bus;
pub mod mapper;

pub mod ppu;
pub mod tile;
pub mod dev;

pub mod cart;

pub mod renderer;

pub struct Emulator {
  pub cpu: Cpu<Bus>,
  pub paused: bool,
}

impl Emulator {
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
      Ok(cart) => Ok(Emulator::new(cart)),
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

  pub fn get_screen(&self) -> &FrameBuffer {
    &self.cpu.bus.ppu.screen.0
  }

  pub fn get_joypad(&mut self) -> &mut Joypad {
    &mut self.cpu.bus.joypad
  }
  
}
