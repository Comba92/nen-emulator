use crate::cart::Mirroring;

use super::Mapper;

#[derive(Default)]
enum PrgMode { #[default] SwapAsc, SwapDesc }
#[derive(Default)]
enum ChrMode { #[default] BiggerFirst, BiggerLast }
#[derive(Default)]
enum Chr1kbMode { #[default] Bank2kb, Bank1kb }

#[derive(Default, PartialEq)]
enum IrqMode { #[default] Scanline, Cycle }

// Mapper 64 https://www.nesdev.org/wiki/RAMBO-1
#[derive(Default)]
pub struct Rambo1 {
  bank_select: usize,
  prg_mode: PrgMode,
  chr_mode: ChrMode,
  chr_1kb_mode: Chr1kbMode,
  mirroring: Mirroring,
  
  bank_selects: [usize; 11],


  irq_count: u8,
  prescaler: isize,
  irq_latch: u8,
  irq_mode: IrqMode,
  irq_reload: bool,
  irq_enabled: bool,

  irq_requested: Option<()>,
}
impl Rambo1 {
  fn write_bank_select(&mut self, val: u8) {
    self.bank_select = val as usize & 0b1111;
    if self.bank_select == 0b1111 {
      self.bank_select = 10;
    }

    self.chr_1kb_mode = match (val >> 5) & 1 != 0 {
      false => Chr1kbMode::Bank2kb,
      true  => Chr1kbMode::Bank1kb,
    };

    self.prg_mode = match (val >> 6) & 1 != 0 {
      false => PrgMode::SwapAsc,
      true  => PrgMode::SwapDesc,
    };

    self.chr_mode = match (val >> 7) != 0 {
      false => ChrMode::BiggerFirst,
      true  => ChrMode::BiggerLast,
    };
  }
}

impl Mapper for Rambo1 {
  fn prg_bank_size(&self) -> usize { 8*1024 }
  fn chr_bank_size(&self) -> usize { 1024 }

  fn prg_addr(&mut self, prg: &[u8], addr: usize) -> usize {
    use PrgMode::*;
    let bank = match (addr, &self.prg_mode) {
      (0x8000..=0x9FFF, SwapAsc)  => self.bank_selects[6],
      (0x8000..=0x9FFF, SwapDesc) => self.bank_selects[10],
      (0xA000..=0xBFFF, _) => self.bank_selects[7],
      (0xC000..=0xDFFF, SwapAsc)  => self.bank_selects[10],
      (0xC000..=0xDFFF, SwapDesc) => self.bank_selects[6],
      (0xE000..=0xFFFF, _) => self.prg_last_bank(prg),
      _ => unreachable!()
    };

    self.prg_bank_addr(prg, bank, addr)
  }

  fn chr_addr(&mut self, chr: &[u8], addr: usize) -> usize {
    use ChrMode::*;
    use Chr1kbMode::*;
    let bank = match(addr, &self.chr_mode, &self.chr_1kb_mode) {
      (0x0000..=0x03FF, BiggerFirst, _) => self.bank_selects[0],
      (0x0400..=0x07FF, BiggerFirst, Bank2kb) => self.bank_selects[0],
      (0x0400..=0x07FF, BiggerFirst, Bank1kb) => self.bank_selects[8],
      (0x0800..=0x0BFF, BiggerFirst, _) => self.bank_selects[1],
      (0x0C00..=0x0FFF, BiggerFirst, Bank2kb) => self.bank_selects[1],
      (0x0C00..=0x0FFF, BiggerFirst, Bank1kb) => self.bank_selects[9],
      (0x1000..=0x13FF, BiggerFirst, _) => self.bank_selects[2],
      (0x1400..=0x17FF, BiggerFirst, _) => self.bank_selects[3],
      (0x1800..=0x1BFF, BiggerFirst, _) => self.bank_selects[4],
      (0x1C00..=0x1FFF, BiggerFirst, _) => self.bank_selects[5],

      (0x0000..=0x03FF, BiggerLast, _) => self.bank_selects[2],
      (0x0400..=0x07FF, BiggerLast, _) => self.bank_selects[3],
      (0x0800..=0x0BFF, BiggerLast, _) => self.bank_selects[4],
      (0x0C00..=0x0FFF, BiggerLast, _) => self.bank_selects[5],
      (0x1000..=0x13FF, BiggerLast, _) => self.bank_selects[0],
      (0x1400..=0x17FF, BiggerLast, Bank2kb) => self.bank_selects[0],
      (0x1400..=0x17FF, BiggerLast, Bank1kb) => self.bank_selects[1],
      (0x1800..=0x1BFF, BiggerLast, _) => self.bank_selects[1],
      (0x1C00..=0x1FFF, BiggerLast, Bank2kb) => self.bank_selects[0],
      (0x1C00..=0x1FFF, BiggerLast, Bank1kb) => self.bank_selects[9],
      _ => unreachable!()
    };

    self.chr_bank_addr(chr, bank, addr)
  }

  fn prg_write(&mut self, _prg: &mut[u8], addr: usize, val: u8) {
    let addr_even = addr % 2 == 0;
    match (addr, addr_even) {
      (0x8000..=0x9FFE, true) => self.write_bank_select(val),
      (0x8001..=0x9FFF, false) => 
        self.bank_selects[self.bank_select] = val as usize,
      
      (0xA000..=0xBFFE, true) => match val & 1 != 0 {
        false => self.mirroring = Mirroring::Vertically,
        true  => self.mirroring = Mirroring::Horizontally,
      }

      (0xC000..=0xDFFE, true) => self.irq_latch = val,
      (0xC001..=0xDFFF, false) => {
        self.irq_mode = match val & 1 != 0 {
          false => IrqMode::Scanline,
          true  => IrqMode::Cycle,
        };
        self.irq_reload = true;
        if self.irq_mode == IrqMode::Cycle {
          self.prescaler = 0;
        }
      }
      (0xE000..=0xFFFE, true) => {
        self.irq_enabled = false;
        self.irq_requested = None;
      }
      (0xE001..=0xFFFF, false) => self.irq_enabled = true,
      _ => unreachable!()
    }
  }

  fn notify_scanline(&mut self) {
    if self.irq_count == 0 || self.irq_reload {
      // idk why, but that -1 fixes it
      self.irq_count = self.irq_latch-1;
      self.irq_reload = false;
    } else {
      self.irq_count -= 1;
    }

    if self.irq_enabled && self.irq_count == 0 {
      self.irq_requested = Some(());
    }
  }

  fn notify_cpu_cycle(&mut self) {
    
  }

  fn poll_irq(&mut self) -> bool {
    self.irq_requested.is_some()
  }

  fn mirroring(&self) -> Option<Mirroring> {
    Some(self.mirroring)
  }
}