use crate::{
  banks::MemConfig,
  cart::{CartHeader, Mirroring},
};

use super::{Banking, Mapper};

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Default, PartialEq)]
enum PrgMode {
  #[default]
  FixLastPages,
  FixFirstPages,
}
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Default, PartialEq)]
enum ChrMode {
  #[default]
  BiggerFirst,
  BiggerLast,
}

// Mapper 04
// https://www.nesdev.org/wiki/MMC3
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Default)]
pub struct MMC3 {
  pub reg_select: u8,

  prg_mode: PrgMode,
  chr_mode: ChrMode,
  pub mirroring: Mirroring,

  sram_read_enabled: bool,
  sram_write_enabled: bool,

  pub irq_count: u8,
  pub irq_latch: u8,
  pub irq_reload: bool,
  pub irq_enabled: bool,

  pub irq_requested: Option<()>,
}

impl MMC3 {
  fn write_bank_select(&mut self, banks: &mut MemConfig, val: u8) {
    self.reg_select = val & 0b111;

    let prg_mode = match (val >> 6) & 1 != 0 {
      false => PrgMode::FixLastPages,
      true => PrgMode::FixFirstPages,
    };
    if prg_mode != self.prg_mode {
      banks.prg.swap_pages(0, 2);
    }
    self.prg_mode = prg_mode;

    let chr_mode = match (val >> 7) != 0 {
      false => ChrMode::BiggerFirst,
      true => ChrMode::BiggerLast,
    };
    if chr_mode != self.chr_mode {
      banks.chr.swap_pages(0, 4);
      banks.chr.swap_pages(1, 5);
      banks.chr.swap_pages(2, 6);
      banks.chr.swap_pages(3, 7);
    }
    self.chr_mode = chr_mode;
  }

  fn update_prg_bank(&mut self, banks: &mut MemConfig, bank: u8) {
    let page = match self.prg_mode {
      PrgMode::FixLastPages => match self.reg_select {
        6 => 0,
        7 => 1,
        _ => unreachable!(),
      },
      PrgMode::FixFirstPages => match self.reg_select {
        7 => 1,
        6 => 2,
        _ => unreachable!(),
      },
    };

    banks.prg.set_page(page, bank as usize);
  }

  fn update_chr_bank(&mut self, banks: &mut MemConfig, bank: u8) {
    let bank = bank as usize;

    match self.chr_mode {
      ChrMode::BiggerFirst => match self.reg_select {
        0 => {
          banks.chr.set_page(0, bank);
          banks.chr.set_page(1, bank + 1);
        }
        1 => {
          banks.chr.set_page(2, bank);
          banks.chr.set_page(3, bank + 1);
        }
        2 => banks.chr.set_page(4, bank),
        3 => banks.chr.set_page(5, bank),
        4 => banks.chr.set_page(6, bank),
        5 => banks.chr.set_page(7, bank),
        _ => unreachable!(),
      },
      ChrMode::BiggerLast => match self.reg_select {
        0 => {
          banks.chr.set_page(4, bank);
          banks.chr.set_page(5, bank + 1);
        }
        1 => {
          banks.chr.set_page(6, bank);
          banks.chr.set_page(7, bank + 1);
        }
        2 => banks.chr.set_page(0, bank),
        3 => banks.chr.set_page(1, bank),
        4 => banks.chr.set_page(2, bank),
        5 => banks.chr.set_page(3, bank),
        _ => unreachable!(),
      },
    }
  }
}

#[cfg_attr(feature = "serde", typetag::serde)]
impl Mapper for MMC3 {
  fn new(header: &CartHeader, banks: &mut MemConfig) -> Box<Self> {
    banks.prg = Banking::new_prg(header, 4);
    banks.chr = Banking::new_chr(header, 8);

    // bank second last page to second last bank by default
    // this page is never set by registers, so not setting it here fuck up everything
    banks.prg.set_page(2, banks.prg.banks_count - 2);
    // last page always fixed to last bank
    banks.prg.set_page_to_last_bank(3);

    let mapper = Self {
      mirroring: header.mirroring,
      ..Default::default()
    };

    Box::new(mapper)
  }

  fn prg_write(&mut self, banks: &mut MemConfig, addr: usize, val: u8) {
    let addr_even = addr % 2 == 0;
    match (addr, addr_even) {
      (0x8000..=0x9FFE, true) => self.write_bank_select(banks, val),
      (0x8001..=0x9FFF, false) => match self.reg_select {
        0 | 1 => self.update_chr_bank(banks, val & !1),
        6 | 7 => self.update_prg_bank(banks, val & 0b11_1111),
        _ => self.update_chr_bank(banks, val),
      },
      (0xA000..=0xBFFE, true) => {
        if self.mirroring != Mirroring::FourScreen {
          self.mirroring = match val & 1 != 0 {
            false => Mirroring::Vertical,
            true => Mirroring::Horizontal,
          };
          banks.vram.update(self.mirroring);
        }
      }
      (0xA001..=0xBFFF, false) => {
        self.sram_write_enabled = val & 0b0100_0000 == 0;
        self.sram_read_enabled = val & 0b1000_0000 != 0;
      }
      (0xC000..=0xDFFE, true) => self.irq_latch = val,
      (0xC001..=0xDFFF, false) => self.irq_reload = true,
      (0xE000..=0xFFFE, true) => {
        self.irq_enabled = false;
        self.irq_requested = None;
      }
      (0xE001..=0xFFFF, false) => self.irq_enabled = true,
      _ => {}
    }
  }

  fn notify_mmc3_scanline(&mut self) {
    if self.irq_count == 0 || self.irq_reload {
      self.irq_count = self.irq_latch;
      self.irq_reload = false;
    } else {
      self.irq_count -= 1;
    }

    if self.irq_enabled && self.irq_count == 0 {
      self.irq_requested = Some(());
    }
  }

  fn poll_irq(&mut self) -> bool {
    self.irq_requested.is_some()
  }
}
