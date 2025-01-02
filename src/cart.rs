use std::{cell::RefCell, rc::Rc};

use crate::mapper::{self, Dummy, Mapper};

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

const NES_MAGIC: [u8; 4] = [0x4E, 0x45, 0x53, 0x1A];
const HEADER_SIZE: usize = 16;
const PRG_ROM_PAGE_SIZE: usize = 1024 * 16;
const CHR_ROM_PAGE_SIZE: usize = 1024 * 8;

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum HeaderFormat { #[default] INes, Nes2_0 }
#[derive(Debug, Default, Clone, Copy)]
pub enum Mirroring { 
  #[default] Horizontal, 
  Vertical,
  SingleScreenA, 
  SingleScreenB, 
  FourScreen
}

#[derive(Debug, Default, Clone, Copy)]
pub enum ConsoleType { #[default] NES, VsSystem, Playchoice10, Other }
#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum ConsoleTiming { NTSC, PAL, World, Dendy, #[default] Unknown }
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
      _ => 1789773
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

impl CartHeader {
  pub fn new(rom: &[u8]) -> Result<Self, &'static str> {
    let mut header = CartHeader::default();

    let magic_str = &rom[0..=3];
    if magic_str != NES_MAGIC {
      return Err("Nintendo header magic values not found");
    }

    header.prg_16kb_banks = rom[4] as usize;
    header.chr_8kb_banks = if rom[5] > 0 { rom[5] } else { 1 } as usize;
    header.uses_chr_ram = rom[5] == 0;

    header.prg_size = header.prg_16kb_banks as usize * PRG_ROM_PAGE_SIZE;
    header.chr_size = header.chr_8kb_banks as usize * CHR_ROM_PAGE_SIZE;
    // iNes header doesn't hold information about chr ram size, so it defaults to 8kb if no chr rom is present
    header.chr_ram_size = if header.uses_chr_ram { CHR_ROM_PAGE_SIZE } else { 0 };
    
    let nametbl_mirroring = rom[6] & 1;
    header.has_alt_mirroring = rom[6] & 0b0000_1000 != 0;
    header.mirroring = match (nametbl_mirroring, header.has_alt_mirroring)  {
      (_, true)   => Mirroring::FourScreen,
      (0, false)  => Mirroring::Horizontal,
      (1, false)  => Mirroring::Vertical,
      _ => unreachable!()
    };

    header.has_battery = rom[6] & 0b0000_0010 != 0;
    header.has_trainer = rom[6] & 0b0000_0100 != 0;

    let mapper_low = rom[6] >> 4;
    let mapper_high = rom[7] & 0b1111_0000;
    header.mapper = (mapper_high | mapper_low) as u16;
    header.mapper_name = mapper::mapper_name(header.mapper).to_string();

    header.format = if rom[7] & 0b0000_1100 == 0x8 { HeaderFormat::Nes2_0 } else { HeaderFormat::INes };
    // This field was a later addition to iNes, so most games do not use it, even if they contain prg_ram.
    // If it is 0, prg ram is inferred as 8kb.
    header.prg_ram_size = if rom[8] > 0 { rom[8] } else { 8 } as usize * 1024;

    let title_start = HEADER_SIZE + header.prg_size-32;
    let title_bytes = &rom[title_start..title_start+16];
    header.game_title = String::from_utf8_lossy(title_bytes)
      .into_owned()
      .chars()
      .filter(|c| c.is_ascii_alphanumeric() || c.is_ascii_punctuation() || c.is_ascii_whitespace())
      .collect::<String>()
      .trim().to_string();

    if header.format == HeaderFormat::INes {
      return Ok(header);
    }

    if rom[9] & 0b1111 == 0xF || rom[9] >> 4 == 0xF {
      return Err("NES 2.0 'exponent-multiplier' notation for rom sizes not implemented")
    }

    header.console_type = match rom[7] & 0b11 {
      0 => ConsoleType::NES,
      1 => ConsoleType::VsSystem,
      2 => ConsoleType::Playchoice10,
      _ => ConsoleType::Other
    };
    
    header.mapper = ((rom[8] as u16 & 0b111) << 8) | header.mapper as u16;
    header.submapper = rom[8] >> 4;
    // header.mapper_name = mapper::mapper_name(header.mapper).to_string();

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
    }

    header.prg_16kb_banks = ((rom[9] as usize & 0b1111) << 8) + rom[4] as usize;
    header.chr_8kb_banks  = ((rom[9] as usize >> 4) << 8)     + rom[5] as usize;

    header.prg_size = header.prg_16kb_banks * PRG_ROM_PAGE_SIZE;
    header.chr_size = header.chr_8kb_banks * CHR_ROM_PAGE_SIZE;

    header.prg_ram_size   = if rom[10] & 0b0000_1111 == 0 { 0 } else {64 << (rom[10] & 0b0000_1111)};
    header.eeprom_size    = if rom[10] & 0b1111_0000 == 0 { 0 } else {64 << (rom[10] >> 4)};
    header.chr_ram_size   = if rom[11] & 0b0000_1111 == 0 { 0 } else {64 << (rom[11] & 0b0000_1111)};
    header.chr_nvram_size = if rom[11] & 0b1111_0000 == 0 { 0 } else {64 << (rom[11] >> 4)};

    header.timing = match rom[12] & 0b11 {
      0 => ConsoleTiming::NTSC,
      1 => ConsoleTiming::PAL,
      2 => ConsoleTiming::World,
      _ => ConsoleTiming::Dendy,
    };

    Ok(header)
  }
}

