use core::cell::RefCell;
use std::{fs, path::Path, rc::Rc};

use crate::mapper::{self, CartMapper, Dummy};

#[derive(Debug, Default, Clone)]
pub struct INesHeader {
  pub prg_16kb_banks: usize,
  pub chr_8kb_banks: usize,
  pub prg_size: usize,
  pub chr_size: usize,
  pub uses_chr_ram: bool,
  pub sram_size: usize,
  pub has_trainer: bool,
  pub has_battery_prg: bool,
  pub has_alt_nametbl: bool,
  pub is_nes_v2: bool,
  pub console_type: ConsoleType,
  pub tv_system: TvSystem,
  pub nametbl_mirroring: Mirroring,
  pub mapper: u8,
  pub nes20_header: Option<Nes20Header>,
}

const NES_STR: [u8; 4] = [0x4E, 0x45, 0x53, 0x1A];
const HEADER_SIZE: usize = 16;
const PRG_ROM_PAGE_SIZE: usize = 1024 * 16;
const CHR_ROM_PAGE_SIZE: usize = 1024 * 8;

#[derive(Debug, Default, Clone, Copy)]
pub enum Mirroring { #[default] Horizontally, Vertically, SingleScreenFirstPage, SingleScreenSecondPage, FourScreen }
#[derive(Debug, Default, Clone, Copy)]
pub enum TvSystem { #[default] NTSC, PAL }
#[derive(Debug, Default, Clone, Copy)]
pub enum ConsoleType { #[default] NES, VsSystem, Playchoice10, Other }
#[derive(Debug, Default, Clone, Copy)]
pub enum ConsoleTiming { #[default] NTSC, PAL, World, Dendy }

// TODO: parse this shit with nom
impl INesHeader {
  pub fn new(rom: &[u8]) -> Result<Self, &'static str> {
    let magic_str = &rom[0..=3];

    if magic_str != NES_STR {
      return Err("Not a valid iNES rom");
    }

    let prg_16kb_banks = rom[4] as usize;
    let chr_8kb_banks = if rom[5] > 0 { rom[5] } else { 1 } as usize;
    let uses_chr_ram = rom[5] == 0;

    let prg_size = rom[4] as usize * PRG_ROM_PAGE_SIZE;
    let chr_size = if !uses_chr_ram { rom[5] as usize * CHR_ROM_PAGE_SIZE} else { CHR_ROM_PAGE_SIZE };
    
    let nametbl_mirroring = rom[6] & 1;
    let has_alt_nametbl = rom[6] & 0b0000_1000 != 0;
    let nametbl_layout = match (nametbl_mirroring, has_alt_nametbl)  {
      (_, true)   => Mirroring::FourScreen,
      (0, false)  => Mirroring::Horizontally,
      (1, false)  => Mirroring::Vertically,
      _ => unreachable!()
    };

    let has_battery_prg = rom[6] & 0b0000_0010 != 0;
    let has_trainer = rom[6] & 0b0000_0100 != 0;

    let mapper_low = rom[6] >> 4;
    let mapper_high = rom[7] & 0b1111_0000;
    let mapper = mapper_high | mapper_low;

    let console_type = match rom[7] & 11 {
      0 => ConsoleType::NES,
      1 => ConsoleType::VsSystem,
      2 => ConsoleType::Playchoice10,
      _ => ConsoleType::Other,
    };

    let is_nes_v2 = rom[7] & 0b0000_1100 == 0x8;
    let sram_size = if rom[8] > 0 { rom[8] as usize } else { CHR_ROM_PAGE_SIZE };

    let tv_system = match rom[9] & 1 {
      0 => TvSystem::NTSC,
      1 => TvSystem::PAL,
      _ => unreachable!()
    };

