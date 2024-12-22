use crate::cart::Mirroring;
use bitfield_struct::bitfield;
use super::{Mapper, DEFAULT_PRG_BANK_SIZE};

#[derive(Default)]
enum IrqMode { #[default] Cycle, Scanline }

#[bitfield(u16, order = Lsb)]
struct Byte {
  #[bits(4)]
  lo: u8,
  #[bits(5)]
  hi: u8,

  #[bits(7)]
  __: u8
}

// Mapper 21, 22, 23, 25
// https://www.nesdev.org/wiki/VRC2_and_VRC4

#[derive(Default)]
pub struct Vrc4 {
  swap_mode: bool,
  wram_ctrl: bool,
  prg_bank0_select: usize,
  prg_bank1_select: usize,
  chr_banks_selects: [Byte; 8],

  prescaler: usize,
  irq_count: u16,
  irq_latch: Byte,
  irq_enabled_after_ack: bool,
  irq_enabled: bool,
  irq_mode: IrqMode,
  irq_requested: Option<()>,

  mirroring: Mirroring,
}

impl Vrc4 {
  pub fn new(mapper_id: usize) -> Self {
    Self::default()
  }
}

impl Mapper for Vrc4 {
  fn prg_bank_size(&self) -> usize { DEFAULT_PRG_BANK_SIZE/2 }
  fn chr_bank_size(&self) -> usize { 1024 }

  fn prg_addr(&mut self, prg: &[u8], addr: usize) -> usize {
    let bank = match (addr, self.swap_mode) {
      (0x8000..=0x9FFF, false) => self.prg_bank0_select,
      (0x8000..=0x9FFF, true)  => self.prg_last_bank(prg)-1,

      (0xA000..=0xBFFF, _) => self.prg_bank1_select,

      (0xC000..=0xDFFF, false) => self.prg_last_bank(prg)-1,
      (0xC000..=0xDFFF, true)  => self.prg_bank0_select,

      (0xE000..=0xFFFF, _) => self.prg_last_bank(prg),
      _ => unreachable!()
    };

    self.prg_bank_addr(prg, bank, addr)
  }

  fn chr_addr(&mut self, chr: &[u8], addr: usize) -> usize {
    let bank = match addr {
      0x0000..=0x03FF => self.chr_banks_selects[0],
      0x0400..=0x07FF => self.chr_banks_selects[1],
      0x0800..=0x0BFF => self.chr_banks_selects[2],
      0x0C00..=0x0FFF => self.chr_banks_selects[3],
      0x1000..=0x13FF => self.chr_banks_selects[4],
      0x1400..=0x17FF => self.chr_banks_selects[5],
      0x1800..=0x1BFF => self.chr_banks_selects[6],
      0x1C00..=0x1FFF => self.chr_banks_selects[7],
      _ => unreachable!()
    };

    self.chr_bank_addr(chr, bank.0 as usize, addr)
  }

  fn prg_write(&mut self, _prg: &mut[u8], addr: usize, val: u8) {
      match addr {
        0x9002 => {
          self.wram_ctrl = val & 0b01 != 0;
          self.swap_mode = val & 0b10 != 0;
        }
        0x8000..=0x8003 => self.prg_bank0_select = val as usize & 0b1_1111,
        0xA000..=0xA003 => self.prg_bank1_select = val as usize & 0b1_1111,
        0x9000..=0x9003 => self.mirroring = match val & 0b11 {
          0 => Mirroring::Vertically,
          1 => Mirroring::Horizontally,
          2 => Mirroring::SingleScreenFirstPage,
          _ => Mirroring::SingleScreenSecondPage,
        },
        0xB000 => self.chr_banks_selects[0].set_lo(val & 0b1111),
        0xB001 => self.chr_banks_selects[0].set_hi(val & 0b1_1111),

        0xB002 => self.chr_banks_selects[1].set_lo(val & 0b1111),
        0xB003 => self.chr_banks_selects[1].set_hi(val & 0b1_1111),

        0xC000 => self.chr_banks_selects[2].set_lo(val & 0b1111),
        0xC001 => self.chr_banks_selects[2].set_hi(val & 0b1_1111),

        0xC002 => self.chr_banks_selects[3].set_lo(val & 0b1111),
        0xC003 => self.chr_banks_selects[3].set_hi(val & 0b1_1111),

        0xD000 => self.chr_banks_selects[4].set_lo(val & 0b1111),
        0xD001 => self.chr_banks_selects[4].set_hi(val & 0b1_1111),

        0xD002 => self.chr_banks_selects[5].set_lo(val & 0b1111),
        0xD003 => self.chr_banks_selects[5].set_hi(val & 0b1_1111),

        0xE000 => self.chr_banks_selects[6].set_lo(val & 0b1111),
        0xE001 => self.chr_banks_selects[6].set_hi(val & 0b1_1111),

        0xE002 => self.chr_banks_selects[7].set_lo(val & 0b1111),
        0xE003 => self.chr_banks_selects[7].set_hi(val & 0b1_1111),

        0xF000 => self.irq_latch.set_lo(val & 0b1111),
        0xF001 => self.irq_latch.set_hi(val & 0b1111),
        0xF002 => {
          self.irq_enabled_after_ack = val & 0b001 != 0;
          self.irq_enabled = val & 0b010 != 0;
          self.irq_mode = match val & 0b100 != 0 {
            false => IrqMode::Scanline,
            true  => IrqMode::Cycle,
          };

          self.irq_requested = None;
          if self.irq_enabled {
            self.irq_count = self.irq_latch.0;
            self.prescaler = 341;
          }
        }
        0xF003 => {
          self.irq_requested = None;
          self.irq_enabled = self.irq_enabled_after_ack;
        }
        _ => {}
      }
  }

  fn mirroring(&self) -> Option<Mirroring> {
    Some(self.mirroring)
  }

  fn notify_cpu_cycle(&mut self) {
    if !self.irq_enabled { return; }

    match self.irq_mode {
      IrqMode::Cycle => {
        self.irq_count += 1;
        if self.irq_count >= 0xFF {
          self.irq_requested = Some(());
          self.irq_count = self.irq_latch.0;
        }
      }
      IrqMode::Scanline => {
        self.prescaler += 3;
        if self.prescaler >= 341 {
          self.prescaler -= 341;
          self.irq_count += 1;
        }
      }
    }
  }

  fn poll_irq(&mut self) -> bool {
    self.irq_requested.is_some()
  }
}
