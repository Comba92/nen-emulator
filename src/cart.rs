use crate::mapper;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Default, Clone)]
pub struct CartHeader {
  pub format: HeaderFormat,
  pub console_type: ConsoleType,
  pub timing: ConsoleTiming,

  pub game_title: String,
  pub has_trainer: bool,
  pub mirroring: Mirroring,
  pub has_alt_mirroring: bool,

  pub mapper: u16,
  pub submapper: u8,
  pub mapper_name: String,

  pub prg_16kb_banks: usize,
  pub chr_8kb_banks: usize,

  pub prg_size: usize,
  pub chr_size: usize,
  pub uses_chr_ram: bool,
  pub chr_ram_size: usize,
  pub has_battery: bool,
  pub prg_ram_size: usize,
  pub eeprom_size: usize,
  pub chr_nvram_size: usize,
}
impl CartHeader {
  pub fn chr_real_size(&self) -> usize {
    // TODO: we dont account of chr nvram here, but ive never seen games using it
    if self.uses_chr_ram {
      self.chr_ram_size
    } else {
      self.chr_size
    }
  }

  pub fn sram_real_size(&self) -> usize {
    if self.has_battery && self.eeprom_size > 0 {
      self.eeprom_size
    } else if self.prg_ram_size > 0 {
      self.prg_ram_size
    } else {
      8 * 1024
    }
  }
}

const NES_MAGIC: [u8; 4] = [0x4E, 0x45, 0x53, 0x1A];
pub const HEADER_SIZE: usize = 16;
const PRG_ROM_PAGE_SIZE: usize = 1024 * 16;
const CHR_ROM_PAGE_SIZE: usize = 1024 * 8;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum HeaderFormat {
  #[default]
  INes,
  Nes2_0,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum Mirroring {
  #[default]
  Horizontal,
  Vertical,
  SingleScreenA,
  SingleScreenB,
  FourScreen,
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Default, Clone, Copy)]
pub enum ConsoleType {
  #[default]
  NES,
  VsSystem,
  Playchoice10,
  Other,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum ConsoleTiming {
  NTSC,
  PAL,
  World,
  Dendy,
  #[default]
  Unknown,
}
impl ConsoleTiming {
  pub fn fps(&self) -> f32 {
    use ConsoleTiming::*;
    match self {
      PAL | Dendy => 50.0070,
      _ => 60.0988,
    }
  }

  pub fn cpu_hz(&self) -> usize {
    use ConsoleTiming::*;
    match self {
      PAL => 1662607,
      Dendy => 1773448,
      _ => 1789773,
    }
  }

  pub fn frame_ppu_cycles(&self) -> usize {
    use ConsoleTiming::*;
    match self {
      PAL | Dendy => 106392,
      _ => 89341,
    }
  }

  pub fn frame_cpu_cycles(&self) -> f32 {
    use ConsoleTiming::*;
    match self {
      PAL => 33247.5,
      Dendy => 35464.0,
      _ => 29780.5,
    }
  }

  pub fn vblank_len(&self) -> usize {
    use ConsoleTiming::*;
    match self {
      PAL => 70,
      _ => 20,
    }
  }
}

pub fn is_nes_rom(rom: &[u8]) -> bool {
  if rom.len() < 4 {
    return false;
  }

  let magic_str = &rom[0..=3];
  magic_str == NES_MAGIC
}

impl CartHeader {
  pub fn new(rom: &[u8]) -> Result<Self, &'static str> {
    let mut header = CartHeader::default();

    if !is_nes_rom(rom) {
      return Err("Nintendo header magic values not found");
    }

    if rom.len() < HEADER_SIZE {
      return Err("File too small to contain a 16 bytes header");
    }

    header.prg_16kb_banks = rom[4] as usize;
    header.chr_8kb_banks = if rom[5] > 0 { rom[5] } else { 1 } as usize;
    header.uses_chr_ram = rom[5] == 0;

    header.prg_size = header.prg_16kb_banks as usize * PRG_ROM_PAGE_SIZE;
    header.chr_size = header.chr_8kb_banks as usize * CHR_ROM_PAGE_SIZE;
    // iNes header doesn't hold information about chr ram size, so it defaults to 8kb if no chr rom is present
    header.chr_ram_size = if header.uses_chr_ram {
      CHR_ROM_PAGE_SIZE
    } else {
      0
    };