    Ok(INesHeader {
      prg_16kb_banks,
      chr_8kb_banks,
      prg_size,
      chr_size,
      uses_chr_ram,
      sram_size,
      has_trainer,
      has_battery_prg,
      is_nes_v2,
      console_type,
      tv_system,
      nametbl_mirroring: nametbl_layout,
      has_alt_nametbl,
      mapper,
      nes20_header: if is_nes_v2 { Some(Nes20Header::new(rom)?) } else { None },
    })
  }
}

#[allow(dead_code)]
#[derive(Debug, Default, Clone)]
pub struct Nes20Header {
  submapper: u8,
  prg_rom_size: usize,
  chr_rom_size: usize,
  prg_ram_size: usize,
  eeprom_size: usize,
  chr_ram_size: usize,
  chr_nvram_size: usize,
  timing: ConsoleTiming,
}

impl Nes20Header {
  pub fn new(rom: &[u8]) -> Result<Self, &'static str> {
    let submapper = rom[8] >> 4;
    
    if rom[9] & 0b1111 == 0b1111 || rom[9] >> 4 == 0b1111 {
      return Err("NES 2.0 'exponent-multiplier' notation for rom sizes not implemented")
    }

    let prg_extended = ((rom[9] as usize & 0b1111) << 8) + rom[4] as usize; 
    let chr_extended = ((rom[9] as usize >> 4) << 8) + rom[5] as usize;

    let prg_rom_size = prg_extended * PRG_ROM_PAGE_SIZE;
    let chr_rom_size = chr_extended * CHR_ROM_PAGE_SIZE;

    let prg_ram_size = if rom[10] & 0b0000_1111 == 0 { 0 } else {64 << (rom[10] & 0b0000_1111)};
    let eeprom_size = if rom[10] & 0b1111_0000 == 0 { 0 } else {64 << (rom[10] >> 4)};
    let chr_ram_size = if rom[11] & 0b0000_1111 == 0 { 0 } else {64 << (rom[11] & 0b0000_1111)};
    let chr_nvram_size = if rom[11] & 0b1111_0000 == 0 { 0 } else {64 << (rom[11] >> 4)};

    let timing = match rom[12] & 0b11 {
      0 => ConsoleTiming::NTSC,
      1 => ConsoleTiming::PAL,
      2 => ConsoleTiming::World,
      3 => ConsoleTiming::Dendy,
      _ => unreachable!()
    };

    Ok(Nes20Header {
      submapper, prg_rom_size, chr_rom_size, prg_ram_size, eeprom_size, chr_ram_size, chr_nvram_size, timing
    })
  }
}

#[derive(Clone)]
pub struct Cart {
  pub header: INesHeader,
  pub prg_rom: Vec<u8>,
  pub chr_rom: Vec<u8>,
  pub mapper: CartMapper,
}

impl Cart {
  pub fn new(rom: &[u8]) -> Result<Self, String> {
    if rom.len() < HEADER_SIZE {
      return Err("Rom file is too small".to_string());
    }
    
    let header = INesHeader::new(&rom)?;

    let prg_start = HEADER_SIZE + if header.has_trainer { 512 } else { 0 };
    let chr_start = prg_start + header.prg_size;

    let prg_rom = rom[prg_start..chr_start].to_vec();
    let chr_rom = if header.uses_chr_ram { [0; 8*1024].to_vec() }
      else { rom[chr_start..chr_start+header.chr_size].to_vec() };

    println!("Loaded ROM: {:#?}", header);

    let mapper = mapper::new_mapper_from_id(header.mapper)?;
    Ok(Cart { header, prg_rom, chr_rom, mapper })
  }

  pub fn from_file(rom_path: &Path) -> Result<Self, String> {
    let rom = fs::read(rom_path).map_err(|e| format!("Couldn't open rom: {e}"))?;
    Cart::new(&rom)
  }
  
  pub fn empty() -> Self {
    Cart { header: INesHeader::default(), prg_rom: Vec::new(), chr_rom: Vec::new(), mapper: Rc::new(RefCell::new(Dummy)) }
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