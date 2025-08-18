use std::{ops::Not, sync::LazyLock};

use blip_buf::BlipBuf;

use crate::{apu::ApuRP2A, bus::MemHandler, cart::Cart, cpu::{self, Cpu6502}, joypad::Joypad, ppu::Ppu2C02};

bitflags::bitflags! {
  #[derive(Debug, Clone, Copy)]
  pub struct Events: u8 {
    const NMI = 1 << 0;
    const PPU_FRAME  = 1 << 2;
    const APU_FRAME = 1 << 3;
    const DMC = 1 << 4;
    const MAPPER = 1 << 5;
  }
}

impl Events {
  pub fn contains_irq(&self) -> bool {
    (*self & (Self::APU_FRAME | Self::DMC | Self::MAPPER)).is_empty().not()
  }
}

pub struct Emu {
  pub cpu: Cpu6502,
  pub ppu: Ppu2C02,
  pub apu: ApuRP2A,
  pub joypad: Joypad,
  pub mem: MemHandler,
  #[cfg(feature = "ram64kb")]
  pub ram: [u8; 64 * 1024],
  pub events: Events,

  pub videobuf: [u8; 256 * 240],
  audiobuf: [i16; 1024],
}

#[derive(Debug, Default)]
pub enum Mirroring {
  #[default] Horizontal,
  Vertical,
  SingleScreenA,
  SingleScreenB,
  FourScreens
}

// TODO: control all default implementations
impl Default for Emu {
  fn default() -> Self {
    Self {
      cpu: Cpu6502::new(),
      ppu: Ppu2C02::new(),
      apu: ApuRP2A::new(),
      joypad: Joypad::default(),
      mem: MemHandler::new(Cart::default()).unwrap(),
      #[cfg(feature = "ram64kb")]
      ram: [0; 64 * 1024],
      events: Events::empty(),

      videobuf: [0; 256 * 240],
      audiobuf: [0; 1024],
    }
  }
}

impl Emu {
  pub fn new(rom: &[u8]) -> Result<Self, String> {
    let cart = Cart::new(rom)?;

    let mut emu = Self {
      cpu: Cpu6502::new(),
      ppu: Ppu2C02::new(),
      apu: ApuRP2A::new(),
      joypad: Joypad::default(),
      mem: MemHandler::new(cart)?,
      #[cfg(feature = "ram64kb")]
      ram: [0; 64 * 1024],
      events: Events::empty(),

      videobuf: [0; 256 * 240],
      audiobuf: [0; 1024],
    };

    emu.cpu.pc = emu.cpu_read16(cpu::RST_VECTOR);
    Ok(emu)
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

      self.apu_step();
      self.mem.mapper.step();
    }
  }

  #[cfg(feature = "ram64kb")]
  pub fn step(&mut self) {
    self.cpu_step();
  }

  pub fn step_until_vblank(&mut self) {
    let cycles = self.cpu.cycles;
    while !self.events.contains(Events::PPU_FRAME) {
      self.step();
    }
    let cycles_run = self.cpu.cycles - cycles;

    self.events.remove(Events::PPU_FRAME);

    self.apu.blip.0.end_frame(self.apu.cycles as u32);
    self.apu.cycles -= cycles_run;
  }

  // TODO: should return slice
  pub fn get_video(&mut self) -> Vec<u8> {
    let mut framebuf = Vec::new();

    for byte in &self.videobuf {
      let color = &DEFAULT_PALETTE[*byte as usize];
      framebuf.push(color.0);
      framebuf.push(color.1);
      framebuf.push(color.2);
      framebuf.push(255);
    }

    framebuf
  }

  pub fn get_audio(&mut self) -> &[i16] {
    let read = self.apu.blip.0.read_samples(&mut self.audiobuf, false);
    &self.audiobuf[..read]
  }
}


// TODO: palette setting
#[derive(Debug)]
pub struct RGBColor(pub u8, pub u8, pub u8);
pub static DEFAULT_PALETTE: LazyLock<[RGBColor; 64]> = LazyLock::new(|| {
  let bytes = include_bytes!("../utils/Composite_wiki.pal");

  let colors: Vec<RGBColor> = bytes
    .chunks(3)
    // we take only the first palette set of 64 colors, more might be in a .pal file
    .take(64)
    .map(|rgb| RGBColor(rgb[0], rgb[1], rgb[2]))
    .collect();

  colors.try_into().unwrap()
});