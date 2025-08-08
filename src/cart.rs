use crate::emu::Mirroring;

#[derive(Default)]
pub struct Cart {
  pub header: CartHeader,
  pub prg: Vec<u8>,
  pub chr: Vec<u8>,
}

#[derive(Default, Debug)]
pub struct CartHeader {
  pub prg_size: usize,
  pub chr_size: usize,
  pub mirroring: Mirroring,
  pub mapper: u8,
  pub has_trainer: bool,
  pub has_chr_ram: bool,
}

const MAGIC: &[u8] = &[0x4e, 0x45, 0x53, 0x1a];
const HEADER_SIZE: usize = 16;
const TRAINER_SIZE: usize = 16;

impl Cart {
  pub fn new(bytes: &[u8]) -> Result<Self, &'static str> {
    if bytes.len() < HEADER_SIZE || &bytes[0..4] != MAGIC { return Err("not a valid iNES ROM"); }

    let mut header = CartHeader::default();
    
    header.prg_size = bytes[4] as usize * 16 * 1024;
    header.has_chr_ram = bytes[5] == 0;
    header.chr_size = if header.has_chr_ram { 8 * 1024 } else { bytes[5] as usize * 16 * 1024 };

    header.mirroring = match bytes[6] & 1 {
      0 => Mirroring::Horizontal,
      _ => Mirroring::Vertical
    };
    header.mapper = (bytes[7] & 0xf0) | (bytes[6] >> 4);
    header.has_trainer = bytes[6] & 0x4 != 0;
    
    let rom_start = if header.has_trainer { HEADER_SIZE + TRAINER_SIZE } else { HEADER_SIZE };
    let prg = bytes[rom_start..rom_start+header.prg_size].to_vec();
    let chr = if header.has_chr_ram {
      vec![0; 8 * 1024]
    } else {
      bytes[rom_start+header.prg_size..].to_vec()
    };

    println!("{:?}", header);

    Ok(Self {
      header,
      prg, chr,
    })
  }
}