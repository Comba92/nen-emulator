use std::marker::{self, PhantomData};

use mmc1::MMC1;
use mmc3::MMC3;
use vrc2_4::VRC2_4;

use crate::cart::{CartHeader, Mirroring};

mod mmc1;
mod mmc3;
mod vrc2_4;

pub fn new_mapper(header: &CartHeader) -> Result<Box<dyn Mapper>, String> {
  let mapper: Box<dyn Mapper> = match header.mapper {
    0 => NROM::new(header),
    1 => MMC1::new(header),
    2 => UxROM::new(header),
    3 => CNROM::new(header),
    4 => MMC3::new(header),
    7 => AxROM::new(header),
    9 => MMC2::new(header),
    11 => ColorDreams::new(header),
    21 | 22 | 23 | 25 => VRC2_4::new(header),
    66 => GxROM::new(header),
    71 => Codemasters::new(header),
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
const MAPPERS_TABLE: [(u16, &'static str); 16] = [
  (0, "NRom"),
  (1, "MMC1"),
  (2, "UxRom"),
  (3, "CNRom (INesMapper003)"),
  (4, "MMC3"),
  (5, "MMC5"),
  (7, "AxRom"),
  (9, "MMC2"),
  (11, "ColorDreams"),
  (21, "VRC2/VRC4"),
  (22, "VRC2/VRC4"),
  (23, "VRC2/VRC4"),
  (25, "VRC2/VRC4"),
  (66, "GxRom"),
  (69, "Sunsoft FME-7"),
  (71, "Codemasters (INesMapper071)"),
];

#[typetag::serde]
pub trait Mapper {
  fn new(header: &CartHeader) -> Box<Self> where Self: Sized;
  fn write(&mut self, addr: usize, val: u8);
  
  fn prg_addr(&mut self, addr: usize) -> usize { addr - 0x8000 }
  fn chr_addr(&mut self, addr: usize) -> usize { addr }

  fn sram_read(&self, ram: &[u8], addr: usize) -> u8 {
    ram[(addr - 0x6000) % ram.len()]
  }

  fn sram_write(&mut self, ram: &mut[u8], addr: usize, val: u8) {
    ram[(addr - 0x6000) % ram.len()] = val;
  }

  fn mirroring(&self) -> Option<Mirroring> { None }

  // Mmc3 scanline notify
  fn notify_scanline(&mut self) {}

  // Generic cpu cycle notify
  fn notify_cpu_cycle(&mut self) {}

  // Mmc5 ppu notify
  fn notify_ppuctrl(&mut self, _val: u8) {}
  fn notify_ppumask(&mut self, _val: u8) {}

  fn poll_irq(&mut self) -> bool { false }
}

#[derive(Debug)]
struct PrgBanking;
#[derive(Debug)]
struct ChrBanking;
#[derive(Debug)]
struct RamBanking;
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

impl Banking<RamBanking> {
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
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct NROM {
  prg_banks: Banking<PrgBanking>,
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

    Box::new(Self { prg_banks })
  }

  fn write(&mut self, _: usize, _: u8) {}

  fn prg_addr(&mut self, addr: usize) -> usize {
    self.prg_banks.addr(addr)
  }
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct UxROM {
  prg_banks: Banking<PrgBanking>,
}

#[typetag::serde]
impl Mapper for UxROM {
  fn new(header: &CartHeader) -> Box<Self> {
    let mut prg_banks = Banking::new_prg(header, 2);
    prg_banks.set_page_to_last_bank(1);
    Box::new(Self { prg_banks })
  }

  fn write(&mut self, _: usize, val: u8) {
    let select = val & 0b1111;
    self.prg_banks.set(0, select as usize);
  }

  fn prg_addr(&mut self, addr: usize) -> usize {
    self.prg_banks.addr(addr)
  }
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct CNROM {
  chr_banks: Banking<ChrBanking>,
}

#[typetag::serde]
impl Mapper for CNROM {
  fn new(header: &CartHeader) -> Box<Self>where Self:Sized {
    let chr_banks = Banking::new_chr(header, 1);
    Box::new(Self { chr_banks })
  }

  fn write(&mut self, _: usize, val: u8) {
    self.chr_banks.set(0, val as usize);
  }

  fn chr_addr(&mut self, addr: usize) -> usize {
    self.chr_banks.addr(addr)
  }
}

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

  fn mirroring(&self) -> Option<Mirroring> {
    Some(self.mirroring)
  }
}

// ColorDreams and GxRom are the same, use PhantomData generics
#[derive(serde::Serialize, serde::Deserialize)]
pub struct ColorDreams {
  prg_banks: Banking<PrgBanking>,
  chr_banks: Banking<ChrBanking>,
}

#[typetag::serde]
impl Mapper for ColorDreams {
  fn new(header: &CartHeader) -> Box<Self> {
    let prg_banks = Banking::new_prg(header, 1);
    let chr_banks = Banking::new_chr(header, 1);
    Box::new(Self {prg_banks, chr_banks})
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
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct GxROM {
  prg_banks: Banking<PrgBanking>,
  chr_banks: Banking<ChrBanking>,
}

#[typetag::serde]
impl Mapper for GxROM {
  fn new(header: &CartHeader) -> Box<Self> {
    let prg_banks = Banking::new_prg(header, 1);
    let chr_banks = Banking::new_chr(header, 1);
    Box::new(Self {prg_banks, chr_banks})
  }

  fn write(&mut self, _: usize, val: u8) {
    let prg_bank = val as usize & 0b11;
    let chr_bank = (val as usize >> 4) & 0b11;

    self.prg_banks.set(0, prg_bank);
    self.chr_banks.set(0, chr_bank);
  }

  fn prg_addr(&mut self, addr: usize) -> usize {
    self.prg_banks.addr(addr)
  }

  fn chr_addr(&mut self, addr: usize) -> usize {
    self.chr_banks.addr(addr)
  }
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct Codemasters {
  prg_banks: Banking<PrgBanking>,
  // https://www.nesdev.org/wiki/INES_Mapper_071#Mirroring_($8000-$9FFF)
  mirroring: Option<Mirroring>,
}

#[typetag::serde]
impl Mapper for Codemasters {
  fn new(header: &CartHeader) -> Box<Self> {
    let mut prg_banks = Banking::new_prg(header, 2);
    prg_banks.set_page_to_last_bank(1);

    Box::new(Self {prg_banks, mirroring: None })
  }

  fn write(&mut self, addr: usize, val: u8) {
    match addr {
      0x9000..=0x9FFF => self.mirroring = match (val >> 4) & 1 != 0 {
        false => Some(Mirroring::SingleScreenA),
        true  => Some(Mirroring::SingleScreenB),
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

  fn mirroring(&self) -> Option<Mirroring> {
    self.mirroring
  }
}

#[derive(Clone, Copy, Default, serde::Serialize, serde::Deserialize)]
enum Mmc2Latch { FD, #[default] FE }
#[derive(serde::Serialize, serde::Deserialize)]
pub struct MMC2 {
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

    // Three 8 KB PRG ROM banks, fixed to the last three banks
    prg_banks.set(1, prg_banks.banks_count-3);
    prg_banks.set(2, prg_banks.banks_count-2);
    prg_banks.set(3, prg_banks.banks_count-1);

    Box::new(Self{
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
    match addr {
      0x0FD8 => self.latch0 = Mmc2Latch::FD,
      0x0FE8 => self.latch0 = Mmc2Latch::FE,
      0x1FD8..=0x1FDF => self.latch1 = Mmc2Latch::FD,
      0x1FE8..=0x1FEF => self.latch1 = Mmc2Latch::FE,
      _ => {}
    };

    res
  }

  fn mirroring(&self) -> Option<Mirroring> {
    Some(self.mirroring)
  }
}

// pub struct FC_001 {
//   prg_banks: Banking<PrgBanking>,
//   chr_banks: Banking<ChrBanking>,
// }

// impl Mapper for FC_001 {
//   fn new(header: &CartHeader) -> Box<Self> {
    
//   }

//   fn write(&mut self, addr: usize, val: u8) {
//     match addr {
//       0x5000 => ,
//       0x5200 => ,
//       0x5100 | 0x5101 =>,
//       0x5300 => ,
//     }
//   }
// }

