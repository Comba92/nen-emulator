use crate::{apu::ApuRP2A, bus::Bus, cart::Cart, cpu::{self, Cpu6502}, disk::Disk, joypad::{Button, Joypad}, mapper::{self, BoxedMapper, Mapper, NROM}, ppu::Ppu2C02, Palette};

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

  pub frame_ready: bool,
  pub videobuf: [u8; 256 * 240],
  audiobuf: [i16; 2 * 1024],

  pub palette: Palette,
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
  #[default] NTSC, PAL
}

pub enum Game {
  Cart(Cart),
  Disk(Disk)
}
impl Game {
  pub fn new(bytes: &[u8]) -> Result<Self, String> {
    // TODO: make this better
    
    let cart = Cart::new(bytes);

    let game = match cart {
      Ok(cart) => Game::Cart(cart),
      Err(e1) => {
        // try to parse as fds rom if not valid nes rom
        let disk = Disk::from(bytes)
          .map_err(|e2| format!("not a valid iNes/NES2.0 or FDS rom: {e1}, {e2}"))?;
        Game::Disk(disk)
      }
    };

    Ok(game)
  }
}

impl Emu {
  pub const NTSC_CLOCK_RATE: usize = 1789773;
  pub const PAL_CLOCK_RATE:  usize = 1662607;
  pub const NTSC_FRAME_RATE: f32 = 60.0988;
  pub const PAL_FRAME_RATE:  f32 = 50.0070;

  pub fn new(rom: &[u8]) -> Result<Self, String> {
    let game = Game::new(rom)?;

    let (mem, mapper) = match game {
      Game::Cart(cart) => {
        let mut mem = Bus::with_cart(cart);
        let mapper: BoxedMapper = mapper::new(&mut mem)?;
        (mem, mapper)
      }
      Game::Disk(disk) => {
        Bus::with_disk(disk)
      },
    };
    
    let palette = Palette::from_pal_file(include_bytes!("../utils/2C02G_wiki.pal")).unwrap();
    
    let mut emu = Self {
      cpu: Cpu6502::new(),
      ppu: Ppu2C02::new(&mem.header.region),
      apu: ApuRP2A::new(&mem.header.region),
      joypad: Joypad::default(),
      mem,
      mapper,

      videobuf: [0; 256 * 240],
      audiobuf: [0; 2 * 1024],
      palette,
      
      frame_ready: false,
      settings: EmuSettings::default()
    };

    emu.cpu.pc = emu.cpu_read16(cpu::RST_VECTOR);
    Ok(emu)
  }

  pub const fn clock_rate(&self) -> usize {
    match self.region() {
      Region::NTSC => Self::NTSC_CLOCK_RATE,
      Region::PAL => Self::PAL_CLOCK_RATE,
    }
  }

  pub const fn frame_rate(&self) -> f32 {
    match self.region() {
      Region::NTSC => Self::NTSC_FRAME_RATE,
      Region::PAL => Self::PAL_FRAME_RATE,
    }
  }

  pub const fn region(&self) -> &Region {
    &self.mem.header.region
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

  pub fn cpu_tick(&mut self) {
    self.cpu.cycles += 1;

    self.apu_step();
    self.mapper.step(&mut self.mem, self.cpu.cycles);

    self.ppu_step();
    self.ppu_step();
    self.ppu_step();
    // PAL systems additionally run 3.2 PPU cycles per CPU cycle
    // meaning, every 5 CPU cycles there is an additional PPU cycle 
    match self.mem.header.region {
      Region::PAL => if self.cpu.cycles % 5 == 0 {
        self.ppu_step()
      }
      _ => {}
    }
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

  pub fn get_video_rgba(&self, buf: &mut [u8]) {
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

  pub fn get_nametables_rgba(&mut self, buf: &mut [u8]) {
    let pttrntbl = self.ppu.ctrl.bg_pttrntbl_addr;
    
    for table in 0..4 {
      let region_y = if table <= 1 { 0 } else { 256 * 240 * 4 * 2 };
      let region_x = if table == 0 || table == 2 { 0 } else { 256 * 4 };

      for i in 0usize..960 {
        let nametbl_addr = 0x2000 + (table * 1024) + i;
        let tile_id = self.ppu_debug_read(nametbl_addr as u16);
        let pttrn_addr = pttrntbl + ((tile_id as u16) << 4);
        let attr_addr = (0x23c0 + (table * 1024)) | (((i/32)/4) << 3) | ((i%32)/4);
        
        let mut attr = self.ppu_debug_read(attr_addr as u16);
        if (i/32) & 0x2 > 0 { attr >>= 4; }
        if (i%32) & 0x2 > 0 { attr >>= 2; }
        attr &= 0x3;

        for row in 0..8 {
          let pttrn_lo = self.ppu_debug_read(pttrn_addr + row).reverse_bits();
          let pttrn_hi = self.ppu_debug_read(pttrn_addr + row + 8).reverse_bits();

          for col in 0..8 {
            let pixel = (((pttrn_hi >> col) & 1) << 1) | ((pttrn_lo >> col) & 1);
            let pixel_color = self.ppu.palettes_read((attr*4 + pixel) as u16);
            let color = self.palette.0[pixel_color as usize];

            // row is 256 * 4 * 2 bytes long
            let y = region_y + (256 * 4 * 2 * ((i/32)*8 + row as usize));
            // pixel is 4 bytes long
            let x = region_x + (4 * ((i%32)*8 + col as usize));

            buf[y + x + 0] = color.0;
            buf[y + x + 1] = color.1;
            buf[y + x + 2] = color.2;
            buf[y + x + 3] = 255;
          }
        }
      }
    }
  }

  // TODO: if audio isn't read, the buffer will overflow and panic
  pub fn get_audio(&mut self) -> &[i16] {
    let read = self.apu.blip.0.read_samples(&mut self.audiobuf[..self.apu.blip.0.samples_avail() as usize], false);
    &self.audiobuf[..read]
  }
}