pub type SharedCart = Rc<RefCell<Cart>>;
pub struct Cart {
  pub header: CartHeader,
  pub prg: Vec<u8>,
  pub chr: Vec<u8>,
  pub mapper: Box<dyn Mapper>,
}

impl Cart {
  pub fn new(rom: &[u8]) -> Result<Self, String> {
    if rom.len() < HEADER_SIZE {
      return Err("File too small to contain a 16 bytes header".to_string());
    }
    
    let header = CartHeader::new(&rom)
      .map_err(|e| format!("Not a valid iNES/Nes2.0 rom, {e}"))?;

    println!("Loaded ROM: {:#?}", header);
    if header.format == HeaderFormat::INes 
      && (header.mapper == 1 || header.mapper == 5)
    {
      eprintln!("WARNING: this game is using the {} mapper, and the rom file has a iNes header. \
        Compatibility is not garanteed. A rom file with a Nes2.0 header is preferred.", header.mapper_name);
    }

    let prg_start = HEADER_SIZE + if header.has_trainer { 512 } else { 0 };
    let chr_start = prg_start + header.prg_size;

    let prg = rom[prg_start..chr_start].to_vec();
    let chr = if header.uses_chr_ram {
      let mut chr = Vec::new();
      chr.resize(header.chr_ram_size, 0);
      chr
    }
    else { 
      rom[chr_start..chr_start+header.chr_size].to_vec()
    };

    let sram_size = if header.has_battery && header.eeprom_size > 0 { 
      header.eeprom_size
    } else { header.prg_ram_size };

    let mapper = mapper::new_mapper(header.mapper, header.submapper, sram_size)?;
    Ok(Cart { header, prg, chr, mapper })
  }
  
  pub fn empty() -> Self {
    Cart { header: CartHeader::default(), prg: Vec::new(), chr: Vec::new(), mapper: Box::new(Dummy) }
  }

  pub fn cart_read(&mut self, addr: usize) -> u8 {
    self.mapper.cart_read(addr)
  }
  pub fn cart_write(&mut self, addr: usize, val: u8) {
    self.mapper.cart_write(addr, val);
  }

  pub fn prg_read(&mut self, addr: usize) -> u8 {
    self.mapper.prg_read(&self.prg, addr)
  }
  pub fn prg_write(&mut self, addr: usize, val: u8) {
    self.mapper.prg_write(&mut self.prg, addr, val);
  }

  pub fn chr_read(&mut self, addr: usize) -> u8 {
    self.mapper.chr_read(&self.chr, addr)
  }
  pub fn chr_write(&mut self, addr: usize, val: u8) {
    self.mapper.chr_write(&mut self.chr, addr, val);
  }

  pub fn vram_read(&mut self, vram: &[u8], addr: usize) -> u8 {
    vram[self.mirror_vram(addr)]
  }

  pub fn vram_write(&self, vram: &mut [u8], addr: usize, val: u8) {
    vram[self.mirror_vram(addr)] = val;
  }

  // Horizontal:
	// 0x0800 [ B ]  [ A ] [ a ]
	// 0x0400 [ A ]  [ B ] [ b ]

	// Vertical:
	// 0x0800 [ B ]  [ A ] [ B ]
	// 0x0400 [ A ]  [ a ] [ b ]

	// Single-page: (based on mapper register)
	// 0x0800 [ B ]  [ A ] [ a ]    [ B ] [ b ]
	// 0x0400 [ A ]  [ a ] [ a ] or [ b ] [ b ]
  pub fn mirror_vram(&self, addr: usize) -> usize {
    let addr = addr - 0x2000;
		let nametbl_idx = addr / 0x400;

		let mirroring = self.mirroring();
    
		use Mirroring::*;
		match (mirroring, nametbl_idx) {
			(Horizontal, 1) | (Horizontal, 2) => addr - 0x400,
			(Horizontal, 3) => addr - 0x400 * 2,
			(Vertical, 2) | (Vertical, 3) => addr - 0x400 * 2,
			(SingleScreenA, _) => addr % 0x400,
			(SingleScreenB, _) => (addr % 0x400) + 0x400,
			// TODO: eventually implement this
			(FourScreen, _) => todo!("Four screen mirroring not implemented"),
			_ => addr,
		}
  }

  pub fn mirroring(&self) -> Mirroring {
    self.mapper.mirroring().unwrap_or(self.header.mirroring)
  }
}

#[cfg(test)]
mod cart_tests {
    use std::fs;
    use super::*;

  #[test]
  fn read_headers() {
    let mut roms = fs::read_dir("./roms/").unwrap();
    while let Some(Ok(file)) = roms.next() {
      let rom = fs::read(file.path()).unwrap();
      let cart = CartHeader::new(&rom);
      match cart {
        Ok(cart) => println!("{:?}", cart),
        Err(e) => println!("{e}")
      }
      println!()
    }
  }
}