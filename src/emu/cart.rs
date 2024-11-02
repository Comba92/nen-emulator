use std::{fs, path::Path};

#[derive(Debug)]
pub struct Cart {
  pub header: CartHeader,
  pub prg_rom: Vec<u8>,
  pub chr_rom: Vec<u8>,
}

#[derive(Debug, Default)]
pub struct CartHeader {
  pub prg_16kb_pages: usize,
  pub chr_8kb_pages: usize,
  pub prg_size: usize,
  pub chr_size: usize,
  pub has_trainer: bool,
  pub has_battery_prg: bool,
  pub has_alt_nametbl: bool,
  pub nametbl_layout: NametableLayout,
  pub mapper: u8,
}

const NES_STR: [u8; 4] = [0x4E, 0x45, 0x53, 0x1A];
const HEADER_SIZE: usize = 16;
const PRG_ROM_PAGE_SIZE: usize = 1024 * 16;
const CHR_ROM_PAGE_SIZE: usize = 1024 * 8;

#[derive(Debug, Default)]
pub enum NametableLayout { Vertical, Horizontal, #[default] None }

impl CartHeader {
  pub fn new(rom: &[u8]) -> Self {
    let magic_str = &rom[0..=3];

    if magic_str != NES_STR {
      panic!("Not a valid NES rom");
    }

    let prg_16kb_pages = rom[4] as usize;
    let chr_8kb_pages = rom[5] as usize;

    let prg_size = rom[4] as usize * PRG_ROM_PAGE_SIZE;
    let chr_size = rom[5] as usize * CHR_ROM_PAGE_SIZE;
    
    let nametbl_layout = match rom[6] & 1 {
      0 => NametableLayout::Vertical,
      1 => NametableLayout::Horizontal,
      _ => unreachable!()
    };

    let has_battery_prg = rom[6] & 0b0000_0010 != 0;
    let has_trainer = rom[6] & 0b0000_0100 != 0;
    let has_alt_nametbl = rom[6] & 0b0000_1000 != 0;

    let mapper_low = rom[6] & 0b1111_0000 >> 4;
    let mapper_high = rom[7] & 0b1111_0000;
    let mapper = mapper_high | mapper_low;

    CartHeader {
      prg_16kb_pages,
      chr_8kb_pages,
      prg_size,
      chr_size,
      has_trainer,
      has_battery_prg,
      nametbl_layout,
      has_alt_nametbl,
      mapper,
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
    
    let header = CartHeader::new(&rom[0..16]);
    let prg_start = if header.has_trainer { 16 + 512 } else { 16 };
    let chr_start = prg_start + header.prg_size as usize;

    let prg_rom = rom[prg_start..chr_start].to_vec();
    let chr_rom = rom[chr_start..chr_start+header.chr_size].to_vec();

    Cart { header, prg_rom, chr_rom }
  }

  pub fn empty() -> Self {
    Cart { header: CartHeader::default(), prg_rom: Vec::new(), chr_rom: Vec::new() }
  }
}