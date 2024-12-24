use core::fmt;
use std::{cell::RefCell, fs, path::Path, rc::Rc};

use crate::mapper::{self, Dummy, Mapper};

#[derive(Debug, Default, Clone)]
pub struct INesHeader {
  pub prg_16kb_banks: usize,
  pub chr_8kb_banks: usize,

  pub prg_size: usize,
  pub chr_size: usize,
  pub uses_chr_ram: bool,
  pub has_battery: bool,

  pub mapper: u8,
  pub mapper_name: &'static str,
  pub mirroring: Mirroring,
  pub has_alt_nametbl: bool,
  
  pub has_trainer: bool,
  pub is_nes2_0: bool,
}

const NES_MAGIC: [u8; 4] = [0x4E, 0x45, 0x53, 0x1A];
const HEADER_SIZE: usize = 16;
const PRG_ROM_PAGE_SIZE: usize = 1024 * 16;
const CHR_ROM_PAGE_SIZE: usize = 1024 * 8;

#[derive(Debug, Default, Clone, Copy)]
pub enum Mirroring { 
  #[default] Horizontal, 
  Vertical,
  SingleScreenA, 
  SingleScreenB, 
  FourScreen
}

impl INesHeader {
  pub fn new(rom: &[u8]) -> Result<Self, &'static str> {
    let magic_str = &rom[0..=3];

    if magic_str != NES_MAGIC {
      return Err("Not a valid iNES/Nes2.0 rom");
    }

    let prg_16kb_banks = rom[4] as usize;
    let chr_8kb_banks = if rom[5] > 0 { rom[5] } else { 1 } as usize;
    let uses_chr_ram = rom[5] == 0;

    let prg_size = prg_16kb_banks as usize * PRG_ROM_PAGE_SIZE;
    let chr_size = if !uses_chr_ram { 
      chr_8kb_banks as usize * CHR_ROM_PAGE_SIZE
    } else { CHR_ROM_PAGE_SIZE };
    
    let nametbl_mirroring = rom[6] & 1;
    let has_alt_nametbl = rom[6] & 0b0000_1000 != 0;
    let mirroring = match (nametbl_mirroring, has_alt_nametbl)  {
      (_, true)   => Mirroring::FourScreen,
      (0, false)  => Mirroring::Horizontal,
      (1, false)  => Mirroring::Vertical,
      _ => unreachable!()
    };

    let has_battery = rom[6] & 0b0000_0010 != 0;
    let has_trainer = rom[6] & 0b0000_0100 != 0;

    let mapper_low = rom[6] >> 4;
    let mapper_high = rom[7] & 0b1111_0000;
    let mapper = mapper_high | mapper_low;
    let mapper_name = mapper::mapper_name(mapper as u16);

    let is_nes2_0 = rom[7] & 0b0000_1100 == 0x8;

    Ok(INesHeader {
      prg_16kb_banks,
      chr_8kb_banks,
      prg_size,
      chr_size,
      uses_chr_ram,
      has_trainer,
      has_battery,
      is_nes2_0,
      mirroring,
      has_alt_nametbl,
      mapper,
      mapper_name,
    })
  }
}


#[derive(Debug, Default, Clone)]
pub struct Nes2_0Header {
  #[allow(unused)]
  pub prg_16kb_banks: usize,
  pub chr_8kb_banks: usize,

  pub prg_size: usize,
  pub chr_size: usize,

  pub prg_ram_size: usize,
  pub eeprom_size: usize,
  pub chr_ram_size: usize,
  pub chr_nvram_size: usize,
  pub has_battery: bool,
  
  pub mirroring: Mirroring,
  pub has_alt_nametbl: bool,
  
  pub mapper: u16,
  pub submapper: u8,
  pub mapper_name: &'static str,
  
  pub has_trainer: bool,
  pub console_type: ConsoleType,
  pub timing: ConsoleTiming,
}

#[derive(Debug, Default, Clone, Copy)]
pub enum TvSystem { #[default] NTSC, PAL }
#[derive(Debug, Default, Clone, Copy)]
pub enum ConsoleType { #[default] NES, VsSystem, Playchoice10, Other }
#[derive(Debug, Default, Clone, Copy)]
pub enum ConsoleTiming { #[default] NTSC, PAL, World, Dendy, Unknown }

impl Nes2_0Header {
  pub fn new(rom: &[u8], ines: &INesHeader) -> Result<Self, &'static str> {
    if rom[9] & 0b1111 == 0xF || rom[9] >> 4 == 0xF {
      return Err("NES 2.0 'exponent-multiplier' notation for rom sizes not implemented")
    }

    let console_type = match rom[7] & 0b11 {
      0 => ConsoleType::NES,
      1 => ConsoleType::VsSystem,
      2 => ConsoleType::Playchoice10,
      _ => ConsoleType::Other
    };
    
    let mapper = ((rom[8] as u16 & 0b111) << 8) | ines.mapper as u16;
    let submapper = rom[8] >> 4;
    let mapper_name = mapper::mapper_name(mapper);

    let prg_16kb_banks = ((rom[9] as usize & 0b1111) << 8) + rom[4] as usize;
    let chr_8kb_banks = ((rom[9] as usize >> 4) << 8)     + rom[5] as usize;

    let prg_size = prg_16kb_banks * PRG_ROM_PAGE_SIZE;
    let chr_size = chr_8kb_banks * CHR_ROM_PAGE_SIZE;

