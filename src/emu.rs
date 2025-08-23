use std::sync::LazyLock;

use crate::{apu::ApuRP2A, bus::Bus, cart::{Cart, CartHeader}, cpu::{self, Cpu6502}, joypad::Joypad, mapper::{mapper_from_header, Mapper, NROM}, ppu::Ppu2C02};

pub struct Emu {
  pub cpu: Cpu6502,
  pub ppu: Ppu2C02,
  pub apu: ApuRP2A,
  pub joypad: Joypad,
  pub mem: Bus,
  pub mapper: Box<dyn Mapper>,

  #[cfg(feature = "ram64kb")]
  pub ram: [u8; 64 * 1024],

  pub rom_header: CartHeader,

  pub frame_ready: bool,
  pub videobuf: [u8; 256 * 240],
  audiobuf: [i16; 1024],
}

#[derive(Debug, Default, Clone, bitcode::Encode, bitcode::Decode)]
pub enum Mirroring {
  #[default] Horizontal,
  Vertical,
  SingleScreenA,
  SingleScreenB,
  FourScreens
}

#[derive(Debug, Default, Clone, bitcode::Encode, bitcode::Decode)]
pub enum Region {
  #[default] NTSC, PAL, World, Dendy
}

// TODO: control all default implementations
impl Default for Emu {
  fn default() -> Self {
    Self {
      cpu: Cpu6502::new(),
      ppu: Ppu2C02::new(),
      apu: ApuRP2A::new(),
      joypad: Joypad::default(),
      mem: Bus::new(Cart::default()).unwrap(),
      mapper: Box::new(NROM),
      
      #[cfg(feature = "ram64kb")]
      ram: [0; 64 * 1024],

      rom_header: Default::default(),
      frame_ready: false,

      videobuf: [0; 256 * 240],
      audiobuf: [0; 1024],
    }
  }
}

impl Emu {
  pub fn new(rom: &[u8]) -> Result<Self, String> {
    let cart = Cart::new(rom)?;

    let rom_header = cart.header.clone();
    let mut mem = Bus::new(cart)?;
    let mapper = mapper_from_header(&rom_header, &mut mem)?;
    
    let mut emu = Self {
      cpu: Cpu6502::new(),
      ppu: Ppu2C02::new(),
      apu: ApuRP2A::new(),
      joypad: Joypad::default(),
      mem,
      mapper,

      #[cfg(feature = "ram64kb")]
      ram: [0; 64 * 1024],

      rom_header,

      videobuf: [0; 256 * 240],
      audiobuf: [0; 1024],
      ..Default::default()
    };

    emu.cpu.pc = emu.cpu_read16(cpu::RST_VECTOR);
    Ok(emu)
  }


  #[deprecated]
  pub fn emu_step(&mut self) {
    let cycles = self.cpu.cycles;
    self.cpu_step();
    
    let cycles_run = self.cpu.cycles - cycles;
    for _ in 0..cycles_run {
      self.ppu_step();
      self.ppu_step();
      self.ppu_step();
    }

    for _ in 0..cycles_run { self.apu_step();}
    for _ in 0..cycles_run { self.mapper.step(&mut self.mem);}
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
    self.mapper.step(&mut self.mem);
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

  // TODO: if audio isn't read, the buffer will overflow and panic
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