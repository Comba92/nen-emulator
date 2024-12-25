#![allow(unused)]

use crate::{cart::Mirroring, mapper::{DEFAULT_CHR_BANK_SIZE, DEFAULT_PRG_BANK_SIZE}};
use super::{Mapper, SRAM_START};

#[derive(Default)]
enum PrgMode { Bank32kb, Bank16kb, BankMixed, #[default] Bank8kb }
#[derive(Default)]
enum ChrMode { Bank8kb, Bank4kb, Bank2kb, #[default] Bank1kb }
#[derive(Default)]
enum ExRamMode { Nametbl, NametblEx, ReadWrite, #[default] ReadOnly }
#[derive(Default)]
enum PpuState { #[default] RenderingBg, RenderingSpr, Blanking }

#[derive(Default, Clone, Copy)]
enum Target {#[default] Rom, Ram }
impl From<u8> for Target {
    fn from(value: u8) -> Self {
      match value {
        0 => Target::Ram,
        _ => Target::Rom
      }
    }
}

#[derive(Default)]
enum NametblMapping { #[default] Page0, Page1, ExRam, FillMode }
#[derive(Default)]
enum VSplitRegion { #[default] Left, Right }

// Mapper 5
// https://www.nesdev.org/wiki/MMC5

pub struct Mmc5 {
  ppu_spr_16: bool,
  ppu_data_sub: bool,
  ppu_state: PpuState,
  
  prg_mode: PrgMode,
  prg_bank_selects: [(usize, Target); 5],
  
  chr_mode: ChrMode,
  chr_bank_selects: [usize; 12],
  chr_bank_select: usize,
  chr_bank_hi: usize,
  
  sram: [u8; 128 * 1024],
  ram_write_lock1: bool,
  ram_write_lock2: bool,
  
  exram_mode: ExRamMode,
  exram: [u8; 1024],

  nametbl_mapping: [NametblMapping; 4],
  fill_mode_tile: u8,
  fill_mode_color: u8,

  vsplit_enabled: bool,
  vsplit_region: VSplitRegion,
  vsplit_count: u8,
  vsplit_scroll: u8,
  vsplit_bank: u8,

  irq_enabled: bool,
  scanline_count: u8,
  irq_value: u8,
  irq_scanline: Option<()>,
  irq_in_frame: bool,

  mirroring: Mirroring,

  multiplicand: u8,
  multiplier: u8,
}

impl Default for Mmc5 {
  fn default() -> Self {
    Self { exram: [0; 1024], exram_mode: Default::default(), sram: [0; 128*1024], prg_mode: Default::default(), prg_bank_selects: [Default::default(); 5], chr_mode: Default::default(), chr_bank_selects: [0; 12], chr_bank_select: Default::default(), chr_bank_hi: Default::default(), ram_write_lock1: Default::default(), ram_write_lock2: Default::default(), nametbl_mapping: Default::default(), fill_mode_tile: Default::default(), fill_mode_color: Default::default(), vsplit_enabled: Default::default(), vsplit_region: Default::default(), vsplit_count: Default::default(), vsplit_scroll: Default::default(), vsplit_bank: Default::default(), irq_enabled: Default::default(), scanline_count: Default::default(), irq_value: Default::default(), irq_scanline: Default::default(), irq_in_frame: Default::default(), multiplicand: 0xFF, multiplier: 0xFF, mirroring: Default::default(), ppu_spr_16: Default::default(), ppu_data_sub: Default::default(), ppu_state: Default::default() }
  }
}

impl Mmc5 {
  fn prg_bank32(&self, addr: usize) -> (usize, Target) {
    let (id, shift) = match addr {
      0x6000..=0x7FFF => (0, 0),
      _ => (4, 2),
    };

    let res = self.prg_bank_selects[id];
    (res.0 >> shift, res.1)
  }

  fn prg_bank16(&self, addr: usize) -> (usize, Target) {
    let (id, shift) = match addr {
      0x6000..=0x7FFF => (0, 0),
      0x8000..=0xBFFF => (2, 1),
      0xC000..=0xFFFF => (4, 1),
      _ => unreachable!()
    };

    let res = self.prg_bank_selects[id];
    (res.0 >> shift, res.1)
  }

  fn prg_bank16_mixed(&self, addr: usize) -> (usize, Target) {
    let (id, shift) = match addr {
      0x6000..=0x7FFF => (0, 0),
      0x8000..=0xBFFF => (2, 1),
      0xC000..=0xDFFF => (3, 0),
      0xE000..=0xFFFF => (4, 0),
      _ => unreachable!()
    };

    let res = self.prg_bank_selects[id];
    (res.0 >> shift, res.1)
  }

  fn prg_bank8(&self, addr: usize) -> (usize, Target) {
    let id = match addr {
      0x6000..=0x7FFF => 0,
      0x8000..=0x9FFF => 1,
      0xA000..=0xBFFF => 2,
      0xC000..=0xDFFF => 3,
      0xE000..=0xFFFF => 4,
      _ => unreachable!()
    };

    self.prg_bank_selects[id]
  }

  fn prg_bank(&self, addr: usize) -> (usize, Target) {
    match self.prg_mode {
      PrgMode::Bank32kb  => self.prg_bank32(addr),
      PrgMode::Bank16kb  => self.prg_bank16(addr),
      PrgMode::BankMixed => self.prg_bank16_mixed(addr),
      PrgMode::Bank8kb   => self.prg_bank8(addr),
    }
  }
}

impl Mapper for Mmc5 {
  fn prg_bank_size(&self) -> usize {
    use PrgMode::*;

    match self.prg_mode {
      Bank32kb  => DEFAULT_PRG_BANK_SIZE*2,
      Bank16kb  => DEFAULT_PRG_BANK_SIZE,
      BankMixed | Bank8kb => DEFAULT_PRG_BANK_SIZE/2,
    }
  }

