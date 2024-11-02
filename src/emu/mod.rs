use std::{path::Path, rc::Rc};

use cart::Cart;
use cpu::Cpu;
use ppu::Ppu;
use bus::Bus;

pub mod cpu;
pub mod ppu;
pub mod bus;

pub mod instr;
pub mod cart;

pub mod ui;

pub struct Emulator {
  pub bus: Rc<Bus>,
  pub cpu: Cpu,
  pub ppu: Ppu,
  pub cart: Cart,
}

impl Emulator {
  pub fn new(rom_path: &Path) -> Self {
    let cart = Cart::new(rom_path);
    Emulator::from_cart(cart)
  }

  pub fn from_cart(cart: Cart) -> Self {
    let bus = Rc::new(Bus::new(&cart));
    let cpu = Cpu::new(Rc::clone(&bus));
    let ppu = Ppu::new(Rc::clone(&bus));

    Emulator {bus, cpu, ppu, cart}
  }

  pub fn debug() -> Self {
    Emulator::from_cart(Cart::empty())
  }

  pub fn step(&mut self) {
    let last_cycles = self.cpu.cycles;
    self.cpu.step();
    
    for _ in 0..3 {
      self.ppu.step(self.cpu.cycles - last_cycles);
    }
  }
}