use std::marker::{self, PhantomData};

use crate::cart::{CartBanking, CartHeader, Mirroring, PrgTarget, VramTarget};

mod mmc1;
mod mmc2;
mod mmc3;
// mod mmc5;
mod konami_irq;
mod vrc2_4;
mod vrc3;
mod vrc6;
mod sunsoft_fme_7;
mod namco129_163;

use mmc1::MMC1;
use mmc2::MMC2;
use mmc3::MMC3;
use vrc2_4::VRC2_4;
use vrc3::VRC3;
use vrc6::VRC6;
use sunsoft_fme_7::SunsoftFME7;
use namco129_163::Namco129_163;

pub fn new_mapper(header: &CartHeader, banks: &mut CartBanking) -> Result<Box<dyn Mapper>, String> {
  let mapper: Box<dyn Mapper> = match header.mapper {
    0 => NROM::new(header, banks),
    1 => MMC1::new(header, banks),
    2 | 94 | 180 => UxROM::new(header, banks),
    3 => CNROM::new(header, banks),
    4 => MMC3::new(header, banks),
    // // 5 => MMC5::new(header, banks),
    7 => AxROM::new(header, banks),
    9 | 10 => MMC2::new(header, banks),
    11 => ColorDreams::new(header, banks),
    19 => Namco129_163::new(header, banks),
    21 | 22 | 23 | 25 => VRC2_4::new(header, banks),
    24 | 26 => VRC6::new(header, banks),
    31 => INesMapper031::new(header, banks),
    66 => GxROM::new(header, banks),
    69 => SunsoftFME7::new(header, banks),
    71 => Codemasters::new(header, banks),
    73 => VRC3::new(header, banks),
    75 => VRC1::new(header, banks),
    78 => INesMapper078::new(header, banks),
    _ => return Err(format!("Mapper {} not implemented", header.mapper))
  };

  Ok(mapper)
}