    let prg_ram_size   = if rom[10] & 0b0000_1111 == 0 { 0 } else {64 << (rom[10] & 0b0000_1111)};
    let eeprom_size    = if rom[10] & 0b1111_0000 == 0 { 0 } else {64 << (rom[10] >> 4)};
    let chr_ram_size   = if rom[11] & 0b0000_1111 == 0 { 0 } else {64 << (rom[11] & 0b0000_1111)};
    let chr_nvram_size = if rom[11] & 0b1111_0000 == 0 { 0 } else {64 << (rom[11] >> 4)};

    let timing = match rom[12] & 0b11 {
      0 => ConsoleTiming::NTSC,
      1 => ConsoleTiming::PAL,
      2 => ConsoleTiming::World,
      3 => ConsoleTiming::Dendy,
      _ => unreachable!()
    };

    Ok(Self {
      console_type,
      mapper,
      submapper,
      mapper_name,
      mirroring: ines.mirroring,
      has_battery: ines.has_battery,
      has_trainer: ines.has_trainer,
      has_alt_nametbl: ines.has_alt_nametbl,

      prg_16kb_banks,
      chr_8kb_banks,
      prg_size,
      chr_size,

      prg_ram_size,
      eeprom_size,
      chr_ram_size,
      chr_nvram_size,
      timing
    })
  }
}

pub trait Header: fmt::Debug {
  fn has_trainer(&self) -> bool;
  fn has_battery(&self) -> bool;
  fn has_chr_ram(&self) -> bool;
  fn prg_size(&self) -> usize;
  fn chr_size(&self) -> usize;
  fn chr_ram_size(&self) -> usize;
  fn sram_size(&self) -> usize;
  fn mirroring(&self) -> Mirroring;
  fn mapper(&self) -> (u16, u8);
  fn timing(&self) -> ConsoleTiming;
}

impl Header for INesHeader {
  fn prg_size(&self) -> usize { self.prg_size }
  fn chr_size(&self) -> usize { self.chr_size }
  fn has_trainer(&self) -> bool { self.has_trainer }
  fn has_battery(&self) -> bool { self.has_battery }
  fn has_chr_ram(&self) -> bool { self.uses_chr_ram }
  // iNes doesn't have information about chr ram nor prg ram sizes. So they default to 8kb
  fn chr_ram_size(&self) -> usize { 8*1024 }
  fn sram_size(&self) -> usize { 8*1024 }
  fn mirroring(&self) -> Mirroring { self.mirroring }
  fn mapper(&self) -> (u16, u8) { (self.mapper as u16, 0) }
  fn timing(&self) -> ConsoleTiming { ConsoleTiming::Unknown }
}

impl Header for Nes2_0Header {
  fn prg_size(&self) -> usize { self.prg_size }
  fn chr_size(&self) -> usize { self.chr_size }
  fn has_trainer(&self) -> bool { self.has_trainer }
  fn has_battery(&self) -> bool { self.has_battery }
  // TODO: figure out if this is the correct thing to do
  fn has_chr_ram(&self) -> bool { self.chr_ram_size > 0 && self.chr_size == 0 }
  fn chr_ram_size(&self) -> usize { self.chr_ram_size }
  fn sram_size(&self) -> usize { self.prg_ram_size }
  fn mirroring(&self) -> Mirroring { self.mirroring }
  fn mapper(&self) -> (u16, u8) { (self.mapper, self.submapper) }
  fn timing(&self) -> ConsoleTiming { self.timing }
}

pub type SharedCart = Rc<RefCell<Cart>>;
pub struct Cart {
  pub header: Box<dyn Header>,
  pub prg: Vec<u8>,
  pub chr: Vec<u8>,
  pub mapper: Box<dyn Mapper>,
}

impl Cart {
  pub fn new(rom: &[u8]) -> Result<Self, String> {
    if rom.len() < HEADER_SIZE {
      return Err("Rom file is too small".to_string());
    }
    
    let ines = INesHeader::new(&rom)?;

    let header: Box<dyn Header> = if ines.is_nes2_0 {
      match Nes2_0Header::new(&rom, &ines) {
        Ok(nes2_0) => Box::new(nes2_0),
        Err(e) => {
          eprintln!("Couldn't parse Nes2.0 header: {e}\nWill be used iNes header instead.");
          Box::new(ines)
        }
      }
    } else { Box::new(ines) };
    
    println!("Loaded ROM: {:#?}", header);

    let prg_start = HEADER_SIZE + if header.has_trainer() { 512 } else { 0 };
    let chr_start = prg_start + header.prg_size();

    let prg = rom[prg_start..chr_start].to_vec();
    
    let chr = if header.has_chr_ram() {
      let mut chr = Vec::new();
      chr.resize(header.chr_ram_size(), 0);
      chr
    }
    else { 
      rom[chr_start..chr_start+header.chr_size()].to_vec()
    };

    let mapper = mapper::new_mapper_from_id(header.mapper().0)?;
    Ok(Cart { header, prg, chr, mapper })
  }

  pub fn from_file(rom_path: &Path) -> Result<Self, String> {
    let rom = fs::read(rom_path).map_err(|e| format!("Couldn't open rom: {e}"))?;
    Cart::new(&rom)
  }
  
  pub fn empty() -> Self {
    Cart { header: Box::new(INesHeader::default()), prg: Vec::new(), chr: Vec::new(), mapper: Box::new(Dummy) }
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
    self.mapper.mirroring().unwrap_or(self.header.mirroring())
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
      let cart = INesHeader::new(&rom);
      match cart {
        Ok(cart) => println!("{:?}", cart),
        Err(e) => println!("{e}")
      }
      println!()
    }
  }
}