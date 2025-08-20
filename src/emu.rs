use std::{collections::VecDeque, sync::LazyLock};

use crate::{apu::ApuRP2A, bus::MemHandler, cart::Cart, cpu::{self, Cpu6502}, dma::Dma, joypad::Joypad, ppu::Ppu2C02};

bitflags::bitflags! {
  #[derive(Debug, Default, Clone)]
  pub struct IrqFlags: u8 {
    const FRAME = 1 << 0;
    const DMC = 1 << 2;
    const MAPPER = 1 << 3;
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

  pub nmi: bool,
  pub irq: IrqFlags,

  pub frame_ready: bool,

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
      
      nmi: false,
      irq: IrqFlags::empty(),
      frame_ready: false,

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

      videobuf: [0; 256 * 240],
      audiobuf: [0; 1024],
      ..Default::default()
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
    }

    for _ in 0..cycles_run { self.apu_step();}
    for _ in 0..cycles_run { self.mem.mapper.step();}
  }

  #[cfg(feature = "ram64kb")]
  pub fn step(&mut self) {
    self.cpu_step();
  }

  pub fn cpu_tick(&mut self) {
    self.cpu.cycles += 1;

    self.ppu_step();
    self.ppu_step();
    self.ppu_step();

    self.apu_step();
    self.mem.mapper.step();
  }

  pub fn step_until_vblank(&mut self) {
    let cycles = self.cpu.cycles;
    while !self.frame_ready {
      self.cpu_step();
    }
    let cycles_run: usize = self.cpu.cycles - cycles;

    self.frame_ready = false;

    self.apu.blip.0.end_frame(self.apu.cycles as u32);
    self.apu.cycles -= cycles_run;
  }

  pub fn get_video_rgba(&mut self, buf: &mut [u8; 256 * 240 * 4]) {
    for (i, color) in self.videobuf.iter()
      .map(|byte| &DEFAULT_PALETTE[*byte as usize])
      .enumerate()
    {
      buf[i * 4 + 0] = color.0;
      buf[i * 4 + 1] = color.1;
      buf[i * 4 + 2] = color.2;
      buf[i * 4 + 3] = 255;
    }
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