pub fn mapper_name(id: u16) -> &'static str {
  MAPPERS_TABLE.iter()
    .find(|m| m.0 == id)
    .map(|m| m.1)
    .unwrap_or("Not implemented")
}
const MAPPERS_TABLE: [(u16, &'static str); 33] = [
  (0, "NROM"),
  (1, "MMC1"),
  (2, "UxROM"),
  (3, "CNROM"),
  (4, "MMC3"),
  (5, "MMC5"),
  (7, "AxROM (Rare)"),
  (9, "MMC2 (Punch-Out!!)"),
  (10, "MMC4"),
  (11, "ColorDreams"),
  (16, "Bandai FCG"),
  (19, "Namco 129/163"),
  (21, "Konami VRC2/VRC4"),
  (22, "Konami VRC2/VRC4"),
  (23, "Konami VRC2/VRC4"),
  (24, "Konami VRC6a (Akumajou Densetsu)"),
  (25, "Konami VRC2/VRC4"),
  (26, "Konami VRC6b (Madara and Esper Dream 2)"),
  (31, "NSF"),
  (34, "BNROM/NINA-001"),
  (48, "Taito TC0690"),
  (66, "GxROM"),
  (68, "Sunsoft4"),
  (69, "Sunsoft5 FME-7"),
  (71, "Codemasters UNROM"),
  (73, "Konami VRC3 (Salamander)"),
  (75, "Konami VRC1"),
  (78, "Irem 74HC161 (Holy Diver and Cosmo Carrier)"),
  (91, "INesMapper091"),
  (94, "UNROM (Senjou no Ookami)"),
  (163, "FC-001"),
  (180, "UNROM (Crazy Climber)"),
  (210, "Namco 175/340"),
];

// Horizontal:
// 0x0800 [ B ]  [ A ] [ a ]
// 0x0400 [ A ]  [ B ] [ b ]

// Vertical:
// 0x0800 [ B ]  [ A ] [ B ]
// 0x0400 [ A ]  [ a ] [ b ]

// Single-page: (based on mapper register)
// 0x0800 [ B ]  [ A ] [ a ]    [ B ] [ b ]
// 0x0400 [ A ]  [ a ] [ a ] or [ b ] [ b ]
// pub fn mirror_nametbl( addr: usize) -> usize {
//   let addr = addr - 0x2000;
//   let nametbl_idx = addr / 0x400;
  
//   use Mirroring::*;
//   match (mirroring, nametbl_idx) {
//     (Horizontal, 1) | (Horizontal, 2) => addr - 0x400,
//     (Horizontal, 3) => addr - 0x400 * 2,
//     (Vertical, 2) | (Vertical, 3) => addr - 0x400 * 2,
//     (SingleScreenA, _) => addr % 0x400,
//     (SingleScreenB, _) => (addr % 0x400) + 0x400,
//     (FourScreen, _) => addr,
//     _ => addr,
//   }
// }


#[typetag::serde(tag = "mmu")]
pub trait Mapper {
  fn new(header: &CartHeader, banks: &mut CartBanking) -> Box<Self> where Self: Sized;
  fn write(&mut self, banks: &mut CartBanking, addr: usize, val: u8);

  fn map_prg_addr(&self, banks: &mut CartBanking, addr: usize) -> PrgTarget {
    match addr {
      0x4020..=0x5FFF => PrgTarget::Cart,
      0x6000..=0x7FFF => PrgTarget::SRam(true, banks.sram.addr(addr)),
      0x8000..=0xFFFF => PrgTarget::Prg(banks.prg.addr(addr)),
      _ => unreachable!()
    }
  }

  fn map_chr_addr(&mut self, banks: &mut CartBanking, addr: usize) -> VramTarget {
    match addr {
      0x0000..=0x1FFF => VramTarget::Chr(banks.chr.addr(addr)),
      0x2000..=0x2FFF => VramTarget::CiRam(banks.vram.addr(addr)),
      _ => unreachable!()
    }
  }

  fn cart_read(&mut self, _addr: usize) -> u8 { 0xFF }
  fn cart_write(&mut self, _banks: &mut CartBanking, _addr: usize, _val: u8) {}

  // Generic cpu cycle notify / apu extension clocking
  fn notify_cpu_cycle(&mut self) {}
  fn get_sample(&self) -> f32 { 0.0 }

  // Mmc3 scanline notify
  fn notify_scanline(&mut self) {}

  // Mmc5 ppu notify
  fn notify_ppuctrl(&mut self, _val: u8) {}
  fn notify_ppumask(&mut self, _val: u8) {}

  fn poll_irq(&mut self) -> bool { false }
}

#[derive(Debug, Default)]
pub struct PrgBanking;
#[derive(Debug, Default)]
pub struct ChrBanking;
#[derive(Debug, Default)]
pub struct SramBanking;
#[derive(Debug, Default)]
pub struct VramBanking;
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Banking<T> {
  data_size: usize,
  bank_size: usize,
  banks_count: usize,
  pages_start: usize,
  // TODO: probably can be just a Vec of u8
  bankings: Box<[usize]>,
  kind: marker::PhantomData<T>
}

impl<T> Banking<T> {
  pub fn new(rom_size: usize, pages_start: usize, page_size: usize, pages_count: usize) -> Self {
    let bankings = vec![0; pages_count].into_boxed_slice();
    let bank_size = page_size;
    let banks_count = rom_size / bank_size;
    Self { bankings, data_size: rom_size, pages_start, bank_size, banks_count, kind: PhantomData::<T> }
  }

  pub fn set(&mut self, page: usize, bank: usize) {
    let pages_count = self.bankings.len();
    self.bankings[page % pages_count] = (bank % self.banks_count) * self.bank_size;
  }

  pub fn swap(&mut self, left: usize, right: usize) {
    self.bankings.swap(left, right);
  }

  pub fn set_page_to_last_bank(&mut self, page: usize) {
    let pages_count = self.bankings.len();
    let last_bank = self.banks_count-1;
    self.bankings[page % pages_count] = last_bank * self.bank_size;
  }

  fn page_to_bank_addr(&self, page: usize, addr: usize) -> usize {
    let pages_count = self.bankings.len();
    self.bankings[page % pages_count] + (addr % self.bank_size)
  }

  pub fn addr(&self, addr: usize) -> usize {
    let page = (addr - self.pages_start) / self.bank_size;
    self.page_to_bank_addr(page, addr)
  }
}

impl Banking<PrgBanking> {
  pub fn new_prg(header: &CartHeader, pages_count: usize) -> Self {
    let pages_size = 32*1024 / pages_count;
    Self::new(header.prg_size, 0x8000, pages_size, pages_count)
  }
}

impl Banking<SramBanking> {
  pub fn new_sram(header: &CartHeader) -> Self {
    Self::new(header.sram_real_size(), 0x6000, 8*1024, 1)
  }
}

impl Banking<ChrBanking> {
  pub fn new_chr(header: &CartHeader, pages_count: usize) -> Self {
    let pages_size = 8*1024 / pages_count;
    Self::new(header.chr_real_size(), 0, pages_size, pages_count)
  }
}

impl Banking<VramBanking> {
  pub fn new_vram(header: &CartHeader) -> Self {
    let mut res = Self::new(4*1024, 0x2000, 1024, 4);
    res.banks_count = 2;
    res.update(header.mirroring);
    res
  }

  pub fn update(&mut self, mirroring: Mirroring) {
    match mirroring {
      Mirroring::Horizontal => {
        self.set(0, 0);
        self.set(1, 0);
        self.set(2, 1);
        self.set(3, 1);
      }
      Mirroring::Vertical => {
        self.set(0, 0);
        self.set(1, 1);
        self.set(2, 0);
        self.set(3, 1);
      }
      Mirroring::SingleScreenA => for i in 0..4 {
        self.set(i, 0);
      }
      Mirroring::SingleScreenB => for i in 0..4 {
        self.set(i, 1);
      }
      Mirroring::FourScreen => for i in 0..4 {
        self.set(i, i);
      }
    }
  }
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Dummy;
#[typetag::serde]
impl Mapper for Dummy {
  fn new(_: &CartHeader, _: &mut CartBanking) -> Box<Self> {
    Box::new(Self)
  }
  fn write(&mut self, _: &mut CartBanking, _: usize, _: u8) {}
}

// Mapper 00
// https://www.nesdev.org/wiki/NROM
#[derive(serde::Serialize, serde::Deserialize)]
pub struct NROM;

#[typetag::serde]
impl Mapper for NROM {
  fn new(header: &CartHeader, banks: &mut CartBanking)-> Box<Self> {
    banks.prg = Banking::new_prg(header, 2);

    if header.prg_size <= 16*1024 {
      banks.prg.set(1, 0);
    } else {
      banks.prg.set(1, 1);
    }

    Box::new(Self)
  }

  fn write(&mut self, _: &mut CartBanking, _: usize, _: u8) {}
}

// Mapper 02
// https://www.nesdev.org/wiki/UxROM
#[derive(serde::Serialize, serde::Deserialize)]
pub struct UxROM {
  banked_page: u8,
}

#[typetag::serde]
impl Mapper for UxROM {
  fn new(header: &CartHeader, banks: &mut CartBanking)-> Box<Self> {
    banks.prg = Banking::new_prg(header, 2);

    // https://www.nesdev.org/wiki/INES_Mapper_180
    let banked_page = if header.mapper == 180 {
      1
    } else {
      banks.prg.set_page_to_last_bank(1);
      0
    };

    Box::new(Self { banked_page })
  }

  fn write(&mut self, banks: &mut CartBanking, _: usize, val: u8) {
    let select = val & 0b1111;
    banks.prg.set(self.banked_page as usize, select as usize);
  }
}

// Mapper 03
// https://www.nesdev.org/wiki/INES_Mapper_003
#[derive(serde::Serialize, serde::Deserialize)]
pub struct CNROM;

#[typetag::serde]
impl Mapper for CNROM {
  fn new(header: &CartHeader, banks: &mut CartBanking)-> Box<Self> {
    banks.chr = Banking::new_chr(header, 1);
    Box::new(Self)
  }

  fn write(&mut self, banks: &mut CartBanking, _: usize, val: u8) {
    banks.chr.set(0, val as usize);
  }
}

// Mapper 07
// https://www.nesdev.org/wiki/AxROM
#[derive(serde::Serialize, serde::Deserialize)]
pub struct AxROM;

#[typetag::serde]
impl Mapper for AxROM {
  fn new(header: &CartHeader, banks: &mut CartBanking)-> Box<Self> {
    banks.prg = Banking::new_prg(header, 1);
    Box::new(Self)
  }

  fn write(&mut self, banks: &mut CartBanking, _: usize, val: u8) {
    let bank = val as usize & 0b111;
    banks.prg.set(0, bank);
    let mirroring = match val & 0b1_0000 != 0 {
      false => Mirroring::SingleScreenA,
      true  => Mirroring::SingleScreenB,
    };
    banks.vram.update(mirroring);
  }
}

// Mapper 11
// https://www.nesdev.org/wiki/Color_Dreams
// TODO: ColorDreams and GxRom are basically the same, use PhantomData generics
#[derive(serde::Serialize, serde::Deserialize)]
pub struct ColorDreams;

#[typetag::serde]
impl Mapper for ColorDreams {
  fn new(header: &CartHeader, banks: &mut CartBanking)-> Box<Self> {
    banks.prg = Banking::new_prg(header, 1);
    banks.chr = Banking::new_chr(header, 1);
    Box::new(Self)
  }

  fn write(&mut self, banks: &mut CartBanking, _: usize, val: u8) {
    let prg_bank = val as usize & 0b11;
    let chr_bank = val as usize >> 4;

    banks.prg.set(0, prg_bank);
    banks.chr.set(0, chr_bank);
  }
}

// Mapper 66
// https://www.nesdev.org/wiki/GxROM
#[derive(serde::Serialize, serde::Deserialize)]
pub struct GxROM;

#[typetag::serde]
impl Mapper for GxROM {
  fn new(header: &CartHeader, banks: &mut CartBanking)-> Box<Self> {
    banks.prg = Banking::new_prg(header, 1);
    banks.chr = Banking::new_chr(header, 1);
    
    Box::new(Self)
  }

  fn write(&mut self, banks: &mut CartBanking, _: usize, val: u8) {
    let chr_bank = val as usize & 0b11;
    let prg_bank = (val as usize >> 4) & 0b11;

    banks.prg.set(0, prg_bank);
    banks.chr.set(0, chr_bank);
  }
}

// Mapper 71
// https://www.nesdev.org/wiki/INES_Mapper_071
#[derive(serde::Serialize, serde::Deserialize)]
pub struct Codemasters;

#[typetag::serde]
impl Mapper for Codemasters {
  fn new(header: &CartHeader, banks: &mut CartBanking)-> Box<Self> {
    banks.prg = Banking::new_prg(header, 2);
    banks.prg.set_page_to_last_bank(1);
    Box::new(Self)
  }

  fn write(&mut self, banks: &mut CartBanking, addr: usize, val: u8) {
    match addr {
      0x9000..=0x9FFF => {
        let mirroring = match (val >> 4) & 1 != 0 {
          false => Mirroring::SingleScreenA,
          true  => Mirroring::SingleScreenB,
        };
        banks.vram.update(mirroring);
      }
      0xC000..=0xFFFF => {
        let bank = val as usize & 0b1111;
        banks.prg.set(0, bank);
      }
      _ => {}
    }
  }  
}

// Mapper 78 (Holy Diver and Cosmo Carrier)
// https://www.nesdev.org/wiki/INES_Mapper_078
#[derive(serde::Serialize, serde::Deserialize)]
pub struct INesMapper078 {
  uses_hv_mirroring: bool,
}

#[typetag::serde]
impl Mapper for INesMapper078 {
  fn new(header: &CartHeader, banks: &mut CartBanking)-> Box<Self> {
    let uses_hv_mirroring = 
      header.has_alt_mirroring || header.submapper == 3;
    
    banks.prg = Banking::new_prg(header, 2);
    banks.chr = Banking::new_chr(header, 1);
    
    let mirroring = if uses_hv_mirroring {
      Mirroring::Horizontal
    } else {
      Mirroring::SingleScreenA
    };
    
    banks.prg.set_page_to_last_bank(1);
    banks.vram.update(mirroring);

    Box::new(Self{uses_hv_mirroring})
  }

  fn write(&mut self, banks: &mut CartBanking, _: usize, val: u8) {
    let prg_bank = val & 0b111;
    let chr_bank = val >> 4;

    banks.prg.set(0, prg_bank as usize);
    banks.chr.set(0, chr_bank as usize);

    let mirroring = if self.uses_hv_mirroring {
      match (val >> 3) & 1 != 0 {
        false => Mirroring::Horizontal,
        true  => Mirroring::Vertical,
      }
    } else {
      match (val >> 3) & 1 != 0 {
        false => Mirroring::SingleScreenA,
        true  => Mirroring::SingleScreenB,
      }
    };

    banks.vram.update(mirroring);
  }
}

// Mapper 31
// https://www.nesdev.org/wiki/INES_Mapper_031
#[derive(serde::Serialize, serde::Deserialize)]
pub struct INesMapper031;

#[typetag::serde]
impl Mapper for INesMapper031 {
  fn new(header: &CartHeader, banks: &mut CartBanking)-> Box<Self> {
    banks.prg = Banking::new_prg(header, 8);
    banks.prg.set_page_to_last_bank(7);
    
    Box::new(Self)
  }

  fn write(&mut self, _: &mut CartBanking, _: usize, _: u8) {}

  fn cart_write(&mut self, banks: &mut CartBanking, addr: usize, val: u8) {
    match addr { 
      0x5000..=0x5FFFF => banks.prg.set(addr, val as usize),
      _ => {}
    }
  }
}

// Mapper 75
// https://www.nesdev.org/wiki/VRC1
#[derive(serde::Serialize, serde::Deserialize)]
pub struct VRC1;

#[typetag::serde]
impl Mapper for VRC1 {
  fn new(header: &CartHeader, banks: &mut CartBanking)-> Box<Self> {
    banks.prg = Banking::new_prg(header, 4);
    banks.prg.set_page_to_last_bank(3);
    banks.chr = Banking::new_chr(header, 2);
    Box::new(Self)
  }

  fn write(&mut self, banks: &mut CartBanking, addr: usize, val: u8) {
    match addr {
      0x8000..=0x8FFF => banks.prg.set(0, val as usize & 0b1111),
      0xA000..=0xAFFF => banks.prg.set(1, val as usize & 0b1111),
      0xC000..=0xCFFF => banks.prg.set(2, val as usize & 0b1111),
      0x9000..=0x9FFF => {
        let mirroring = match val & 1 != 0 {
          false => Mirroring::Vertical,
          true  => Mirroring::Horizontal,
        };
        banks.vram.update(mirroring);

        let bank0 = banks.chr.bankings[0];
        let bank0_hi = (val as usize >> 1) & 1;
        banks.chr.set(0, (bank0_hi << 5) | bank0);

        let bank1 = banks.chr.bankings[1];
        let bank1_hi = (val as usize >> 1) & 1;
        banks.chr.set(1, (bank1_hi << 5) | bank1);
      }
      0xE000..=0xEFFF => banks.chr.set(0, val as usize),
      0xF000..=0xFFFF => banks.chr.set(1, val as usize),
      _ => {}
    }
  }
}