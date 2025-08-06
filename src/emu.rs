use std::sync::LazyLock;

use crate::{bus::MemHandler, cart::Cart, cpu::{self, Cpu6502}, ppu::Ppu2C02};

bitflags::bitflags! {
  #[derive(Debug)]
  pub struct Interrupts: u8 {
    const NMI = 1 << 0;
    const IRQ = 1 << 1;
  }
}

pub struct Emu {
  pub cpu: Cpu6502,
  pub ppu: Ppu2C02,
  pub mem: MemHandler,
  #[cfg(feature = "ram64kb")]
  pub ram: [u8; 64 * 1024],
  pub interrupts: Interrupts,

  pub framebuf: [u8; 256 * 240],
  pub frame_ready: Option<()>,
}

#[derive(Debug, Default)]
pub enum Mirroring {
  #[default] Horizontal,
  Vertical,
  SingleScreenA,
  SingleScreenB,
  FourScreens
}

impl Emu {
  pub fn new(cart: Cart) -> Self {
    let mut emu = Self {
      cpu: Cpu6502::new(),
      ppu: Ppu2C02::new(),
      mem: MemHandler::new(cart),
      #[cfg(feature = "ram64kb")]
      ram: [0; 64 * 1024],
      interrupts: Interrupts::empty(),
      framebuf: [0; 256 * 240],
      frame_ready: None,
    };

    emu.cpu.pc = emu.cpu_read16(cpu::RST_VECTOR);
    emu
  }


  #[cfg(not(feature = "ram64kb"))]
  pub fn step(&mut self) {
    let cycles = self.cpu.cycles;
    self.cpu_step();
    
    let cycles_run = self.cpu.cycles - cycles;
    for _ in 0..cycles_run {
      self.ppu_step();
      self.ppu_step();
      self.ppu_step();
      // self.ppu_step_simple();
      // self.ppu_step_simple();
      // self.ppu_step_simple();
    }
  }
  
  #[cfg(feature = "ram64kb")]
  pub fn step(&mut self) {
    self.cpu_step();
  }

  pub fn step_until_vblank(&mut self) {
    while self.cpu.cycles < 133 {
      self.step();
    }
    
    self.cpu.cycles -= 133;
  }
}

#[derive(Debug)]
pub struct RGBColor(pub u8, pub u8, pub u8);
pub static SYS_COLORS: LazyLock<[RGBColor; 64]> = LazyLock::new(|| {
  let bytes = include_bytes!("../utils/2C02G_wiki.pal");

  let colors: Vec<RGBColor> = bytes
    .chunks(3)
    // we take only the first palette set of 64 colors, more might be in a .pal file
    .take(64)
    .map(|rgb| RGBColor(rgb[0], rgb[1], rgb[2]))
    .collect();

  colors.try_into().unwrap()
});