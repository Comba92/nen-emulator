use crate::{emu::{Mirroring, Region}, games_db::GAMES_DB};

#[derive(Default)]
pub struct Cart {
  pub header: CartHeader,
  pub prg: Vec<u8>,
  pub chr: Vec<u8>,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub enum HeaderFormat {
  #[default] Headerless, INes, Nes2_0, Unif,
}

// TODO: just hold a static reference to game data here retard
// https://www.nesdev.org/wiki/INES
#[derive(Default, Debug, Clone)]
pub struct CartHeader {
  pub format: HeaderFormat,
  pub mapper: u16,
  pub submapper: u8,
  pub region: Region,

  pub prg_size: usize,
  pub chr_size: usize,
  pub wram_size: usize,

  pub volatile_ram_size: usize,
  pub non_volatile_ram_size: usize,
  
  pub mirroring: Mirroring,
  pub alt_mirroring: bool,
  
  pub expansions: u8,

  pub has_battery: bool,
  pub has_chr_ram: bool,
  pub has_trainer: bool,
}

impl CartHeader {
  const INES_MAGIC: &[u8] = &[0x4e, 0x45, 0x53, 0x1a];
  const INES_HEADER_SIZE: usize = 16;
  const UNIF_MAGIC: &[u8] = &[0x55, 0x4e, 0x49, 0x46];
  const UNIF_HEADER_SIZE: usize = 32;
  const TRAINER_SIZE: usize = 512;

  pub fn is_valid_ines(bytes: &[u8]) -> bool {
    bytes.len() >= Self::INES_HEADER_SIZE && &bytes[0..4] == Self::INES_MAGIC
  }

  pub fn is_valid_unif(bytes: &[u8]) -> bool {
    bytes.len() >= Self::UNIF_HEADER_SIZE && &bytes[0..4] == Self::UNIF_MAGIC
  }

  pub fn len(&self) -> usize {
    match self.format {
      HeaderFormat::Headerless => 0,
      HeaderFormat::Unif => Self::UNIF_HEADER_SIZE,
      HeaderFormat::INes | HeaderFormat::Nes2_0 => 
        if self.has_trainer { Self::INES_HEADER_SIZE + Self::TRAINER_SIZE } else { Self::INES_HEADER_SIZE },
    }
  }

  pub fn from(bytes: &[u8]) -> Result<Self, &'static str> {
    let header = Self::parse(bytes);

    match header {
      Ok(mut header) => {
        // DEBUG
        if let Some(entry) = GAMES_DB.query(&bytes[header.len()..]) {
          println!("==[GAME LOADED]==\n{:?}", entry);
        }

        // we only trust Nes2.0 format
        if header.format != HeaderFormat::Nes2_0 {
          let entry = GAMES_DB.query(&bytes[header.len()..]);
          if let Some(entry) = entry {
            header = entry.into();
            header.format = HeaderFormat::INes;
          }
        }
        Ok(header)
      }
      Err(e) => GAMES_DB.query(bytes)
        .ok_or(e)
        .map(|x| x.into()),
    }
  }

  pub fn parse(bytes: &[u8]) -> Result<Self, &'static str> {
    // TODO: UNIF support
    if Self::is_valid_unif(bytes) {
      return Err("valid UNIF ROM, but not yet supported by this emulator")
    }
    
    if !Self::is_valid_ines(bytes) {
      return Err("not a valid iNES/NES2.0 ROM")
    }

    let mut header = CartHeader::default();
    header.format = HeaderFormat::INes;
    
    header.prg_size = bytes[4] as usize * 16 * 1024;
    header.has_chr_ram = bytes[5] == 0;
    header.chr_size = bytes[5] as usize * 8 * 1024;
    // default wram to 8kb if iNes, as we can't tell
    header.wram_size = 8 * 1024;

    header.mapper = ((bytes[7] & 0xf0) | (bytes[6] >> 4)) as u16;
    header.has_battery = bytes[6] & 0x2 != 0;
    header.has_trainer = bytes[6] & 0x4 != 0;
    header.alt_mirroring = bytes[6] & 0x8 != 0;

    header.mirroring = match bytes[6] & 1 {
      0 => Mirroring::Horizontal,
      _ => Mirroring::Vertical
    };

    let version = bytes[7] & 0xc;

    if version == 0x08 {
      // NES 2.0
      if bytes[9] & 0xf == 0x0f || bytes[9] & 0xf0 == 0xf0 {
        return Err("exponent-multiplier notation not supported")
      }


      header.format = HeaderFormat::Nes2_0;
      header.mapper |= (bytes[8] as u16 & 0xf) << 8;
      header.submapper = bytes[8] >> 4;

      let prg_ram_shift = bytes[10] & 0xf;
      let prg_nvram_shift = bytes[10] >> 4;

      // TODO: some MMC5 games have both, handle that case
      // https://www.nesdev.org/wiki/MMC5#PRG-RAM_configurations

      let prg_ram_size = if prg_ram_shift > 0 { 64 << prg_ram_shift} else { 0 };
      let prg_nvram_size = if prg_nvram_shift > 0 { 64 << prg_nvram_shift} else { 0 };
      // we only take nvram is ram is zero
      header.wram_size = if prg_ram_shift > 0 {
        prg_ram_size
      } else if prg_nvram_shift > 0 {
        prg_nvram_size
      } else {
        0
      };
      header.volatile_ram_size = prg_ram_size;
      header.non_volatile_ram_size = prg_nvram_size;

      // we only take chr ram if chr rom is zero
      if header.chr_size == 0 {
        let chr_ram_shift = bytes[11] & 0xf;
        let chr_nvram_shift = bytes[11] >> 4;

        header.chr_size = if chr_ram_shift > 0 {
          64 << chr_ram_shift
        } else if chr_nvram_shift > 0 {
          64 << chr_nvram_shift
        } else {
          8 * 1024
        };

      }

      header.region = match bytes[12] & 0b11 {
        1 | 3 => Region::PAL,
        _ => Region::NTSC,
      };
      header.expansions = bytes[15] & 0x3f;
    } else if version == 0 && bytes[12..=15].iter().all(|x| *x == 0) {
      // https://www.nesdev.org/wiki/INES#Variant_comparison
      // iNES with PRG RAM and TV system field
      // header.wram_size = bytes[8] as usize * 8 * 1024;
      header.wram_size = 32 * 1024;

      header.region = match bytes[9] {
        1 => Region::PAL,
        _ => Region::NTSC,
      };
    } else {
      // archaic iNes, default wram to 8kb
      header.wram_size = 8 * 1024;
    }

    // if chr rom is 0, default to 8kb
    header.chr_size = if header.chr_size == 0 { 8 * 1024 } else { header.chr_size };

    Ok(header)
  }
}

impl Cart {
  pub fn new(rom_bytes: &[u8]) -> Result<Self, &'static str> {
    let header = CartHeader::from(rom_bytes)?;

    // only iNes supported
    let rom_start = header.len();
    println!("{header:?}");
    let prg = rom_bytes[rom_start..rom_start+header.prg_size].to_vec();
    let chr = if header.has_chr_ram {
      vec![0; header.chr_size]
    } else {
      let chr_start = rom_start+header.prg_size;
      rom_bytes[chr_start..chr_start+header.chr_size].to_vec()
    };

    // DEBUG
    println!("[DEBUG] {:?}", header);

    Ok(Self {
      header,
      prg, chr,
    })
  }
}