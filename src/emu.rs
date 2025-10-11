use std::path::{Path, PathBuf};

use crate::{apu::ApuRP2A, bus::Bus, cpu::{self, Cpu6502}, joypad::Joypad, mapper::{self, BoxedMapper, Mapper}, ppu::Ppu2C02, rom::{Cart, CartHeader, Disk}, Palette};

#[derive(Default)]
pub struct EmuSettings {
  pub random_ram: bool,
  pub no_sprite_limit: bool,
  pub disable_background: bool,
  pub disable_sprites: bool,
  pub pal_borders: bool,
  pub battery_saving: bool,

  pub audio_sample_rate: usize,
  pub volume: f32,
  pub disable_pulse0: bool,
  pub disable_pulse1: bool,
  pub disable_triangle: bool,
  pub disable_noise: bool,
  pub disable_dmc: bool,
  pub disable_ext_audio: bool,
}
impl EmuSettings {
  pub fn new() -> Self {
    Self {
      // no_sprite_limit: true,
      ..Default::default()
    }
  }
}

pub struct Emu {
  pub cpu: Cpu6502,
  pub ppu: Ppu2C02,
  pub apu: ApuRP2A,
  pub joypad: Joypad,
  pub mem: Bus,
  pub mapper: Box<dyn Mapper>,

  pub(crate) frame_ready: bool,
  pub(crate) videobuf: [u8; 256 * 240],
  audiobuf: [i16; 2 * 1024],
  audio_read: bool,

  pub(crate) palette: Palette,
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

type LoadError = Box<dyn std::error::Error>;

pub enum Game {
  Cart(Cart),
  Disk(Disk)
}
impl Game {
  pub fn from(bytes: &[u8]) -> Result<Self, LoadError> {
    if CartHeader::is_valid_ines(bytes) {
      Ok(Game::Cart(Cart::from(bytes)?))
    } else if Disk::is_valid_fds(bytes) {
      Ok(Game::Disk(Disk::from(bytes)?))
    } else {
      // might be headless rom
      Ok(Game::Cart(Cart::from(bytes)?))
    }
  }
}

impl Emu {
  pub const NTSC_CLOCK_RATE: usize = 1789773;
  pub const PAL_CLOCK_RATE:  usize = 1662607;
  pub const NTSC_FRAME_RATE: f32 = 60.0988;
  pub const PAL_FRAME_RATE:  f32 = 50.0070;

  pub fn load_rom_from_bytes(rom: &[u8]) -> Result<Self, LoadError> {
    let game = Game::from(rom)?;

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
      audio_read: false,
      palette,
      
      frame_ready: false,
      settings: EmuSettings::new()
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

  pub const fn header(&self) -> &CartHeader {
    &self.mem.header
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

  pub fn emu_step_until_vblank(&mut self) {
    // TODO: temporary solution
    if !self.audio_read { self.apu.blip.0.clear(); }
    self.audio_read = false;

    let cycles = self.cpu.cycles;
    while !self.frame_ready {
      self.cpu_step();
    }
    let cycles_run: usize = self.cpu.cycles - cycles;

    self.frame_ready = false;

    self.apu.blip.0.end_frame(self.apu.cycles).unwrap();
    self.apu.cycles -= cycles_run;
  }

  pub fn emu_reset(&mut self) {
    self.cpu_reset();
    self.ppu.reset();
    self.apu.reset();

    // TODO: should reload wram!
    // TODO: some mappers need to be reset too
  }

  pub fn save_battery(&self) -> Option<&[u8]> {
    if self.mem.header.has_battery {
      if self.mem.header.mapper == 1 && self.mem.wram.len() == 16 * 1024 {
        // https://www.nesdev.org/wiki/MMC1#SxROM_board_types
        // Even if the SOROM and SZROM boards utilizes a battery, it is connected to only one PRG-RAM chip. The first RAM chip will not retain its data, but the second one will.
        return Some(&self.mem.wram[8 * 1024..])
      }

      if self.mem.header.mapper == 5 && self.mem.wram.len() == 16 * 1024 {
        // https://www.nesdev.org/wiki/MMC5#Other_PRG-RAM_notes
        // Games with 16K PRG-RAM only battery-save the first 8K.
        return Some(&self.mem.wram[..8 * 1024])
      }

      Some(&self.mem.wram)
    } else {
      None
    }
  }

  pub fn load_battery(&mut self, bytes: &[u8]) -> Result<(), LoadError> {
    if !self.mem.header.has_battery {
      return Err("game has no battery".into())
    } else if bytes.len() != self.mem.wram.len() {
      return Err("invalid save ram dump provided, size doesn't match".into())
    } else {
      self.mem.wram.copy_from_slice(bytes);
      Ok(())
    }
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

  pub fn get_audio(&mut self) -> &[i16] {
    let read = self.apu.blip.0.read_samples(&mut self.audiobuf[..self.apu.blip.0.avail as usize], false);
    self.audio_read = true;
    
    &self.audiobuf[..read]
  }

  pub fn load_rom_from_file<P: AsRef<Path>>(path: P) -> Result<Self, LoadError> {
    use std::io::{Read, Seek};

    let mut bytes = Vec::new();
    let file = std::fs::File::open(path)?;
    let mut reader = std::io::BufReader::new(file);
    reader.read_to_end(&mut bytes)?;

    let res = Emu::load_rom_from_bytes(&bytes);
    match res {
      Ok(_) => res,
      Err(_) => {
        reader.rewind()?;
        bytes.clear();

        if let Ok(mut archive) = zip::read::ZipArchive::new(&mut reader) {
          // it is a zip file
          let mut zip = archive.by_index(0)?;
          zip.read_to_end(&mut bytes)?;
          Emu::load_rom_from_bytes(&bytes)
        } else {
          // not a zip file either
          res
        }
      }
    }
  }

  pub fn save_battery_to_file<P: AsRef<Path>>(&self, path: P) -> std::io::Result<bool> {
    use std::io::Write;

    if let Some(sram) = self.save_battery() {
      let mut save_path = PathBuf::from(path.as_ref());
      save_path.set_extension("sram");

      let file = std::fs::File::create(&save_path)?;
      let mut writer = std::io::BufWriter::new(file);
      writer.write_all(sram)?;
      Ok(true)
    } else {
      Ok(false)
    }
  }

  pub fn load_battery_from_file<P: AsRef<Path>>(&mut self, path: P) -> Result<(), LoadError> {
    use std::io::Read;
    
    let mut load_path = PathBuf::from(path.as_ref());
    load_path.set_extension("sram");
    if let Ok(file) = std::fs::File::open(&load_path) {
      let mut buf = Vec::new();
      let mut reader = std::io::BufReader::new(file);
      reader.read_to_end(&mut buf)?;
      self.load_battery(&buf)
    } else {
      Err("no sram dump file found".into())
    }
  }
}