    let nametbl_mirroring = rom[6] & 1;
    header.has_alt_mirroring = rom[6] & 0b0000_1000 != 0;
    header.mirroring = match (nametbl_mirroring, header.has_alt_mirroring) {
      (_, true) => Mirroring::FourScreen,
      (0, false) => Mirroring::Horizontal,
      (1, false) => Mirroring::Vertical,
      _ => unreachable!(),
    };

    header.has_battery = rom[6] & 0b0000_0010 != 0;
    header.has_trainer = rom[6] & 0b0000_0100 != 0;

    let mapper_low = rom[6] >> 4;
    let mapper_high = rom[7] & 0b1111_0000;
    header.mapper = (mapper_high | mapper_low) as u16;
    // header.mapper_name = mapper::mapper_name(header.mapper).to_string();

    header.format = if rom[7] & 0b0000_1100 == 0x8 {
      HeaderFormat::Nes2_0
    } else {
      HeaderFormat::INes
    };
    // This field was a later addition to iNes, so most games do not use it, even if they contain prg_ram.
    // If it is 0, prg ram is inferred as 8kb.
    header.prg_ram_size = rom[8] as usize * 1024;

    let title_start = HEADER_SIZE + header.prg_size - 32;
    let title_bytes = &rom[title_start..title_start + 16];
    header.game_title = String::from_utf8_lossy(title_bytes)
      .into_owned()
      .chars()
      .filter(|c| {
        c.is_ascii_alphanumeric() || c.is_ascii_punctuation() || c.is_ascii_whitespace()
      })
      .collect::<String>()
      .trim()
      .to_string();

    if header.format == HeaderFormat::INes {
      return Ok(header);
    }

    if rom[9] & 0b1111 == 0xF || rom[9] >> 4 == 0xF {
      return Err("NES 2.0 'exponent-multiplier' notation for ROM sizes not implemented");
    }

    header.console_type = match rom[7] & 0b11 {
      0 => ConsoleType::NES,
      1 => ConsoleType::VsSystem,
      2 => ConsoleType::Playchoice10,
      _ => ConsoleType::Other,
    };

    header.mapper = ((rom[8] as u16 & 0b111) << 8) | header.mapper as u16;
    header.submapper = rom[8] >> 4;
    header.mapper_name = mapper::mapper_name(header.mapper).to_string();

    if header.mapper == 1 {
      let ext = match header.submapper {
        1 => "/SURom",
        2 => "/SORom",
        4 => "/SXRom",
        _ => "",
      };
      header.mapper_name.push_str(ext);
    } else if header.mapper == 4 && header.submapper == 1 {
      header.mapper_name = String::from("MMC6");
    } else if header.mapper == 34 {
      if header.submapper == 1 {
        header.mapper_name = String::from("NINA-001");
      } else if header.submapper == 2 {
        header.mapper_name = String::from("BNROM");
      }
    }

    header.prg_16kb_banks = ((rom[9] as usize & 0b1111) << 8) + rom[4] as usize;
    header.chr_8kb_banks = ((rom[9] as usize >> 4) << 8) + rom[5] as usize;

    header.prg_size = header.prg_16kb_banks * PRG_ROM_PAGE_SIZE;
    header.chr_size = header.chr_8kb_banks * CHR_ROM_PAGE_SIZE;

    header.prg_ram_size = if rom[10] & 0b0000_1111 == 0 {
      0
    } else {
      64 << (rom[10] & 0b0000_1111)
    };
    header.eeprom_size = if rom[10] & 0b1111_0000 == 0 {
      0
    } else {
      64 << (rom[10] >> 4)
    };
    header.chr_ram_size = if rom[11] & 0b0000_1111 == 0 {
      0
    } else {
      64 << (rom[11] & 0b0000_1111)
    };
    header.chr_nvram_size = if rom[11] & 0b1111_0000 == 0 {
      0
    } else {
      64 << (rom[11] >> 4)
    };

    header.timing = match rom[12] & 0b11 {
      0 => ConsoleTiming::NTSC,
      1 => ConsoleTiming::PAL,
      2 => ConsoleTiming::World,
      _ => ConsoleTiming::Dendy,
    };

    Ok(header)
  }
}
