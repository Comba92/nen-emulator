use std::{cell::RefCell, fs, path::Path, rc::Rc};

use crate::mapper::{self, CartMapper, Dummy};

#[derive(Debug, Default, Clone)]
pub struct CartHeader {
  pub prg_16kb_banks: usize,
  pub chr_8kb_banks: usize,
  pub prg_size: usize,
  pub chr_size: usize,
  pub uses_chr_ram: bool,
  pub has_trainer: bool,
  pub has_battery_prg: bool,
  pub has_alt_nametbl: bool,
  pub is_nes_v2: bool,
  pub tv_system: TvSystem,
  pub nametbl_mirroring: Mirroring,
  pub mapper: u8,
}

const NES_STR: [u8; 4] = [0x4E, 0x45, 0x53, 0x1A];
const HEADER_SIZE: usize = 16;
const PRG_ROM_PAGE_SIZE: usize = 1024 * 16;
const CHR_ROM_PAGE_SIZE: usize = 1024 * 8;

#[derive(Debug, Default, Clone, Copy)]
pub enum Mirroring { #[default] Horizontally, Vertically, FourScreen }
#[derive(Debug, Default, Clone, Copy)]
pub enum TvSystem { #[default] NTSC, PAL }

impl CartHeader {
  pub fn new(rom: &[u8]) -> Result<Self, &'static str> {
    let magic_str = &rom[0..=3];

    if magic_str != NES_STR {
      return Err("Not a valid iNES rom");
    }

    let prg_16kb_banks = rom[4] as usize;
    let chr_8kb_banks = rom[5] as usize;
    let uses_chr_ram = chr_8kb_banks == 0;

    let prg_size = rom[4] as usize * PRG_ROM_PAGE_SIZE;
    let chr_size = if rom[5] > 0 { rom[5] as usize * CHR_ROM_PAGE_SIZE} else { 8*1024 };
    
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

    let is_nes_v2 = rom[7] & 0b0000_1100 != 0;
    let tv_system = match rom[9] & 1 {
      0 => TvSystem::NTSC,
      1 => TvSystem::PAL,
      _ => unreachable!()
    };

    Ok(CartHeader {
      prg_16kb_banks,
      chr_8kb_banks,
      prg_size,
      chr_size,
      uses_chr_ram,
      has_trainer,
      has_battery_prg,
      is_nes_v2,
      tv_system,
      nametbl_mirroring: nametbl_layout,
      has_alt_nametbl,
      mapper,
    })
  }
}


#[derive(Clone)]
pub struct Cart {
  pub header: CartHeader,
  pub prg_rom: Vec<u8>,
  pub chr_rom: Vec<u8>,
  pub mapper: CartMapper,
}

impl Cart {
  pub fn from_file(rom_path: &Path) -> Result<Self, String> {
    let rom = fs::read(rom_path).map_err(|e| format!("Couldn't open rom: {e}"))?;
    Cart::new(&rom)
  }

  pub fn new(rom: &[u8]) -> Result<Self, String> {
    if rom.len() < HEADER_SIZE {
      return Err("Rom file is too small".to_string());
    }
    
    let header = CartHeader::new(&rom[0..16])?;

    println!("{:#?}", header);
    if header.is_nes_v2 {
      eprintln!("This rom has NES 2.0 format! Might not run correctly...")
      // return Err("NES 2.0 format not supported");
    }

    let prg_start = HEADER_SIZE + if header.has_trainer { 512 } else { 0 };
    let chr_start = prg_start + header.prg_size;

    let prg_rom = rom[prg_start..chr_start].to_vec();
    let chr_rom = if header.uses_chr_ram { [0; 8*1024].to_vec() }
      else { rom[chr_start..chr_start+header.chr_size].to_vec() };

    let mapper = mapper::new_mapper_from_id(header.mapper)?;
    Ok(Cart { header, prg_rom, chr_rom, mapper })
  }

  pub fn empty() -> Self {
    Cart { header: CartHeader::default(), prg_rom: Vec::new(), chr_rom: Vec::new(), mapper: Rc::new(RefCell::new(Dummy)) }
  }
}