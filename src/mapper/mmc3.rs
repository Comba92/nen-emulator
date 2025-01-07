use crate::cart::{CartHeader, Mirroring};

use super::{Banking, ChrBanking, Mapper, PrgBanking};

#[derive(Default, PartialEq, serde::Serialize, serde::Deserialize)]
enum PrgMode { #[default] FixLastPages, FixFirstPages }
#[derive(Default, PartialEq, serde::Serialize, serde::Deserialize)]
enum ChrMode { #[default] BiggerFirst, BiggerLast }

// Mapper 04
// https://www.nesdev.org/wiki/MMC3
#[derive(serde::Serialize, serde::Deserialize)]
pub struct MMC3 {
  reg_select: u8,

  prg_banks: Banking<PrgBanking>,
  chr_banks: Banking<ChrBanking>,

  prg_mode: PrgMode,
  chr_mode: ChrMode,
  mirroring: Mirroring,

  sram_read_enabled: bool,
  sram_write_enabled: bool,

  irq_counter: u8,
  irq_latch: u8,
  irq_reload: bool,
  irq_enabled: bool,

  irq_requested: Option<()>,
}

impl MMC3 {
  fn write_bank_select(&mut self, val: u8) {
    self.reg_select = val & 0b111;

    let prg_mode = match (val >> 6) & 1 != 0 {
      false => PrgMode::FixLastPages,
      true  => PrgMode::FixFirstPages,
    };
    if prg_mode != self.prg_mode {
      self.prg_banks.swap(0, 2);
    }
    self.prg_mode = prg_mode;

    let chr_mode = match (val >> 7) != 0 {
      false => ChrMode::BiggerFirst,
      true  => ChrMode::BiggerLast,
    };
    if chr_mode != self.chr_mode {
      self.chr_banks.swap(0, 4);
      self.chr_banks.swap(1, 5);
      self.chr_banks.swap(2, 6);
      self.chr_banks.swap(3, 7);
    }
    self.chr_mode = chr_mode;
  }

  fn update_prg_bank(&mut self, bank: u8) {
    let page = match self.prg_mode {
      PrgMode::FixLastPages => {
        match self.reg_select {
          6 => 0,
          7 => 1,
          _ => unreachable!()
        }
      }
      PrgMode::FixFirstPages => {
        match self.reg_select {
          7 => 1,
          6 => 2,
          _ => unreachable!()
        }
      }
    };

    self.prg_banks.set(page, bank as usize);
  }

  fn update_chr_bank(&mut self, bank: u8) {
    let bank = bank as usize;

    match self.chr_mode {
      ChrMode::BiggerFirst => {
        match self.reg_select {
          0 => {
            self.chr_banks.set(0, bank);
            self.chr_banks.set(1, bank+1);
          }
          1 => {
            self.chr_banks.set(2, bank);
            self.chr_banks.set(3, bank+1);
          }
          2 => self.chr_banks.set(4, bank),
          3 => self.chr_banks.set(5, bank),
          4 => self.chr_banks.set(6, bank),
          5 => self.chr_banks.set(7, bank),
          _ => unreachable!()
        }
      }
      ChrMode::BiggerLast => {
        match self.reg_select {
          0 => {
            self.chr_banks.set(4, bank);
            self.chr_banks.set(5, bank+1);
          }
          1 => {
            self.chr_banks.set(6, bank);
            self.chr_banks.set(7, bank+1);
          }
          2 => self.chr_banks.set(0, bank),
          3 => self.chr_banks.set(1, bank),
          4 => self.chr_banks.set(2, bank),
          5 => self.chr_banks.set(3, bank),
          _ => unreachable!()
        }
      }
    }
  }
}

#[typetag::serde]
impl Mapper for MMC3 {
  fn new(header: &CartHeader) -> Box<Self>where Self:Sized {
    let mut prg_banks = Banking::new_prg(header, 4);
    let chr_banks = Banking::new_chr(header, 8);

    // last page always fixed to last bank
    prg_banks.set_page_to_last_bank(3);
    // bank second last page to second last bank by default
    // this page is never set by registers, so not setting it here fuck up everything
    prg_banks.set(2, prg_banks.banks_count-2);

    let mapper = Self {
      prg_banks, chr_banks,
      reg_select: 0,
      prg_mode: Default::default(),
      chr_mode: Default::default(),
      mirroring: Default::default(),
      sram_read_enabled: false,
      sram_write_enabled: false,
      irq_counter: 0, irq_latch: 0,
      irq_reload: false, irq_enabled: false,
      irq_requested: None,
    };

    Box::new(mapper)
  }

  fn write(&mut self, addr: usize, val: u8) {
    let addr_even = addr % 2 == 0;
    match (addr, addr_even) {
      (0x8000..=0x9FFE, true) => self.write_bank_select(val),
      (0x8001..=0x9FFF, false) => match self.reg_select {
        0 | 1 => self.update_chr_bank(val & !1),
        6 | 7 => self.update_prg_bank(val & 0b11_1111),
        _ => self.update_chr_bank(val),
      }
      (0xA000..=0xBFFE, true) => match val & 1 != 0 {
        false => self.mirroring = Mirroring::Vertical,
        true  => self.mirroring = Mirroring::Horizontal,
      }
      (0xA001..=0xBFFF, false) => {
        self.sram_write_enabled = val & 0b0100_0000 == 0;
        self.sram_read_enabled  = val & 0b1000_0000 != 0;
      }
      (0xC000..=0xDFFE, true) => self.irq_latch = val,
      (0xC001..=0xDFFF, false) => self.irq_reload = true,
      (0xE000..=0xFFFE, true) => {
        self.irq_enabled = false;
        self.irq_requested = None;
      }
      (0xE001..=0xFFFF, false) => self.irq_enabled = true,
      _ => unreachable!()
    }
  }

  fn prg_addr(&mut self, addr: usize) -> usize {
    self.prg_banks.addr(addr)
  }

  fn chr_addr(&mut self, addr: usize) -> usize {
    self.chr_banks.addr(addr)
  }

  fn notify_scanline(&mut self) {
    if self.irq_counter == 0 || self.irq_reload {
      self.irq_counter = self.irq_latch;
      self.irq_reload = false;
    } else {
      self.irq_counter -= 1;
    }

    if self.irq_enabled && self.irq_counter == 0 {
      self.irq_requested = Some(());
    }
  }

  fn poll_irq(&mut self) -> bool {
    self.irq_requested.is_some()
  }

  fn mirroring(&self) -> Option<Mirroring> {
    Some(self.mirroring)
  }
}