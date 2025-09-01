use crate::{apu::ApuRP2A, bus::Bus, cart::Cart, cpu::{self, Cpu6502}, disk::Disk, joypad::Joypad, mapper::{self, Mapper, NROM}, ppu::Ppu2C02, Palette};

#[derive(Default)]
pub struct EmuSettings {
  pub random_ram: bool,
  pub no_sprite_limit: bool,
  pub disable_background: bool,
  pub disable_sprites: bool,
  pub pal_borders: bool,

  pub audio_frequency: usize,
  pub volume: f32,
}

pub struct Emu {
  pub cpu: Cpu6502,
  pub ppu: Ppu2C02,
  pub apu: ApuRP2A,
  pub joypad: Joypad,
  pub mem: Bus,
  pub mapper: Box<dyn Mapper>,

  #[cfg(feature = "ram64kb")]
  pub ram: [u8; 64 * 1024],

  pub frame_ready: bool,
  pub videobuf: [u8; 256 * 240],
  audiobuf: [i16; 1024],

  palette: Palette,
  pub settings: EmuSettings,
}

#[derive(Debug, Default, Clone, PartialEq, bitcode::Encode, bitcode::Decode)]
pub enum Mirroring {
  #[default] Horizontal,
  Vertical,
  LowTable,
  HighTable,
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
      mem: Bus::with_cart(Cart::default()),
      mapper: Box::new(NROM),
      
      #[cfg(feature = "ram64kb")]
      ram: [0; 64 * 1024],

      frame_ready: false,

      videobuf: [0; 256 * 240],
      audiobuf: [0; 1024],

      palette: Palette::default(),
      settings: EmuSettings::default()
    }
  }
}

pub enum Game {
  Cart(Cart),
  Disk(Disk)
}
impl Game {
  pub fn new(bytes: &[u8]) -> Result<Self, &'static str> {
    let cart = Cart::new(bytes);

    let game = match cart {
      Ok(cart) => Game::Cart(cart),
      Err(_) => {
        // try to parse as fds rom if not valid nes rom
        let disk = Disk::from(bytes).map_err(|_| "not a valid iNes/NES2.0 or FDS rom")?;
        Game::Disk(disk)
      }
    };

    Ok(game)
  }
}

impl Emu {
  pub fn new(rom: &[u8]) -> Result<Self, String> {
    let game = Game::new(rom)?;

    let (mem, mapper) = match game {
      Game::Cart(cart) => {
        let mut mem = Bus::with_cart(cart);
        let mapper = mapper::new(&mut mem)?;
        (mem, mapper)
      }
      Game::Disk(disk) => {
        // return Err("loaded disk, not supported yet".to_string());
        Bus::with_disk(disk)
      },
    };
    
    let palette = Palette::from_pal_file(include_bytes!("../utils/2C02G_wiki.pal")).unwrap();
    
    let mut emu = Self {
      cpu: Cpu6502::new(),
      ppu: Ppu2C02::new(),
      apu: ApuRP2A::new(),
      joypad: Joypad::default(),
      mem,
      mapper,

      #[cfg(feature = "ram64kb")]
      ram: [0; 64 * 1024],

      videobuf: [0; 256 * 240],
      audiobuf: [0; 1024],
      palette,
      
      frame_ready: false,
      settings: EmuSettings::default()
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
    for _ in 0..cycles_run { self.mapper.step(&mut self.mem, self.cpu.cycles);}
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
    self.mapper.step(&mut self.mem, self.cpu.cycles);
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
      .map(|byte| self.palette.0[*byte as usize])
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

  pub fn load_palette(&mut self, bytes: &[u8]) {
    if let Some(pal) = Palette::from_pal_file(bytes) {
      self.palette = pal;
    } else {
      println!("not a valid palette file");
    }
  }

  pub fn load_disk(&mut self, bytes: &[u8]) {
    todo!()
  }
}