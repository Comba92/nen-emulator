use std::{fs, path::Path};

#[derive(Debug)]
pub struct Cart {
  pub header: CartHeader,
  pub prg_rom: Vec<u8>,
  pub chr_rom: Vec<u8>,
}

#[derive(Debug, Default)]
pub struct CartHeader {
  pub prg_size: usize,
  pub chr_size: usize,
  pub has_trainer: bool,
}

const ASCII_NES: [u8; 4] = [0x4E, 0x45, 0x53, 0x1A];
const HEADER_SIZE: usize = 16;
const PRG_ROM_PAGE_SIZE: usize = 1024 * 16;
const CHR_ROM_PAGE_SIZE: usize = 1024 * 8;

impl CartHeader {
  pub fn new(rom: &Vec<u8>) -> Self {
    let magic = &rom[0..=3];

    if magic != ASCII_NES {
      panic!("Not a valid NES rom");
    }

    let prg_size = rom[4] as usize * PRG_ROM_PAGE_SIZE;
    let chr_size = rom[5] as usize * CHR_ROM_PAGE_SIZE;
    let has_trainer = rom[6] & 0b0000_0100 != 0;

    CartHeader {
      prg_size,
      chr_size,
      has_trainer,
    }
  }
}

impl Cart {
  pub fn new(rom_path: &Path) -> Self {
    let rom = fs::read(rom_path)
      .expect(format!("Couldn't locate rom file at {:?}", rom_path).as_str());
    if rom.len() < HEADER_SIZE {
      panic!("Rom file is too small");
    }
    
    let header = CartHeader::new(&rom);
    let prg_start = if header.has_trainer { 512 } else { 0 };
    let chr_start = prg_start + header.prg_size as usize;

    if rom.len() < HEADER_SIZE + chr_start + header.chr_size {
      panic!("Rom file is too small");
    }

    let prg_rom = rom[prg_start..chr_start].to_vec();
    let chr_rom = rom[chr_start..chr_start+header.chr_size].to_vec();

    Cart { header, prg_rom, chr_rom }
  }

  pub fn empty() -> Self {
    Cart { header: CartHeader::default(), prg_rom: Vec::new(), chr_rom: Vec::new() }
  }
}