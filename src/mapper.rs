use std::marker::{self, PhantomData};

use crate::{cart::{CartBanking, CartHeader, Mirroring, PpuTarget, PrgTarget}, ppu::RenderingState};

mod mmc1;
mod mmc2;
mod mmc3;
mod mmc5;
mod konami_irq;
mod vrc2_4;
mod vrc3;
mod vrc6;
mod vrc7;
mod sunsoft4;
mod sunsoft_fme_7;
mod namco129_163;
mod bandai_fcg;
mod unrom512;
mod gtrom;

use bandai_fcg::BandaiFCG;
use gtrom::GTROM;
use mmc1::MMC1;
use mmc2::MMC2;
use mmc3::MMC3;
use mmc5::MMC5;
use unrom512::UNROM512;
use vrc2_4::VRC2_4;
use vrc3::VRC3;
use vrc6::VRC6;
use vrc7::VRC7;
use sunsoft4::Sunsoft4;
use sunsoft_fme_7::SunsoftFME7;
use namco129_163::Namco129_163;

pub fn new_mapper(header: &CartHeader, banks: &mut CartBanking) -> Result<Box<dyn Mapper>, String> {
  let mapper: Box<dyn Mapper> = match header.mapper {
    0 => NROM::new(header, banks),
    1 => MMC1::new(header, banks),
    2 | 180 => UxROM::new(header, banks),
    3 => CNROM::new(header, banks),
    4 => MMC3::new(header, banks),
    5 => MMC5::new(header, banks),
    7 => AxROM::new(header, banks),
    9 | 10 => MMC2::new(header, banks),
    11 => ColorDreams::new(header, banks),
    13 => CPROM::new(header, banks),
    16 => BandaiFCG::new(header, banks),
    19 => Namco129_163::new(header, banks),
    21 | 22 | 23 | 25 => VRC2_4::new(header, banks),
    24 | 26 => VRC6::new(header, banks),
    30 => UNROM512::new(header, banks),
    31 => INesMapper031::new(header, banks),
    34 => INesMapper034::new(header, banks),
    66 => GxROM::new(header, banks),
    68 => Sunsoft4::new(header, banks),
    69 => SunsoftFME7::new(header, banks),
    71 => Codemasters::new(header, banks),
    73 => VRC3::new(header, banks),
    75 => VRC1::new(header, banks),
    78 => INesMapper078::new(header, banks),
    85 => VRC7::new(header, banks),
    87 => INesMapper087::new(header, banks),
    111 => GTROM::new(header, banks),
    206 => INesMapper206::new(header, banks),
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
const MAPPERS_TABLE: [(u16, &'static str); 39] = [
  (0, "NROM"),
  (1, "MMC1"),
  (2, "UxROM"),
  (3, "CNROM"),
  (4, "MMC3"),
  (5, "MMC5"),
  (7, "AxROM"),
  (9, "MMC2 (Punch-Out!!)"),
  (10, "MMC4"),
  (11, "ColorDreams"),
  (13, "CPROM"),
  (16, "Bandai FCG"),
  (19, "Namco 129/163"),
  (21, "Konami VRC2/VRC4"),
  (22, "Konami VRC2/VRC4"),
  (23, "Konami VRC2/VRC4"),
  (24, "Konami VRC6a (Akumajou Densetsu)"),
  (25, "Konami VRC2/VRC4"),
  (26, "Konami VRC6b (Madara and Esper Dream 2)"),
  (30, "UNROM 512"),
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
  (85, "VRC7 (Lagrange Point and Tiny Toon Adventures 2)"),
  (87, "Jaleco87"),
  (91, "J.Y. Company"),
  (94, "UNROM (Senjou no Ookami)"),
  (111, "GTROM (Cheapocabra)"),
  (163, "FC-001"),
  (180, "UNROM (Crazy Climber)"),
  (206, "Namco 118/Tengen MIMIC-1"),
  (210, "Namco 175/340"),
];

pub fn set_byte_hi(dst: u16, val: u8) -> u16 {
  (dst & 0x00FF) | ((val as u16) << 8)
}

pub fn set_byte_lo(dst: u16, val: u8) -> u16 {
  (dst & 0xFF00) | val as u16
}

#[typetag::serde(tag = "mmu")]
pub trait Mapper {
  fn new(header: &CartHeader, banks: &mut CartBanking) -> Box<Self> where Self: Sized;
  fn prg_write(&mut self, banks: &mut CartBanking, addr: usize, val: u8);

  fn map_prg_addr(&mut self, banks: &mut CartBanking, addr: usize) -> PrgTarget {
    match addr {
      0x4020..=0x5FFF => PrgTarget::Cart,
      0x6000..=0x7FFF => PrgTarget::SRam(true, banks.sram.translate(addr)),
      0x8000..=0xFFFF => PrgTarget::Prg(banks.prg.translate(addr)),
      _ => unreachable!()
    }
  }

  fn map_ppu_addr(&mut self, banks: &mut CartBanking, addr: usize) -> PpuTarget {
    match addr {
      0x0000..=0x1FFF => PpuTarget::Chr(banks.chr.translate(addr)),
      0x2000..=0x2FFF => PpuTarget::CiRam(banks.ciram.translate(addr)),
      _ => unreachable!()
    }
  }

  fn cart_read(&mut self, _addr: usize) -> u8 { 0xFF }
  fn cart_write(&mut self, _banks: &mut CartBanking, _addr: usize, _val: u8) {}
  
  fn exram_read(&mut self, _addr: usize) -> u8 { 0xFF }
  fn exram_write(&mut self, _addr: usize, _val: u8) {}

  fn poll_irq(&mut self) -> bool { false }
  
  // Generic cpu cycle notify / apu extension clocking
  fn notify_cpu_cycle(&mut self) {}
  fn get_sample(&self) -> f32 { 0.0 }

  // Mmc3 scanline notify
  fn notify_mmc3_scanline(&mut self) {}

  // Mmc5 ppu notify
  fn notify_ppuctrl(&mut self, _val: u8) {}
  fn notify_ppumask(&mut self, _val: u8) {}
  fn notify_ppu_state(&mut self, _state: RenderingState) {}
  fn notify_mmc5_scanline(&mut self) {}
}

#[derive(Debug, Default)]
pub struct PrgBanking;
#[derive(Debug, Default)]
pub struct ChrBanking;
#[derive(Debug, Default)]
pub struct SramBanking;
#[derive(Debug, Default)]
pub struct CiramBanking;
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
pub struct Banking<T> {
  data_size: usize,
  bank_size: usize,
  bank_size_shift: usize,
  banks_count: usize,
  banks_count_shift: usize,
  pages_start: usize,
  bankings: Box<[usize]>,
  kind: marker::PhantomData<T>
}

// https://stackoverflow.com/questions/25787613/division-and-multiplication-by-power-of-2
impl<T> Banking<T> {
  pub fn new(rom_size: usize, pages_start: usize, page_size: usize, pages_count: usize) -> Self {
    let bankings = vec![0; pages_count].into_boxed_slice();
    let bank_size = page_size;
    let banks_count = rom_size / bank_size;
    let bank_size_shift = bank_size.checked_ilog2().unwrap_or_default() as usize;
    let banks_count_shift = banks_count.checked_ilog2().unwrap_or_default() as usize;
    Self { bankings, data_size: rom_size, pages_start, bank_size, bank_size_shift, banks_count, banks_count_shift, kind: PhantomData::<T> }
  }

  pub fn set_page(&mut self, page: usize, bank: usize) {
    // some games might write bigger bank numbers than really avaible
    // let bank = bank % self.banks_count;
    let bank = bank & (self.banks_count-1);
    // i do not expect to write outside the slots array.
    // self.bankings[page] = bank * self.bank_size;
    self.bankings[page] = bank << self.bank_size_shift;
  }

  pub fn swap_pages(&mut self, left: usize, right: usize) {
    self.bankings.swap(left, right);
  }

  pub fn set_page_to_last_bank(&mut self, page: usize) {
    let last_bank = self.banks_count-1;
    self.set_page(page, last_bank);
  }

  fn page_to_bank_addr(&self, page: usize, addr: usize) -> usize {
    // i do not expect to write outside the slots array here either. 
    // the bus object should take responsibilty to always pass correct addresses in range.
    // self.bankings[page] + (addr % self.bank_size)
    self.bankings[page] + (addr & (self.bank_size-1))
  }

  pub fn translate(&self, addr: usize) -> usize {
    // let page = (addr - self.pages_start) / self.bank_size;
    let page = (addr - self.pages_start) >> self.bank_size_shift;
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

impl Banking<CiramBanking> {
  pub fn new_ciram(header: &CartHeader) -> Self {
    let mut res = Self::new(4*1024, 0x2000, 1024, 4);
    if header.mirroring != Mirroring::FourScreen {
      res.banks_count = 2;
    }

    res.update(header.mirroring);
    res
  }

  pub fn update(&mut self, mirroring: Mirroring) {
    match mirroring {
      Mirroring::Horizontal => {
        self.set_page(0, 0);
        self.set_page(1, 0);
        self.set_page(2, 1);
        self.set_page(3, 1);
      }
      Mirroring::Vertical => {
        self.set_page(0, 0);
        self.set_page(1, 1);
        self.set_page(2, 0);
        self.set_page(3, 1);
      }
      Mirroring::SingleScreenA => for i in 0..4 {
        self.set_page(i, 0);
      }
      Mirroring::SingleScreenB => for i in 0..4 {
        self.set_page(i, 1);
      }
      Mirroring::FourScreen => for i in 0..4 {
        self.set_page(i, i);
      }
    }
  }
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Dummy;
#[typetag::serde]
impl Mapper for Dummy {
  fn new(_: &CartHeader, _: &mut CartBanking) -> Box<Self> { Box::new(Self) }
  fn prg_write(&mut self, _: &mut CartBanking, _: usize, _: u8) {}
}

// Mapper 00
// https://www.nesdev.org/wiki/NROM
#[derive(serde::Serialize, serde::Deserialize)]
pub struct NROM;

#[typetag::serde]
impl Mapper for NROM {
  fn new(header: &CartHeader, banks: &mut CartBanking) -> Box<Self> {
    banks.prg = Banking::new_prg(header, 2);

    if header.prg_size <= 16*1024 {
      banks.prg.set_page(1, 0);
    } else {
      banks.prg.set_page(1, 1);
    }

    Box::new(Self)
  }

  fn prg_write(&mut self, _: &mut CartBanking, _: usize, _: u8) {}
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

  fn prg_write(&mut self, banks: &mut CartBanking, _: usize, val: u8) {
    let select = val & 0b1111;
    banks.prg.set_page(self.banked_page as usize, select as usize);
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

  fn prg_write(&mut self, banks: &mut CartBanking, _: usize, val: u8) {
    banks.chr.set_page(0, val as usize);
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

  fn prg_write(&mut self, banks: &mut CartBanking, _: usize, val: u8) {
    let bank = val as usize & 0b111;
    banks.prg.set_page(0, bank);
    let mirroring = match val & 0b1_0000 != 0 {
      false => Mirroring::SingleScreenA,
      true  => Mirroring::SingleScreenB,
    };
    banks.ciram.update(mirroring);
  }
}

// Mapper 11
// https://www.nesdev.org/wiki/Color_Dreams
// TODO: ColorDreams and GxRom are basically the same, merge into one
#[derive(serde::Serialize, serde::Deserialize)]
pub struct ColorDreams;

#[typetag::serde]
impl Mapper for ColorDreams {
  fn new(header: &CartHeader, banks: &mut CartBanking)-> Box<Self> {
    banks.prg = Banking::new_prg(header, 1);
    banks.chr = Banking::new_chr(header, 1);
    Box::new(Self)
  }

  fn prg_write(&mut self, banks: &mut CartBanking, _: usize, val: u8) {
    let prg_bank = val as usize & 0b11;
    let chr_bank = val as usize >> 4;

    banks.prg.set_page(0, prg_bank);
    banks.chr.set_page(0, chr_bank);
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

  fn prg_write(&mut self, banks: &mut CartBanking, _: usize, val: u8) {
    let chr_bank = val as usize & 0b11;
    let prg_bank = (val as usize >> 4) & 0b11;

    banks.prg.set_page(0, prg_bank);
    banks.chr.set_page(0, chr_bank);
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

  fn prg_write(&mut self, banks: &mut CartBanking, addr: usize, val: u8) {
    match addr {
      0x9000..=0x9FFF => {
        let mirroring = match (val >> 4) & 1 != 0 {
          false => Mirroring::SingleScreenA,
          true  => Mirroring::SingleScreenB,
        };
        banks.ciram.update(mirroring);
      }
      0xC000..=0xFFFF => {
        let bank = val as usize & 0b1111;
        banks.prg.set_page(0, bank);
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
    banks.ciram.update(mirroring);

    Box::new(Self{uses_hv_mirroring})
  }

  fn prg_write(&mut self, banks: &mut CartBanking, _: usize, val: u8) {
    let prg_bank = val & 0b111;
    let chr_bank = val >> 4;

    banks.prg.set_page(0, prg_bank as usize);
    banks.chr.set_page(0, chr_bank as usize);

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

    banks.ciram.update(mirroring);
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

  fn prg_write(&mut self, _: &mut CartBanking, _: usize, _: u8) {}

  fn cart_write(&mut self, banks: &mut CartBanking, addr: usize, val: u8) {
    match addr { 
      0x5000..=0x5FFFF => banks.prg.set_page(addr, val as usize),
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

  fn prg_write(&mut self, banks: &mut CartBanking, addr: usize, val: u8) {
    match addr {
      0x8000..=0x8FFF => banks.prg.set_page(0, val as usize & 0b1111),
      0xA000..=0xAFFF => banks.prg.set_page(1, val as usize & 0b1111),
      0xC000..=0xCFFF => banks.prg.set_page(2, val as usize & 0b1111),
      0x9000..=0x9FFF => {
        let mirroring = match val & 1 != 0 {
          false => Mirroring::Vertical,
          true  => Mirroring::Horizontal,
        };
        banks.ciram.update(mirroring);

        let bank0 = banks.chr.bankings[0];
        let bank0_hi = (val as usize >> 1) & 1;
        banks.chr.set_page(0, (bank0_hi << 5) | bank0);

        let bank1 = banks.chr.bankings[1];
        let bank1_hi = (val as usize >> 1) & 1;
        banks.chr.set_page(1, (bank1_hi << 5) | bank1);
      }
      0xE000..=0xEFFF => banks.chr.set_page(0, val as usize),
      0xF000..=0xFFFF => banks.chr.set_page(1, val as usize),
      _ => {}
    }
  }
}

// Mapper 206
// https://www.nesdev.org/wiki/INES_Mapper_206
#[derive(serde::Serialize, serde::Deserialize)]
pub struct INesMapper206 {
  mmc3: MMC3,
}

#[typetag::serde]
impl Mapper for INesMapper206 {
  fn new(header: &CartHeader, banks: &mut CartBanking) -> Box<Self> {
    let mmc3 = *MMC3::new(header, banks);
    Box::new(Self{mmc3})
  }

  fn prg_write(&mut self, banks: &mut CartBanking, addr: usize, val: u8) {
    self.mmc3.prg_write(banks, addr, val);
  }
}


// Mapper 87
// https://www.nesdev.org/wiki/INES_Mapper_087
#[derive(serde::Serialize, serde::Deserialize)]
pub struct INesMapper087;

#[typetag::serde]
impl Mapper for INesMapper087 {
  fn new(header: &CartHeader, banks: &mut CartBanking) -> Box<Self> {
    banks.prg = Banking::new_prg(header, 2);

    if header.prg_size <= 16*1024 {
      banks.prg.set_page(1, 0);
    } else {
      banks.prg.set_page(1, 1);
    }
    Box::new(Self)
  }

  fn prg_write(&mut self, banks: &mut CartBanking, _: usize, val: u8) {
    let bank = ((val & 0b01) << 1) | ((val & 0b10) >> 1);
    banks.chr.set_page(0, bank as usize);
  }

  fn map_prg_addr(&mut self, banks: &mut CartBanking, addr: usize) -> PrgTarget {
    match addr {
      0x6000..=0x7FFF => PrgTarget::Prg(addr),
      0x8000..=0xFFFF => PrgTarget::Prg(banks.prg.translate(addr)),
      _ => unreachable!()
    }
  }
}

// Mapper 34
// https://www.nesdev.org/wiki/INES_Mapper_034
#[derive(serde::Serialize, serde::Deserialize)]
pub struct INesMapper034 {
  submapper: u8,
}

#[typetag::serde]
impl Mapper for INesMapper034 {
  fn new(header: &CartHeader, banks: &mut CartBanking) -> Box<Self> {
    let submapper = if header.submapper != 0 {
      header.submapper
    } else if header.chr_real_size() > 8 * 1024 { 
      1 
    } else { 2 };

    banks.prg = Banking::new_prg(header, 1);

    if submapper == 2 {
      banks.chr = Banking::new_chr(header, 1);
    } else {
      banks.chr = Banking::new_chr(header, 2);
    }

    Box::new(Self { submapper })
  }

  fn prg_write(&mut self, banks: &mut CartBanking, addr: usize, val: u8) {
    match (addr, self.submapper) {
      (0x7FFD, 1) | (0x8000..=0xFFFF, 2) => banks.prg.set_page(0, val as usize & 0b11),
      (0x7FFE, 1) => banks.chr.set_page(0, val as usize & 0b1111),
      (0x7FFF, 1) => banks.chr.set_page(1, val as usize & 0b1111),
      _ => {}
    }
  }
}

// Mapper 13
// https://www.nesdev.org/wiki/CPROM
#[derive(serde::Serialize, serde::Deserialize)]
pub struct CPROM;

#[typetag::serde]
impl Mapper for CPROM {
  fn new(header: &CartHeader, banks: &mut CartBanking) -> Box<Self> {
    banks.chr = Banking::new_chr(header, 2);
    Box::new(Self)
  }

  fn prg_write(&mut self, banks: &mut CartBanking, _: usize, val: u8) {
    banks.chr.set_page(1, val as usize & 0b11);
  }
}