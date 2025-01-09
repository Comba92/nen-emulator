use std::marker::{self, PhantomData};

use mmc1::MMC1;
use mmc3::MMC3;
use vrc2_4::VRC2_4;
use vrc3::VRC3;
use vrc6::VRC6;

use crate::cart::{CartHeader, Mirroring, VRamTarget};

mod mmc1;
mod mmc3;
mod konami_irq;
mod vrc2_4;
mod vrc3;
mod vrc6;

pub fn new_mapper(header: &CartHeader) -> Result<Box<dyn Mapper>, String> {
  let mapper: Box<dyn Mapper> = match header.mapper {
    0 => NROM::new(header),
    1 => MMC1::new(header),
    2 | 94 => UxROM::new(header),
    3 => CNROM::new(header),
    4 => MMC3::new(header),
    7 => AxROM::new(header),
    9 | 10 => MMC2::new(header),
    11 => ColorDreams::new(header),
    21 | 22 | 23 | 25 => VRC2_4::new(header),
    24 | 26 => VRC6::new(header),
    66 => GxROM::new(header),
    71 => Codemasters::new(header),
    73 => VRC3::new(header),
    78 => INesMapper078::new(header),
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
const MAPPERS_TABLE: [(u16, &'static str); 30] = [
  (0, "NROM"),
  (1, "MMC1"),
  (2, "UxROM"),
  (3, "CNROM (INesMapper003)"),
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
  (48, "Taito TC0690 (INesMapper048)"),
  (66, "GxROM"),
  (68, "Sunsoft4 (INesMapper068)"),
  (69, "Sunsoft5 FME-7"),
  (71, "Codemasters UNROM (INesMapper071)"),
  (73, "Konami VRC3 (Salamander)"),
  (75, "Konami VRC1"),
  (78, "Irem 74HC161 (INesMapper078) (Holy Diver and Cosmo Carrier)"),
  (94, "UNROM (Senjou no Ookami)"),
  (163, "FC-001 (INesMapper163)"),
  (180, "UNROM (Crazy Climber)"),
  (210, "Namco 175/340 (INesMapper210"),
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
pub fn mirror_nametbl(mirroring: Mirroring, addr: usize) -> usize {
  let addr = addr - 0x2000;
  let nametbl_idx = addr / 0x400;
  
  use Mirroring::*;
  match (mirroring, nametbl_idx) {
    (Horizontal, 1) | (Horizontal, 2) => addr - 0x400,
    (Horizontal, 3) => addr - 0x400 * 2,
    (Vertical, 2) | (Vertical, 3) => addr - 0x400 * 2,
    (SingleScreenA, _) => addr % 0x400,
    (SingleScreenB, _) => (addr % 0x400) + 0x400,
    (FourScreen, _) => addr,
    _ => addr,
  }
}


#[typetag::serde]
pub trait Mapper {
  fn new(header: &CartHeader) -> Box<Self> where Self: Sized;
  fn write(&mut self, addr: usize, val: u8);
  
  fn prg_addr(&mut self, addr: usize) -> usize { addr - 0x8000 }
  fn chr_addr(&mut self, addr: usize) -> usize { addr }

  fn vram_addr(&mut self, addr: usize) -> (VRamTarget, usize) {
    (VRamTarget::CiRam, mirror_nametbl(self.mirroring(), addr))
  }

  fn cart_read(&self, _addr: usize) -> u8 { 0xFF }
  fn cart_write(&mut self, _addr: usize, _val: u8) {}
  fn sram_read(&self, ram: &[u8], addr: usize) -> u8 {
    ram[(addr - 0x6000) % ram.len()]
  }

  fn sram_write(&mut self, ram: &mut[u8], addr: usize, val: u8) {
    ram[(addr - 0x6000) % ram.len()] = val;
  }

  fn mirroring(&self) -> Mirroring;

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

struct PrgBanking;
struct ChrBanking;
struct SRamBanking;
struct VRamBanking;
#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct Banking<T> {
  data_size: usize,
  bank_size: usize,
  banks_count: usize,
  // TODO: probably can be just a Vec of u8
  bankings: Box<[usize]>,
  kind: marker::PhantomData<T>
}

impl<T> Banking<T> {
  pub fn new(rom_size: usize, page_size: usize, pages_count: usize) -> Self {
    let bankings = vec![0; pages_count].into_boxed_slice();
    let bank_size = page_size;
    let banks_count = rom_size / bank_size;
    Self { bankings, data_size: rom_size, bank_size, banks_count, kind: PhantomData::<T> }
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
}

impl Banking<PrgBanking> {
  pub fn new_prg(header: &CartHeader, pages_count: usize) -> Self {
    let pages_size = 32*1024 / pages_count;
    Self::new(header.prg_size, pages_size, pages_count)
  }

  pub fn addr(&self, addr: usize) -> usize {
    let page = (addr - 0x8000) / self.bank_size;
    self.page_to_bank_addr(page, addr)
  }
}

impl Banking<ChrBanking> {
  pub fn new_chr(header: &CartHeader, pages_count: usize) -> Self {
    let pages_size = 8*1024 / pages_count;
    Self::new(header.chr_real_size(), pages_size, pages_count)
  }

  pub fn addr(&self, addr: usize) -> usize {
    let page = addr / self.bank_size;
    self.page_to_bank_addr(page, addr)
  }
}

impl Banking<VRamBanking> {
  pub fn new_vram(vram_size: usize) -> Self {
    Self::new(vram_size, 1024, 4)
  }

  pub fn addr(&self, addr: usize) -> usize {
    let page = (addr - 0x2000) / self.bank_size;
    self.page_to_bank_addr(page, addr)
  }
}

impl Banking<SRamBanking> {
  pub fn new_sram(header: &CartHeader) -> Self {
    Self::new(header.sram_real_size(), 8*1024, 1)
  }

  pub fn addr(&self, addr: usize) -> usize {
    let page = (addr - 0x6000) / self.bank_size;
    self.page_to_bank_addr(page, addr)
  }
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Dummy;
#[typetag::serde]
impl Mapper for Dummy {
  fn new(_: &CartHeader) -> Box<Self> where Self: Sized {
    Box::new(Dummy)
  }
  fn write(&mut self, _: usize, _: u8) {}
  fn mirroring(&self) -> Mirroring { Default::default() }
}

// Mapper 00
// https://www.nesdev.org/wiki/NROM
#[derive(serde::Serialize, serde::Deserialize)]
pub struct NROM {
  prg_banks: Banking<PrgBanking>,
  mirroring: Mirroring,
}

#[typetag::serde]
impl Mapper for NROM {
  fn new(header: &CartHeader) -> Box<Self> {
    let mut prg_banks = Banking::new_prg(header, 2);

    if header.prg_size <= 16*1024 {
      prg_banks.set(1, 0);
    } else {
      prg_banks.set(1, 1);
    }

    let mirroring = header.mirroring;
    Box::new(Self { prg_banks, mirroring })
  }

  fn write(&mut self, _: usize, _: u8) {}

  fn prg_addr(&mut self, addr: usize) -> usize {
    self.prg_banks.addr(addr)
  }

  fn mirroring(&self) -> Mirroring { self.mirroring }
}

// Mapper 02
// https://www.nesdev.org/wiki/UxROM
#[derive(serde::Serialize, serde::Deserialize)]
pub struct UxROM {
  banked_page: u8,
  prg_banks: Banking<PrgBanking>,
  mirroring: Mirroring,
}

#[typetag::serde]
impl Mapper for UxROM {
  fn new(header: &CartHeader) -> Box<Self> {
    let mut prg_banks = Banking::new_prg(header, 2);
    let mirroring = header.mirroring;

    // https://www.nesdev.org/wiki/INES_Mapper_180
    let banked_page = if header.mapper == 180 {
      1
    } else {
      prg_banks.set_page_to_last_bank(1);
      0
    };

    Box::new(Self { prg_banks, mirroring, banked_page })
  }

  fn write(&mut self, _: usize, val: u8) {
    let select = val & 0b1111;
    self.prg_banks.set(self.banked_page as usize, select as usize);
  }

  fn prg_addr(&mut self, addr: usize) -> usize {
    self.prg_banks.addr(addr)
  }

  fn mirroring(&self) -> Mirroring { self.mirroring }
}

// Mapper 03
// https://www.nesdev.org/wiki/INES_Mapper_003
#[derive(serde::Serialize, serde::Deserialize)]
pub struct CNROM {
  chr_banks: Banking<ChrBanking>,
  mirroring: Mirroring,
}

#[typetag::serde]
impl Mapper for CNROM {
  fn new(header: &CartHeader) -> Box<Self>where Self:Sized {
    let chr_banks = Banking::new_chr(header, 1);
    let mirroring = header.mirroring;

    Box::new(Self { chr_banks, mirroring })
  }

  fn write(&mut self, _: usize, val: u8) {
    self.chr_banks.set(0, val as usize);
  }

  fn chr_addr(&mut self, addr: usize) -> usize {
    self.chr_banks.addr(addr)
  }

  fn mirroring(&self) -> Mirroring { self.mirroring }
}

// Mapper 07
// https://www.nesdev.org/wiki/AxROM
#[derive(serde::Serialize, serde::Deserialize)]
pub struct AxROM {
  prg_banks: Banking<PrgBanking>,
  mirroring: Mirroring,
}

#[typetag::serde]
impl Mapper for AxROM {
  fn new(header: &CartHeader) -> Box<Self> {
    let prg_banks = Banking::new_prg(header, 1);
    Box::new(Self {prg_banks, mirroring: Mirroring::SingleScreenA })
  }

  fn write(&mut self, _: usize, val: u8) {
    let bank = val as usize & 0b111;
    self.prg_banks.set(0, bank);
    self.mirroring = match val & 0b1_0000 != 0 {
      false => Mirroring::SingleScreenA,
      true  => Mirroring::SingleScreenB,
    };
  }

  fn prg_addr(&mut self, addr: usize) -> usize {
    self.prg_banks.addr(addr)
  }

  fn mirroring(&self) -> Mirroring { self.mirroring }
}

// Mapper 11
// https://www.nesdev.org/wiki/Color_Dreams
// TODO: ColorDreams and GxRom are basically the same, use PhantomData generics
#[derive(serde::Serialize, serde::Deserialize)]
pub struct ColorDreams {
  prg_banks: Banking<PrgBanking>,
  chr_banks: Banking<ChrBanking>,
  mirroring: Mirroring,
}

#[typetag::serde]
impl Mapper for ColorDreams {
  fn new(header: &CartHeader) -> Box<Self> {
    let prg_banks = Banking::new_prg(header, 1);
    let chr_banks = Banking::new_chr(header, 1);
    let mirroring = header.mirroring;
    Box::new(Self {prg_banks, chr_banks, mirroring})
  }

  fn write(&mut self, _: usize, val: u8) {
    let prg_bank = val as usize & 0b11;
    let chr_bank = val as usize >> 4;

    self.prg_banks.set(0, prg_bank);
    self.chr_banks.set(0, chr_bank);
  }

  fn prg_addr(&mut self, addr: usize) -> usize {
    self.prg_banks.addr(addr)
  }

  fn chr_addr(&mut self, addr: usize) -> usize {
    self.chr_banks.addr(addr)
  }

  fn mirroring(&self) -> Mirroring { self.mirroring }
}

// Mapper 66
// https://www.nesdev.org/wiki/GxROM
#[derive(serde::Serialize, serde::Deserialize)]
pub struct GxROM {
  prg_banks: Banking<PrgBanking>,
  chr_banks: Banking<ChrBanking>,
  mirroring: Mirroring,
}

#[typetag::serde]
impl Mapper for GxROM {
  fn new(header: &CartHeader) -> Box<Self> {
    let prg_banks = Banking::new_prg(header, 1);
    let chr_banks = Banking::new_chr(header, 1);
    let mirroring = header.mirroring;
    Box::new(Self {prg_banks, chr_banks, mirroring})
  }

  fn write(&mut self, _: usize, val: u8) {
    let chr_bank = val as usize & 0b11;
    let prg_bank = (val as usize >> 4) & 0b11;

    self.prg_banks.set(0, prg_bank);
    self.chr_banks.set(0, chr_bank);
  }

  fn prg_addr(&mut self, addr: usize) -> usize {
    self.prg_banks.addr(addr)
  }

  fn chr_addr(&mut self, addr: usize) -> usize {
    self.chr_banks.addr(addr)
  }

  fn mirroring(&self) -> Mirroring { self.mirroring }
}

// Mapper 71
// https://www.nesdev.org/wiki/INES_Mapper_071
#[derive(serde::Serialize, serde::Deserialize)]
pub struct Codemasters {
  prg_banks: Banking<PrgBanking>,
  mirroring: Mirroring,
}

#[typetag::serde]
impl Mapper for Codemasters {
  fn new(header: &CartHeader) -> Box<Self> {
    let mut prg_banks = Banking::new_prg(header, 2);
    prg_banks.set_page_to_last_bank(1);
    let mirroring = header.mirroring;
    Box::new(Self {prg_banks, mirroring })
  }

  fn write(&mut self, addr: usize, val: u8) {
    match addr {
      0x9000..=0x9FFF => self.mirroring = match (val >> 4) & 1 != 0 {
        false => Mirroring::SingleScreenA,
        true  => Mirroring::SingleScreenB,
      },
      0xC000..=0xFFFF => {
        let bank = val as usize & 0b1111;
        self.prg_banks.set(0, bank);
      }
      _ => {}
    }
  }

  fn prg_addr(&mut self, addr: usize) -> usize {
    self.prg_banks.addr(addr)
  }

  fn mirroring(&self) -> Mirroring { self.mirroring }
}

// Mapper 09 / 10
// https://www.nesdev.org/wiki/MMC2
// https://www.nesdev.org/wiki/MMC4 
#[derive(Clone, Copy, Default, serde::Serialize, serde::Deserialize)]
enum Mmc2Latch { FD, #[default] FE }
#[derive(serde::Serialize, serde::Deserialize)]
pub struct MMC2 {
  mapper: u16,
  prg_banks: Banking<PrgBanking>,
  chr_banks0: Banking<ChrBanking>,
  chr_banks1: Banking<ChrBanking>,
  latch0: Mmc2Latch,
  latch1: Mmc2Latch,

  mirroring: Mirroring,
}

#[typetag::serde]
impl Mapper for MMC2 {
  fn new(header: &CartHeader) -> Box<Self> {
    let mut prg_banks = Banking::new_prg(header, 4);
    let chr_banks0 = Banking::new_chr(header, 2);
    let chr_banks1 = Banking::new_chr(header, 2);

    match header.mapper {
      9 => {
        // MMC2 - Three 8 KB PRG ROM banks, fixed to the last three banks
        prg_banks.set(1, prg_banks.banks_count-3);
        prg_banks.set(2, prg_banks.banks_count-2);
        prg_banks.set(3, prg_banks.banks_count-1);
      }
      10 => {
        // MMC4
        prg_banks.set_page_to_last_bank(1);
      }
      _ => unreachable!(),
    }


    Box::new(Self{
      mapper: header.mapper,
      prg_banks, 
      chr_banks0, chr_banks1,
      latch0: Mmc2Latch::FE, 
      latch1: Mmc2Latch::FE, 
      mirroring: Default::default()
    })
  }

  fn write(&mut self, addr: usize, val: u8) {
    let val = val as usize & 0b1_1111;
    
    match addr {
      0xA000..=0xAFFF => self.prg_banks.set(0, val & 0b1111),
      0xB000..=0xBFFF => self.chr_banks0.set(0, val),
      0xC000..=0xCFFF => self.chr_banks0.set(1, val),
      0xD000..=0xDFFF => self.chr_banks1.set(0, val),
      0xE000..=0xEFFF => self.chr_banks1.set(1, val),
      0xF000..=0xFFFF => {
          self.mirroring = match val & 1 {
              0 => Mirroring::Vertical,
              _ => Mirroring::Horizontal,
          };
      }
      _ => unreachable!()
    }
  }

  fn prg_addr(&mut self, addr: usize) -> usize {
    self.prg_banks.addr(addr)
  }

  fn chr_addr(&mut self, addr: usize) -> usize {
    let res = match addr {
      0x0000..=0x0FFF => self.chr_banks0.page_to_bank_addr(self.latch0 as usize, addr),
      0x1000..=0x1FFF => self.chr_banks1.page_to_bank_addr(self.latch1 as usize, addr),
      _ => unreachable!()
    };

    // https://www.nesdev.org/wiki/MMC2#CHR_banking
    // https://www.nesdev.org/wiki/MMC4#Banks
    match (addr, self.mapper) {
      (0x0FD8, 9) | (0x0FD8..0x0FDF, 10) => self.latch0 = Mmc2Latch::FD,
      (0x0FE8, 9) | (0x0FE8..0x0FEF, 10) => self.latch0 = Mmc2Latch::FE,
      (0x1FD8..=0x1FDF, _) => self.latch1 = Mmc2Latch::FD,
      (0x1FE8..=0x1FEF, _) => self.latch1 = Mmc2Latch::FE,
      _ => {}
    };

    match addr {
      0x0FD8..0x0FDF => self.latch0 = Mmc2Latch::FD,
      0x0FE8..0x0FEF => self.latch0 = Mmc2Latch::FE,
      0x1FD8..=0x1FDF => self.latch1 = Mmc2Latch::FD,
      0x1FE8..=0x1FEF => self.latch1 = Mmc2Latch::FE,
      _ => {}
    };

    res
  }

  fn mirroring(&self) -> Mirroring { self.mirroring }
}

// Mapper 78 (Holy Diver and Cosmo Carrier)
// https://www.nesdev.org/wiki/INES_Mapper_078
#[derive(serde::Serialize, serde::Deserialize)]
pub struct INesMapper078 {
  prg_banks: Banking<PrgBanking>,
  chr_banks: Banking<ChrBanking>,
  mirroring: Mirroring,
  uses_hv_mirroring: bool,
}

#[typetag::serde]
impl Mapper for INesMapper078 {
  fn new(header: &CartHeader) -> Box<Self> {
    let uses_hv_mirroring = 
      header.has_alt_mirroring || header.submapper == 3;
    
    let mut prg_banks = Banking::new_prg(header, 2);
    let chr_banks = Banking::new_chr(header, 1);
    
    let mirroring = if uses_hv_mirroring {
      Mirroring::Horizontal
    } else {
      Mirroring::SingleScreenA
    };

    prg_banks.set_page_to_last_bank(1);
    Box::new(Self{prg_banks,chr_banks,uses_hv_mirroring,mirroring})
  }

  fn write(&mut self, _: usize, val: u8) {
    let prg_bank = val & 0b111;
    let chr_bank = val >> 4;

    self.prg_banks.set(0, prg_bank as usize);
    self.chr_banks.set(0, chr_bank as usize);

    self.mirroring = if self.uses_hv_mirroring {
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
  }

  fn prg_addr(&mut self, addr: usize) -> usize {
    self.prg_banks.addr(addr)
  }

  fn chr_addr(&mut self, addr: usize) -> usize {
    self.chr_banks.addr(addr)
  }

  fn mirroring(&self) -> Mirroring { self.mirroring }
} 