  fn chr_bank_size(&self) -> usize {
    use ChrMode::*;

    match self.chr_mode {
      Bank8kb => DEFAULT_CHR_BANK_SIZE,
      Bank4kb => DEFAULT_CHR_BANK_SIZE/2,
      Bank2kb => DEFAULT_CHR_BANK_SIZE/4,
      Bank1kb => DEFAULT_CHR_BANK_SIZE/8,
    }
  }

  fn prg_read(&mut self, prg: &[u8], addr: usize) -> u8 {
    match self.prg_bank(addr) {
      (bank, Target::Ram) => 0,
      (bank, Target::Rom) => prg[self.prg_bank_addr(prg, bank, addr)],
    }
  }

  fn prg_write(&mut self, prg: &mut[u8], addr: usize, val: u8) {
    match self.prg_bank(addr) {
      (bank, Target::Ram) => {},
      (bank, Target::Rom) => prg[self.prg_bank_addr(prg, bank, addr)] = val,
    }
  }

  fn cart_read(&mut self, addr: usize) -> u8 {
      match addr {
        0x5204 => {
          let irq_ack = self.irq_scanline.take().is_some() as u8;
          (irq_ack << 7) | ((self.irq_in_frame as u8) << 6)
        },
        0x5025 => (self.multiplicand * self.multiplier) & 0x00FF,
        0x5206 => (((self.multiplicand as u16 * self.multiplier as u16) & 0xFF00) >> 8) as u8,
        0x5C00..=0x5FFF => {
          match self.exram_mode {
            ExRamMode::ReadWrite | ExRamMode::ReadOnly => self.exram[addr - 0x5C00],
            _ => 0,
          }
        }

        // TODO: open bus behaviour
        _ => 0,
      }
  }

  fn cart_write(&mut self, addr: usize, val: u8) {
    match addr {
      0x5100 => self.prg_mode = match val & 0b11 {
        0 => PrgMode::Bank32kb,
        1 => PrgMode::Bank16kb,
        2 => PrgMode::BankMixed,
        _ => PrgMode::Bank8kb,
      },
      0x5101 => self.chr_mode = match val & 0b11 {
        0 => ChrMode::Bank8kb,
        1 => ChrMode::Bank4kb,
        2 => ChrMode::Bank2kb,
        _ => ChrMode::Bank1kb,
      },
      0x5102 => self.ram_write_lock1 = val & 0b11 == 0x02,
      0x5103 => self.ram_write_lock2 = val & 0b11 == 0x01,
      0x5104 => self.exram_mode = match val & 0b11 {
        0b00 => ExRamMode::Nametbl,
        0b01 => ExRamMode::NametblEx,
        0b10 => ExRamMode::ReadWrite,
        _    => ExRamMode::ReadOnly,
      },

      0x5105 => {
        for i in 0..4 {
          let bits = (val >> (i*2)) & 0b11;
          self.nametbl_mapping[i] = match bits {
            0 => NametblMapping::Page0,
            1 => NametblMapping::Page1,
            2 => NametblMapping::ExRam,
            _ => NametblMapping::FillMode,
          };
        }
      }

      0x5106 => self.fill_mode_tile = val,
      0x5107 => self.fill_mode_color = val & 0b11,

      0x5113..=0x5117 => {
        // https://www.nesdev.org/wiki/MMC5#PRG_Bankswitching_($5113-$5117)
        let target = match addr {
          0x5113 => Target::Ram,
          0x5117 => Target::Rom,
          _ => Target::from(val >> 7),
        };

        self.prg_bank_selects[addr - 0x5113] = (val as usize & 0b0111_1111, target);
      }

      0x5120..=0x512B => {
        // https://www.nesdev.org/wiki/MMC5#CHR_Bankswitching_($5120-$5130)
        self.chr_bank_selects[addr - 0x5120] = val as usize;
      }
      0x5130 => self.chr_bank_hi = val as usize & 0b11,

      0x5200 => {
        self.vsplit_enabled = (val >> 7) != 0;
        self.vsplit_region = match (val >> 6) & 1 != 0 {
          false => VSplitRegion::Left,
          true  => VSplitRegion::Right,
        };
        self.vsplit_count = val & 0b1_1111;
      }
      0x5201 => self.vsplit_scroll = val,
      0x5202 => self.vsplit_bank = val,

      0x5203 => self.irq_value = val,
      0x5204 => self.irq_enabled = (val >> 7) & 1 != 0,

      0x5205 => self.multiplicand = val,
      0x5206 => self.multiplier = val,

      0x5C00..=0x5FFF => {
        match (&self.exram_mode, self.irq_in_frame) {
          (ExRamMode::Nametbl | ExRamMode::NametblEx, true) 
          | (ExRamMode::ReadWrite, _) => self.exram[addr - 0x5C00] = val,
          _ => {}
        }
      }
      _ => {}
    }
  }

  fn notify_scanline(&mut self) {
    todo!()
  }

  fn notify_ppuctrl(&mut self, val: u8) {
    self.ppu_spr_16 = val >> 5 != 0;
  }

  fn notify_ppumask(&mut self, val: u8) {
    self.ppu_data_sub = (val >> 3) & 0b11 != 0;
  }

  fn mirroring(&self) -> Option<Mirroring> {
    Some(self.mirroring)
  }
}