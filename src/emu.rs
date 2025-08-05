use crate::{bus::NesMemHandler, cart::Cart, cpu::{self, Cpu6502}};

bitflags::bitflags! {
  pub struct Interrupts: u8 {
    const NMI = 1 << 0;
    const IRQ = 1 << 1;
  }
}

pub struct Emu<T: Mem> {
  pub cpu: Cpu6502,
  pub mem: T,
  pub interrupts: Interrupts,
}

#[derive(Debug, Default)]
pub enum Mirroring {
  #[default] Horizontal,
  Vertical,
  SingleScreenA,
  SingleScreenB,
  FourScreens
}

pub trait Mem {
  fn read(&mut self, addr: u16) -> u8;
  fn write(&mut self, addr: u16, val: u8);
}

pub struct Ram64kb {
  pub mem: [u8; 64 * 1024],
}
impl Ram64kb {
  pub fn new() -> Self {
    Self { mem: [0; 64 * 1024] }
  }
}

impl Mem for Ram64kb {
  fn read(&mut self, addr: u16) -> u8 {
    self.mem[addr as usize]
  }

  fn write(&mut self, addr: u16, val: u8) {
    self.mem[addr as usize] = val;
  }
}

impl Emu<Ram64kb> {
  pub fn with_ram64kb() -> Self {
    Self {
      cpu: Cpu6502::new(),
      mem: Ram64kb::new(),
      interrupts: Interrupts::empty(),
    }
  }
}

impl Emu<NesMemHandler> {
  pub fn new(cart: Cart) -> Self {
    let mut emu = Self {
      cpu: Cpu6502::new(),
      mem: NesMemHandler::new(cart),
      interrupts: Interrupts::empty(),
    };

    emu.cpu.pc = emu.read16(cpu::RST_VECTOR);
    emu
  }
}

impl<T: Mem> Emu<T> {
  pub fn step(&mut self) {
    self.cpu_step();
  }
}