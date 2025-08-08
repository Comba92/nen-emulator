use std::sync::LazyLock;

use crate::{bus::MemHandler, cart::Cart, cpu::{self, Cpu6502}, joypad::Joypad, ppu::Ppu2C02};

bitflags::bitflags! {
  #[derive(Debug)]
  pub struct Events: u8 {
    const IRQ = 1 << 0;
    const NMI = 1 << 1;
    const FRAME  = 1 << 7;
  }
}

pub struct Emu {
  pub cpu: Cpu6502,
  pub ppu: Ppu2C02,
  pub joypad: Joypad,
  pub mem: MemHandler,
  #[cfg(feature = "ram64kb")]
  pub ram: [u8; 64 * 1024],
  pub events: Events,

  pub framebuf: [u8; 256 * 240],
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
      joypad: Joypad::default(),
      mem: MemHandler::new(cart),
      #[cfg(feature = "ram64kb")]
      ram: [0; 64 * 1024],
      events: Events::empty(),

      framebuf: [0; 256 * 240],
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

      self.mem.mapper.step();
    }
  }
  
  #[cfg(feature = "ram64kb")]
  pub fn step(&mut self) {
    self.cpu_step();
  }

  pub fn step_until_vblank(&mut self) {
    while !self.events.contains(Events::FRAME) {
      self.step();
    }
    
    self.events.remove(Events::FRAME);
  }
}

#[derive(Debug)]
pub struct RGBColor(pub u8, pub u8, pub u8);
pub static SYS_COLORS: LazyLock<[RGBColor; 64]> = LazyLock::new(|| {
  let bytes = include_bytes!("../utils/Composite_wiki.pal");

  let colors: Vec<RGBColor> = bytes
    .chunks(3)
    // we take only the first palette set of 64 colors, more might be in a .pal file
    .take(64)
    .map(|rgb| RGBColor(rgb[0], rgb[1], rgb[2]))
    .collect();

  colors.try_into().unwrap()
});