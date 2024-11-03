use std::{path::Path, rc::Rc};

use cart::Cart;
use cpu::Cpu;
use bus::Bus;
use ppu::Ppu;

pub mod cpu;
pub mod ppu;
pub mod bus;

pub mod instr;
pub mod cart;

pub mod ui;

pub struct Emulator {
  pub bus: Rc<Bus>,
  pub cpu: Cpu,
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
    let ppu = Ppu::new(&cart);
    bus.connect(ppu);

    Emulator {bus, cpu, cart}
  }

  pub fn debug() -> Self {
    Emulator::from_cart(Cart::empty())
  }

  pub fn step(&mut self) -> bool {
    if self.bus.ppu().nmi_requested { return true }

    let last_cycles = self.cpu.cycles;
    self.cpu.step();
    
    for _ in 0..3 {
      self.bus.step(self.cpu.cycles - last_cycles);
    }

    self.bus.ppu().nmi_requested
  }
}