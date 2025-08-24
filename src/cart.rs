use crate::emu::{Mirroring, Region};

#[derive(Default)]
pub struct Cart {
  pub header: CartHeader,
  pub prg: Vec<u8>,
  pub chr: Vec<u8>,
}

// https://www.nesdev.org/wiki/INES
#[derive(Default, Debug, Clone)]
pub struct CartHeader {
  pub prg_size: usize,
  pub chr_size: usize,
  pub prg_ram_size: usize,
  pub prg_nvram_size: usize,
  
  pub mirroring: Mirroring,
  pub region: Region,
  pub alt_mirroring: bool,
  
  pub mapper: u16,
  pub submapper: u8,
  pub extensions: u8,

  pub has_trainer: bool,
  pub has_chr_ram: bool,
  pub has_battery: bool,
  pub is_nes2_0: bool,
}

const MAGIC: &[u8] = &[0x4e, 0x45, 0x53, 0x1a];
const HEADER_SIZE: usize = 16;
const TRAINER_SIZE: usize = 16;

// TODO: UNIF support

impl Cart {
  pub fn new(bytes: &[u8]) -> Result<Self, &'static str> {
    if bytes.len() < HEADER_SIZE || &bytes[0..4] != MAGIC { return Err("not a valid iNES ROM"); }

    let mut header = CartHeader::default();
    
    header.prg_size = bytes[4] as usize * 16 * 1024;
    header.has_chr_ram = bytes[5] == 0;
    header.chr_size = if header.has_chr_ram { 8 * 1024 } else { bytes[5] as usize * 8 * 1024 };
    // we default wram to 8kb
    header.prg_ram_size = 8 * 1024;

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
      header.is_nes2_0 = true;
      header.mapper |= (bytes[8] as u16 & 0xf) << 8;
      header.submapper = bytes[8] >> 4;

      let prg_ram_shift = bytes[10] & 0xf;
      let prg_nvram_shift = bytes[10] >> 4;
      header.prg_ram_size = if prg_ram_shift == 0 { 8 * 1024 } else { 64 << prg_ram_shift };
      header.prg_nvram_size = if prg_nvram_shift == 0 { 0 } else { 64 << prg_nvram_shift };

      let chr_ram_shift = bytes[11] & 0xf;
      let chr_nvram_shift = bytes[11] >> 4;
      header.chr_size = if chr_ram_shift != 0 {
        64 << chr_ram_shift
      } else {
        64 << chr_nvram_shift
      };

      header.region = match bytes[12] & 0b11 {
        0 => Region::NTSC,
        1 => Region::PAL,
        2 => Region::World,
        _ => Region::Dendy,
      };
      header.extensions = bytes[15] & 0x3f;
    } else if version == 0 && bytes[12..=15].iter().all(|x| *x == 0) {
      // iNES with PRG RAM and TV system field
      header.prg_ram_size = if bytes[8] == 0 { 8 * 1024 } else { bytes[8] as usize * 8 * 1024 };
      header.region = match bytes[9] {
        1 => Region::PAL,
        _ => Region::NTSC,
      };
    }
    
    let rom_start = if header.has_trainer { HEADER_SIZE + TRAINER_SIZE } else { HEADER_SIZE };
    let prg = bytes[rom_start..rom_start+header.prg_size].to_vec();
    let chr = if header.has_chr_ram {
      vec![0; header.chr_size]
    } else {
      let chr_start = rom_start+header.prg_size;
      bytes[chr_start..chr_start + header.chr_size].to_vec()
    };

    println!("{:?}", header);

    Ok(Self {
      header,
      prg, chr,
    })
  }
}